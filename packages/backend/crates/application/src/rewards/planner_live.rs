pub fn materialize_reward_quote_plan_for_live_orderbook(
    plan: &RewardQuotePlan,
    books: &HashMap<String, RewardOrderBook>,
    config: &RewardBotConfig,
) -> std::result::Result<RewardLiveQuoteMaterialization, String> {
    materialize_reward_quote_plan_for_live_orderbook_at(
        plan,
        books,
        config,
        OffsetDateTime::now_utc(),
    )
}

pub fn materialize_reward_quote_plan_for_live_orderbook_at(
    plan: &RewardQuotePlan,
    books: &HashMap<String, RewardOrderBook>,
    config: &RewardBotConfig,
    now: OffsetDateTime,
) -> std::result::Result<RewardLiveQuoteMaterialization, String> {
    materialize_reward_quote_plan_for_live_orderbook_inner(plan, books, config, now, None)
}

pub fn materialize_reward_quote_plan_for_live_orderbook_with_fair_value_at(
    plan: &RewardQuotePlan,
    books: &HashMap<String, RewardOrderBook>,
    config: &RewardBotConfig,
    now: OffsetDateTime,
    estimate: &RewardFairValueEstimate,
) -> std::result::Result<RewardLiveQuoteMaterialization, String> {
    materialize_reward_quote_plan_for_live_orderbook_inner(
        plan,
        books,
        config,
        now,
        Some(LiveFairValueEdgeContext {
            fair_yes: estimate.fair_yes,
            fair_no: estimate.fair_no,
            uncertainty_cents: estimate.uncertainty_cents,
        }),
    )
}

fn materialize_reward_quote_plan_for_live_orderbook_inner(
    plan: &RewardQuotePlan,
    books: &HashMap<String, RewardOrderBook>,
    config: &RewardBotConfig,
    now: OffsetDateTime,
    fair_value: Option<LiveFairValueEdgeContext>,
) -> std::result::Result<RewardLiveQuoteMaterialization, String> {
    let (yes_token, no_token) = reward_quote_plan_tokens(plan)?;
    let yes_state = get_token_book_state(&yes_token, books, config, now);
    let no_state = get_token_book_state(&no_token, books, config, now);
    let midpoint = yes_state
        .as_ref()
        .map(|state| state.midpoint)
        .or_else(|| no_state.as_ref().map(|state| Decimal::ONE - state.midpoint))
        .ok_or_else(|| "missing fresh orderbook midpoint for live quote".to_string())?;
    let metrics = build_market_book_metrics(&yes_token, &no_token, books, midpoint, config);
    let quote_mode = selected_live_quote_mode(plan, config, metrics.as_ref());
    if quote_mode == RewardPlanQuoteMode::None {
        let reason = metrics
            .as_ref()
            .and_then(|metrics| metrics.reason.clone())
            .unwrap_or_else(|| "auto quote mode skipped this market".to_string());
        return Err(reason);
    }

    let midpoint_is_in_double_range =
        midpoint >= config.min_midpoint && midpoint <= config.max_midpoint;
    if !midpoint_is_in_double_range
        && !matches!(
            quote_mode,
            RewardPlanQuoteMode::SingleYes | RewardPlanQuoteMode::SingleNo
        )
    {
        return Err("midpoint is too close to 0/1 for the first rewards strategy".to_string());
    }

    let max_spread_cents = Decimal::min(
        normalize_reward_spread_cents(plan.rewards_max_spread),
        config.max_spread_cents,
    );
    if max_spread_cents <= Decimal::ZERO {
        return Err("invalid rewards spread setting".to_string());
    }

    let max_spread = max_spread_cents / decimal("100");
    let no_mid = Decimal::ONE - midpoint;
    let yes_quote_midpoint = yes_state.as_ref().map_or(midpoint, |state| state.midpoint);
    let no_quote_midpoint = no_state.as_ref().map_or(no_mid, |state| state.midpoint);
    let provider_edge_buffer_cents = plan
        .ai_advisory
        .as_ref()
        .map_or(Decimal::ZERO, |advisory| {
            reward_ai_edge_buffer_cents(advisory, config)
        });
    let yes_bid = select_live_quote_bid(
        "YES",
        &yes_state,
        yes_quote_midpoint,
        max_spread,
        provider_edge_buffer_cents,
        fair_value.map(|context| (context.fair_yes, context.uncertainty_cents)),
        config,
    )?;
    let no_bid = select_live_quote_bid(
        "NO",
        &no_state,
        no_quote_midpoint,
        max_spread,
        provider_edge_buffer_cents,
        fair_value.map(|context| (context.fair_no, context.uncertainty_cents)),
        config,
    )?;

    let mut effective_quote_mode = quote_mode;
    let legs = match quote_mode {
        RewardPlanQuoteMode::Double => {
            match make_double_live_quote_legs(
                &yes_token,
                yes_bid,
                &no_token,
                no_bid,
                plan.rewards_min_size,
                config,
            ) {
                Ok(legs) => legs,
                Err(double_error) => {
                    if let Some((fallback_mode, legs)) = make_single_side_live_fallback_legs(
                        &yes_token,
                        yes_bid,
                        yes_quote_midpoint,
                        &yes_state,
                        &no_token,
                        no_bid,
                        no_quote_midpoint,
                        &no_state,
                        max_spread,
                        plan.rewards_min_size,
                        config,
                    ) {
                        effective_quote_mode = fallback_mode;
                        legs
                    } else {
                        return Err(double_error);
                    }
                }
            }
        }
        RewardPlanQuoteMode::SingleYes => {
            let yes_bid = yes_bid.ok_or_else(|| live_quote_selection_error("YES", config))?;
            let leg = make_single_quote_leg(&yes_token, yes_bid.price, plan.rewards_min_size)
                .ok_or_else(|| "rewards minimum size cannot be materialized".to_string())?;
            vec![leg]
        }
        RewardPlanQuoteMode::SingleNo => {
            let no_bid = no_bid.ok_or_else(|| live_quote_selection_error("NO", config))?;
            let leg = make_single_quote_leg(&no_token, no_bid.price, plan.rewards_min_size)
                .ok_or_else(|| "rewards minimum size cannot be materialized".to_string())?;
            vec![leg]
        }
        RewardPlanQuoteMode::None => unreachable!("none quote mode returned earlier"),
    };

    Ok(RewardLiveQuoteMaterialization {
        quote_mode: effective_quote_mode,
        recommended_quote_mode: metrics
            .as_ref()
            .map(|metrics| metrics.recommended_quote_mode),
        book_metrics: metrics,
        midpoint,
        legs,
    })
}

#[derive(Debug, Clone, Copy)]
struct LiveBidSelection {
    price: Decimal,
    rank: u16,
}

#[derive(Debug, Clone, Copy)]
struct LiveFairValueEdgeContext {
    fair_yes: Decimal,
    fair_no: Decimal,
    uncertainty_cents: Decimal,
}

fn select_live_quote_bid(
    label: &str,
    state: &Option<TokenBookState>,
    midpoint: Decimal,
    max_spread: Decimal,
    provider_edge_buffer_cents: Decimal,
    fair_value: Option<(Decimal, Decimal)>,
    config: &RewardBotConfig,
) -> std::result::Result<Option<LiveBidSelection>, String> {
    // Preserve hard book-risk reasons instead of collapsing them into a
    // generic "no safe rank" result. Callers use that distinction for audit
    // and immediate cancellation.
    validate_live_token_spread(label, state, config.max_market_spread_cents)?;
    let Some(state) = state.as_ref() else {
        return Ok(None);
    };
    let start = config.quote_bid_rank.clamp(1, 3);
    let end = config.quote_max_bid_rank.clamp(start, 3);
    for rank in start..=end {
        let Some(price) = quote_bid_price(state, rank) else {
            continue;
        };
        if validate_live_quote_bid(
            label,
            price,
            midpoint,
            &Some(state.clone()),
            max_spread,
            config.max_market_spread_cents,
            rank,
        )
        .is_err()
        {
            continue;
        }
        if !live_quote_preserves_trading_edge(
            price,
            midpoint,
            provider_edge_buffer_cents,
            fair_value,
            config,
        ) {
            continue;
        }
        return Ok(Some(LiveBidSelection { price, rank }));
    }
    Ok(None)
}

fn live_quote_preserves_trading_edge(
    price: Decimal,
    midpoint: Decimal,
    provider_edge_buffer_cents: Decimal,
    fair_value: Option<(Decimal, Decimal)>,
    config: &RewardBotConfig,
) -> bool {
    if !config.fair_value_enabled {
        return true;
    }
    let (fair_price, market_uncertainty_cents) = fair_value
        .unwrap_or((midpoint, config.fair_value_uncertainty_buffer_cents));
    let raw_edge_cents = ((fair_price - price) * decimal("100")).round_dp(4);
    let effective_edge_cents = (raw_edge_cents
        - market_uncertainty_cents
        - provider_edge_buffer_cents)
        .round_dp(4);
    raw_edge_cents >= config.fair_value_min_raw_edge_cents
        && effective_edge_cents >= config.fair_value_min_effective_edge_cents
}

fn live_quote_selection_error(label: &str, config: &RewardBotConfig) -> String {
    format!(
        "{label} has no safe bid between rank {} and {} preserving trading edge",
        config.quote_bid_rank, config.quote_max_bid_rank
    )
}

fn make_double_live_quote_legs(
    yes_token: &RewardToken,
    yes_bid: Option<LiveBidSelection>,
    no_token: &RewardToken,
    no_bid: Option<LiveBidSelection>,
    rewards_min_size: Decimal,
    config: &RewardBotConfig,
) -> std::result::Result<Vec<RewardQuoteLeg>, String> {
    let yes_bid = yes_bid.ok_or_else(|| live_quote_selection_error("YES", config))?;
    let no_bid = no_bid.ok_or_else(|| live_quote_selection_error("NO", config))?;
    let safety = config.safety_margin_cents / decimal("100");
    if yes_bid.price + no_bid.price > Decimal::ONE - safety {
        return Err("YES/NO bids do not leave enough safety margin".to_string());
    }
    make_quote_legs(
        yes_token,
        yes_bid.price,
        no_token,
        no_bid.price,
        rewards_min_size,
    )
        .ok_or_else(|| "rewards minimum size cannot be materialized".to_string())
}

fn make_single_side_live_fallback_legs(
    yes_token: &RewardToken,
    yes_bid: Option<LiveBidSelection>,
    yes_midpoint: Decimal,
    yes_state: &Option<TokenBookState>,
    no_token: &RewardToken,
    no_bid: Option<LiveBidSelection>,
    no_midpoint: Decimal,
    no_state: &Option<TokenBookState>,
    max_spread: Decimal,
    rewards_min_size: Decimal,
    config: &RewardBotConfig,
) -> Option<(RewardPlanQuoteMode, Vec<RewardQuoteLeg>)> {
    if config.quote_mode != RewardQuoteMode::Auto
        || config.selection_mode != RewardSelectionMode::Enforce
        || !config.dominant_single_side_enabled
    {
        return None;
    }

    let yes = make_single_side_live_fallback_leg(
        RewardPlanQuoteMode::SingleYes,
        "YES",
        yes_token,
        yes_bid,
        yes_midpoint,
        yes_state,
        max_spread,
        rewards_min_size,
        config.max_market_spread_cents,
    );
    let no = make_single_side_live_fallback_leg(
        RewardPlanQuoteMode::SingleNo,
        "NO",
        no_token,
        no_bid,
        no_midpoint,
        no_state,
        max_spread,
        rewards_min_size,
        config.max_market_spread_cents,
    );

    let selected = match (yes, no) {
        (Some(yes), Some(no)) => {
            let yes_notional = yes.1.price * yes.1.size;
            let no_notional = no.1.price * no.1.size;
            if yes_notional <= no_notional { yes } else { no }
        }
        (Some(yes), None) => yes,
        (None, Some(no)) => no,
        (None, None) => return None,
    };

    Some((selected.0, vec![selected.1]))
}

fn make_single_side_live_fallback_leg(
    quote_mode: RewardPlanQuoteMode,
    label: &str,
    token: &RewardToken,
    bid: Option<LiveBidSelection>,
    midpoint: Decimal,
    state: &Option<TokenBookState>,
    max_spread: Decimal,
    rewards_min_size: Decimal,
    max_market_spread_cents: Decimal,
) -> Option<(RewardPlanQuoteMode, RewardQuoteLeg)> {
    let bid = bid?;
    validate_live_quote_bid(
        label,
        bid.price,
        midpoint,
        state,
        max_spread,
        max_market_spread_cents,
        bid.rank,
    )
    .ok()?;
    make_single_quote_leg(token, bid.price, rewards_min_size).map(|leg| (quote_mode, leg))
}

fn selected_live_quote_mode(
    plan: &RewardQuotePlan,
    config: &RewardBotConfig,
    metrics: Option<&RewardMarketBookMetrics>,
) -> RewardPlanQuoteMode {
    if config.quote_mode == RewardQuoteMode::Auto
        && config.selection_mode == RewardSelectionMode::Enforce
    {
        let deterministic = selected_reward_quote_mode(config, metrics);
        if deterministic == RewardPlanQuoteMode::None {
            return RewardPlanQuoteMode::None;
        }
        if matches!(
            plan.quote_mode,
            RewardPlanQuoteMode::SingleYes | RewardPlanQuoteMode::SingleNo
        ) && deterministic == RewardPlanQuoteMode::Double
        {
            return plan.quote_mode;
        }
        return deterministic;
    }
    if matches!(
        plan.quote_mode,
        RewardPlanQuoteMode::SingleYes | RewardPlanQuoteMode::SingleNo
    ) {
        return plan.quote_mode;
    }
    selected_reward_quote_mode(config, metrics)
}

fn validate_live_quote_bid(
    label: &str,
    bid: Decimal,
    midpoint: Decimal,
    state: &Option<TokenBookState>,
    max_spread: Decimal,
    max_market_spread_cents: Decimal,
    quote_bid_rank: u16,
) -> std::result::Result<(), String> {
    validate_live_token_spread(label, state, max_market_spread_cents)?;
    if midpoint - bid > max_spread {
        return Err(format!(
            "{label} bid-{quote_bid_rank} is outside the rewards spread limit"
        ));
    }
    if bid_touches_ask(state, bid) {
        return Err(format!("{label} bid would touch best ask"));
    }
    Ok(())
}

fn validate_live_token_spread(
    label: &str,
    state: &Option<TokenBookState>,
    max_market_spread_cents: Decimal,
) -> std::result::Result<(), String> {
    if max_market_spread_cents <= Decimal::ZERO {
        return Ok(());
    }
    let Some(state) = state else {
        return Ok(());
    };
    let (Some(best_bid), Some(best_ask)) = (state.best_bid, state.best_ask) else {
        return Ok(());
    };
    if best_bid <= Decimal::ZERO || best_ask <= best_bid {
        return Ok(());
    }
    let spread_cents = ((best_ask - best_bid) * decimal("100")).round_dp(4);
    if spread_cents > max_market_spread_cents {
        return Err(format!(
            "{label} live token spread {spread_cents}c exceeds max market spread {max_market_spread_cents}c"
        ));
    }
    Ok(())
}

fn reward_quote_plan_tokens(
    plan: &RewardQuotePlan,
) -> std::result::Result<(RewardToken, RewardToken), String> {
    let yes = plan
        .legs
        .iter()
        .find(|leg| leg.outcome.trim().eq_ignore_ascii_case("yes"))
        .map(|leg| leg.token_id.clone());
    let no = plan
        .legs
        .iter()
        .find(|leg| leg.outcome.trim().eq_ignore_ascii_case("no"))
        .map(|leg| leg.token_id.clone());
    let (yes, no) = match (yes, no) {
        (Some(yes), Some(no)) => (yes, no),
        (Some(yes), None) => {
            let no = plan
                .orderbook_token_ids
                .iter()
                .find(|token_id| token_id.as_str() != yes.as_str())
                .cloned()
                .ok_or_else(|| "quote plan missing NO token for live validation".to_string())?;
            (yes, no)
        }
        (None, Some(no)) => {
            let yes = plan
                .orderbook_token_ids
                .iter()
                .find(|token_id| token_id.as_str() != no.as_str())
                .cloned()
                .ok_or_else(|| "quote plan missing YES token for live validation".to_string())?;
            (yes, no)
        }
        (None, None) => {
            return Err("quote plan has no token legs for live validation".to_string());
        }
    };
    Ok((
        RewardToken {
            token_id: yes,
            outcome: "Yes".to_string(),
            price: None,
        },
        RewardToken {
            token_id: no,
            outcome: "No".to_string(),
            price: None,
        },
    ))
}
