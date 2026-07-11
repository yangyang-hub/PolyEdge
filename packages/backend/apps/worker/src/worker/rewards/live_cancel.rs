type RewardPlanIndex<'a> =
    HashMap<(&'a str, RewardStrategyProfile), &'a RewardQuotePlan>;

fn reward_live_plan_index(plans: &[RewardQuotePlan]) -> RewardPlanIndex<'_> {
    plans
        .iter()
        .map(|plan| ((plan.condition_id.as_str(), plan.strategy_profile), plan))
        .collect()
}

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
    let plan_index = reward_live_plan_index(plans);
    let now = OffsetDateTime::now_utc();
    let mut hard_candidates = Vec::new();
    let mut drift_candidates = Vec::new();
    for (order_id, reason) in open_orders
        .iter()
        .filter(|order| order.status.is_open_like())
        .filter_map(|order| {
            let order_config = reward_live_plan_for_order(&plan_index, order)
                .map(|plan| {
                    config
                        .config_for_strategy_bucket(plan.strategy_bucket)
                        .config_for_strategy_profile(plan.strategy_profile)
                })
                .unwrap_or_else(|| config.config_for_strategy_profile(order.strategy_profile));
            live_cancel_reason(
                &order_config,
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
    plans: &RewardPlanIndex<'_>,
    books: &HashMap<String, RewardOrderBook>,
    book_history: &HashMap<String, VecDeque<BookSnapshot>>,
    _open_orders: &[ManagedRewardOrder],
    _account: &RewardAccountState,
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
    if reward_quote_plan_event_window_blocks_new_buy(plan)
        && order.status == ManagedRewardOrderStatus::Planned
        && order.external_order_id.is_none()
    {
        let reason = plan
            .event_window
            .as_ref()
            .map(|assessment| assessment.reason.as_str())
            .unwrap_or("event window blocks new BUY quotes");
        return Some(format!("event window blocks new BUY submission: {reason}"));
    }
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
    let leg = plan.legs.iter().find(|leg| leg.token_id == order.token_id);
    // Ineligible means stop adding exposure, not automatically canceling an
    // otherwise safe resting order. Explicit provider/event/fair-value actions
    // above still cancel. Missing legs are expected for stop-new plans.
    if !plan.eligible {
        return None;
    }
    let Some(leg) = leg else {
        return Some("token no longer appears in live quote plan".to_string());
    };
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
    if let Some(reason) = live_fill_velocity_cancel_reason(config, books, book_history, order, now)
    {
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

fn live_provider_cancel_reason(
    config: &RewardBotConfig,
    plan: &RewardQuotePlan,
    order: &ManagedRewardOrder,
) -> Option<String> {
    // AI advisory is structurally incapable of cancelling a resting order.
    // Only evidence-backed info-risk actions enter this cancellation path.
    if config.info_risk_enabled
        && config.info_risk_mode == polyedge_application::RewardSelectionMode::Enforce
        && let Some(risk) = &plan.info_risk
    {
        let action = reward_info_risk_effective_action(
            risk,
            config.info_risk_avoid_level,
            config.info_risk_min_confidence,
        );
        if action.cancels_outcome(&order.outcome) {
            return Some(format!(
                "info risk {} requires {} cancellation: {}",
                risk.risk_type.as_str(),
                action.as_str(),
                risk.summary
            ));
        }
    }
    None
}

fn live_order_trading_edge_cancel_reason(
    config: &RewardBotConfig,
    plan: &RewardQuotePlan,
    order: &ManagedRewardOrder,
) -> Option<String> {
    if !config.fair_value_enabled || order.side != RewardOrderSide::Buy {
        return None;
    }
    let decision = plan.fair_value.as_ref()?;
    if let Some(reason) = &decision.estimate.do_not_quote_reason {
        return Some(format!("fair-value estimate unsafe for resting order: {reason}"));
    }
    let fair_price = if order.outcome.eq_ignore_ascii_case("no") {
        decision.estimate.fair_no
    } else {
        decision.estimate.fair_yes
    };
    let raw_edge_cents = ((fair_price - order.price) * Decimal::from(100_u64)).round_dp(4);
    let provider_buffer = plan.ai_advisory.as_ref().map_or(Decimal::ZERO, |advisory| {
        reward_ai_edge_buffer_cents(advisory, config)
    });
    let effective_edge_cents =
        (raw_edge_cents - decision.estimate.uncertainty_cents - provider_buffer).round_dp(4);
    if raw_edge_cents < config.fair_value_min_raw_edge_cents
        || effective_edge_cents < config.fair_value_min_effective_edge_cents
    {
        return Some(format!(
            "resting BUY trading edge unsafe: raw={raw_edge_cents}c effective={effective_edge_cents}c"
        ));
    }
    None
}

fn reward_live_plan_for_order<'a>(
    plans: &'a RewardPlanIndex<'a>,
    order: &ManagedRewardOrder,
) -> Option<&'a RewardQuotePlan> {
    plans
        .get(&(order.condition_id.as_str(), order.strategy_profile))
        .copied()
}

fn reward_live_missing_order_plan_reason(
    plans: &RewardPlanIndex<'_>,
    order: &ManagedRewardOrder,
) -> String {
    if plans
        .keys()
        .any(|(condition_id, _)| *condition_id == order.condition_id.as_str())
    {
        format!(
            "strategy profile {} no longer appears in live quote plan",
            order.strategy_profile.as_str()
        )
    } else {
        "market no longer offers rewards".to_string()
    }
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
