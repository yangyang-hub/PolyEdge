pub fn apply_low_competition_metrics_to_quote_plans(
    plans: &mut [RewardQuotePlan],
    books: &HashMap<String, RewardOrderBook>,
    book_history: &HashMap<String, VecDeque<BookSnapshot>>,
    open_orders: &[ManagedRewardOrder],
    account: &RewardAccountState,
    config: &RewardBotConfig,
) {
    if !config.low_competition_mode.is_enabled() {
        return;
    }

    let now = OffsetDateTime::now_utc();
    let low_config = config.config_for_strategy_bucket(RewardStrategyBucket::LowCompetition);
    for plan in plans
        .iter_mut()
        .filter(|plan| plan.strategy_bucket == RewardStrategyBucket::LowCompetition)
    {
        apply_low_competition_metrics_to_plan(
            plan,
            books,
            book_history,
            open_orders,
            account,
            config,
            &low_config,
            now,
        );
    }
}

fn apply_low_competition_metrics_to_plan(
    plan: &mut RewardQuotePlan,
    books: &HashMap<String, RewardOrderBook>,
    book_history: &HashMap<String, VecDeque<BookSnapshot>>,
    open_orders: &[ManagedRewardOrder],
    account: &RewardAccountState,
    config: &RewardBotConfig,
    low_config: &RewardBotConfig,
    now: OffsetDateTime,
) {
    let inherited_skip = plan
        .live_skip_until
        .is_some_and(|skip_until| skip_until > now);
    let materialized = match materialize_reward_quote_plan_for_live_orderbook(plan, books, low_config)
    {
        Ok(materialized) => materialized,
        Err(reason) => {
            let mut rejection_reasons =
                vec![format!("live orderbook validation failed: {reason}")];
            push_low_competition_mode_rejections(&mut rejection_reasons, config);
            let data_unavailable =
                low_competition_rejection_reasons_include_data_unavailable(&rejection_reasons);
            reject_low_competition_plan(
                plan,
                empty_low_competition_metrics(rejection_reasons),
                config,
                now,
                data_unavailable,
            );
            return;
        }
    };
    plan.quote_mode = materialized.quote_mode;
    plan.recommended_quote_mode = materialized.recommended_quote_mode;
    plan.book_metrics = materialized.book_metrics;
    plan.midpoint = Some(materialized.midpoint);
    plan.legs = materialized.legs;

    let mut metrics = build_low_competition_metrics(
        plan,
        books,
        book_history,
        open_orders,
        account,
        config,
        materialized.midpoint,
        now,
    );
    if inherited_skip {
        metrics
            .rejection_reasons
            .push(plan.live_skip_reason.clone().unwrap_or_else(|| {
                "recent live orderbook validation failed".to_string()
            }));
        metrics.eligible_for_low_competition = false;
    }
    if config.low_competition_mode == RewardLowCompetitionMode::Observe {
        plan.eligible = false;
        plan.reason = if metrics.eligible_for_low_competition {
            "low-competition observe only: metrics pass".to_string()
        } else {
            format!(
                "low-competition observe only: {}",
                metrics.rejection_reasons.join("; ")
            )
        };
    } else if metrics.eligible_for_low_competition {
        plan.eligible = true;
        plan.reason =
            "eligible for low-competition sleeve pending AI and info-risk gates".to_string();
    } else {
        plan.eligible = false;
        let data_unavailable =
            low_competition_rejection_reasons_include_data_unavailable(&metrics.rejection_reasons);
        if !data_unavailable {
            plan.quote_mode = RewardPlanQuoteMode::None;
            plan.legs.clear();
        }
        plan.reason = low_competition_rejection_reason(&metrics.rejection_reasons, data_unavailable);
    }
    plan.low_competition_metrics = Some(metrics);
    plan.updated_at = now;
}

fn build_low_competition_metrics(
    plan: &RewardQuotePlan,
    books: &HashMap<String, RewardOrderBook>,
    book_history: &HashMap<String, VecDeque<BookSnapshot>>,
    open_orders: &[ManagedRewardOrder],
    account: &RewardAccountState,
    config: &RewardBotConfig,
    yes_midpoint: Decimal,
    now: OffsetDateTime,
) -> RewardLowCompetitionMetrics {
    let mut rejection_reasons = Vec::new();
    push_low_competition_mode_rejections(&mut rejection_reasons, config);

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
    let competition_probe_notional = if config.low_competition_probe_notional_usd > Decimal::ZERO {
        config.low_competition_probe_notional_usd
    } else {
        planned_notional
    }
    .round_dp(4);
    let qualified_competition_usd = plan
        .legs
        .iter()
        .map(|leg| {
            let midpoint = midpoint_for_low_competition_leg(leg, yes_midpoint);
            qualified_competition_usd_for_leg(leg, midpoint, max_spread, books, open_orders)
        })
        .sum::<Decimal>()
        .round_dp(4);
    let denominator = Decimal::max(
        qualified_competition_usd + competition_probe_notional,
        competition_probe_notional,
    );
    let competition_share_bps =
        decimal_ratio_bps(competition_probe_notional, denominator).round_dp(2);
    let competition_multiple = if competition_probe_notional > Decimal::ZERO {
        (qualified_competition_usd / competition_probe_notional).round_dp(4)
    } else {
        Decimal::ZERO
    };
    let estimated_reward_per_100_usd_day = if denominator > Decimal::ZERO {
        (plan.total_daily_rate * decimal("100") / denominator).round_dp(4)
    } else {
        Decimal::ZERO
    };
    let competition_density = if plan.total_daily_rate > Decimal::ZERO {
        (qualified_competition_usd / Decimal::max(plan.total_daily_rate, Decimal::ONE))
            .round_dp(4)
    } else {
        Decimal::ZERO
    };
    let exit_depth_usd = low_competition_exit_depth_usd(plan, books).round_dp(4);
    let exit_slippage_cents = low_competition_exit_slippage_cents(plan, books);
    let bad_fill_recovery_days = low_competition_bad_fill_recovery_days(
        plan,
        exit_slippage_cents,
        estimated_reward_per_100_usd_day,
    );
    let (sample_count, midpoint_range_cents, top_of_book_flip_count) =
        low_competition_history_metrics(plan, book_history, config, now);
    let allocation = low_competition_allocation_metrics(plan, open_orders, account);

    if config.low_competition_max_competition_usd > Decimal::ZERO
        && qualified_competition_usd > config.low_competition_max_competition_usd
    {
        rejection_reasons.push(format!(
            "qualified competition ${qualified_competition_usd} exceeds ${}",
            config.low_competition_max_competition_usd
        ));
    }
    if config.low_competition_min_competition_share_bps > 0
        && competition_share_bps
            < Decimal::from(config.low_competition_min_competition_share_bps)
    {
        rejection_reasons.push(format!(
            "competition share {competition_share_bps}bps below {}bps",
            config.low_competition_min_competition_share_bps
        ));
    }
    if config.low_competition_max_competition_multiple > Decimal::ZERO
        && competition_multiple > config.low_competition_max_competition_multiple
    {
        rejection_reasons.push(format!(
            "competition multiple {competition_multiple} exceeds {}",
            config.low_competition_max_competition_multiple
        ));
    }
    if config.low_competition_max_account_allocation_bps > 0
        && allocation.account_allocation_bps
            > Decimal::from(config.low_competition_max_account_allocation_bps)
    {
        rejection_reasons.push(format!(
            "low-competition account allocation {}bps exceeds {}bps",
            allocation.account_allocation_bps,
            config.low_competition_max_account_allocation_bps
        ));
    }
    if config.low_competition_max_market_allocation_bps > 0
        && allocation.market_allocation_bps
            > Decimal::from(config.low_competition_max_market_allocation_bps)
    {
        rejection_reasons.push(format!(
            "condition allocation {}bps exceeds {}bps",
            allocation.market_allocation_bps,
            config.low_competition_max_market_allocation_bps
        ));
    }
    if estimated_reward_per_100_usd_day < config.low_competition_min_reward_per_100_usd_day {
        rejection_reasons.push(format!(
            "estimated reward/100/day {estimated_reward_per_100_usd_day} below {}",
            config.low_competition_min_reward_per_100_usd_day
        ));
    }
    let required_exit_depth = Decimal::max(
        config.low_competition_min_exit_depth_usd,
        planned_notional * config.low_competition_min_exit_depth_multiple,
    );
    if exit_depth_usd < required_exit_depth {
        rejection_reasons.push(format!(
            "exit depth ${exit_depth_usd} below required ${required_exit_depth}"
        ));
    }
    if exit_slippage_cents.is_none() {
        rejection_reasons.push("insufficient bid depth to estimate exit slippage".to_string());
    }
    if let Some(slippage) = exit_slippage_cents
        && config.low_competition_max_entry_exit_slippage_cents > Decimal::ZERO
        && slippage > config.low_competition_max_entry_exit_slippage_cents
    {
        rejection_reasons.push(format!(
            "entry exit slippage {slippage}c exceeds {}c",
            config.low_competition_max_entry_exit_slippage_cents
        ));
    }
    if let Some(days) = bad_fill_recovery_days
        && config.low_competition_max_bad_fill_recovery_days > Decimal::ZERO
        && days > config.low_competition_max_bad_fill_recovery_days
    {
        rejection_reasons.push(format!(
            "bad-fill recovery {days} days exceeds {} days",
            config.low_competition_max_bad_fill_recovery_days
        ));
    }
    if sample_count < config.low_competition_min_book_samples {
        rejection_reasons.push(format!(
            "book history samples {sample_count} below required {}",
            config.low_competition_min_book_samples
        ));
    }
    if let Some(range) = midpoint_range_cents {
        if range > config.low_competition_max_midpoint_range_cents {
            rejection_reasons.push(format!(
                "midpoint range {range}c exceeds {}c",
                config.low_competition_max_midpoint_range_cents
            ));
        }
    } else {
        rejection_reasons.push("book history midpoint range unavailable".to_string());
    }
    if let Some(flips) = top_of_book_flip_count
        && config.low_competition_max_top_of_book_flip_count > 0
        && flips > config.low_competition_max_top_of_book_flip_count
    {
        rejection_reasons.push(format!(
            "top-of-book flips {flips} exceed {}",
            config.low_competition_max_top_of_book_flip_count
        ));
    }

    // 早期剔除分类：competition_multiple 超过候选阈值说明该市场盘口竞争极其激烈，
    // 只是流动性/成交量低才被归为低竞争候选。该标签仅用于下游 prewarm/observation
    // 降级，绝不进入 rejection_reasons，不影响 eligible_for_low_competition 或 enforce 流程。
    let not_low_competition = config.low_competition_candidate_max_competition_multiple
        > Decimal::ZERO
        && competition_multiple > config.low_competition_candidate_max_competition_multiple;
    let not_low_competition_reason = if not_low_competition {
        Some(format!(
            "competition multiple {competition_multiple} above early-exclusion {}",
            config.low_competition_candidate_max_competition_multiple
        ))
    } else {
        None
    };

    RewardLowCompetitionMetrics {
        planned_notional_usd: planned_notional,
        competition_probe_notional_usd: competition_probe_notional,
        qualified_competition_usd,
        competition_share_bps,
        competition_multiple,
        estimated_reward_per_100_usd_day,
        competition_density,
        account_effective_available_usd: allocation.account_effective_available_usd,
        low_competition_open_buy_notional_usd: allocation
            .low_competition_open_buy_notional_usd,
        low_competition_open_buy_notional_usd_after_plan: allocation
            .low_competition_open_buy_notional_usd_after_plan,
        condition_buy_notional_usd_after_plan: allocation.condition_buy_notional_usd_after_plan,
        account_allocation_bps: allocation.account_allocation_bps,
        market_allocation_bps: allocation.market_allocation_bps,
        exit_depth_usd,
        exit_slippage_cents,
        bad_fill_recovery_days,
        midpoint_range_cents,
        top_of_book_flip_count,
        sample_count,
        eligible_for_low_competition: rejection_reasons.is_empty(),
        rejection_reasons,
        not_low_competition,
        not_low_competition_reason,
    }
}

#[derive(Debug, Clone, Copy)]
struct LowCompetitionAllocationMetrics {
    account_effective_available_usd: Decimal,
    low_competition_open_buy_notional_usd: Decimal,
    low_competition_open_buy_notional_usd_after_plan: Decimal,
    condition_buy_notional_usd_after_plan: Decimal,
    account_allocation_bps: Decimal,
    market_allocation_bps: Decimal,
}

fn empty_low_competition_metrics(
    rejection_reasons: Vec<String>,
) -> RewardLowCompetitionMetrics {
    RewardLowCompetitionMetrics {
        planned_notional_usd: Decimal::ZERO,
        competition_probe_notional_usd: Decimal::ZERO,
        qualified_competition_usd: Decimal::ZERO,
        competition_share_bps: Decimal::ZERO,
        competition_multiple: Decimal::ZERO,
        estimated_reward_per_100_usd_day: Decimal::ZERO,
        competition_density: Decimal::ZERO,
        account_effective_available_usd: Decimal::ZERO,
        low_competition_open_buy_notional_usd: Decimal::ZERO,
        low_competition_open_buy_notional_usd_after_plan: Decimal::ZERO,
        condition_buy_notional_usd_after_plan: Decimal::ZERO,
        account_allocation_bps: Decimal::ZERO,
        market_allocation_bps: Decimal::ZERO,
        exit_depth_usd: Decimal::ZERO,
        exit_slippage_cents: None,
        bad_fill_recovery_days: None,
        midpoint_range_cents: None,
        top_of_book_flip_count: None,
        sample_count: 0,
        eligible_for_low_competition: false,
        rejection_reasons,
        not_low_competition: false,
        not_low_competition_reason: None,
    }
}

fn low_competition_allocation_metrics(
    plan: &RewardQuotePlan,
    open_orders: &[ManagedRewardOrder],
    account: &RewardAccountState,
) -> LowCompetitionAllocationMetrics {
    let low_competition_open_buy_notional_usd =
        low_competition_open_buy_notional(open_orders).round_dp(4);
    let existing_condition_buy_notional =
        condition_open_buy_notional(open_orders, &plan.condition_id).round_dp(4);
    let missing_plan_buy_notional =
        missing_plan_buy_notional(plan, open_orders).round_dp(4);
    let low_competition_open_buy_notional_usd_after_plan =
        (low_competition_open_buy_notional_usd + missing_plan_buy_notional).round_dp(4);
    let condition_buy_notional_usd_after_plan =
        (existing_condition_buy_notional + missing_plan_buy_notional).round_dp(4);
    let account_effective_available_usd =
        account_effective_available_after_unmanaged_external_buys(account);
    let account_allocation_bps = decimal_ratio_bps(
        low_competition_open_buy_notional_usd_after_plan,
        account_effective_available_usd,
    )
    .round_dp(2);
    let market_allocation_bps = decimal_ratio_bps(
        condition_buy_notional_usd_after_plan,
        account_effective_available_usd,
    )
    .round_dp(2);

    LowCompetitionAllocationMetrics {
        account_effective_available_usd,
        low_competition_open_buy_notional_usd,
        low_competition_open_buy_notional_usd_after_plan,
        condition_buy_notional_usd_after_plan,
        account_allocation_bps,
        market_allocation_bps,
    }
}

fn account_effective_available_after_unmanaged_external_buys(
    account: &RewardAccountState,
) -> Decimal {
    // Reads the snapshot-frozen external occupancy; see
    // `sync_external_open_order_state`. This also unifies the low-competition
    // sleeve with the standard funding gate calibration — the old local
    // recompute did not filter internal order ids and could over-discount
    // managed occupancy, biasing the low-competition gate optimistic.
    (account.available_usd - account.unmanaged_external_buy_notional)
        .max(Decimal::ZERO)
        .round_dp(4)
}

fn low_competition_open_buy_notional(open_orders: &[ManagedRewardOrder]) -> Decimal {
    open_orders
        .iter()
        .filter(|order| {
            order.side == RewardOrderSide::Buy
                && order.status.is_open_like()
                && order.strategy_bucket == RewardStrategyBucket::LowCompetition
        })
        .map(order_remaining_notional)
        .sum()
}

fn condition_open_buy_notional(
    open_orders: &[ManagedRewardOrder],
    condition_id: &str,
) -> Decimal {
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

fn reject_low_competition_plan(
    plan: &mut RewardQuotePlan,
    metrics: RewardLowCompetitionMetrics,
    config: &RewardBotConfig,
    now: OffsetDateTime,
    preserve_quote_metadata: bool,
) {
    plan.eligible = false;
    if config.low_competition_mode == RewardLowCompetitionMode::Enforce && !preserve_quote_metadata
    {
        plan.quote_mode = RewardPlanQuoteMode::None;
        plan.legs.clear();
    }
    plan.reason = if config.low_competition_mode == RewardLowCompetitionMode::Observe {
        format!(
            "low-competition observe only: {}",
            metrics.rejection_reasons.join("; ")
        )
    } else {
        low_competition_rejection_reason(&metrics.rejection_reasons, preserve_quote_metadata)
    };
    plan.low_competition_metrics = Some(metrics);
    plan.updated_at = now;
}

fn low_competition_rejection_reason(
    rejection_reasons: &[String],
    data_unavailable: bool,
) -> String {
    let prefix = if data_unavailable {
        "low-competition data unavailable"
    } else {
        "low-competition gate rejected"
    };
    format!("{prefix}: {}", rejection_reasons.join("; "))
}

fn low_competition_rejection_reasons_include_data_unavailable(
    rejection_reasons: &[String],
) -> bool {
    rejection_reasons
        .iter()
        .any(|reason| low_competition_rejection_reason_is_data_unavailable(reason))
}

fn low_competition_rejection_reason_is_data_unavailable(reason: &str) -> bool {
    reason.contains("missing fresh orderbook midpoint")
        || reason.contains("book metrics unavailable")
        || reason.contains("book history")
        || reason.contains("samples")
        || reason.contains("insufficient bid depth to estimate exit slippage")
}

fn push_low_competition_mode_rejections(rejection_reasons: &mut Vec<String>, config: &RewardBotConfig) {
    if config.low_competition_mode != RewardLowCompetitionMode::Enforce {
        return;
    }
    if config.low_competition_max_open_orders == 0 {
        rejection_reasons.push("low-competition max open orders is zero".to_string());
    }
    if !config.ai_advisory_enabled {
        rejection_reasons.push("low-competition enforce requires AI advisory".to_string());
    }
    if !config.info_risk_enabled || config.info_risk_mode != RewardSelectionMode::Enforce {
        rejection_reasons.push(
            "low-competition enforce requires info-risk enforce mode".to_string(),
        );
    }
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

fn low_competition_exit_depth_usd(
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

fn low_competition_exit_slippage_cents(
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

fn low_competition_bad_fill_recovery_days(
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

fn low_competition_history_metrics(
    plan: &RewardQuotePlan,
    book_history: &HashMap<String, VecDeque<BookSnapshot>>,
    config: &RewardBotConfig,
    now: OffsetDateTime,
) -> (u64, Option<Decimal>, Option<u64>) {
    let cutoff = now - TimeDuration::seconds(config.low_competition_observation_window_sec as i64);
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
            .filter_map(low_competition_snapshot_state)
            .collect::<Vec<_>>();
        if snapshots.is_empty() {
            return (0, None, None);
        }
        let current_count = snapshots.len() as u64;
        sample_count = Some(sample_count.map_or(current_count, |count| count.min(current_count)));
        let (min_midpoint, max_midpoint) = snapshots.iter().fold(
            (snapshots[0].0, snapshots[0].0),
            |(min_value, max_value), (midpoint, _, _)| {
                (Decimal::min(min_value, *midpoint), Decimal::max(max_value, *midpoint))
            },
        );
        let range_cents = ((max_midpoint - min_midpoint) * decimal("100")).round_dp(4);
        max_midpoint_range = Some(max_midpoint_range.map_or(range_cents, |range| {
            Decimal::max(range, range_cents)
        }));
        total_flips += snapshots
            .windows(2)
            .filter(|pair| pair[0].1 != pair[1].1 || pair[0].2 != pair[1].2)
            .count() as u64;
    }

    (sample_count.unwrap_or_default(), max_midpoint_range, Some(total_flips))
}

fn low_competition_snapshot_state(snapshot: &BookSnapshot) -> Option<(Decimal, Decimal, Decimal)> {
    let bid = snapshot.bids.first()?.price;
    let ask = snapshot.asks.first()?.price;
    if bid <= Decimal::ZERO || ask <= Decimal::ZERO {
        return None;
    }
    Some(((bid + ask) / decimal("2"), bid, ask))
}

fn midpoint_for_low_competition_leg(leg: &RewardQuoteLeg, yes_midpoint: Decimal) -> Decimal {
    if leg.outcome.trim().eq_ignore_ascii_case("no") {
        Decimal::ONE - yes_midpoint
    } else {
        yes_midpoint
    }
}

fn decimal_abs(value: Decimal) -> Decimal {
    if value < Decimal::ZERO {
        -value
    } else {
        value
    }
}

#[cfg(test)]
mod low_competition_unmanaged_tests {
    //! Guards the snapshot-frozen external-occupancy fix: funding must read
    //! `account.unmanaged_external_buy_notional` (frozen at the last CLOB
    //! open-order snapshot) directly, so the bot cancelling its own managed
    //! buys between snapshots no longer spikes the external-occupancy estimate
    //! and oscillates eligible_markets to 0.
    use super::*;

    fn account(available_usd: Decimal, unmanaged: Decimal) -> RewardAccountState {
        let mut state = RewardAccountState::fresh(
            "acct",
            available_usd,
            OffsetDateTime::from_unix_timestamp(0).expect("valid timestamp"),
        );
        state.unmanaged_external_buy_notional = unmanaged;
        state
    }

    #[test]
    fn effective_available_subtracts_only_frozen_unmanaged() {
        // Fresh account: no frozen external occupancy → full available pool.
        assert_eq!(
            account_effective_available_after_unmanaged_external_buys(&account(
                Decimal::from(100),
                Decimal::ZERO
            )),
            Decimal::from(100)
        );
        // Frozen external occupancy is subtracted from the pool.
        assert_eq!(
            account_effective_available_after_unmanaged_external_buys(&account(
                Decimal::from(100),
                Decimal::from(30)
            )),
            Decimal::from(70)
        );
    }

    #[test]
    fn effective_available_clamps_to_zero() {
        // Frozen unmanaged larger than available never yields a negative budget.
        assert_eq!(
            account_effective_available_after_unmanaged_external_buys(&account(
                Decimal::from(10),
                Decimal::from(30)
            )),
            Decimal::ZERO
        );
    }
}
