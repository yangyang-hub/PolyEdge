//! Deterministic multi-wallet execution runtime.
//!
//! The runtime reconciles a manually authored strategy version to each wallet
//! target. It has no market discovery, AI/provider, event, or fair-value
//! stages. All external order side effects are fenced by a durable action
//! idempotency key and all wallet work is serialized behind a wallet mutex.

use crate::{
    orderbook::{CachedOrderBook, OrderbookSupervisor},
    secrets::WalletSecretResolver,
};
use async_trait::async_trait;
use polyedge_connectors::{
    LivePolymarketCancelOutcome, LivePolymarketConnector, LivePolymarketExecutionOutcome,
    LivePolymarketTokenOrderRequest, PolymarketAcceptedOrderStatus, PolymarketDataApiConnector,
    PolymarketOpenOrder, PolymarketTokenOrderSide,
};
use polyedge_domain::{
    AppError, ManagedOrder, ManagedOrderStatus, MarketStatus, QuoteOutcome, QuotePricingMode,
    Result, StrategyQuoteSlot, StrategyStatus, StrategySubscriptionStatus, StrategyVersion,
    StrategyVersionStatus, WalletAccount, WalletAccountState, WalletAccountStatus,
    WalletExecutionJob, WalletRiskPolicy,
};
use rust_decimal::Decimal;
use serde_json::json;
use std::{collections::HashMap, sync::Arc, time::Duration};
use time::OffsetDateTime;
use tokio::{
    sync::{Mutex, Semaphore, watch},
    task::JoinSet,
};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct ExecutionRuntimeConfig {
    pub poll_interval: Duration,
    pub lease_duration: Duration,
    pub max_wallet_concurrency: usize,
}

impl Default for ExecutionRuntimeConfig {
    fn default() -> Self {
        Self {
            poll_interval: Duration::from_secs(5),
            lease_duration: Duration::from_secs(30),
            max_wallet_concurrency: 8,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ExecutionContext {
    pub job: WalletExecutionJob,
    pub wallet: WalletAccount,
    pub subscription_id: i64,
    pub subscription_status: StrategySubscriptionStatus,
    pub subscription_wallet_enabled: bool,
    pub strategy_status: StrategyStatus,
    pub strategy_active_from: OffsetDateTime,
    pub effective_active_until: OffsetDateTime,
    pub market_status: MarketStatus,
    pub strategy_version: StrategyVersion,
    pub market_id: i64,
    pub slots: Vec<StrategyQuoteSlot>,
    pub outcomes: HashMap<QuoteOutcome, String>,
    pub managed_orders: Vec<ManagedOrder>,
    pub risk_policy: WalletRiskPolicy,
    pub account_state: WalletAccountState,
    pub market_position_notional: Decimal,
    pub trading_enabled: bool,
    pub kill_switch_locked: bool,
    pub force_cancel_all: bool,
}

#[must_use]
pub fn desired_state_active_at(context: &ExecutionContext, now: OffsetDateTime) -> bool {
    context.trading_enabled
        && !context.kill_switch_locked
        && !context.force_cancel_all
        && context.wallet.status == WalletAccountStatus::Active
        && context.wallet.trading_enabled
        && context.subscription_wallet_enabled
        && context.subscription_status == StrategySubscriptionStatus::Active
        && context.strategy_status == StrategyStatus::Active
        && context.market_status == MarketStatus::Open
        && context.strategy_version.status == StrategyVersionStatus::Published
        && now >= context.strategy_active_from
        && now < context.effective_active_until
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionKind {
    Place,
    Cancel,
    Replace,
}

impl ActionKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Place => "place_order",
            Self::Cancel => "cancel_order",
            Self::Replace => "replace_order",
        }
    }
}

#[derive(Debug, Clone)]
pub struct ActionHandle {
    pub action_id: i64,
    pub idempotency_key: String,
}

#[derive(Debug, Clone)]
pub struct ActionProposal {
    pub action: ActionKind,
    pub slot_id: Option<i64>,
    pub managed_order_id: Option<i64>,
    pub idempotency_key: String,
    pub reason: String,
    pub request: serde_json::Value,
}

#[derive(Debug, Clone)]
pub struct TargetOrder {
    pub slot: StrategyQuoteSlot,
    pub token_id: String,
    pub price: Decimal,
    pub quantity: Decimal,
    pub post_only: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DesiredAction {
    Keep,
    Place,
    Cancel,
    Replace,
    Blocked,
}

#[derive(Debug, Clone)]
pub struct ReconcileDecision {
    pub action: DesiredAction,
    pub reason: &'static str,
}

#[derive(Debug, Clone, Copy)]
pub struct RiskSnapshot {
    pub open_orders: i64,
    pub open_buy_notional: Decimal,
    pub market_position_notional: Decimal,
    pub total_position_notional: Decimal,
    pub available_collateral: Decimal,
}

#[derive(Debug, Clone)]
pub struct WalletPositionSnapshotEntry {
    pub token_id: String,
    pub quantity: Decimal,
    pub average_price: Decimal,
    pub realized_pnl: Decimal,
}

#[derive(Debug, Clone)]
pub struct WalletPositionSnapshot {
    pub observed_at: OffsetDateTime,
    pub positions: Vec<WalletPositionSnapshotEntry>,
}

#[derive(Debug, Clone, Copy)]
pub struct WalletPositionRiskTotals {
    pub total_position_notional: Decimal,
    pub market_position_notional: Decimal,
}

#[async_trait]
pub trait ExecutionStore: Send + Sync {
    /// Persistently expire due source strategies before desired-state jobs are
    /// generated. Implementations must also expire their subscriptions and
    /// emit the durable expire command in the same transaction.
    async fn expire_due_strategies(&self, limit: i64) -> Result<u64>;

    /// Create one pending job per enabled wallet target for the currently
    /// published version. Implementations must be idempotent for a cycle.
    async fn enqueue_active_wallet_reconciles(&self) -> Result<usize>;
    async fn claim_next_job(
        &self,
        owner: &str,
        lease_duration: Duration,
    ) -> Result<Option<WalletExecutionJob>>;
    async fn load_execution_context(&self, job: &WalletExecutionJob) -> Result<ExecutionContext>;
    async fn renew_job_lease(
        &self,
        _job: &WalletExecutionJob,
        _owner: &str,
        _lease_duration: Duration,
    ) -> Result<()> {
        Err(AppError::conflict(
            "EXECUTION_LEASE_RENEW_UNSUPPORTED",
            "execution store does not implement lease renewal",
        ))
    }
    async fn begin_action(
        &self,
        job: &WalletExecutionJob,
        proposal: ActionProposal,
    ) -> Result<Option<ActionHandle>>;
    async fn mark_action_succeeded(
        &self,
        action: &ActionHandle,
        result: serde_json::Value,
    ) -> Result<()>;
    async fn mark_action_failed(
        &self,
        action: &ActionHandle,
        error_code: &str,
        message: &str,
        unknown: bool,
    ) -> Result<()>;
    async fn record_order_submitted(
        &self,
        context: &ExecutionContext,
        target: &TargetOrder,
        generation: i64,
        action: &ActionHandle,
        external_order_id: &str,
    ) -> Result<()>;
    async fn mark_order_cancelled(&self, order: &ManagedOrder, action: &ActionHandle)
    -> Result<()>;
    async fn mark_order_unknown(
        &self,
        _job: &WalletExecutionJob,
        _order: &ManagedOrder,
        _reason: &str,
    ) -> Result<()> {
        Err(AppError::conflict(
            "EXECUTION_ORDER_UNKNOWN_UNSUPPORTED",
            "execution store does not implement unknown-order fencing",
        ))
    }
    async fn reconcile_managed_order(
        &self,
        _job: &WalletExecutionJob,
        _order: &ManagedOrder,
        _status: ManagedOrderStatus,
        _filled_quantity: Decimal,
        _reason: &str,
    ) -> Result<()> {
        Err(AppError::conflict(
            "EXECUTION_ORDER_RECONCILE_UNSUPPORTED",
            "execution store does not implement managed-order reconciliation",
        ))
    }
    async fn finish_job(
        &self,
        job: &WalletExecutionJob,
        success: bool,
        error: Option<&AppError>,
    ) -> Result<()>;
    async fn update_wallet_balance(
        &self,
        wallet_id: i64,
        available_collateral: Decimal,
    ) -> Result<()>;
    async fn replace_wallet_positions(
        &self,
        wallet_id: i64,
        market_id: i64,
        snapshot: WalletPositionSnapshot,
    ) -> Result<WalletPositionRiskTotals>;
}

pub struct RuntimeSupervisor {
    store: Arc<dyn ExecutionStore>,
    orderbooks: Arc<OrderbookSupervisor>,
    secrets: WalletSecretResolver,
    data_api: PolymarketDataApiConnector,
    config: ExecutionRuntimeConfig,
    owner: String,
    wallet_locks: Arc<Mutex<HashMap<i64, Arc<Mutex<()>>>>>,
}

impl RuntimeSupervisor {
    pub fn new(
        store: Arc<dyn ExecutionStore>,
        orderbooks: Arc<OrderbookSupervisor>,
        secrets: WalletSecretResolver,
        data_api: PolymarketDataApiConnector,
        config: ExecutionRuntimeConfig,
    ) -> Result<Self> {
        if config.poll_interval.is_zero()
            || config.lease_duration.is_zero()
            || config.max_wallet_concurrency == 0
        {
            return Err(AppError::invalid_input(
                "EXECUTION_RUNTIME_CONFIG_INVALID",
                "execution poll/lease durations and concurrency must be positive",
            ));
        }
        Ok(Self {
            store,
            orderbooks,
            secrets,
            data_api,
            config,
            owner: format!("server-{}", Uuid::now_v7()),
            wallet_locks: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    pub async fn run(self: Arc<Self>, mut shutdown: watch::Receiver<bool>) {
        let mut interval = tokio::time::interval(self.config.poll_interval);
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            tokio::select! {
                changed = shutdown.changed() => {
                    if changed.is_err() || *shutdown.borrow() { break; }
                }
                _ = interval.tick() => {
                    if let Err(error) = self.run_cycle().await {
                        tracing::error!(code = error.code(), error = %error, "execution cycle failed");
                    }
                }
            }
        }
    }

    pub async fn run_cycle(&self) -> Result<usize> {
        let _ = self.store.expire_due_strategies(500).await?;
        let _ = self.store.enqueue_active_wallet_reconciles().await?;
        let semaphore = Arc::new(Semaphore::new(self.config.max_wallet_concurrency));
        let mut tasks = JoinSet::new();
        let mut claimed = 0usize;
        while let Some(job) = self
            .store
            .claim_next_job(&self.owner, self.config.lease_duration)
            .await?
        {
            claimed += 1;
            let permit = semaphore.clone().acquire_owned().await.map_err(|_| {
                AppError::internal(
                    "EXECUTION_CONCURRENCY_CLOSED",
                    "execution concurrency semaphore closed",
                )
            })?;
            let supervisor = self.clone_for_task();
            tasks.spawn(async move {
                let _permit = permit;
                supervisor.process_job(job).await
            });
            if claimed >= self.config.max_wallet_concurrency * 4 {
                break;
            }
        }
        while let Some(result) = tasks.join_next().await {
            match result {
                Ok(Ok(())) => {}
                Ok(Err(error)) => {
                    tracing::error!(code = error.code(), error = %error, "wallet execution failed")
                }
                Err(error) => tracing::error!(error = %error, "wallet execution task panicked"),
            }
        }
        Ok(claimed)
    }

    fn clone_for_task(&self) -> Self {
        Self {
            store: Arc::clone(&self.store),
            orderbooks: Arc::clone(&self.orderbooks),
            secrets: self.secrets.clone(),
            data_api: self.data_api.clone(),
            config: self.config.clone(),
            owner: self.owner.clone(),
            wallet_locks: Arc::clone(&self.wallet_locks),
        }
    }

    async fn process_job(&self, job: WalletExecutionJob) -> Result<()> {
        let wallet_lock = {
            let mut locks = self.wallet_locks.lock().await;
            locks
                .entry(job.wallet_id)
                .or_insert_with(|| Arc::new(Mutex::new(())))
                .clone()
        };
        let _wallet_guard = wallet_lock.lock().await;
        let mut context = match self.store.load_execution_context(&job).await {
            Ok(context) => context,
            Err(error) => {
                self.store.finish_job(&job, false, Some(&error)).await?;
                return Err(error);
            }
        };
        let result = async {
            self.store
                .renew_job_lease(&job, &self.owner, self.config.lease_duration)
                .await?;
            let config = self.secrets.resolve(&context.wallet).await?;
            let connector = LivePolymarketConnector::connect(&config).await?;
            if !desired_state_active_at(&context, OffsetDateTime::now_utc()) {
                return self.reconcile_wallet(&connector, &context).await;
            }
            let balance = connector.refresh_balance().await?;
            let available = (balance.balance / Decimal::from(1_000_000_u64)).round_dp(8);
            context.account_state.available_collateral = available;
            self.store
                .update_wallet_balance(context.wallet.id, available)
                .await?;
            let positions = self
                .data_api
                .fetch_wallet_positions(&context.wallet.funder_address)
                .await?
                .into_iter()
                .map(|position| WalletPositionSnapshotEntry {
                    token_id: position.token_id,
                    quantity: position.quantity,
                    average_price: position.average_price,
                    realized_pnl: position.realized_pnl,
                })
                .collect();
            let totals = self
                .store
                .replace_wallet_positions(
                    context.wallet.id,
                    context.market_id,
                    WalletPositionSnapshot {
                        observed_at: OffsetDateTime::now_utc(),
                        positions,
                    },
                )
                .await?;
            context.account_state.total_position_notional = totals.total_position_notional;
            context.market_position_notional = totals.market_position_notional;
            self.reconcile_wallet(&connector, &context).await
        }
        .await;
        match result {
            Ok(()) => {
                self.store.finish_job(&job, true, None).await?;
                Ok(())
            }
            Err(error) => {
                self.store.finish_job(&job, false, Some(&error)).await?;
                Err(error)
            }
        }
    }

    async fn reconcile_wallet(
        &self,
        connector: &LivePolymarketConnector,
        context: &ExecutionContext,
    ) -> Result<()> {
        let now = OffsetDateTime::now_utc();
        let mut context = context.clone();
        let venue_orders = connector.list_open_orders().await?;
        self.verify_managed_orders(
            connector,
            &context.job,
            &mut context.managed_orders,
            &venue_orders,
        )
        .await?;
        let context = &context;
        let mut actual_by_slot = context
            .managed_orders
            .iter()
            .filter(|order| reconciliation::is_open_like_order_status(order.status))
            .filter(|order| order.quote_slot_id.is_some())
            .filter_map(|order| order.quote_slot_id.map(|slot_id| (slot_id, order)))
            .collect::<HashMap<_, _>>();
        let mut replacements = 0i64;
        let mut risk = risk_snapshot(context);
        risk.open_orders = risk.open_orders.max(venue_orders.len() as i64);
        let trading_allowed = desired_state_active_at(context, now);
        for slot in context.slots.iter().filter(|slot| slot.enabled) {
            self.store
                .renew_job_lease(&context.job, &self.owner, self.config.lease_duration)
                .await?;
            let token_id = context.outcomes.get(&slot.outcome).ok_or_else(|| {
                AppError::conflict(
                    "EXECUTION_OUTCOME_TOKEN_MISSING",
                    format!("slot {} has no token mapping", slot.slot_key),
                )
            })?;
            let actual = actual_by_slot.remove(&slot.id);
            let Some(book) = self.orderbooks.get(token_id).await else {
                if let Some(order) = actual {
                    self.cancel_order(connector, context, order, "BOOK_MISSING")
                        .await?;
                    apply_cancel_to_risk(&mut risk, order);
                }
                continue;
            };
            let target = if trading_allowed {
                build_target(
                    slot,
                    token_id,
                    &book,
                    now,
                    context.strategy_version.book_freshness_ms,
                )?
            } else {
                None
            };
            let Some(target) = target else {
                if let Some(order) = actual {
                    self.cancel_order(connector, context, order, "TARGET_BLOCKED")
                        .await?;
                    apply_cancel_to_risk(&mut risk, order);
                }
                continue;
            };
            let decision = compare_target(&target, actual, now, &context.strategy_version);
            match decision.action {
                DesiredAction::Keep => {}
                DesiredAction::Place => {
                    ensure_risk_budget_snapshot(&context.risk_policy, &risk, &target)?;
                    self.place_order(
                        connector,
                        context,
                        &target,
                        actual.map(|order| order.generation + 1).unwrap_or(1),
                    )
                    .await?;
                    apply_place_to_risk(&mut risk, &target);
                }
                DesiredAction::Cancel => {
                    if let Some(order) = actual {
                        self.cancel_order(connector, context, order, decision.reason)
                            .await?;
                        apply_cancel_to_risk(&mut risk, order);
                    }
                }
                DesiredAction::Replace => {
                    if replacements >= context.strategy_version.max_replaces_per_cycle {
                        continue;
                    }
                    if let Some(order) = actual {
                        self.cancel_order(connector, context, order, decision.reason)
                            .await?;
                        apply_cancel_to_risk(&mut risk, order);
                        ensure_risk_budget_snapshot(&context.risk_policy, &risk, &target)?;
                        self.place_order(connector, context, &target, order.generation + 1)
                            .await?;
                        apply_place_to_risk(&mut risk, &target);
                        replacements += 1;
                    }
                }
                DesiredAction::Blocked => {}
            }
        }
        // Slots removed from a published version must be cancelled; they are
        // never left live merely because no current target was generated.
        for order in actual_by_slot.into_values() {
            self.store
                .renew_job_lease(&context.job, &self.owner, self.config.lease_duration)
                .await?;
            self.cancel_order(connector, context, order, "SLOT_NOT_IN_TARGET")
                .await?;
            apply_cancel_to_risk(&mut risk, order);
        }
        Ok(())
    }

    async fn place_order(
        &self,
        connector: &LivePolymarketConnector,
        context: &ExecutionContext,
        target: &TargetOrder,
        generation: i64,
    ) -> Result<()> {
        let key = stable_idempotency_key(
            context.wallet.id,
            context.strategy_version.id,
            target.slot.id,
            generation,
            ActionKind::Place,
        );
        let request = LivePolymarketTokenOrderRequest {
            client_order_id: key.clone(),
            connector_name: "polymarket".to_string(),
            token_id: target.token_id.clone(),
            side: PolymarketTokenOrderSide::Buy,
            limit_price: polyedge_domain::Probability::new(target.price)?,
            quantity: polyedge_domain::Quantity::new(target.quantity)?,
            post_only: target.post_only,
        };
        let Some(action) = self
            .store
            .begin_action(
                &context.job,
                ActionProposal {
                    action: ActionKind::Place,
                    slot_id: Some(target.slot.id),
                    managed_order_id: None,
                    idempotency_key: key,
                    reason: "TARGET_PLACE".to_string(),
                    request: json!({"token_id": target.token_id, "price": target.price.to_string(), "quantity": target.quantity.to_string()}),
                },
            )
            .await?
        else {
            return Ok(());
        };
        match connector.find_matching_open_token_order(&request).await {
            Ok(Some(acceptance)) => {
                if let Err(error) = self
                    .store
                    .record_order_submitted(
                        context,
                        target,
                        generation,
                        &action,
                        &acceptance.order_id,
                    )
                    .await
                {
                    self.store
                        .mark_action_failed(&action, error.code(), error.message(), true)
                        .await?;
                    return Err(error);
                }
                self.store
                    .mark_action_succeeded(
                        &action,
                        json!({"recovered": true, "order_id": acceptance.order_id}),
                    )
                    .await
            }
            Ok(None) => match connector.submit_token_order(&request).await {
                Ok(LivePolymarketExecutionOutcome::Accepted(acceptance))
                    if matches!(
                        acceptance.status,
                        PolymarketAcceptedOrderStatus::Live
                            | PolymarketAcceptedOrderStatus::Unmatched
                    ) =>
                {
                    if let Err(error) = self
                        .store
                        .record_order_submitted(
                            context,
                            target,
                            generation,
                            &action,
                            &acceptance.order_id,
                        )
                        .await
                    {
                        self.store
                            .mark_action_failed(&action, error.code(), error.message(), true)
                            .await?;
                        return Err(error);
                    }
                    self.store
                        .mark_action_succeeded(&action, json!({"order_id": acceptance.order_id}))
                        .await
                }
                Ok(LivePolymarketExecutionOutcome::Accepted(_acceptance)) => {
                    self.store
                        .mark_action_failed(
                            &action,
                            "POLYMARKET_ORDER_NOT_LIVE",
                            "venue did not return a live order",
                            true,
                        )
                        .await?;
                    Err(AppError::conflict(
                        "POLYMARKET_ORDER_NOT_LIVE",
                        "venue did not return a live order",
                    ))
                }
                Ok(LivePolymarketExecutionOutcome::Rejected(rejection)) => {
                    self.store
                        .mark_action_failed(&action, &rejection.code, &rejection.message, false)
                        .await?;
                    Err(AppError::conflict(
                        "POLYMARKET_ORDER_REJECTED",
                        rejection.message,
                    ))
                }
                Err(error) => {
                    self.store
                        .mark_action_failed(&action, error.code(), error.message(), true)
                        .await?;
                    Err(error)
                }
            },
            Err(error) => {
                self.store
                    .mark_action_failed(&action, error.code(), error.message(), true)
                    .await?;
                Err(error)
            }
        }
    }

    async fn cancel_order(
        &self,
        connector: &LivePolymarketConnector,
        context: &ExecutionContext,
        order: &ManagedOrder,
        reason: &str,
    ) -> Result<()> {
        let external_id = order.external_order_id.as_deref().ok_or_else(|| {
            AppError::conflict(
                "EXECUTION_ORDER_EXTERNAL_ID_MISSING",
                format!("managed order {} has no venue id", order.id),
            )
        })?;
        let key = stable_idempotency_key(
            context.wallet.id,
            context.strategy_version.id,
            order.quote_slot_id.unwrap_or_default(),
            order.generation,
            ActionKind::Cancel,
        );
        let Some(action) = self
            .store
            .begin_action(
                &context.job,
                ActionProposal {
                    action: ActionKind::Cancel,
                    slot_id: order.quote_slot_id,
                    managed_order_id: Some(order.id),
                    idempotency_key: key,
                    reason: reason.to_string(),
                    request: json!({"external_order_id": external_id}),
                },
            )
            .await?
        else {
            return Ok(());
        };
        let request = polyedge_connectors::LivePolymarketCancelOrderRequest {
            connector_name: "polymarket".to_string(),
            external_order_id: external_id.to_string(),
        };
        match connector.cancel_order(&request).await {
            Ok(LivePolymarketCancelOutcome::Accepted(acceptance)) => {
                if let Err(error) = self.store.mark_order_cancelled(order, &action).await {
                    self.store
                        .mark_action_failed(&action, error.code(), error.message(), true)
                        .await?;
                    return Err(error);
                }
                self.store
                    .mark_action_succeeded(
                        &action,
                        json!({"cancelled_at": acceptance.cancelled_at}),
                    )
                    .await
            }
            Ok(LivePolymarketCancelOutcome::Rejected(rejection)) => {
                self.store
                    .mark_action_failed(&action, &rejection.code, &rejection.message, true)
                    .await?;
                Err(AppError::conflict(
                    "POLYMARKET_ORDER_CANCEL_REJECTED",
                    rejection.message,
                ))
            }
            Err(error) => {
                self.store
                    .mark_action_failed(&action, error.code(), error.message(), true)
                    .await?;
                Err(error)
            }
        }
    }
}

mod reconciliation;

include!("execution/planning.rs");

#[cfg(test)]
mod tests {
    use super::*;
    include!("execution/tests.rs");
}
