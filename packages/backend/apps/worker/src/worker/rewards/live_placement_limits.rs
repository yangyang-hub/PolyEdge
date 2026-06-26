fn live_condition_budget_capped_by_positions(
    config: &RewardBotConfig,
    plan_legs: &[RewardQuoteLeg],
    positions: &[RewardPosition],
    raw_budget: Decimal,
) -> Decimal {
    let mut budget = raw_budget;
    if config.max_position_usd > Decimal::ZERO {
        let min_headroom = plan_legs
            .iter()
            .map(|leg| {
                let current = positions
                    .iter()
                    .find(|p| p.token_id == leg.token_id && p.size > Decimal::ZERO)
                    .map(|p| (p.size * leg.price).round_dp(4))
                    .unwrap_or_default();
                (config.max_position_usd - current).max(Decimal::ZERO)
            })
            .min()
            .unwrap_or(raw_budget);
        // Both legs share one condition collateral; cap total so each leg
        // stays within its position limit.
        budget = Decimal::min(budget, min_headroom * Decimal::from(plan_legs.len().max(1) as u64));
    }
    if config.max_global_position_usd > Decimal::ZERO {
        let current = live_global_inventory_notional(positions);
        let headroom = (config.max_global_position_usd - current).max(Decimal::ZERO);
        budget = Decimal::min(budget, headroom);
    }
    budget
}

fn live_low_competition_global_open_order_cap(
    config: &RewardBotConfig,
    max_open_orders: usize,
) -> usize {
    if max_open_orders == 0 || config.low_competition_global_open_order_share_bps == 0 {
        return 0;
    }
    ((max_open_orders * usize::from(config.low_competition_global_open_order_share_bps)) / 10_000)
        .max(1)
}

fn live_low_competition_open_order_count(orders: &[ManagedRewardOrder]) -> usize {
    orders
        .iter()
        .filter(|order| {
            order.status.is_open_like()
                && order.strategy_bucket == RewardStrategyBucket::LowCompetition
        })
        .count()
}

fn apply_live_funding_precheck(
    config: &RewardBotConfig,
    account: &RewardAccountState,
    plans: &mut [RewardQuotePlan],
    books: &HashMap<String, RewardOrderBook>,
    open_orders: &[ManagedRewardOrder],
    positions: &[RewardPosition],
) -> usize {
    if config.max_markets == 0 || config.max_open_orders == 0 {
        return 0;
    }

    let available_for_new_condition = live_available_usd_after_unmanaged_external_buys(account);
    let mut blocked = 0usize;

    for plan in plans.iter_mut().filter(|plan| plan.eligible) {
        if live_condition_has_active_exposure(&plan.condition_id, open_orders, positions) {
            continue;
        }

        let plan_config = config.config_for_strategy_bucket(plan.strategy_bucket);
        let Ok(materialized) =
            materialize_reward_quote_plan_for_live_orderbook(plan, books, &plan_config)
        else {
            continue;
        };
        apply_live_quote_plan_materialization(plan, materialized, OffsetDateTime::now_utc());

        let existing_market_buy_notional =
            live_market_buy_notional(open_orders, &plan.condition_id);
        let raw_budget =
            (available_for_new_condition - existing_market_buy_notional).max(Decimal::ZERO);
        let condition_budget =
            live_condition_budget_capped_by_positions(config, &plan.legs, positions, raw_budget);
        let rescaled_legs = live_rescaled_quote_legs_for_budget(plan, condition_budget);
        let missing_plan_buy_notional =
            live_missing_plan_buy_notional(&rescaled_legs, open_orders, &plan.condition_id);

        if missing_plan_buy_notional > Decimal::ZERO
            && existing_market_buy_notional + missing_plan_buy_notional
                > available_for_new_condition
            && mark_live_funding_skip(
                plan,
                existing_market_buy_notional,
                missing_plan_buy_notional,
                available_for_new_condition,
                OffsetDateTime::now_utc(),
            )
        {
            blocked += 1;
        }
    }

    blocked
}

fn live_condition_has_active_exposure(
    condition_id: &str,
    open_orders: &[ManagedRewardOrder],
    positions: &[RewardPosition],
) -> bool {
    open_orders
        .iter()
        .any(|order| order.condition_id == condition_id && order.status.is_open_like())
        || positions
            .iter()
            .any(|position| position.condition_id == condition_id && position.size > Decimal::ZERO)
}

fn live_rescaled_quote_legs_for_budget(
    plan: &RewardQuotePlan,
    condition_budget: Decimal,
) -> Vec<RewardQuoteLeg> {
    match plan.quote_mode {
        RewardPlanQuoteMode::SingleYes | RewardPlanQuoteMode::SingleNo => {
            if let Some(leg) = plan.legs.first() {
                let token = RewardToken {
                    token_id: leg.token_id.clone(),
                    outcome: leg.outcome.clone(),
                    price: None,
                };
                vec![scale_single_leg_for_budget(
                    &token,
                    leg.price,
                    plan.rewards_min_size,
                    condition_budget,
                )]
            } else {
                plan.legs.clone()
            }
        }
        _ => {
            let yes = plan
                .legs
                .iter()
                .find(|leg| leg.outcome.trim().eq_ignore_ascii_case("yes"));
            let no = plan
                .legs
                .iter()
                .find(|leg| leg.outcome.trim().eq_ignore_ascii_case("no"));
            if let (Some(yes), Some(no)) = (yes, no) {
                let yes_token = RewardToken {
                    token_id: yes.token_id.clone(),
                    outcome: yes.outcome.clone(),
                    price: None,
                };
                let no_token = RewardToken {
                    token_id: no.token_id.clone(),
                    outcome: no.outcome.clone(),
                    price: None,
                };
                scale_double_legs_for_budget(
                    &yes_token,
                    yes.price,
                    &no_token,
                    no.price,
                    plan.rewards_min_size,
                    condition_budget,
                )
            } else {
                plan.legs.clone()
            }
        }
    }
}

fn live_missing_plan_buy_notional(
    legs: &[RewardQuoteLeg],
    orders: &[ManagedRewardOrder],
    condition_id: &str,
) -> Decimal {
    legs.iter()
        .filter(|leg| {
            !orders.iter().any(|order| {
                order.condition_id == condition_id
                    && order.token_id == leg.token_id
                    && order.side == RewardOrderSide::Buy
                    && order.status.is_open_like()
            }) && !orders.iter().any(|order| {
                order.token_id == leg.token_id
                    && order.side == RewardOrderSide::Sell
                    && order.status.is_open_like()
            })
        })
        .map(|leg| (leg.price * leg.size).round_dp(4))
        .sum()
}

fn mark_live_funding_skip(
    plan: &mut RewardQuotePlan,
    existing_market_buy_notional: Decimal,
    missing_plan_buy_notional: Decimal,
    available_for_new_condition: Decimal,
    now: OffsetDateTime,
) -> bool {
    let reason = format!(
        "live funding below rewards minimum: existing condition BUY notional {existing_market_buy_notional}, missing minimum quote notional {missing_plan_buy_notional}, available {available_for_new_condition}"
    );
    let changed = plan.eligible
        || plan.quote_mode != RewardPlanQuoteMode::None
        || plan.reason != reason
        || plan.live_skip_until.is_some()
        || plan.live_skip_reason.is_some();
    if changed {
        plan.eligible = false;
        plan.quote_mode = RewardPlanQuoteMode::None;
        plan.reason = reason;
        plan.live_skip_until = None;
        plan.live_skip_reason = None;
        plan.updated_at = now;
    }
    changed
}

fn live_market_buy_notional(orders: &[ManagedRewardOrder], condition_id: &str) -> Decimal {
    orders
        .iter()
        .filter(|order| {
            order.condition_id == condition_id
                && order.side == RewardOrderSide::Buy
                && order.status.is_open_like()
        })
        .map(|order| {
            (order.price * (order.size - order.filled_size).max(Decimal::ZERO)).round_dp(4)
        })
        .sum()
}
