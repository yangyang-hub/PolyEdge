pub fn apply_reward_opportunity_metrics_to_quote_plans(
    plans: &mut [RewardQuotePlan],
    books: &HashMap<String, RewardOrderBook>,
    book_history: &HashMap<String, VecDeque<BookSnapshot>>,
    open_orders: &[ManagedRewardOrder],
    account: &RewardAccountState,
    config: &RewardBotConfig,
    now: OffsetDateTime,
) {
    apply_reward_opportunity_metrics_to_quote_plans_inner(
        plans,
        books,
        book_history,
        open_orders,
        account,
        config,
        now,
        true,
    );
}

pub fn refresh_reward_opportunity_metrics_for_quote_plans(
    plans: &mut [RewardQuotePlan],
    books: &HashMap<String, RewardOrderBook>,
    book_history: &HashMap<String, VecDeque<BookSnapshot>>,
    open_orders: &[ManagedRewardOrder],
    account: &RewardAccountState,
    config: &RewardBotConfig,
    now: OffsetDateTime,
) {
    apply_reward_opportunity_metrics_to_quote_plans_inner(
        plans,
        books,
        book_history,
        open_orders,
        account,
        config,
        now,
        false,
    );
}

fn apply_reward_opportunity_metrics_to_quote_plans_inner(
    plans: &mut [RewardQuotePlan],
    books: &HashMap<String, RewardOrderBook>,
    book_history: &HashMap<String, VecDeque<BookSnapshot>>,
    open_orders: &[ManagedRewardOrder],
    account: &RewardAccountState,
    config: &RewardBotConfig,
    now: OffsetDateTime,
    allow_score_promotion: bool,
) {
    for plan in plans {
        let plan_config = config
            .config_for_strategy_bucket(plan.strategy_bucket)
            .config_for_strategy_profile(plan.strategy_profile);
        if !config.opportunity_metrics_enabled {
            plan.opportunity_metrics = None;
            plan.updated_at = now;
            continue;
        }
        apply_reward_opportunity_metrics_to_plan(
            plan,
            books,
            book_history,
            open_orders,
            account,
            &plan_config,
            now,
            allow_score_promotion,
        );
    }
}

fn apply_reward_opportunity_metrics_to_plan(
    plan: &mut RewardQuotePlan,
    books: &HashMap<String, RewardOrderBook>,
    book_history: &HashMap<String, VecDeque<BookSnapshot>>,
    open_orders: &[ManagedRewardOrder],
    account: &RewardAccountState,
    config: &RewardBotConfig,
    now: OffsetDateTime,
    allow_score_promotion: bool,
) {
    let base_score = reward_opportunity_base_score(plan);
    let materialized =
        materialize_reward_quote_plan_for_live_orderbook_at(plan, books, config, now);
    let metrics = match materialized {
        Ok(materialized) => {
            let midpoint = materialized.midpoint;
            plan.quote_mode = materialized.quote_mode;
            plan.recommended_quote_mode = materialized.recommended_quote_mode;
            plan.book_metrics = materialized.book_metrics;
            plan.midpoint = Some(midpoint);
            plan.legs = materialized.legs;
            build_reward_opportunity_metrics(
                plan,
                books,
                book_history,
                open_orders,
                account,
                config,
                midpoint,
                base_score,
                now,
            )
        }
        Err(reason) => empty_reward_opportunity_metrics(vec![format!(
            "live orderbook validation unavailable for opportunity scoring: {reason}"
        )]),
    };

    let adjusted_score = (base_score + metrics.score_adjustment)
        .max(Decimal::ZERO)
        .min(decimal("100"))
        .round_dp(2);
    plan.score = adjusted_score;
    if plan.quote_mode != RewardPlanQuoteMode::None
        && allow_score_promotion
        && plan.reason == "score is below threshold"
        && adjusted_score >= config.min_market_score
    {
        plan.eligible = true;
        plan.pre_ai_eligible = true;
        plan.reason = format!(
            "eligible pending live orderbook validation for {} quotes",
            plan.quote_mode.as_str()
        );
    } else if plan.quote_mode != RewardPlanQuoteMode::None
        && plan.eligible
        && adjusted_score < config.min_market_score
    {
        plan.eligible = false;
        plan.pre_ai_eligible = false;
        plan.reason = "score is below threshold after opportunity adjustment".to_string();
    }
    // Competition hard gate: a separate, higher threshold that hard-blocks
    // overcrowded markets. Unlike `opportunity_max_competition_multiple` (which
    // only emits a warning), exceeding this flips the plan ineligible. Runs in
    // both the initial and refresh paths (this function is shared), and because
    // `refresh_live_quote_plan_readiness` only touches already-eligible plans,
    // a hard-blocked plan cannot be re-enabled downstream.
    if config.opportunity_competition_hard_gate_enabled
        && config.opportunity_competition_hard_gate_multiple > Decimal::ZERO
        && plan.quote_mode != RewardPlanQuoteMode::None
        && metrics.competition_multiple > config.opportunity_competition_hard_gate_multiple
    {
        plan.eligible = false;
        plan.pre_ai_eligible = false;
        plan.reason = format!(
            "competition multiple {} exceeds hard gate {}",
            metrics.competition_multiple, config.opportunity_competition_hard_gate_multiple
        );
    }
    plan.opportunity_metrics = Some(metrics);
    plan.updated_at = now;
}

fn reward_opportunity_base_score(plan: &RewardQuotePlan) -> Decimal {
    if let Some(metrics) = &plan.opportunity_metrics {
        (plan.score - metrics.score_adjustment)
            .max(Decimal::ZERO)
            .min(decimal("100"))
            .round_dp(2)
    } else {
        plan.score
    }
}

#[allow(clippy::too_many_arguments)]
fn build_reward_opportunity_metrics(
    plan: &RewardQuotePlan,
    books: &HashMap<String, RewardOrderBook>,
    book_history: &HashMap<String, VecDeque<BookSnapshot>>,
    open_orders: &[ManagedRewardOrder],
    account: &RewardAccountState,
    config: &RewardBotConfig,
    yes_midpoint: Decimal,
    base_score: Decimal,
    now: OffsetDateTime,
) -> RewardOpportunityMetrics {
    let max_spread = Decimal::min(
        normalize_reward_spread_cents(plan.rewards_max_spread),
        config.max_spread_cents,
    ) / decimal("100");
    let planned_notional = plan
        .legs
        .iter()
        .map(|leg| (leg.price * leg.size).round_dp(4))
        .sum::<Decimal>()
        .round_dp(4);
    let probe_notional = if config.opportunity_probe_notional_usd > Decimal::ZERO {
        config.opportunity_probe_notional_usd
    } else {
        planned_notional
    }
    .round_dp(4);
    let qualified_competition_usd = plan
        .legs
        .iter()
        .map(|leg| {
            let midpoint = midpoint_for_opportunity_leg(leg, yes_midpoint);
            qualified_competition_usd_for_leg(leg, midpoint, max_spread, books, open_orders)
        })
        .sum::<Decimal>()
        .round_dp(4);
    let denominator =
        Decimal::max(qualified_competition_usd + probe_notional, probe_notional).round_dp(4);
    let competition_share_bps = decimal_ratio_bps(probe_notional, denominator).round_dp(2);
    let competition_multiple = if probe_notional > Decimal::ZERO {
        (qualified_competition_usd / probe_notional).round_dp(4)
    } else {
        Decimal::ZERO
    };
    let estimated_reward_per_100_usd_day = if denominator > Decimal::ZERO {
        (plan.total_daily_rate * decimal("100") / denominator).round_dp(4)
    } else {
        Decimal::ZERO
    };
    let competition_density = if plan.total_daily_rate > Decimal::ZERO {
        (qualified_competition_usd / Decimal::max(plan.total_daily_rate, Decimal::ONE)).round_dp(4)
    } else {
        Decimal::ZERO
    };
    let exit_depth_usd = opportunity_exit_depth_usd(plan, books).round_dp(4);
    let exit_slippage_cents = opportunity_exit_slippage_cents(plan, books);
    let bad_fill_recovery_days = opportunity_bad_fill_recovery_days(
        plan,
        exit_slippage_cents,
        estimated_reward_per_100_usd_day,
    );
    let (sample_count, midpoint_range_cents, top_of_book_flip_count) =
        opportunity_history_metrics(plan, book_history, config, now);
    let allocation = opportunity_allocation_metrics(plan, open_orders, account);
    let warnings = reward_opportunity_warnings(
        config,
        planned_notional,
        competition_multiple,
        estimated_reward_per_100_usd_day,
        exit_depth_usd,
        exit_slippage_cents,
        bad_fill_recovery_days,
        sample_count,
        midpoint_range_cents,
        top_of_book_flip_count,
        &allocation,
    );
    let (reward_score, competition_score, exit_score, stability_score, opportunity_score) =
        reward_opportunity_score(
            config,
            planned_notional,
            competition_multiple,
            estimated_reward_per_100_usd_day,
            exit_depth_usd,
            exit_slippage_cents,
            sample_count,
            midpoint_range_cents,
            top_of_book_flip_count,
        );
    let score_adjustment = reward_opportunity_score_adjustment(opportunity_score, base_score);

    RewardOpportunityMetrics {
        planned_notional_usd: planned_notional,
        probe_notional_usd: probe_notional,
        qualified_competition_usd,
        competition_share_bps,
        competition_multiple,
        estimated_reward_per_100_usd_day,
        competition_density,
        account_effective_available_usd: allocation.account_effective_available_usd,
        open_buy_notional_usd: allocation.open_buy_notional_usd,
        open_buy_notional_usd_after_plan: allocation.open_buy_notional_usd_after_plan,
        condition_buy_notional_usd_after_plan: allocation.condition_buy_notional_usd_after_plan,
        account_allocation_bps: allocation.account_allocation_bps,
        market_allocation_bps: allocation.market_allocation_bps,
        exit_depth_usd,
        exit_slippage_cents,
        bad_fill_recovery_days,
        midpoint_range_cents,
        top_of_book_flip_count,
        sample_count,
        reward_score,
        competition_score,
        exit_score,
        stability_score,
        opportunity_score,
        score_adjustment,
        warnings,
    }
}

#[derive(Debug, Clone, Copy)]
struct OpportunityAllocationMetrics {
    account_effective_available_usd: Decimal,
    open_buy_notional_usd: Decimal,
    open_buy_notional_usd_after_plan: Decimal,
    condition_buy_notional_usd_after_plan: Decimal,
    account_allocation_bps: Decimal,
    market_allocation_bps: Decimal,
}

fn opportunity_allocation_metrics(
    plan: &RewardQuotePlan,
    open_orders: &[ManagedRewardOrder],
    account: &RewardAccountState,
) -> OpportunityAllocationMetrics {
    let open_buy_notional_usd = open_buy_notional(open_orders).round_dp(4);
    let existing_condition_buy_notional =
        condition_open_buy_notional(open_orders, &plan.condition_id).round_dp(4);
    let missing_plan_buy_notional = missing_plan_buy_notional(plan, open_orders).round_dp(4);
    let open_buy_notional_usd_after_plan =
        (open_buy_notional_usd + missing_plan_buy_notional).round_dp(4);
    let condition_buy_notional_usd_after_plan =
        (existing_condition_buy_notional + missing_plan_buy_notional).round_dp(4);
    let account_effective_available_usd =
        account_effective_available_after_unmanaged_external_buys(account);
    let account_allocation_bps = decimal_ratio_bps(
        open_buy_notional_usd_after_plan,
        account_effective_available_usd,
    )
    .round_dp(2);
    let market_allocation_bps = decimal_ratio_bps(
        condition_buy_notional_usd_after_plan,
        account_effective_available_usd,
    )
    .round_dp(2);

    OpportunityAllocationMetrics {
        account_effective_available_usd,
        open_buy_notional_usd,
        open_buy_notional_usd_after_plan,
        condition_buy_notional_usd_after_plan,
        account_allocation_bps,
        market_allocation_bps,
    }
}

fn open_buy_notional(open_orders: &[ManagedRewardOrder]) -> Decimal {
    open_orders
        .iter()
        .filter(|order| order.side == RewardOrderSide::Buy && order.status.is_open_like())
        .map(order_remaining_notional)
        .sum()
}

#[allow(clippy::too_many_arguments)]
fn reward_opportunity_warnings(
    config: &RewardBotConfig,
    planned_notional: Decimal,
    competition_multiple: Decimal,
    estimated_reward_per_100_usd_day: Decimal,
    exit_depth_usd: Decimal,
    exit_slippage_cents: Option<Decimal>,
    bad_fill_recovery_days: Option<Decimal>,
    sample_count: u64,
    midpoint_range_cents: Option<Decimal>,
    top_of_book_flip_count: Option<u64>,
    allocation: &OpportunityAllocationMetrics,
) -> Vec<String> {
    let mut warnings = Vec::new();
    if estimated_reward_per_100_usd_day < config.opportunity_min_reward_per_100_usd_day {
        warnings.push(format!(
            "estimated reward/100/day {estimated_reward_per_100_usd_day} below {}",
            config.opportunity_min_reward_per_100_usd_day
        ));
    }
    if config.opportunity_max_competition_multiple > Decimal::ZERO
        && competition_multiple > config.opportunity_max_competition_multiple
    {
        warnings.push(format!(
            "competition multiple {competition_multiple} exceeds {}",
            config.opportunity_max_competition_multiple
        ));
    }
    if config.opportunity_max_account_allocation_bps > 0
        && allocation.account_allocation_bps
            > Decimal::from(config.opportunity_max_account_allocation_bps)
    {
        warnings.push(format!(
            "account allocation {}bps exceeds {}bps",
            allocation.account_allocation_bps, config.opportunity_max_account_allocation_bps
        ));
    }
    if config.opportunity_max_market_allocation_bps > 0
        && allocation.market_allocation_bps
            > Decimal::from(config.opportunity_max_market_allocation_bps)
    {
        warnings.push(format!(
            "condition allocation {}bps exceeds {}bps",
            allocation.market_allocation_bps, config.opportunity_max_market_allocation_bps
        ));
    }
    let required_exit_depth = reward_opportunity_required_exit_depth(config, planned_notional);
    if exit_depth_usd < required_exit_depth {
        warnings.push(format!(
            "exit depth ${exit_depth_usd} below reference ${required_exit_depth}"
        ));
    }
    match exit_slippage_cents {
        Some(slippage)
            if config.opportunity_max_entry_exit_slippage_cents > Decimal::ZERO
                && slippage > config.opportunity_max_entry_exit_slippage_cents =>
        {
            warnings.push(format!(
                "entry exit slippage {slippage}c exceeds {}c",
                config.opportunity_max_entry_exit_slippage_cents
            ));
        }
        None => warnings.push("insufficient bid depth to estimate exit slippage".to_string()),
        _ => {}
    }
    if let Some(days) = bad_fill_recovery_days
        && config.opportunity_max_bad_fill_recovery_days > Decimal::ZERO
        && days > config.opportunity_max_bad_fill_recovery_days
    {
        warnings.push(format!(
            "bad-fill recovery {days} days exceeds {} days",
            config.opportunity_max_bad_fill_recovery_days
        ));
    }
    if sample_count < config.opportunity_min_book_samples {
        warnings.push(format!(
            "book history samples {sample_count} below reference {}",
            config.opportunity_min_book_samples
        ));
    }
    match midpoint_range_cents {
        Some(range) if range > config.opportunity_max_midpoint_range_cents => {
            warnings.push(format!(
                "midpoint range {range}c exceeds {}c",
                config.opportunity_max_midpoint_range_cents
            ));
        }
        None => warnings.push("book history midpoint range unavailable".to_string()),
        _ => {}
    }
    if let Some(flips) = top_of_book_flip_count
        && config.opportunity_max_top_of_book_flip_count > 0
        && flips > config.opportunity_max_top_of_book_flip_count
    {
        warnings.push(format!(
            "top-of-book flips {flips} exceed {}",
            config.opportunity_max_top_of_book_flip_count
        ));
    }
    warnings
}

#[allow(clippy::too_many_arguments)]
fn reward_opportunity_score(
    config: &RewardBotConfig,
    planned_notional: Decimal,
    competition_multiple: Decimal,
    estimated_reward_per_100_usd_day: Decimal,
    exit_depth_usd: Decimal,
    exit_slippage_cents: Option<Decimal>,
    sample_count: u64,
    midpoint_range_cents: Option<Decimal>,
    top_of_book_flip_count: Option<u64>,
) -> (Decimal, Decimal, Decimal, Decimal, Decimal) {
    let reward_score = ratio_score(
        estimated_reward_per_100_usd_day,
        config.opportunity_min_reward_per_100_usd_day,
    );
    let competition_score = inverse_ratio_score(
        competition_multiple,
        config.opportunity_max_competition_multiple,
    );
    let required_exit_depth = reward_opportunity_required_exit_depth(config, planned_notional);
    let mut exit_score = ratio_score(exit_depth_usd, required_exit_depth);
    if let Some(slippage) = exit_slippage_cents
        && config.opportunity_max_entry_exit_slippage_cents > Decimal::ZERO
    {
        exit_score = Decimal::min(
            exit_score,
            inverse_ratio_score(slippage, config.opportunity_max_entry_exit_slippage_cents),
        );
    } else if exit_slippage_cents.is_none() {
        exit_score = Decimal::ZERO;
    }
    let sample_score = ratio_score(
        Decimal::from(sample_count),
        Decimal::from(config.opportunity_min_book_samples),
    );
    let range_score = midpoint_range_cents.map_or(Decimal::ZERO, |range| {
        inverse_ratio_score(range, config.opportunity_max_midpoint_range_cents)
    });
    let flip_score = top_of_book_flip_count.map_or(Decimal::ZERO, |flips| {
        inverse_ratio_score(
            Decimal::from(flips),
            Decimal::from(config.opportunity_max_top_of_book_flip_count),
        )
    });
    let stability_score = ((sample_score + range_score + flip_score) / decimal("3")).round_dp(4);
    let total_weight = config.opportunity_reward_weight
        + config.opportunity_competition_weight
        + config.opportunity_exit_weight
        + config.opportunity_stability_weight;
    let opportunity_score = if total_weight > Decimal::ZERO {
        ((reward_score * config.opportunity_reward_weight
            + competition_score * config.opportunity_competition_weight
            + exit_score * config.opportunity_exit_weight
            + stability_score * config.opportunity_stability_weight)
            / total_weight
            * decimal("100"))
        .round_dp(2)
    } else {
        Decimal::ZERO
    };
    (
        (reward_score * decimal("100")).round_dp(2),
        (competition_score * decimal("100")).round_dp(2),
        (exit_score * decimal("100")).round_dp(2),
        (stability_score * decimal("100")).round_dp(2),
        opportunity_score,
    )
}

fn reward_opportunity_score_adjustment(opportunity_score: Decimal, base_score: Decimal) -> Decimal {
    let adjustment = ((opportunity_score - decimal("50")) / decimal("5")).round_dp(2);
    let max_bonus = (decimal("100") - base_score).max(Decimal::ZERO);
    let max_penalty = base_score.max(Decimal::ZERO);
    adjustment
        .max(decimal("-10"))
        .min(decimal("10"))
        .min(max_bonus)
        .max(-max_penalty)
}

fn ratio_score(value: Decimal, target: Decimal) -> Decimal {
    if target <= Decimal::ZERO {
        return Decimal::ONE;
    }
    (value / target).max(Decimal::ZERO).min(Decimal::ONE)
}

fn inverse_ratio_score(value: Decimal, target: Decimal) -> Decimal {
    if target <= Decimal::ZERO {
        return Decimal::ONE;
    }
    (Decimal::ONE - (value / target))
        .max(Decimal::ZERO)
        .min(Decimal::ONE)
}

fn reward_opportunity_required_exit_depth(
    config: &RewardBotConfig,
    planned_notional: Decimal,
) -> Decimal {
    Decimal::max(
        config.opportunity_min_exit_depth_usd,
        planned_notional * config.opportunity_min_exit_depth_multiple,
    )
    .round_dp(4)
}

fn opportunity_history_metrics(
    plan: &RewardQuotePlan,
    book_history: &HashMap<String, VecDeque<BookSnapshot>>,
    config: &RewardBotConfig,
    now: OffsetDateTime,
) -> (u64, Option<Decimal>, Option<u64>) {
    let cutoff = now - TimeDuration::seconds(config.opportunity_observation_window_sec as i64);
    let mut sample_count: Option<u64> = None;
    let mut max_midpoint_range: Option<Decimal> = None;
    let mut total_flips = 0u64;

    for leg in &plan.legs {
        let Some(history) = book_history.get(&leg.token_id) else {
            return (0, None, None);
        };
        let snapshots = history
            .iter()
            .filter(|snapshot| snapshot.observed_at >= cutoff)
            .filter_map(opportunity_snapshot_state)
            .collect::<Vec<_>>();
        if snapshots.is_empty() {
            return (0, None, None);
        }
        let current_count = snapshots.len() as u64;
        sample_count = Some(sample_count.map_or(current_count, |count| count.min(current_count)));
        let (min_midpoint, max_midpoint) = snapshots.iter().fold(
            (snapshots[0].0, snapshots[0].0),
            |(min_value, max_value), (midpoint, _, _)| {
                (
                    Decimal::min(min_value, *midpoint),
                    Decimal::max(max_value, *midpoint),
                )
            },
        );
        let range_cents = ((max_midpoint - min_midpoint) * decimal("100")).round_dp(4);
        max_midpoint_range =
            Some(max_midpoint_range.map_or(range_cents, |range| Decimal::max(range, range_cents)));
        total_flips += snapshots
            .windows(2)
            .filter(|pair| pair[0].1 != pair[1].1 || pair[0].2 != pair[1].2)
            .count() as u64;
    }

    (
        sample_count.unwrap_or_default(),
        max_midpoint_range,
        Some(total_flips),
    )
}

fn empty_reward_opportunity_metrics(warnings: Vec<String>) -> RewardOpportunityMetrics {
    RewardOpportunityMetrics {
        planned_notional_usd: Decimal::ZERO,
        probe_notional_usd: Decimal::ZERO,
        qualified_competition_usd: Decimal::ZERO,
        competition_share_bps: Decimal::ZERO,
        competition_multiple: Decimal::ZERO,
        estimated_reward_per_100_usd_day: Decimal::ZERO,
        competition_density: Decimal::ZERO,
        account_effective_available_usd: Decimal::ZERO,
        open_buy_notional_usd: Decimal::ZERO,
        open_buy_notional_usd_after_plan: Decimal::ZERO,
        condition_buy_notional_usd_after_plan: Decimal::ZERO,
        account_allocation_bps: Decimal::ZERO,
        market_allocation_bps: Decimal::ZERO,
        exit_depth_usd: Decimal::ZERO,
        exit_slippage_cents: None,
        bad_fill_recovery_days: None,
        midpoint_range_cents: None,
        top_of_book_flip_count: None,
        sample_count: 0,
        reward_score: Decimal::ZERO,
        competition_score: Decimal::ZERO,
        exit_score: Decimal::ZERO,
        stability_score: Decimal::ZERO,
        opportunity_score: Decimal::ZERO,
        score_adjustment: Decimal::ZERO,
        warnings,
    }
}

fn account_effective_available_after_unmanaged_external_buys(
    account: &RewardAccountState,
) -> Decimal {
    // Reads the snapshot-frozen external occupancy; see
    // `sync_external_open_order_state`. Recomputing it locally can over-discount
    // managed occupancy while the bot is cancelling or replacing its own orders.
    (account.available_usd - account.unmanaged_external_buy_notional)
        .max(Decimal::ZERO)
        .round_dp(4)
}

fn condition_open_buy_notional(open_orders: &[ManagedRewardOrder], condition_id: &str) -> Decimal {
    open_orders
        .iter()
        .filter(|order| {
            order.condition_id == condition_id
                && order.side == RewardOrderSide::Buy
                && order.status.is_open_like()
        })
        .map(order_remaining_notional)
        .sum()
}

fn missing_plan_buy_notional(
    plan: &RewardQuotePlan,
    open_orders: &[ManagedRewardOrder],
) -> Decimal {
    plan.legs
        .iter()
        .filter(|leg| {
            !open_orders.iter().any(|order| {
                order.condition_id == plan.condition_id
                    && order.token_id == leg.token_id
                    && order.side == RewardOrderSide::Buy
                    && order.status.is_open_like()
            }) && !open_orders.iter().any(|order| {
                order.token_id == leg.token_id
                    && order.side == RewardOrderSide::Sell
                    && order.status.is_open_like()
            })
        })
        .map(|leg| leg.notional_usd.max(leg.price * leg.size).round_dp(4))
        .sum()
}

fn order_remaining_notional(order: &ManagedRewardOrder) -> Decimal {
    (order.price * (order.size - order.filled_size).max(Decimal::ZERO)).round_dp(4)
}

fn decimal_ratio_bps(numerator: Decimal, denominator: Decimal) -> Decimal {
    if denominator <= Decimal::ZERO {
        if numerator <= Decimal::ZERO {
            return Decimal::ZERO;
        }
        return decimal("10000");
    }
    numerator * decimal("10000") / denominator
}

fn qualified_competition_usd_for_leg(
    leg: &RewardQuoteLeg,
    midpoint: Decimal,
    max_spread: Decimal,
    books: &HashMap<String, RewardOrderBook>,
    open_orders: &[ManagedRewardOrder],
) -> Decimal {
    let Some(book) = books.get(&leg.token_id) else {
        return Decimal::ZERO;
    };
    let raw: Decimal = book
        .bids
        .iter()
        .filter(|level| level.price > Decimal::ZERO && level.size > Decimal::ZERO)
        .filter(|level| decimal_abs(midpoint - level.price) <= max_spread)
        .map(|level| (level.price * level.size).round_dp(4))
        .sum();
    let own: Decimal = open_orders
        .iter()
        .filter(|order| {
            order.token_id == leg.token_id
                && order.side == RewardOrderSide::Buy
                && order.status.is_open_like()
                && decimal_abs(midpoint - order.price) <= max_spread
        })
        .map(|order| {
            (order.price * (order.size - order.filled_size).max(Decimal::ZERO)).round_dp(4)
        })
        .sum();
    (raw - own).max(Decimal::ZERO)
}

fn opportunity_exit_depth_usd(
    plan: &RewardQuotePlan,
    books: &HashMap<String, RewardOrderBook>,
) -> Decimal {
    let mut depths = plan
        .legs
        .iter()
        .filter_map(|leg| books.get(&leg.token_id))
        .map(|book| {
            book.bids
                .iter()
                .filter(|level| level.price > Decimal::ZERO && level.size > Decimal::ZERO)
                .map(|level| (level.price * level.size).round_dp(4))
                .sum::<Decimal>()
        })
        .collect::<Vec<_>>();
    if depths.len() != plan.legs.len() {
        return Decimal::ZERO;
    }
    depths.sort();
    depths.first().copied().unwrap_or_default()
}

fn opportunity_exit_slippage_cents(
    plan: &RewardQuotePlan,
    books: &HashMap<String, RewardOrderBook>,
) -> Option<Decimal> {
    let mut worst = Decimal::ZERO;
    for leg in &plan.legs {
        let book = books.get(&leg.token_id)?;
        let mut remaining = leg.size;
        let mut exit_notional = Decimal::ZERO;
        for level in &book.bids {
            if remaining <= Decimal::ZERO {
                break;
            }
            if level.price <= Decimal::ZERO || level.size <= Decimal::ZERO {
                continue;
            }
            let filled = Decimal::min(remaining, level.size);
            exit_notional += filled * level.price;
            remaining -= filled;
        }
        if remaining > Decimal::ZERO || leg.size <= Decimal::ZERO {
            return None;
        }
        let avg_exit = exit_notional / leg.size;
        let slippage = ((leg.price - avg_exit).max(Decimal::ZERO) * decimal("100")).round_dp(4);
        worst = Decimal::max(worst, slippage);
    }
    Some(worst)
}

fn opportunity_bad_fill_recovery_days(
    plan: &RewardQuotePlan,
    exit_slippage_cents: Option<Decimal>,
    estimated_reward_per_100_usd_day: Decimal,
) -> Option<Decimal> {
    let slippage_cents = exit_slippage_cents?;
    if slippage_cents <= Decimal::ZERO {
        return Some(Decimal::ZERO);
    }
    let planned_notional = plan
        .legs
        .iter()
        .map(|leg| (leg.price * leg.size).round_dp(4))
        .sum::<Decimal>();
    let estimated_daily_reward =
        (estimated_reward_per_100_usd_day * planned_notional / decimal("100")).round_dp(8);
    if estimated_daily_reward <= Decimal::ZERO {
        return None;
    }
    let total_size = plan.legs.iter().map(|leg| leg.size).sum::<Decimal>();
    let estimated_slippage_cost = (total_size * slippage_cents / decimal("100")).round_dp(8);
    Some((estimated_slippage_cost / estimated_daily_reward).round_dp(4))
}

fn opportunity_snapshot_state(snapshot: &BookSnapshot) -> Option<(Decimal, Decimal, Decimal)> {
    let bid = snapshot.bids.first()?.price;
    let ask = snapshot.asks.first()?.price;
    if bid <= Decimal::ZERO || ask <= Decimal::ZERO {
        return None;
    }
    Some(((bid + ask) / decimal("2"), bid, ask))
}

fn midpoint_for_opportunity_leg(leg: &RewardQuoteLeg, yes_midpoint: Decimal) -> Decimal {
    if leg.outcome.trim().eq_ignore_ascii_case("no") {
        Decimal::ONE - yes_midpoint
    } else {
        yes_midpoint
    }
}

fn decimal_abs(value: Decimal) -> Decimal {
    if value < Decimal::ZERO { -value } else { value }
}
