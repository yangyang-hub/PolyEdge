const REWARD_EVENT_CANCEL_MAX_DRAIN_TOKENS: usize = 512;

struct RewardEventCancelGuard {
    handle: JoinHandle<()>,
}

impl RewardEventCancelGuard {
    fn spawn(
        state: AppState,
        connector: LivePolymarketConnector,
        orderbook_cache: Arc<RewardOrderbookLocalCache>,
        mut rx: mpsc::Receiver<String>,
    ) -> Self {
        let handle = tokio::spawn(async move {
            let mut pending_tokens = HashSet::new();
            let mut book_history: HashMap<String, VecDeque<BookSnapshot>> = HashMap::new();

            while let Some(token_id) = rx.recv().await {
                remember_reward_orderbook_cancel_token(&mut pending_tokens, &token_id);
                drain_reward_orderbook_cancel_tokens(&mut rx, &mut pending_tokens);
                if pending_tokens.is_empty() {
                    continue;
                }

                let trace_id = new_trace_id();
                let tokens = std::mem::take(&mut pending_tokens);
                match run_reward_orderbook_event_cancel_fast_path(
                    &state,
                    &connector,
                    orderbook_cache.as_ref(),
                    &mut book_history,
                    tokens,
                    &trace_id,
                )
                .await
                {
                    Ok(report) if report.risk_cancelled_orders > 0 => {
                        info!(
                            trace_id = %trace_id,
                            risk_cancelled = report.risk_cancelled_orders,
                            "event-driven reward cancel fast path",
                        );
                    }
                    Ok(_) => {}
                    Err(error) => {
                        warn!(
                            trace_id = %trace_id,
                            error = %error,
                            "event-driven reward cancel fast path failed"
                        );
                    }
                }
            }
        });
        Self { handle }
    }
}

impl Drop for RewardEventCancelGuard {
    fn drop(&mut self) {
        self.handle.abort();
    }
}

fn remember_reward_orderbook_cancel_token(pending_tokens: &mut HashSet<String>, token_id: &str) {
    let token_id = token_id.trim();
    if !token_id.is_empty() {
        pending_tokens.insert(token_id.to_string());
    }
}

fn drain_reward_orderbook_cancel_tokens(
    rx: &mut mpsc::Receiver<String>,
    pending_tokens: &mut HashSet<String>,
) {
    for _ in 0..REWARD_EVENT_CANCEL_MAX_DRAIN_TOKENS {
        match rx.try_recv() {
            Ok(token_id) => remember_reward_orderbook_cancel_token(pending_tokens, &token_id),
            Err(mpsc::error::TryRecvError::Empty | mpsc::error::TryRecvError::Disconnected) => {
                break;
            }
        }
    }
}

fn remember_reward_event_cancel_plan_token(
    token_id: &str,
    seen: &mut HashSet<String>,
    active_order_tokens: &mut Vec<String>,
) {
    let token_id = token_id.trim();
    if token_id.is_empty() || !seen.insert(token_id.to_string()) {
        return;
    }
    active_order_tokens.push(token_id.to_string());
}

fn reward_event_cancel_active_order_tokens(
    _plans: &[RewardQuotePlan],
    open_orders: &[ManagedRewardOrder],
    token_ids: &HashSet<String>,
) -> Vec<String> {
    let mut active_order_tokens = Vec::new();
    let mut seen = HashSet::new();

    for order in open_orders.iter().filter(|order| {
        order.status.is_open_like()
            && live_event_cancel_order_matches_updated_tokens(order, token_ids)
    }) {
        remember_reward_event_cancel_plan_token(
            &order.token_id,
            &mut seen,
            &mut active_order_tokens,
        );
    }

    active_order_tokens
}

async fn run_reward_orderbook_event_cancel_fast_path(
    state: &AppState,
    connector: &LivePolymarketConnector,
    orderbook_cache: &RewardOrderbookLocalCache,
    book_history: &mut HashMap<String, VecDeque<BookSnapshot>>,
    token_ids: HashSet<String>,
    trace_id: &str,
) -> Result<RewardBotRunReport> {
    if token_ids.is_empty() {
        return Ok(RewardBotRunReport::default());
    }

    let cycle = state.reward_bot_service.current_live_cycle_state().await?;
    let active_order_tokens =
        reward_event_cancel_active_order_tokens(&cycle.plans, &cycle.open_orders, &token_ids);
    if active_order_tokens.is_empty() {
        return Ok(RewardBotRunReport::default());
    }

    let books =
        fetch_cached_reward_books(state, Some(orderbook_cache), &active_order_tokens).await?;
    record_reward_book_history(book_history, &books);
    let kill_switch = state.risk_service.read_state().await?.kill_switch;
    let cancel_candidates = live_event_hard_cancel_candidates_with_account(
        &cycle.config,
        &cycle.plans,
        &cycle.open_orders,
        &books,
        book_history,
        &cycle.account,
        &token_ids,
        kill_switch,
    );

    let mut report = RewardBotRunReport {
        books_fetched: books.len(),
        ..RewardBotRunReport::default()
    };
    if cancel_candidates.is_empty() {
        return Ok(report);
    }

    let mut account = cycle.account.clone();
    let mut open_orders = cycle.open_orders.clone();
    for (order_id, reason) in cancel_candidates {
        let Some(index) = open_orders.iter().position(|order| order.id == order_id) else {
            continue;
        };
        let order = open_orders[index].clone();
        match cancel_one_live_reward_order(connector, order, &reason, trace_id).await? {
            LiveRewardOrderUpdate::Changed(updated, event) => {
                open_orders[index] = updated.clone();
                if !live_cancel_result_is_unknown(&updated) {
                    report.cancelled_orders += 1;
                    report.risk_cancelled_orders += 1;
                }
                persist_live_reward_updates(
                    state,
                    &mut account,
                    Vec::new(),
                    vec![updated],
                    Vec::new(),
                    vec![event],
                    &report,
                    trace_id,
                )
                .await?;
            }
            LiveRewardOrderUpdate::Unchanged(event) | LiveRewardOrderUpdate::Retryable(event) => {
                persist_live_reward_updates(
                    state,
                    &mut account,
                    Vec::new(),
                    Vec::new(),
                    Vec::new(),
                    vec![event],
                    &report,
                    trace_id,
                )
                .await?;
            }
            LiveRewardOrderUpdate::CancelReplace(_) => {
                unreachable!("cancel_one_live_reward_order never returns CancelReplace")
            }
        }
    }

    if report.risk_cancelled_orders > 0 {
        debug!(
            trace_id = %trace_id,
            tokens = token_ids.len(),
            risk_cancelled = report.risk_cancelled_orders,
            "event-driven reward cancel fast path completed"
        );
    }
    Ok(report)
}

#[cfg(test)]
fn live_event_hard_cancel_candidates(
    config: &RewardBotConfig,
    plans: &[RewardQuotePlan],
    open_orders: &[ManagedRewardOrder],
    books: &HashMap<String, RewardOrderBook>,
    book_history: &HashMap<String, VecDeque<BookSnapshot>>,
    token_ids: &HashSet<String>,
    kill_switch: bool,
) -> Vec<(String, String)> {
    let account = RewardAccountState::fresh(
        &config.account_id,
        config.account_capital_usd,
        OffsetDateTime::now_utc(),
    );
    live_event_hard_cancel_candidates_with_account(
        config,
        plans,
        open_orders,
        books,
        book_history,
        &account,
        token_ids,
        kill_switch,
    )
}

fn live_event_hard_cancel_candidates_with_account(
    config: &RewardBotConfig,
    plans: &[RewardQuotePlan],
    open_orders: &[ManagedRewardOrder],
    books: &HashMap<String, RewardOrderBook>,
    book_history: &HashMap<String, VecDeque<BookSnapshot>>,
    _account: &RewardAccountState,
    token_ids: &HashSet<String>,
    kill_switch: bool,
) -> Vec<(String, String)> {
    let plan_index = reward_live_plan_index(plans);
    let now = OffsetDateTime::now_utc();
    open_orders
        .iter()
        .filter(|order| {
            order.status.is_open_like()
                && live_event_cancel_order_matches_updated_tokens(order, token_ids)
        })
        .filter_map(|order| {
            let order_config = reward_live_plan_for_order(&plan_index, order)
                .map(|plan| {
                    config
                        .config_for_strategy_bucket(plan.strategy_bucket)
                        .config_for_strategy_profile(plan.strategy_profile)
                })
                .unwrap_or_else(|| config.config_for_strategy_profile(order.strategy_profile));
            live_event_hard_cancel_reason(
                &order_config,
                &plan_index,
                books,
                book_history,
                order,
                now,
                kill_switch,
            )
            .map(|reason| (order.id.clone(), reason))
        })
        .collect()
}

fn live_event_cancel_order_matches_updated_tokens(
    order: &ManagedRewardOrder,
    token_ids: &HashSet<String>,
) -> bool {
    token_ids.contains(&order.token_id)
}

fn live_event_hard_cancel_reason(
    config: &RewardBotConfig,
    plans: &RewardPlanIndex<'_>,
    books: &HashMap<String, RewardOrderBook>,
    book_history: &HashMap<String, VecDeque<BookSnapshot>>,
    order: &ManagedRewardOrder,
    now: OffsetDateTime,
    kill_switch: bool,
) -> Option<String> {
    if live_order_has_post_only_violation(order) {
        if order
            .reason
            .contains("cancel accepted; awaiting final reconciliation")
            && !live_cancel_final_reconciliation_retry_due(order, now)
        {
            return None;
        }
        return live_cancel_retry_due(order, now)
            .then(|| "post-only violation requires cancellation".to_string());
    }
    if order.reason.contains("cancellation must be retried") {
        return live_cancel_retry_due(order, now)
            .then(|| "previous cancellation attempt left the order live".to_string());
    }
    if order.reason.contains("awaiting final reconciliation")
        || live_submission_was_attempted(order)
    {
        return None;
    }
    if kill_switch && order.side == RewardOrderSide::Buy {
        return Some("global kill switch is active".to_string());
    }
    if order.side == RewardOrderSide::Sell
        && order.status == ManagedRewardOrderStatus::ExitPending
        && order.external_order_id.is_none()
    {
        return None;
    }
    if let Some(reason) = live_quote_book_missing_or_empty_reason(books, &order.token_id) {
        return Some(reason);
    }
    let stale_age_ms = live_quote_book_stale_age_ms(config, books, &order.token_id, now);
    if order.side != RewardOrderSide::Buy {
        return stale_age_ms
            .map(|age_ms| live_orderbook_stale_reason(age_ms, config.stale_book_ms));
    }

    let Some(plan) = reward_live_plan_for_order(plans, order) else {
        return Some(reward_live_missing_order_plan_reason(plans, order));
    };
    if reward_quote_plan_event_window_cancels_open_buy(plan) {
        let reason = plan
            .event_window
            .as_ref()
            .map(|assessment| assessment.reason.as_str())
            .unwrap_or("event window requires BUY cancellation");
        return Some(format!("event window requires BUY cancellation: {reason}"));
    }
    if let Some(reason) = live_provider_cancel_reason(config, plan, order) {
        return Some(reason);
    }
    if let Some(age_ms) = stale_age_ms {
        if live_stale_orderbook_cancel_grace_active(config, order, now) {
            return None;
        }
        return Some(live_orderbook_stale_reason(age_ms, config.stale_book_ms));
    }
    if let Some(reason) = live_token_spread_cancel_reason(config, books, order) {
        return Some(reason);
    }
    if let Some(best_ask) = reward_buy_touching_ask(order, books) {
        return Some(format!(
            "post-only buy would touch best ask {best_ask} at order price {}",
            order.price
        ));
    }
    if let Some(reason) = live_order_trading_edge_cancel_reason(config, plan, order) {
        return Some(reason);
    }
    if !plan.eligible {
        return None;
    }
    if !plan.legs.iter().any(|leg| leg.token_id == order.token_id) {
        return Some("token no longer appears in live quote plan".to_string());
    }
    if let Some(reason) = live_min_depth_cancel_reason(config, books, order) {
        return Some(reason);
    }
    if let Some(reason) = live_bid_rank_cancel_reason(config, books, order) {
        return Some(reason);
    }
    if let Some(reason) = live_depth_drop_cancel_reason(config, books, book_history, order, now) {
        return Some(reason);
    }
    if let Some(reason) = live_fill_velocity_cancel_reason(config, books, book_history, order, now)
    {
        return Some(reason);
    }
    if let Some(reason) = live_mass_cancel_reason(config, books, book_history, order, now) {
        return Some(reason);
    }
    None
}
