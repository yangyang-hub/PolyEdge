pub fn low_competition_live_cancel_reason(
    config: &RewardBotConfig,
    plan: &RewardQuotePlan,
    books: &HashMap<String, RewardOrderBook>,
    book_history: &HashMap<String, VecDeque<BookSnapshot>>,
    open_orders: &[ManagedRewardOrder],
    account: &RewardAccountState,
) -> Option<String> {
    if config.low_competition_mode != RewardLowCompetitionMode::Enforce
        || plan.strategy_bucket != RewardStrategyBucket::LowCompetition
    {
        return None;
    }

    let now = OffsetDateTime::now_utc();
    let metrics = low_competition_cancel_metrics(
        config,
        plan,
        books,
        book_history,
        open_orders,
        account,
        now,
    )?;
    let historical_metrics = low_competition_cancel_historical_metrics(
        config,
        plan,
        book_history,
        open_orders,
        account,
        now,
    );

    let mut reasons = Vec::new();
    push_low_competition_cancel_competition_reasons(
        &mut reasons,
        config,
        &metrics,
        historical_metrics.as_ref(),
    );
    push_low_competition_cancel_allocation_reasons(&mut reasons, config, &metrics);
    push_low_competition_cancel_exit_reasons(&mut reasons, config, plan, open_orders, &metrics);
    push_low_competition_cancel_stability_reasons(&mut reasons, config, &metrics);

    if reasons.is_empty() {
        None
    } else {
        Some(format!(
            "low-competition cancel gate rejected: {}",
            reasons.join("; ")
        ))
    }
}

fn low_competition_cancel_metrics(
    config: &RewardBotConfig,
    plan: &RewardQuotePlan,
    books: &HashMap<String, RewardOrderBook>,
    book_history: &HashMap<String, VecDeque<BookSnapshot>>,
    open_orders: &[ManagedRewardOrder],
    account: &RewardAccountState,
    now: OffsetDateTime,
) -> Option<RewardLowCompetitionMetrics> {
    let low_config = config.config_for_strategy_bucket(RewardStrategyBucket::LowCompetition);
    let materialized =
        materialize_reward_quote_plan_for_live_orderbook(plan, books, &low_config).ok()?;
    Some(low_competition_metrics_for_materialized_plan(
        config,
        plan,
        books,
        book_history,
        open_orders,
        account,
        materialized,
        now,
    ))
}

fn low_competition_cancel_historical_metrics(
    config: &RewardBotConfig,
    plan: &RewardQuotePlan,
    book_history: &HashMap<String, VecDeque<BookSnapshot>>,
    open_orders: &[ManagedRewardOrder],
    account: &RewardAccountState,
    now: OffsetDateTime,
) -> Option<RewardLowCompetitionMetrics> {
    let books = low_competition_historical_books(config, plan, book_history, now)?;
    let mut low_config = config.config_for_strategy_bucket(RewardStrategyBucket::LowCompetition);
    low_config.stale_book_ms = 0;
    let materialized =
        materialize_reward_quote_plan_for_live_orderbook(plan, &books, &low_config).ok()?;
    Some(low_competition_metrics_for_materialized_plan(
        config,
        plan,
        &books,
        &HashMap::new(),
        open_orders,
        account,
        materialized,
        now,
    ))
}

fn low_competition_metrics_for_materialized_plan(
    config: &RewardBotConfig,
    plan: &RewardQuotePlan,
    books: &HashMap<String, RewardOrderBook>,
    book_history: &HashMap<String, VecDeque<BookSnapshot>>,
    open_orders: &[ManagedRewardOrder],
    account: &RewardAccountState,
    materialized: RewardLiveQuoteMaterialization,
    now: OffsetDateTime,
) -> RewardLowCompetitionMetrics {
    let mut materialized_plan = plan.clone();
    materialized_plan.quote_mode = materialized.quote_mode;
    materialized_plan.recommended_quote_mode = materialized.recommended_quote_mode;
    materialized_plan.book_metrics = materialized.book_metrics;
    materialized_plan.midpoint = Some(materialized.midpoint);
    materialized_plan.legs = materialized.legs;

    build_low_competition_metrics(
        &materialized_plan,
        books,
        book_history,
        open_orders,
        account,
        config,
        materialized.midpoint,
        now,
    )
}

fn low_competition_historical_books(
    config: &RewardBotConfig,
    plan: &RewardQuotePlan,
    book_history: &HashMap<String, VecDeque<BookSnapshot>>,
    now: OffsetDateTime,
) -> Option<HashMap<String, RewardOrderBook>> {
    let target = now - TimeDuration::seconds(config.low_competition_cancel_confirm_sec as i64);
    let mut books = HashMap::new();
    for leg in &plan.legs {
        if books.contains_key(&leg.token_id) {
            continue;
        }
        let snapshot = book_history
            .get(&leg.token_id)?
            .iter()
            .rev()
            .find(|snapshot| snapshot.observed_at <= target)?;
        books.insert(
            leg.token_id.clone(),
            RewardOrderBook {
                token_id: leg.token_id.clone(),
                bids: snapshot.bids.clone(),
                asks: snapshot.asks.clone(),
                observed_at: snapshot.observed_at,
                confirmed_at: snapshot.observed_at,
            },
        );
    }
    Some(books)
}

fn push_low_competition_cancel_competition_reasons(
    reasons: &mut Vec<String>,
    config: &RewardBotConfig,
    metrics: &RewardLowCompetitionMetrics,
    historical_metrics: Option<&RewardLowCompetitionMetrics>,
) {
    if config.low_competition_min_competition_share_bps > 0 {
        let threshold = low_competition_cancel_share_threshold_bps(config);
        if metrics.competition_share_bps < threshold
            && historical_metrics
                .is_some_and(|historical| historical.competition_share_bps < threshold)
        {
            reasons.push(format!(
                "competition share {}bps below cancel threshold {}bps",
                metrics.competition_share_bps, threshold
            ));
        }
    }

    if config.low_competition_max_competition_multiple > Decimal::ZERO {
        let threshold = low_competition_cancel_competition_multiple_threshold(config);
        if metrics.competition_multiple > threshold
            && historical_metrics
                .is_some_and(|historical| historical.competition_multiple > threshold)
        {
            reasons.push(format!(
                "competition multiple {} exceeds cancel threshold {}",
                metrics.competition_multiple, threshold
            ));
        }
    }
}

fn push_low_competition_cancel_allocation_reasons(
    reasons: &mut Vec<String>,
    config: &RewardBotConfig,
    metrics: &RewardLowCompetitionMetrics,
) {
    if config.low_competition_max_account_allocation_bps > 0
        && metrics.account_allocation_bps
            > Decimal::from(config.low_competition_max_account_allocation_bps)
    {
        reasons.push(format!(
            "low-competition account allocation {}bps exceeds {}bps",
            metrics.account_allocation_bps, config.low_competition_max_account_allocation_bps
        ));
    }
    if config.low_competition_max_market_allocation_bps > 0
        && metrics.market_allocation_bps
            > Decimal::from(config.low_competition_max_market_allocation_bps)
    {
        reasons.push(format!(
            "condition allocation {}bps exceeds {}bps",
            metrics.market_allocation_bps, config.low_competition_max_market_allocation_bps
        ));
    }
}

fn push_low_competition_cancel_exit_reasons(
    reasons: &mut Vec<String>,
    config: &RewardBotConfig,
    plan: &RewardQuotePlan,
    open_orders: &[ManagedRewardOrder],
    metrics: &RewardLowCompetitionMetrics,
) {
    let required_depth =
        low_competition_cancel_required_exit_depth(config, plan, open_orders, metrics);
    if metrics.exit_depth_usd < required_depth {
        reasons.push(format!(
            "exit depth ${} below cancel threshold ${required_depth}",
            metrics.exit_depth_usd
        ));
    }

    if let Some(slippage) = metrics.exit_slippage_cents
        && config.low_competition_cancel_max_exit_slippage_cents > Decimal::ZERO
        && slippage > config.low_competition_cancel_max_exit_slippage_cents
    {
        reasons.push(format!(
            "exit slippage {slippage}c exceeds cancel threshold {}c",
            config.low_competition_cancel_max_exit_slippage_cents
        ));
    }
}

fn push_low_competition_cancel_stability_reasons(
    reasons: &mut Vec<String>,
    config: &RewardBotConfig,
    metrics: &RewardLowCompetitionMetrics,
) {
    let threshold = low_competition_cancel_midpoint_range_threshold(config);
    if let Some(range) = metrics.midpoint_range_cents
        && range > threshold
    {
        reasons.push(format!(
            "midpoint range {range}c exceeds cancel threshold {threshold}c"
        ));
    }
}

fn low_competition_cancel_share_threshold_bps(config: &RewardBotConfig) -> Decimal {
    Decimal::from(config.low_competition_min_competition_share_bps)
        * Decimal::from(config.low_competition_cancel_share_threshold_ratio_bps)
        / decimal("10000")
}

fn low_competition_cancel_competition_multiple_threshold(
    config: &RewardBotConfig,
) -> Decimal {
    config.low_competition_max_competition_multiple
        * config.low_competition_cancel_competition_multiple_factor
}

fn low_competition_cancel_required_exit_depth(
    config: &RewardBotConfig,
    plan: &RewardQuotePlan,
    open_orders: &[ManagedRewardOrder],
    metrics: &RewardLowCompetitionMetrics,
) -> Decimal {
    let current_condition_notional =
        condition_open_buy_notional(open_orders, &plan.condition_id).round_dp(4);
    let reference_notional = if current_condition_notional > Decimal::ZERO {
        current_condition_notional
    } else {
        metrics.planned_notional_usd
    };
    Decimal::max(
        config.low_competition_cancel_min_exit_depth_usd,
        reference_notional * config.low_competition_cancel_exit_depth_multiple,
    )
    .round_dp(4)
}

fn low_competition_cancel_midpoint_range_threshold(config: &RewardBotConfig) -> Decimal {
    Decimal::max(
        config.low_competition_max_midpoint_range_cents,
        config.low_competition_cancel_midpoint_range_floor_cents,
    )
}
