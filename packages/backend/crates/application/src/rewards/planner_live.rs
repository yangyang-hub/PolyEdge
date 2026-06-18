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

    if config.quote_size_usd <= Decimal::ZERO {
        return Err("quote size is zero".to_string());
    }

    let yes_bid = yes_state
        .as_ref()
        .and_then(|state| quote_bid_price(state, config.quote_bid_rank));
    let no_bid = no_state
        .as_ref()
        .and_then(|state| quote_bid_price(state, config.quote_bid_rank));
    let max_spread = max_spread_cents / decimal("100");
    let no_mid = Decimal::ONE - midpoint;
    let yes_quote_midpoint = yes_state.as_ref().map_or(midpoint, |state| state.midpoint);
    let no_quote_midpoint = no_state.as_ref().map_or(no_mid, |state| state.midpoint);

    let mut effective_quote_mode = quote_mode;
    let legs = match quote_mode {
        RewardPlanQuoteMode::Double => {
            let yes_bid = yes_bid
                .ok_or_else(|| format!("YES book does not have bid-{}", config.quote_bid_rank))?;
            let no_bid = no_bid
                .ok_or_else(|| format!("NO book does not have bid-{}", config.quote_bid_rank))?;
            validate_live_quote_bid(
                "YES",
                yes_bid,
                yes_quote_midpoint,
                &yes_state,
                max_spread,
                config.quote_bid_rank,
            )?;
            validate_live_quote_bid(
                "NO",
                no_bid,
                no_quote_midpoint,
                &no_state,
                max_spread,
                config.quote_bid_rank,
            )?;
            let safety = config.safety_margin_cents / decimal("100");
            if yes_bid + no_bid > Decimal::ONE - safety {
                return Err("YES/NO bids do not leave enough safety margin".to_string());
            }
            if let Some(legs) = make_quote_legs(
                &yes_token,
                yes_bid,
                &no_token,
                no_bid,
                plan.rewards_min_size,
                config,
            ) {
                legs
            } else if let Some((fallback_mode, legs)) = make_single_side_budget_fallback_legs(
                    &yes_token,
                    yes_bid,
                    &no_token,
                    no_bid,
                    plan.rewards_min_size,
                    config,
                ) {
                effective_quote_mode = fallback_mode;
                legs
            } else {
                return Err("per-market budget cannot satisfy rewards minimum size".to_string());
            }
        }
        RewardPlanQuoteMode::SingleYes => {
            let yes_bid = yes_bid
                .ok_or_else(|| format!("YES book does not have bid-{}", config.quote_bid_rank))?;
            validate_live_quote_bid(
                "YES",
                yes_bid,
                yes_quote_midpoint,
                &yes_state,
                max_spread,
                config.quote_bid_rank,
            )?;
            let leg = make_single_quote_leg(&yes_token, yes_bid, plan.rewards_min_size, config)
                .ok_or_else(|| {
                    "per-market budget cannot satisfy rewards minimum size".to_string()
                })?;
            vec![leg]
        }
        RewardPlanQuoteMode::SingleNo => {
            let no_bid = no_bid
                .ok_or_else(|| format!("NO book does not have bid-{}", config.quote_bid_rank))?;
            validate_live_quote_bid(
                "NO",
                no_bid,
                no_quote_midpoint,
                &no_state,
                max_spread,
                config.quote_bid_rank,
            )?;
            let leg = make_single_quote_leg(&no_token, no_bid, plan.rewards_min_size, config)
                .ok_or_else(|| {
                    "per-market budget cannot satisfy rewards minimum size".to_string()
                })?;
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
    quote_bid_rank: u16,
) -> std::result::Result<(), String> {
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
