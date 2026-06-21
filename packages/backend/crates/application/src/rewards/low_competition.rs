pub fn apply_low_competition_metrics_to_quote_plans(
    plans: &mut [RewardQuotePlan],
    books: &HashMap<String, RewardOrderBook>,
    book_history: &HashMap<String, VecDeque<BookSnapshot>>,
    open_orders: &[ManagedRewardOrder],
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
            reject_low_competition_plan(
                plan,
                RewardLowCompetitionMetrics {
                    planned_notional_usd: Decimal::ZERO,
                    qualified_competition_usd: Decimal::ZERO,
                    estimated_reward_per_100_usd_day: Decimal::ZERO,
                    competition_density: Decimal::ZERO,
                    exit_depth_usd: Decimal::ZERO,
                    exit_slippage_cents: None,
                    midpoint_range_cents: None,
                    top_of_book_flip_count: None,
                    sample_count: 0,
                    eligible_for_low_competition: false,
                    rejection_reasons,
                },
                config,
                now,
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
        plan.quote_mode = RewardPlanQuoteMode::None;
        plan.legs.clear();
        plan.reason = format!(
            "low-competition gate rejected: {}",
            metrics.rejection_reasons.join("; ")
        );
    }
    plan.low_competition_metrics = Some(metrics);
    plan.updated_at = now;
}

fn build_low_competition_metrics(
    plan: &RewardQuotePlan,
    books: &HashMap<String, RewardOrderBook>,
    book_history: &HashMap<String, VecDeque<BookSnapshot>>,
    open_orders: &[ManagedRewardOrder],
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
        qualified_competition_usd + planned_notional,
        planned_notional,
    );
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
    let (sample_count, midpoint_range_cents, top_of_book_flip_count) =
        low_competition_history_metrics(plan, book_history, config, now);

    if config.low_competition_max_competition_usd > Decimal::ZERO
        && qualified_competition_usd > config.low_competition_max_competition_usd
    {
        rejection_reasons.push(format!(
            "qualified competition ${qualified_competition_usd} exceeds ${}",
            config.low_competition_max_competition_usd
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

    RewardLowCompetitionMetrics {
        planned_notional_usd: planned_notional,
        qualified_competition_usd,
        estimated_reward_per_100_usd_day,
        competition_density,
        exit_depth_usd,
        exit_slippage_cents,
        midpoint_range_cents,
        top_of_book_flip_count,
        sample_count,
        eligible_for_low_competition: rejection_reasons.is_empty(),
        rejection_reasons,
    }
}

fn reject_low_competition_plan(
    plan: &mut RewardQuotePlan,
    metrics: RewardLowCompetitionMetrics,
    config: &RewardBotConfig,
    now: OffsetDateTime,
) {
    plan.eligible = false;
    if config.low_competition_mode == RewardLowCompetitionMode::Enforce {
        plan.quote_mode = RewardPlanQuoteMode::None;
        plan.legs.clear();
    }
    plan.reason = if config.low_competition_mode == RewardLowCompetitionMode::Observe {
        format!(
            "low-competition observe only: {}",
            metrics.rejection_reasons.join("; ")
        )
    } else {
        format!(
            "low-competition gate rejected: {}",
            metrics.rejection_reasons.join("; ")
        )
    };
    plan.low_competition_metrics = Some(metrics);
    plan.updated_at = now;
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
