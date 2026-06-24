#[cfg(test)]
fn live_cancel_candidates(
    config: &RewardBotConfig,
    plans: &[RewardQuotePlan],
    open_orders: &[ManagedRewardOrder],
    books: &HashMap<String, RewardOrderBook>,
    book_history: &HashMap<String, VecDeque<BookSnapshot>>,
    kill_switch: bool,
) -> Vec<(String, String)> {
    let account = RewardAccountState::fresh(
        &config.account_id,
        config.account_capital_usd,
        OffsetDateTime::now_utc(),
    );
    live_cancel_candidates_with_account(
        config,
        plans,
        open_orders,
        books,
        book_history,
        &account,
        kill_switch,
    )
}

fn live_cancel_candidates_with_account(
    config: &RewardBotConfig,
    plans: &[RewardQuotePlan],
    open_orders: &[ManagedRewardOrder],
    books: &HashMap<String, RewardOrderBook>,
    book_history: &HashMap<String, VecDeque<BookSnapshot>>,
    account: &RewardAccountState,
    kill_switch: bool,
) -> Vec<(String, String)> {
    let plan_index: HashMap<&str, &RewardQuotePlan> = plans
        .iter()
        .map(|plan| (plan.condition_id.as_str(), plan))
        .collect();
    let now = OffsetDateTime::now_utc();
    let mut hard_candidates = Vec::new();
    let mut drift_candidates = Vec::new();
    for (order_id, reason) in open_orders
        .iter()
        .filter(|order| order.status.is_open_like())
        .filter_map(|order| {
            live_cancel_reason(
                config,
                &plan_index,
                books,
                book_history,
                open_orders,
                account,
                order,
                now,
                kill_switch,
            )
            .map(|reason| (order.id.clone(), reason))
        })
    {
        if live_cancel_reason_is_requote_drift(&reason) {
            drift_candidates.push((order_id, reason));
        } else {
            hard_candidates.push((order_id, reason));
        }
    }

    let drift_limit = usize::from(config.requote_drift_max_cancels_per_cycle);
    if drift_limit > 0 {
        hard_candidates.extend(drift_candidates.into_iter().take(drift_limit));
    }
    hard_candidates
}

fn live_cancel_reason(
    config: &RewardBotConfig,
    plans: &HashMap<&str, &RewardQuotePlan>,
    books: &HashMap<String, RewardOrderBook>,
    book_history: &HashMap<String, VecDeque<BookSnapshot>>,
    open_orders: &[ManagedRewardOrder],
    account: &RewardAccountState,
    order: &ManagedRewardOrder,
    now: OffsetDateTime,
    kill_switch: bool,
) -> Option<String> {
    if live_order_has_post_only_violation(order) {
        if order.reason.contains("cancel accepted; awaiting final reconciliation")
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
    let Some(plan) = plans.get(order.condition_id.as_str()) else {
        return Some("market no longer offers rewards".to_string());
    };
    if !plan.eligible {
        return Some("market dropped below eligibility threshold".to_string());
    }
    let Some(leg) = plan.legs.iter().find(|leg| leg.token_id == order.token_id) else {
        return Some("token no longer appears in live quote plan".to_string());
    };
    if let Some(age_ms) = stale_age_ms {
        if live_stale_orderbook_cancel_grace_active(config, order, now) {
            return None;
        }
        return Some(live_orderbook_stale_reason(age_ms, config.stale_book_ms));
    }
    if let Some(best_ask) = reward_buy_touching_ask(order, books) {
        return Some(format!(
            "post-only buy would touch best ask {best_ask} at order price {}",
            order.price
        ));
    }
    if order.strategy_bucket == RewardStrategyBucket::LowCompetition
        || plan.strategy_bucket == RewardStrategyBucket::LowCompetition
    {
        if plan.strategy_bucket != RewardStrategyBucket::LowCompetition {
            return Some("low-competition order no longer has low-competition quote plan".to_string());
        }
        return low_competition_live_cancel_reason(
            config,
            plan,
            books,
            book_history,
            open_orders,
            account,
        );
    }
    if let Some(reason) =
        live_requote_drift_cancel_reason(config, book_history, order, leg.price, now)
    {
        return Some(reason);
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
    if let Some(reason) = live_fill_velocity_cancel_reason(config, books, book_history, order, now) {
        return Some(reason);
    }
    if let Some(reason) = live_mass_cancel_reason(config, books, book_history, order, now) {
        return Some(reason);
    }
    if let Some(reason) = live_requote_age_cancel_reason(config, order, now) {
        return Some(reason);
    }
    None
}

fn live_cancel_retry_due(order: &ManagedRewardOrder, now: OffsetDateTime) -> bool {
    now >= order.updated_at + LIVE_CANCEL_RETRY_MIN_INTERVAL
}

fn live_cancel_final_reconciliation_retry_due(
    order: &ManagedRewardOrder,
    now: OffsetDateTime,
) -> bool {
    now >= order.updated_at + LIVE_CANCEL_FINAL_RECONCILIATION_RETRY_AFTER
}
