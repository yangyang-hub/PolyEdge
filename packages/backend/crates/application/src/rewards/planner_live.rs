pub fn materialize_reward_quote_plan_for_live_orderbook(
    plan: &RewardQuotePlan,
    books: &HashMap<String, RewardOrderBook>,
    config: &RewardBotConfig,
) -> std::result::Result<RewardLiveQuoteMaterialization, String> {
    let now = OffsetDateTime::now_utc();
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

    let quote_bid_rank = selected_live_quote_bid_rank(plan, config);
    let yes_bid = yes_state
        .as_ref()
        .and_then(|state| quote_bid_price(state, quote_bid_rank));
    let no_bid = no_state
        .as_ref()
        .and_then(|state| quote_bid_price(state, quote_bid_rank));
    let max_spread = max_spread_cents / decimal("100");
    let no_mid = Decimal::ONE - midpoint;
    let yes_quote_midpoint = yes_state.as_ref().map_or(midpoint, |state| state.midpoint);
    let no_quote_midpoint = no_state.as_ref().map_or(no_mid, |state| state.midpoint);

    let mut effective_quote_mode = quote_mode;
    let legs = match quote_mode {
        RewardPlanQuoteMode::Double => {
            match make_double_live_quote_legs(
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
                quote_bid_rank,
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
                        quote_bid_rank,
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
            let yes_bid =
                yes_bid.ok_or_else(|| format!("YES book does not have bid-{quote_bid_rank}"))?;
            validate_live_quote_bid(
                "YES",
                yes_bid,
                yes_quote_midpoint,
                &yes_state,
                max_spread,
                config.max_market_spread_cents,
                quote_bid_rank,
            )?;
            let leg = make_single_quote_leg(&yes_token, yes_bid, plan.rewards_min_size)
                .ok_or_else(|| "rewards minimum size cannot be materialized".to_string())?;
            vec![leg]
        }
        RewardPlanQuoteMode::SingleNo => {
            let no_bid =
                no_bid.ok_or_else(|| format!("NO book does not have bid-{quote_bid_rank}"))?;
            validate_live_quote_bid(
                "NO",
                no_bid,
                no_quote_midpoint,
                &no_state,
                max_spread,
                config.max_market_spread_cents,
                quote_bid_rank,
            )?;
            let leg = make_single_quote_leg(&no_token, no_bid, plan.rewards_min_size)
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

fn make_double_live_quote_legs(
    yes_token: &RewardToken,
    yes_bid: Option<Decimal>,
    yes_midpoint: Decimal,
    yes_state: &Option<TokenBookState>,
    no_token: &RewardToken,
    no_bid: Option<Decimal>,
    no_midpoint: Decimal,
    no_state: &Option<TokenBookState>,
    max_spread: Decimal,
    rewards_min_size: Decimal,
    quote_bid_rank: u16,
    config: &RewardBotConfig,
) -> std::result::Result<Vec<RewardQuoteLeg>, String> {
    let yes_bid = yes_bid.ok_or_else(|| format!("YES book does not have bid-{quote_bid_rank}"))?;
    let no_bid = no_bid.ok_or_else(|| format!("NO book does not have bid-{quote_bid_rank}"))?;
    validate_live_quote_bid(
        "YES",
        yes_bid,
        yes_midpoint,
        yes_state,
        max_spread,
        config.max_market_spread_cents,
        quote_bid_rank,
    )?;
    validate_live_quote_bid(
        "NO",
        no_bid,
        no_midpoint,
        no_state,
        max_spread,
        config.max_market_spread_cents,
        quote_bid_rank,
    )?;
    let safety = config.safety_margin_cents / decimal("100");
    if yes_bid + no_bid > Decimal::ONE - safety {
        return Err("YES/NO bids do not leave enough safety margin".to_string());
    }
    make_quote_legs(yes_token, yes_bid, no_token, no_bid, rewards_min_size)
        .ok_or_else(|| "rewards minimum size cannot be materialized".to_string())
}

fn make_single_side_live_fallback_legs(
    yes_token: &RewardToken,
    yes_bid: Option<Decimal>,
    yes_midpoint: Decimal,
    yes_state: &Option<TokenBookState>,
    no_token: &RewardToken,
    no_bid: Option<Decimal>,
    no_midpoint: Decimal,
    no_state: &Option<TokenBookState>,
    max_spread: Decimal,
    rewards_min_size: Decimal,
    quote_bid_rank: u16,
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
        quote_bid_rank,
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
        quote_bid_rank,
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
    bid: Option<Decimal>,
    midpoint: Decimal,
    state: &Option<TokenBookState>,
    max_spread: Decimal,
    rewards_min_size: Decimal,
    quote_bid_rank: u16,
    max_market_spread_cents: Decimal,
) -> Option<(RewardPlanQuoteMode, RewardQuoteLeg)> {
    let bid = bid?;
    validate_live_quote_bid(
        label,
        bid,
        midpoint,
        state,
        max_spread,
        max_market_spread_cents,
        quote_bid_rank,
    )
    .ok()?;
    make_single_quote_leg(token, bid, rewards_min_size).map(|leg| (quote_mode, leg))
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

fn selected_live_quote_bid_rank(plan: &RewardQuotePlan, config: &RewardBotConfig) -> u16 {
    let configured = config.quote_bid_rank.clamp(1, 3);
    let hinted = plan
        .ai_advisory
        .as_ref()
        .and_then(|advisory| reward_ai_strategy_hint_bid_rank(advisory, config));
    hinted.map_or(configured, |rank| configured.max(rank.clamp(1, 3)))
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
        .ok_or_else(|| "quote plan missing YES token for live validation".to_string())?;
    let no = plan
        .legs
        .iter()
        .find(|leg| leg.outcome.trim().eq_ignore_ascii_case("no"))
        .ok_or_else(|| "quote plan missing NO token for live validation".to_string())?;
    Ok((
        RewardToken {
            token_id: yes.token_id.clone(),
            outcome: "Yes".to_string(),
            price: None,
        },
        RewardToken {
            token_id: no.token_id.clone(),
            outcome: "No".to_string(),
            price: None,
        },
    ))
}
