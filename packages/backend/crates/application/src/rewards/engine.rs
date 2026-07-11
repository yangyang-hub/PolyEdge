const REWARD_ENGINE_ORDERBOOK_WAITING_REASON_PREFIX: &str =
    "waiting for fresh orderbook data from subscription";
const REWARD_ENGINE_ORDERBOOK_PLACEMENT_STALE_HEADROOM_MS: i128 = 10_000;

#[derive(Debug)]
pub struct RewardLiveEngineInput<'a> {
    pub cycle: RewardLiveCycle,
    pub books: &'a HashMap<String, RewardOrderBook>,
    pub book_history: &'a HashMap<String, VecDeque<BookSnapshot>>,
    pub now: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RewardDecisionSet {
    pub cycle: RewardLiveCycle,
    pub fair_value_estimates: Vec<RewardFairValueEstimate>,
    pub funding_precheck_blocked: usize,
    pub readiness_changed: bool,
    pub first_quote_entry_changed: bool,
}

pub struct RewardDecisionEngine;

impl RewardDecisionEngine {
    #[must_use]
    pub fn evaluate_pre_provider(input: RewardLiveEngineInput<'_>) -> RewardDecisionSet {
        let RewardLiveEngineInput {
            mut cycle,
            books,
            book_history,
            now,
        } = input;

        apply_reward_opportunity_metrics_to_quote_plans(
            &mut cycle.plans,
            books,
            book_history,
            &cycle.open_orders,
            &cycle.account,
            &cycle.config,
        );
        let fair_value_estimates = apply_reward_fair_values_to_quote_plans(
            &mut cycle.plans,
            books,
            book_history,
            &cycle.config,
            now,
        );
        let funding_precheck_blocked = apply_reward_live_funding_precheck(
            &cycle.config,
            &cycle.account,
            &mut cycle.plans,
            books,
            &cycle.open_orders,
            &cycle.positions,
            now,
        );
        apply_reward_market_selection_to_quote_plans(&mut cycle.plans, &cycle.config);
        mark_reward_pre_ai_eligible_quote_plans(
            &mut cycle.plans,
            &mut cycle.pre_ai_eligible_condition_ids,
        );
        refresh_reward_decision_readiness(&mut cycle.plans);

        RewardDecisionSet {
            cycle,
            fair_value_estimates,
            funding_precheck_blocked,
            readiness_changed: false,
            first_quote_entry_changed: false,
        }
    }

    #[must_use]
    pub fn evaluate_post_provider(
        mut cycle: RewardLiveCycle,
        now: OffsetDateTime,
    ) -> RewardDecisionSet {
        let first_quote_entry_changed = apply_first_quote_entry_gates(
            &mut cycle.plans,
            &cycle.previous_plans,
            &cycle.open_orders,
            &cycle.positions,
            &cycle.config,
            now,
        );
        apply_reward_market_selection_to_quote_plans(&mut cycle.plans, &cycle.config);
        refresh_reward_decision_readiness(&mut cycle.plans);

        RewardDecisionSet {
            cycle,
            fair_value_estimates: Vec::new(),
            funding_precheck_blocked: 0,
            readiness_changed: false,
            first_quote_entry_changed,
        }
    }

    #[must_use]
    pub fn refresh_snapshot(input: RewardLiveEngineInput<'_>) -> RewardDecisionSet {
        let RewardLiveEngineInput {
            mut cycle,
            books,
            book_history,
            now,
        } = input;

        let readiness_changed =
            refresh_reward_live_quote_plan_readiness(&cycle.config, &mut cycle.plans, books, now);
        refresh_reward_opportunity_metrics_for_quote_plans(
            &mut cycle.plans,
            books,
            book_history,
            &cycle.open_orders,
            &cycle.account,
            &cycle.config,
        );
        let fair_value_estimates = apply_reward_fair_values_to_quote_plans(
            &mut cycle.plans,
            books,
            book_history,
            &cycle.config,
            now,
        );
        apply_reward_market_selection_to_quote_plans(&mut cycle.plans, &cycle.config);
        refresh_reward_decision_readiness(&mut cycle.plans);

        RewardDecisionSet {
            cycle,
            fair_value_estimates,
            funding_precheck_blocked: 0,
            readiness_changed,
            first_quote_entry_changed: false,
        }
    }
}

pub fn mark_reward_pre_ai_eligible_quote_plans(
    plans: &mut [RewardQuotePlan],
    pre_ai_eligible_condition_ids: &mut Vec<String>,
) {
    pre_ai_eligible_condition_ids.clear();
    for plan in plans {
        plan.pre_ai_eligible = plan.eligible;
        refresh_reward_quote_plan_readiness(plan);
        if plan.pre_ai_eligible {
            if plan.orderbook_token_ids.is_empty() {
                plan.orderbook_token_ids = reward_engine_quote_plan_leg_token_ids(&plan.legs);
            }
            pre_ai_eligible_condition_ids.push(plan.condition_id.clone());
        }
    }
}

fn refresh_reward_decision_readiness(plans: &mut [RewardQuotePlan]) {
    for plan in plans {
        refresh_reward_quote_plan_readiness(plan);
    }
}

fn reward_engine_quote_plan_leg_token_ids(legs: &[RewardQuoteLeg]) -> Vec<String> {
    let mut token_ids = Vec::new();
    let mut seen = HashSet::new();
    for leg in legs {
        if leg.token_id.trim().is_empty() || !seen.insert(leg.token_id.clone()) {
            continue;
        }
        token_ids.push(leg.token_id.clone());
    }
    token_ids
}

pub fn apply_reward_live_funding_precheck(
    config: &RewardBotConfig,
    account: &RewardAccountState,
    plans: &mut [RewardQuotePlan],
    books: &HashMap<String, RewardOrderBook>,
    open_orders: &[ManagedRewardOrder],
    positions: &[RewardPosition],
    now: OffsetDateTime,
) -> usize {
    if (config.max_markets == 0 || config.max_open_orders == 0)
        && (!config.balanced_merge_enabled
            || config.balanced_merge_max_markets == 0
            || config.balanced_merge_max_open_orders == 0)
    {
        return 0;
    }

    let available_for_new_condition = reward_live_available_usd_after_unmanaged_external_buys(account);
    let mut blocked = 0usize;

    for plan in plans.iter_mut().filter(|plan| plan.eligible) {
        if reward_live_condition_has_active_exposure(&plan.condition_id, open_orders, positions) {
            continue;
        }

        let plan_config = config
            .config_for_strategy_bucket(plan.strategy_bucket)
            .config_for_strategy_profile(plan.strategy_profile);
        let Ok(materialized) =
            materialize_reward_quote_plan_for_live_orderbook(plan, books, &plan_config)
        else {
            continue;
        };
        apply_reward_live_quote_plan_materialization(plan, materialized, now);

        let existing_market_buy_notional =
            reward_live_market_buy_notional(open_orders, &plan.condition_id);
        let raw_budget =
            (available_for_new_condition - existing_market_buy_notional).max(Decimal::ZERO);
        let position_budget = reward_live_condition_budget_capped_by_positions(
            &plan_config,
            &plan.legs,
            positions,
            raw_budget,
        );
        let provider_multiplier = reward_live_provider_size_multiplier(&plan_config, plan);
        let condition_budget =
            reward_live_condition_budget_capped_by_provider(&plan_config, plan, position_budget);
        let rescaled_legs = reward_live_rescaled_quote_legs_for_budget(plan, condition_budget);
        let missing_plan_buy_notional =
            reward_live_missing_plan_buy_notional(&rescaled_legs, open_orders, &plan.condition_id);
        let projected_condition_buy_notional =
            existing_market_buy_notional + missing_plan_buy_notional;

        if provider_multiplier < Decimal::ONE && missing_plan_buy_notional > condition_budget {
            let max_condition_notional = existing_market_buy_notional + condition_budget;
            if missing_plan_buy_notional > Decimal::ZERO
                && mark_reward_live_provider_size_skip(
                    plan,
                    existing_market_buy_notional,
                    missing_plan_buy_notional,
                    max_condition_notional,
                    now,
                )
            {
                blocked += 1;
            }
            continue;
        }

        if missing_plan_buy_notional > Decimal::ZERO
            && projected_condition_buy_notional > available_for_new_condition
            && mark_reward_live_funding_skip(
                plan,
                existing_market_buy_notional,
                missing_plan_buy_notional,
                available_for_new_condition,
                now,
            )
        {
            blocked += 1;
        }
    }

    blocked
}

pub fn refresh_reward_live_quote_plan_readiness(
    config: &RewardBotConfig,
    plans: &mut [RewardQuotePlan],
    books: &HashMap<String, RewardOrderBook>,
    now: OffsetDateTime,
) -> bool {
    let mut changed = false;
    for plan in plans.iter_mut().filter(|plan| plan.eligible) {
        let plan_config = config
            .config_for_strategy_bucket(plan.strategy_bucket)
            .config_for_strategy_profile(plan.strategy_profile);
        match materialize_reward_quote_plan_for_live_orderbook(plan, books, &plan_config) {
            Ok(materialized) => {
                if apply_reward_live_quote_plan_materialization(plan, materialized, now) {
                    changed = true;
                }
                if reward_quote_plan_event_window_blocks_new_buy(plan) {
                    if mark_reward_live_event_window_new_buy_blocked(plan, now) {
                        changed = true;
                    }
                    refresh_reward_quote_plan_readiness(plan);
                    continue;
                }
                if let Some(wait_reason) =
                    reward_live_orderbook_placement_wait_reason(&plan_config, &plan.legs, books, now)
                {
                    if mark_reward_live_orderbook_waiting(plan, wait_reason, now) {
                        changed = true;
                    }
                }
            }
            Err(reason) => {
                if let Some(wait_reason) =
                    reward_live_orderbook_wait_reason(&plan_config, plan, books, now)
                {
                    if mark_reward_live_orderbook_waiting(plan, wait_reason, now) {
                        changed = true;
                    }
                } else {
                    mark_reward_live_orderbook_validation_skip(plan, reason, now);
                    changed = true;
                }
            }
        }
        refresh_reward_quote_plan_readiness(plan);
    }
    changed
}

fn apply_reward_live_quote_plan_materialization(
    plan: &mut RewardQuotePlan,
    materialized: RewardLiveQuoteMaterialization,
    now: OffsetDateTime,
) -> bool {
    let changed = plan.quote_mode != materialized.quote_mode
        || plan.recommended_quote_mode != materialized.recommended_quote_mode
        || plan.book_metrics != materialized.book_metrics
        || plan.midpoint != Some(materialized.midpoint)
        || plan.legs != materialized.legs
        || !plan.eligible
        || plan.reason
            != format!(
                "eligible for live post-only {} quotes",
                materialized.quote_mode.as_str()
            )
        || plan.live_skip_until.is_some()
        || plan.live_skip_reason.is_some();

    if changed {
        plan.quote_mode = materialized.quote_mode;
        plan.recommended_quote_mode = materialized.recommended_quote_mode;
        plan.book_metrics = materialized.book_metrics;
        plan.midpoint = Some(materialized.midpoint);
        plan.legs = materialized.legs;
        plan.eligible = true;
        plan.reason = format!(
            "eligible for live post-only {} quotes",
            plan.quote_mode.as_str()
        );
        plan.live_skip_until = None;
        plan.live_skip_reason = None;
        plan.updated_at = now;
        refresh_reward_quote_plan_readiness(plan);
    }

    changed
}

fn reward_live_condition_budget_capped_by_positions(
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
                    .find(|position| position.token_id == leg.token_id && position.size > Decimal::ZERO)
                    .map(|position| (position.size * leg.price).round_dp(4))
                    .unwrap_or_default();
                (config.max_position_usd - current).max(Decimal::ZERO)
            })
            .min()
            .unwrap_or(raw_budget);
        budget = Decimal::min(
            budget,
            min_headroom * Decimal::from(plan_legs.len().max(1) as u64),
        );
    }
    if config.max_global_position_usd > Decimal::ZERO {
        let current = reward_live_global_inventory_notional(positions);
        let headroom = (config.max_global_position_usd - current).max(Decimal::ZERO);
        budget = Decimal::min(budget, headroom);
    }
    budget
}

fn reward_live_condition_budget_capped_by_provider(
    config: &RewardBotConfig,
    plan: &RewardQuotePlan,
    raw_budget: Decimal,
) -> Decimal {
    let multiplier = reward_live_provider_size_multiplier(config, plan);
    (raw_budget * multiplier)
        .max(Decimal::ZERO)
        .min(raw_budget)
        .round_dp(4)
}

fn reward_live_provider_size_multiplier(
    config: &RewardBotConfig,
    plan: &RewardQuotePlan,
) -> Decimal {
    let ai = plan.ai_advisory.as_ref().map_or(Decimal::ONE, |advisory| {
        reward_ai_size_multiplier(advisory, config)
    });
    let info = plan.info_risk.as_ref().map_or(Decimal::ONE, |risk| {
        reward_info_risk_size_multiplier(risk, config)
    });
    (ai * info).max(Decimal::ZERO).min(Decimal::ONE)
}

fn reward_live_condition_has_active_exposure(
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

fn reward_live_rescaled_quote_legs_for_budget(
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

fn reward_live_missing_plan_buy_notional(
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

fn mark_reward_live_funding_skip(
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
        refresh_reward_quote_plan_readiness(plan);
    }
    changed
}

fn mark_reward_live_provider_size_skip(
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
        refresh_reward_quote_plan_readiness(plan);
    }
    changed
}

fn reward_live_market_buy_notional(
    orders: &[ManagedRewardOrder],
    condition_id: &str,
) -> Decimal {
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

fn reward_live_available_usd_after_unmanaged_external_buys(
    account: &RewardAccountState,
) -> Decimal {
    (account.available_usd - account.unmanaged_external_buy_notional).max(Decimal::ZERO)
}

fn reward_live_global_inventory_notional(positions: &[RewardPosition]) -> Decimal {
    positions
        .iter()
        .filter(|position| position.size > Decimal::ZERO)
        .map(|position| position.size * position.avg_price)
        .sum()
}

fn mark_reward_live_orderbook_validation_skip(
    plan: &mut RewardQuotePlan,
    reason: String,
    now: OffsetDateTime,
) {
    let skip_until = now + REWARD_LIVE_ORDERBOOK_VALIDATION_SKIP_TTL;
    plan.eligible = false;
    plan.quote_mode = RewardPlanQuoteMode::None;
    plan.reason = format!("live orderbook validation skipped until {skip_until}: {reason}");
    plan.live_skip_until = Some(skip_until);
    plan.live_skip_reason = Some(reason);
    plan.updated_at = now;
    refresh_reward_quote_plan_readiness(plan);
}

fn mark_reward_live_event_window_new_buy_blocked(
    plan: &mut RewardQuotePlan,
    now: OffsetDateTime,
) -> bool {
    let reason = plan
        .event_window
        .as_ref()
        .map(|assessment| format!("event window blocks new BUY quotes: {}", assessment.reason))
        .unwrap_or_else(|| "event window blocks new BUY quotes".to_string());
    let changed = plan.reason != reason || plan.quote_readiness != RewardQuoteReadiness::Blocked;
    if changed {
        plan.reason = reason;
        plan.quote_readiness = RewardQuoteReadiness::Blocked;
        plan.updated_at = now;
    }
    changed
}

fn mark_reward_live_orderbook_waiting(
    plan: &mut RewardQuotePlan,
    reason: String,
    now: OffsetDateTime,
) -> bool {
    let changed = !plan.eligible
        || plan.reason != reason
        || plan.live_skip_until.is_some()
        || plan.live_skip_reason.is_some();
    if changed {
        plan.eligible = true;
        plan.reason = reason;
        plan.live_skip_until = None;
        plan.live_skip_reason = None;
        plan.updated_at = now;
        refresh_reward_quote_plan_readiness(plan);
    }
    changed
}

fn reward_live_orderbook_wait_reason(
    config: &RewardBotConfig,
    plan: &RewardQuotePlan,
    books: &HashMap<String, RewardOrderBook>,
    now: OffsetDateTime,
) -> Option<String> {
    reward_live_orderbook_wait_reason_for_legs(config, &plan.legs, books, now, false)
}

fn reward_live_orderbook_placement_wait_reason(
    config: &RewardBotConfig,
    legs: &[RewardQuoteLeg],
    books: &HashMap<String, RewardOrderBook>,
    now: OffsetDateTime,
) -> Option<String> {
    reward_live_orderbook_wait_reason_for_legs(config, legs, books, now, true)
}

fn reward_live_orderbook_wait_reason_for_legs(
    config: &RewardBotConfig,
    legs: &[RewardQuoteLeg],
    books: &HashMap<String, RewardOrderBook>,
    now: OffsetDateTime,
    require_placement_headroom: bool,
) -> Option<String> {
    let mut reasons = Vec::new();
    let mut seen = HashSet::new();

    for leg in legs {
        if !seen.insert(leg.token_id.clone()) {
            continue;
        }
        let label = if leg.outcome.trim().is_empty() {
            leg.token_id.as_str()
        } else {
            leg.outcome.as_str()
        };
        let Some(book) = books.get(&leg.token_id) else {
            reasons.push(format!("{label} orderbook unavailable"));
            continue;
        };
        if book.bids.is_empty() || book.asks.is_empty() {
            reasons.push(format!("{label} orderbook is empty"));
            continue;
        }
        if config.stale_book_ms == 0 {
            continue;
        }
        let age_ms = reward_live_orderbook_age_ms(book, now);
        if reward_live_orderbook_age_is_stale(age_ms, config.stale_book_ms) {
            reasons.push(format!(
                "{label} {}",
                reward_live_orderbook_stale_reason(age_ms, config.stale_book_ms)
            ));
            continue;
        }
        if require_placement_headroom {
            let max_placement_age_ms = reward_live_orderbook_max_placement_age_ms(config);
            if age_ms > max_placement_age_ms {
                reasons.push(format!(
                    "{label} orderbook too close to stale: age_ms={age_ms}, max_placement_age_ms={max_placement_age_ms}, max_age_ms={}",
                    config.stale_book_ms
                ));
            }
        }
    }

    if reasons.is_empty() {
        None
    } else {
        Some(format!(
            "{REWARD_ENGINE_ORDERBOOK_WAITING_REASON_PREFIX}: {}",
            reasons.join("; ")
        ))
    }
}

fn reward_live_orderbook_age_ms(book: &RewardOrderBook, now: OffsetDateTime) -> i128 {
    (now - book.confirmed_at).whole_milliseconds()
}

fn reward_live_orderbook_age_is_stale(age_ms: i128, stale_book_ms: u64) -> bool {
    age_ms < 0 || age_ms > i128::from(stale_book_ms)
}

fn reward_live_orderbook_stale_reason(age_ms: i128, stale_book_ms: u64) -> String {
    format!("orderbook stale for live order: age_ms={age_ms}, max_age_ms={stale_book_ms}")
}

fn reward_live_orderbook_max_placement_age_ms(config: &RewardBotConfig) -> i128 {
    if config.stale_book_ms == 0 {
        return i128::MAX;
    }
    let stale_ms = i128::from(config.stale_book_ms);
    let headroom = REWARD_ENGINE_ORDERBOOK_PLACEMENT_STALE_HEADROOM_MS.min(stale_ms / 2);
    stale_ms.saturating_sub(headroom)
}

/// The full set of state changes produced by a single rewards tick. The
/// store persists it atomically via `apply_tick_outcome`.
#[derive(Debug, Clone, PartialEq)]
pub struct RewardTickOutcome {
    pub account: RewardAccountState,
    pub markets: Vec<RewardMarket>,
    pub plans: Vec<RewardQuotePlan>,
    /// New and modified managed orders, keyed by `id` (upserted).
    pub orders: Vec<ManagedRewardOrder>,
    /// Positions to upsert, keyed by `(account_id, token_id)`.
    pub positions: Vec<RewardPosition>,
    pub fills: Vec<RewardFill>,
    pub merge_intents: Vec<RewardMergeIntent>,
    pub events: Vec<RewardRiskEvent>,
    pub report: RewardBotRunReport,
}
