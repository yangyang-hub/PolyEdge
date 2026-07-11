fn live_condition_budget_capped_by_global_position(
    config: &RewardBotConfig,
    positions: &[RewardPosition],
    orders: &[ManagedRewardOrder],
    unmanaged_external_buy_notional: Decimal,
    raw_budget: Decimal,
) -> Decimal {
    let mut budget = raw_budget;
    if config.max_global_position_usd > Decimal::ZERO {
        let current = live_global_inventory_notional(positions)
            + live_global_open_buy_notional(orders)
            + unmanaged_external_buy_notional.max(Decimal::ZERO);
        let headroom = (config.max_global_position_usd - current).max(Decimal::ZERO);
        budget = Decimal::min(budget, headroom);
    }
    budget
}

fn live_global_open_buy_notional(orders: &[ManagedRewardOrder]) -> Decimal {
    orders
        .iter()
        .filter(|order| order.side == RewardOrderSide::Buy && order.status.is_open_like())
        .map(|order| {
            (order.price * (order.size - order.filled_size).max(Decimal::ZERO)).round_dp(4)
        })
        .sum()
}

fn live_provider_size_multiplier(config: &RewardBotConfig, plan: &RewardQuotePlan) -> Decimal {
    let ai = plan.ai_advisory.as_ref().map_or(Decimal::ONE, |advisory| {
        reward_ai_size_multiplier(advisory, config)
    });
    let info = plan.info_risk.as_ref().map_or(Decimal::ONE, |risk| {
        reward_info_risk_size_multiplier(risk, config)
    });
    (ai * info).max(Decimal::ZERO).min(Decimal::ONE)
}

fn live_provider_condition_budget(
    config: &RewardBotConfig,
    plan: &RewardQuotePlan,
) -> Decimal {
    (config.maker_market_budget_usd * live_provider_size_multiplier(config, plan))
        .max(Decimal::ZERO)
        .min(config.maker_market_budget_usd.max(Decimal::ZERO))
        .round_dp(4)
}

fn live_rescaled_quote_legs_for_budget(
    plan: &RewardQuotePlan,
    condition_budget: Decimal,
    config: &RewardBotConfig,
    positions: &[RewardPosition],
) -> Vec<RewardQuoteLeg> {
    let legs = match plan.quote_mode {
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
                let yes_weight = live_inventory_quote_weight(config, positions, yes);
                let no_weight = live_inventory_quote_weight(config, positions, no);
                scale_double_legs_for_weighted_budget(
                    &yes_token,
                    yes.price,
                    &no_token,
                    no.price,
                    plan.rewards_min_size,
                    condition_budget,
                    yes_weight,
                    no_weight,
                )
            } else {
                plan.legs.clone()
            }
        }
    };
    live_cap_quote_legs_by_position_headroom(config, positions, plan.rewards_min_size, legs)
}

fn live_cap_quote_legs_by_position_headroom(
    config: &RewardBotConfig,
    positions: &[RewardPosition],
    rewards_min_size: Decimal,
    legs: Vec<RewardQuoteLeg>,
) -> Vec<RewardQuoteLeg> {
    if config.max_position_usd <= Decimal::ZERO {
        return legs;
    }
    legs.into_iter()
        .filter_map(|leg| {
            let current = positions
                .iter()
                .find(|position| position.token_id == leg.token_id && position.size > Decimal::ZERO)
                .map(|position| (position.size * leg.price).round_dp(4))
                .unwrap_or_default();
            let headroom = (config.max_position_usd - current).max(Decimal::ZERO);
            let notional = (leg.price * leg.size).round_dp(4);
            if notional <= headroom {
                return Some(leg);
            }
            if headroom <= Decimal::ZERO {
                return None;
            }
            let token = RewardToken {
                token_id: leg.token_id,
                outcome: leg.outcome,
                price: None,
            };
            let capped = scale_single_leg_for_budget(
                &token,
                leg.price,
                rewards_min_size,
                headroom,
            );
            ((capped.price * capped.size).round_dp(4) <= headroom).then_some(capped)
        })
        .collect()
}

fn live_inventory_quote_weight(
    config: &RewardBotConfig,
    positions: &[RewardPosition],
    leg: &RewardQuoteLeg,
) -> Decimal {
    if !config.inventory_skew_enabled
        || config.inventory_skew_strength <= Decimal::ZERO
        || config.max_position_usd <= Decimal::ZERO
    {
        return Decimal::ONE;
    }
    let inventory_notional = positions
        .iter()
        .find(|position| position.token_id == leg.token_id && position.size > Decimal::ZERO)
        .map(|position| (position.size * leg.price).round_dp(4))
        .unwrap_or_default();
    let utilization = (inventory_notional / config.max_position_usd)
        .max(Decimal::ZERO)
        .min(Decimal::ONE);
    (Decimal::ONE - config.inventory_skew_strength * utilization)
        .max(Decimal::new(5, 2))
        .min(Decimal::ONE)
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

fn mark_live_provider_size_skip(
    plan: &mut RewardQuotePlan,
    existing_market_buy_notional: Decimal,
    missing_plan_buy_notional: Decimal,
    max_condition_notional: Decimal,
    now: OffsetDateTime,
) -> bool {
    let reason = format!(
        "provider size adjustment below required rewards quote: existing condition BUY notional {existing_market_buy_notional}, missing minimum quote notional {missing_plan_buy_notional}, adjusted condition budget {max_condition_notional}"
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

fn mark_live_market_budget_skip(
    plan: &mut RewardQuotePlan,
    existing_market_buy_notional: Decimal,
    missing_plan_buy_notional: Decimal,
    maker_market_budget_usd: Decimal,
    now: OffsetDateTime,
) -> bool {
    let reason = format!(
        "maker market budget below required rewards quote: existing condition BUY notional {existing_market_buy_notional}, missing minimum quote notional {missing_plan_buy_notional}, configured condition cap {maker_market_budget_usd}"
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

fn mark_live_inventory_headroom_skip(
    plan: &mut RewardQuotePlan,
    missing_plan_buy_notional: Decimal,
    inventory_headroom: Decimal,
    now: OffsetDateTime,
) -> bool {
    let reason = format!(
        "inventory headroom below required rewards quote: missing minimum quote notional {missing_plan_buy_notional}, available inventory headroom {inventory_headroom}"
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
