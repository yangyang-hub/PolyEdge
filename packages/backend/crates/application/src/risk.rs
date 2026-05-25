use super::{
    AuditLogEntry, AuditLogSink, AuthenticatedActor, MarketEventService, MarketView,
    ModeTransitionCommand, SignalView, SystemModeService,
};
use async_trait::async_trait;
use polyedge_domain::{
    AppError, AuditResult, ExposureRatio, Probability, Result, SignalLifecycleState,
    SignedUsdAmount, SystemMode, TradabilityStatus, UsdAmount,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use time::OffsetDateTime;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskStateSnapshot {
    pub kill_switch: bool,
    pub daily_pnl: SignedUsdAmount,
    pub gross_exposure: ExposureRatio,
    pub net_exposure: ExposureRatio,
    pub open_alerts: u32,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
    pub version: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskStateView {
    pub mode: SystemMode,
    pub kill_switch: bool,
    pub daily_pnl: SignedUsdAmount,
    pub gross_exposure: ExposureRatio,
    pub net_exposure: ExposureRatio,
    pub open_alerts: u32,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
    pub version: i64,
}

#[derive(Debug, Clone)]
pub struct RiskPolicy {
    pub exposure_reference_nav: UsdAmount,
    pub min_signal_confidence: Probability,
    pub min_edge_to_execute: Probability,
    pub max_open_alerts: u32,
    pub max_daily_loss: UsdAmount,
    pub max_gross_exposure: ExposureRatio,
    pub max_net_exposure: ExposureRatio,
}

#[derive(Debug, Clone)]
pub struct ApproveSignalCommand {
    pub signal_id: String,
    pub reason: String,
    pub expected_version: Option<i64>,
    pub request_id: String,
    pub trace_id: String,
    pub actor: AuthenticatedActor,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApproveSignalReceipt {
    pub signal: SignalView,
    pub risk_state: RiskStateView,
    pub replayed: bool,
}

#[derive(Debug, Clone)]
pub struct RejectSignalCommand {
    pub signal_id: String,
    pub reason: String,
    pub expected_version: Option<i64>,
    pub request_id: String,
    pub trace_id: String,
    pub actor: AuthenticatedActor,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RejectSignalReceipt {
    pub signal: SignalView,
    pub risk_state: RiskStateView,
    pub replayed: bool,
}

#[derive(Debug, Clone)]
pub struct TriggerKillSwitchCommand {
    pub reason: String,
    pub expected_version: Option<i64>,
    pub request_id: String,
    pub trace_id: String,
    pub actor: AuthenticatedActor,
}

#[derive(Debug, Clone)]
pub struct ReleaseKillSwitchCommand {
    pub reason: String,
    pub to_mode: SystemMode,
    pub expected_version: Option<i64>,
    pub request_id: String,
    pub trace_id: String,
    pub actor: AuthenticatedActor,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KillSwitchReceipt {
    pub risk_state: RiskStateView,
    pub replayed: bool,
}

#[async_trait]
pub trait RiskStateStore: Send + Sync {
    async fn current(&self) -> Result<RiskStateSnapshot>;

    async fn set_kill_switch(
        &self,
        kill_switch: bool,
        trace_id: &str,
        expected_version: Option<i64>,
    ) -> Result<RiskStateSnapshot>;

    async fn update_metrics(
        &self,
        daily_pnl: SignedUsdAmount,
        gross_exposure: ExposureRatio,
        net_exposure: ExposureRatio,
        trace_id: &str,
    ) -> Result<RiskStateSnapshot>;
}

pub struct RiskService {
    policy: RiskPolicy,
    risk_state_store: Arc<dyn RiskStateStore>,
    market_event_service: Arc<MarketEventService>,
    system_mode_service: Arc<SystemModeService>,
    audit_log_sink: Arc<dyn AuditLogSink>,
}

impl RiskService {
    pub fn new(
        policy: RiskPolicy,
        risk_state_store: Arc<dyn RiskStateStore>,
        market_event_service: Arc<MarketEventService>,
        system_mode_service: Arc<SystemModeService>,
        audit_log_sink: Arc<dyn AuditLogSink>,
    ) -> Self {
        Self {
            policy,
            risk_state_store,
            market_event_service,
            system_mode_service,
            audit_log_sink,
        }
    }

    #[must_use]
    pub fn policy(&self) -> &RiskPolicy {
        &self.policy
    }

    pub async fn read_state(&self) -> Result<RiskStateView> {
        let snapshot = self.risk_state_store.current().await?;
        let mode_snapshot = self.system_mode_service.read_mode().await?;

        Ok(RiskStateView {
            mode: mode_snapshot.mode,
            kill_switch: snapshot.kill_switch || mode_snapshot.mode == SystemMode::KillSwitchLocked,
            daily_pnl: snapshot.daily_pnl,
            gross_exposure: snapshot.gross_exposure,
            net_exposure: snapshot.net_exposure,
            open_alerts: snapshot.open_alerts,
            updated_at: snapshot.updated_at,
            version: snapshot.version,
        })
    }

    pub async fn sync_execution_metrics(
        &self,
        daily_pnl: SignedUsdAmount,
        gross_exposure: ExposureRatio,
        net_exposure: ExposureRatio,
        trace_id: &str,
    ) -> Result<RiskStateView> {
        let snapshot = self
            .risk_state_store
            .update_metrics(daily_pnl, gross_exposure, net_exposure, trace_id)
            .await?;
        let mode_snapshot = self.system_mode_service.read_mode().await?;

        Ok(RiskStateView {
            mode: mode_snapshot.mode,
            kill_switch: snapshot.kill_switch || mode_snapshot.mode == SystemMode::KillSwitchLocked,
            daily_pnl: snapshot.daily_pnl,
            gross_exposure: snapshot.gross_exposure,
            net_exposure: snapshot.net_exposure,
            open_alerts: snapshot.open_alerts,
            updated_at: snapshot.updated_at,
            version: snapshot.version,
        })
    }

    pub async fn approve_signal(
        &self,
        command: ApproveSignalCommand,
    ) -> Result<ApproveSignalReceipt> {
        if command.signal_id.trim().is_empty() {
            return Err(AppError::invalid_input(
                "SIGNAL_ID_REQUIRED",
                "signal id must not be empty",
            ));
        }

        if command.reason.trim().is_empty() {
            return Err(AppError::invalid_input(
                "SIGNAL_APPROVAL_REASON_REQUIRED",
                "approval reason must not be empty",
            ));
        }

        let signal = self
            .market_event_service
            .get_signal(&command.signal_id)
            .await?;
        let market = self
            .market_event_service
            .get_market(&signal.market_id)
            .await?;
        let risk_state = self.read_state().await?;

        if let Err(error) = evaluate_signal_approval(&signal, &market, &risk_state, &self.policy) {
            self.append_signal_audit(
                "signal.approve",
                &command.request_id,
                &command.trace_id,
                &command.actor,
                &command.signal_id,
                &command.reason,
                AuditResult::Rejected,
                Some(error.code().to_string()),
            )
            .await?;
            return Err(error);
        }

        let approval_note = format!(
            "Manually approved in {} mode: {}",
            risk_state.mode.as_str(),
            command.reason.trim(),
        );

        let signal = match self
            .market_event_service
            .approve_signal(
                &command.signal_id,
                &command.actor.user_id,
                &approval_note,
                &command.trace_id,
                command.expected_version,
            )
            .await
        {
            Ok(signal) => signal,
            Err(error) => {
                self.append_signal_audit(
                    "signal.approve",
                    &command.request_id,
                    &command.trace_id,
                    &command.actor,
                    &command.signal_id,
                    &command.reason,
                    AuditResult::Rejected,
                    Some(error.code().to_string()),
                )
                .await?;
                return Err(error);
            }
        };

        self.append_signal_audit(
            "signal.approve",
            &command.request_id,
            &command.trace_id,
            &command.actor,
            &command.signal_id,
            &command.reason,
            AuditResult::Succeeded,
            None,
        )
        .await?;

        Ok(ApproveSignalReceipt {
            signal,
            risk_state,
            replayed: false,
        })
    }

    pub async fn reject_signal(&self, command: RejectSignalCommand) -> Result<RejectSignalReceipt> {
        if command.signal_id.trim().is_empty() {
            return Err(AppError::invalid_input(
                "SIGNAL_ID_REQUIRED",
                "signal id must not be empty",
            ));
        }

        if command.reason.trim().is_empty() {
            return Err(AppError::invalid_input(
                "SIGNAL_REJECTION_REASON_REQUIRED",
                "rejection reason must not be empty",
            ));
        }

        let signal = self
            .market_event_service
            .get_signal(&command.signal_id)
            .await?;
        let risk_state = self.read_state().await?;

        if let Err(error) = evaluate_signal_rejection(&signal, &risk_state) {
            self.append_signal_audit(
                "signal.reject",
                &command.request_id,
                &command.trace_id,
                &command.actor,
                &command.signal_id,
                &command.reason,
                AuditResult::Rejected,
                Some(error.code().to_string()),
            )
            .await?;
            return Err(error);
        }

        let rejection_note = format!(
            "Manually rejected in {} mode: {}",
            risk_state.mode.as_str(),
            command.reason.trim(),
        );

        let signal = match self
            .market_event_service
            .reject_signal(
                &command.signal_id,
                &command.actor.user_id,
                &rejection_note,
                &command.trace_id,
                command.expected_version,
            )
            .await
        {
            Ok(signal) => signal,
            Err(error) => {
                self.append_signal_audit(
                    "signal.reject",
                    &command.request_id,
                    &command.trace_id,
                    &command.actor,
                    &command.signal_id,
                    &command.reason,
                    AuditResult::Rejected,
                    Some(error.code().to_string()),
                )
                .await?;
                return Err(error);
            }
        };

        self.append_signal_audit(
            "signal.reject",
            &command.request_id,
            &command.trace_id,
            &command.actor,
            &command.signal_id,
            &command.reason,
            AuditResult::Succeeded,
            None,
        )
        .await?;

        Ok(RejectSignalReceipt {
            signal,
            risk_state,
            replayed: false,
        })
    }

    pub async fn trigger_kill_switch(
        &self,
        command: TriggerKillSwitchCommand,
    ) -> Result<KillSwitchReceipt> {
        if command.reason.trim().is_empty() {
            return Err(AppError::invalid_input(
                "KILL_SWITCH_REASON_REQUIRED",
                "kill switch reason must not be empty",
            ));
        }

        let current = self.read_state().await?;
        if current.kill_switch {
            return Err(AppError::conflict(
                "RISK_KILL_SWITCH_ALREADY_ACTIVE",
                "kill switch is already active",
            ));
        }

        if current.mode != SystemMode::LiveAuto {
            return Err(AppError::conflict(
                "RISK_KILL_SWITCH_MODE_INVALID",
                "kill switch can only be triggered while live_auto mode is active",
            ));
        }

        self.risk_state_store
            .set_kill_switch(true, &command.trace_id, command.expected_version)
            .await?;

        self.system_mode_service
            .transition_mode_without_idempotency(ModeTransitionCommand {
                to_mode: SystemMode::KillSwitchLocked,
                reason: format!("kill switch triggered: {}", command.reason.trim()),
                request_id: command.request_id.clone(),
                trace_id: command.trace_id.clone(),
                idempotency_key: format!("kill-switch-trigger-{}", command.trace_id),
                request_hash: command.trace_id.clone(),
                actor: command.actor.clone(),
                required_scope: polyedge_domain::StepUpScope::SystemKillSwitchTrigger,
            })
            .await?;

        self.audit_log_sink
            .append(AuditLogEntry {
                occurred_at: OffsetDateTime::now_utc(),
                request_id: command.request_id,
                trace_id: command.trace_id,
                actor: command.actor,
                action: "system.kill_switch.trigger".to_string(),
                resource_type: "risk_state".to_string(),
                resource_id: "global".to_string(),
                reason: command.reason,
                result: AuditResult::Succeeded,
                error_code: None,
            })
            .await?;

        Ok(KillSwitchReceipt {
            risk_state: self.read_state().await?,
            replayed: false,
        })
    }

    pub async fn release_kill_switch(
        &self,
        command: ReleaseKillSwitchCommand,
    ) -> Result<KillSwitchReceipt> {
        if command.reason.trim().is_empty() {
            return Err(AppError::invalid_input(
                "KILL_SWITCH_REASON_REQUIRED",
                "kill switch reason must not be empty",
            ));
        }

        if matches!(
            command.to_mode,
            SystemMode::KillSwitchLocked | SystemMode::LiveAuto | SystemMode::ManualConfirm
        ) {
            return Err(AppError::invalid_input(
                "KILL_SWITCH_RELEASE_MODE_INVALID",
                "kill switch release target must be research or paper_trade",
            ));
        }

        let current = self.read_state().await?;
        if !current.kill_switch {
            return Err(AppError::conflict(
                "RISK_KILL_SWITCH_NOT_ACTIVE",
                "kill switch is not currently active",
            ));
        }

        self.risk_state_store
            .set_kill_switch(false, &command.trace_id, command.expected_version)
            .await?;

        if current.mode != command.to_mode {
            self.system_mode_service
                .transition_mode_without_idempotency(ModeTransitionCommand {
                    to_mode: command.to_mode,
                    reason: format!("kill switch released: {}", command.reason.trim()),
                    request_id: command.request_id.clone(),
                    trace_id: command.trace_id.clone(),
                    idempotency_key: format!("kill-switch-release-{}", command.trace_id),
                    request_hash: command.trace_id.clone(),
                    actor: command.actor.clone(),
                    required_scope: polyedge_domain::StepUpScope::SystemKillSwitchRelease,
                })
                .await?;
        }

        self.audit_log_sink
            .append(AuditLogEntry {
                occurred_at: OffsetDateTime::now_utc(),
                request_id: command.request_id,
                trace_id: command.trace_id,
                actor: command.actor,
                action: "system.kill_switch.release".to_string(),
                resource_type: "risk_state".to_string(),
                resource_id: "global".to_string(),
                reason: command.reason,
                result: AuditResult::Succeeded,
                error_code: None,
            })
            .await?;

        Ok(KillSwitchReceipt {
            risk_state: self.read_state().await?,
            replayed: false,
        })
    }

    async fn append_signal_audit(
        &self,
        action: &str,
        request_id: &str,
        trace_id: &str,
        actor: &AuthenticatedActor,
        signal_id: &str,
        reason: &str,
        result: AuditResult,
        error_code: Option<String>,
    ) -> Result<()> {
        self.audit_log_sink
            .append(AuditLogEntry {
                occurred_at: OffsetDateTime::now_utc(),
                request_id: request_id.to_string(),
                trace_id: trace_id.to_string(),
                actor: actor.clone(),
                action: action.to_string(),
                resource_type: "signal".to_string(),
                resource_id: signal_id.to_string(),
                reason: reason.to_string(),
                result,
                error_code,
            })
            .await
    }
}

fn evaluate_signal_approval(
    signal: &SignalView,
    market: &MarketView,
    risk_state: &RiskStateView,
    policy: &RiskPolicy,
) -> Result<()> {
    if risk_state.kill_switch {
        return Err(AppError::forbidden(
            "RISK_KILL_SWITCH_ACTIVE",
            "signal approval is blocked while the kill switch is active",
        ));
    }

    if risk_state.mode != SystemMode::ManualConfirm {
        return Err(AppError::conflict(
            "STATE_APPROVAL_MODE_INVALID",
            "signal approval is only available in manual_confirm mode",
        ));
    }

    if signal.approved_by_user_id.is_some() {
        return Err(AppError::conflict(
            "STATE_SIGNAL_ALREADY_APPROVED",
            "signal has already been approved",
        ));
    }

    if signal.rejected_by_user_id.is_some() {
        return Err(AppError::conflict(
            "STATE_SIGNAL_ALREADY_REJECTED",
            "signal has already been rejected for the current version",
        ));
    }

    if !matches!(
        signal.lifecycle_state,
        SignalLifecycleState::New | SignalLifecycleState::Active
    ) {
        return Err(AppError::conflict(
            "STATE_SIGNAL_NOT_APPROVABLE",
            "only new or active signals can be approved",
        ));
    }

    match market.tradability_status {
        TradabilityStatus::Blocked => {
            return Err(AppError::forbidden(
                "RISK_MARKET_BLOCKED",
                "market is blocked from execution",
            ));
        }
        TradabilityStatus::ObserveOnly => {
            return Err(AppError::forbidden(
                "RISK_MARKET_OBSERVE_ONLY",
                "market is in observe_only mode and cannot be approved for execution",
            ));
        }
        TradabilityStatus::Tradable | TradabilityStatus::ManualReview => {}
    }

    if signal.confidence.value() < policy.min_signal_confidence.value() {
        return Err(AppError::forbidden(
            "RISK_CONFIDENCE_TOO_LOW",
            "signal confidence is below the configured approval threshold",
        ));
    }

    if signal.edge.value().abs() < policy.min_edge_to_execute.value() {
        return Err(AppError::forbidden(
            "RISK_EDGE_TOO_LOW",
            "signal edge is below the configured approval threshold",
        ));
    }

    if risk_state.open_alerts > policy.max_open_alerts {
        return Err(AppError::forbidden(
            "RISK_OPEN_ALERT_LIMIT_EXCEEDED",
            "open risk alerts exceed the configured approval threshold",
        ));
    }

    if risk_state.daily_pnl.value() <= -policy.max_daily_loss.value() {
        return Err(AppError::forbidden(
            "RISK_DAILY_LOSS_LIMIT_EXCEEDED",
            "daily pnl breach prevents approving new execution",
        ));
    }

    if risk_state.gross_exposure.value() >= policy.max_gross_exposure.value() {
        return Err(AppError::forbidden(
            "RISK_GROSS_EXPOSURE_LIMIT_EXCEEDED",
            "gross exposure is already at or above the configured limit",
        ));
    }

    if risk_state.net_exposure.value() >= policy.max_net_exposure.value() {
        return Err(AppError::forbidden(
            "RISK_NET_EXPOSURE_LIMIT_EXCEEDED",
            "net exposure is already at or above the configured limit",
        ));
    }

    Ok(())
}

fn evaluate_signal_rejection(signal: &SignalView, risk_state: &RiskStateView) -> Result<()> {
    if !matches!(
        risk_state.mode,
        SystemMode::ManualConfirm | SystemMode::KillSwitchLocked
    ) {
        return Err(AppError::conflict(
            "STATE_REJECTION_MODE_INVALID",
            "signal rejection is only available in manual_confirm or kill_switch_locked mode",
        ));
    }

    if signal.approved_by_user_id.is_some() {
        return Err(AppError::conflict(
            "STATE_SIGNAL_ALREADY_APPROVED",
            "approved signals cannot be rejected for the current version",
        ));
    }

    if signal.rejected_by_user_id.is_some() {
        return Err(AppError::conflict(
            "STATE_SIGNAL_ALREADY_REJECTED",
            "signal has already been rejected for the current version",
        ));
    }

    if !matches!(
        signal.lifecycle_state,
        SignalLifecycleState::New | SignalLifecycleState::Active | SignalLifecycleState::Weakened
    ) {
        return Err(AppError::conflict(
            "STATE_SIGNAL_NOT_REJECTABLE",
            "only new, active, or weakened signals can be rejected",
        ));
    }

    Ok(())
}
