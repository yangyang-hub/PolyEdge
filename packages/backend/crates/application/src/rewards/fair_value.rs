const REWARD_FAIR_VALUE_SOURCE_MARKET_IMPLIED: &str = "market_implied_robust";

pub fn apply_reward_fair_values_to_quote_plans(
    plans: &mut [RewardQuotePlan],
    books: &HashMap<String, RewardOrderBook>,
    book_history: &HashMap<String, VecDeque<BookSnapshot>>,
    config: &RewardBotConfig,
    now: OffsetDateTime,
) -> Vec<RewardFairValueEstimate> {
    plans
        .iter_mut()
        .filter_map(|plan| {
            apply_reward_fair_value_to_quote_plan(plan, books, book_history, config, now)
        })
        .collect()
}

pub fn apply_reward_fair_value_to_quote_plan(
    plan: &mut RewardQuotePlan,
    books: &HashMap<String, RewardOrderBook>,
    book_history: &HashMap<String, VecDeque<BookSnapshot>>,
    config: &RewardBotConfig,
    now: OffsetDateTime,
) -> Option<RewardFairValueEstimate> {
    if !config.fair_value_enabled {
        plan.fair_value = None;
        return None;
    }

    let estimate = estimate_reward_fair_value(plan, books, book_history, config, now)
        .unwrap_or_else(|reason| empty_reward_fair_value_estimate(plan, reason, config, now));
    let decision = build_reward_fair_value_decision(plan, estimate.clone(), config);
    if !decision.passed
        && plan.quote_mode != RewardPlanQuoteMode::None
        && (plan.eligible || plan.pre_ai_eligible)
    {
        plan.eligible = false;
        plan.pre_ai_eligible = false;
        plan.reason = format!("fair value gate: {}", decision.reason);
    }
    plan.fair_value = Some(decision);
    plan.updated_at = now;
    Some(estimate)
}

fn estimate_reward_fair_value(
    plan: &RewardQuotePlan,
    books: &HashMap<String, RewardOrderBook>,
    book_history: &HashMap<String, VecDeque<BookSnapshot>>,
    config: &RewardBotConfig,
    now: OffsetDateTime,
) -> std::result::Result<RewardFairValueEstimate, String> {
    let (yes_token_id, no_token_id) = reward_fair_value_token_ids(plan)?;
    let yes = reward_fair_value_current_token_state(&yes_token_id, books, config, now);
    let no = reward_fair_value_current_token_state(&no_token_id, books, config, now);

    let mut components = Vec::new();
    if let Some(state) = yes {
        components.push(RewardFairValueComponent {
            source: "current_yes_midpoint".to_string(),
            value: state.midpoint,
            weight: Decimal::ONE,
            confidence: decimal("0.9"),
            reason: "fresh YES midpoint".to_string(),
        });
    }
    if let Some(state) = no {
        components.push(RewardFairValueComponent {
            source: "current_no_inverse_midpoint".to_string(),
            value: Decimal::ONE - state.midpoint,
            weight: Decimal::ONE,
            confidence: decimal("0.9"),
            reason: "fresh inverse NO midpoint".to_string(),
        });
    }

    let (history_components, history_sample_count, history_range_cents) =
        reward_fair_value_history_components(&yes_token_id, &no_token_id, book_history, config, now);
    components.extend(history_components);

    if components.is_empty() {
        return Err("no fresh or historical midpoint components available".to_string());
    }

    let fair_yes = weighted_reward_fair_value(&components)?;
    let fair_no = Decimal::ONE - fair_yes;
    let market_midpoint_yes = reward_fair_value_market_midpoint_yes(&components);
    let midpoint_deviation_cents = reward_fair_value_midpoint_deviation(yes, no);
    let max_current_spread_cents =
        Decimal::max(yes.map_or(Decimal::ZERO, |state| state.spread_cents), no.map_or(Decimal::ZERO, |state| state.spread_cents));

    let deviation_component = midpoint_deviation_cents.unwrap_or(Decimal::ZERO);
    let history_component = history_range_cents.unwrap_or_else(|| {
        if config.fair_value_min_history_samples > 0 {
            config.fair_value_uncertainty_buffer_cents
        } else {
            Decimal::ZERO
        }
    });
    let uncertainty_cents = Decimal::max(
        config.fair_value_uncertainty_buffer_cents,
        (max_current_spread_cents / decimal("2") + deviation_component + history_component / decimal("2")).round_dp(4),
    );
    let mut confidence = (Decimal::ONE - uncertainty_cents / decimal("20"))
        .max(decimal("0.05"))
        .min(decimal("0.99"))
        .round_dp(4);
    if history_sample_count < config.fair_value_min_history_samples {
        confidence = (confidence - decimal("0.1")).max(decimal("0.05"));
    }

    let do_not_quote_reason =
        reward_fair_value_rejection_reason(config, confidence, midpoint_deviation_cents);

    Ok(RewardFairValueEstimate {
        condition_id: plan.condition_id.clone(),
        source: REWARD_FAIR_VALUE_SOURCE_MARKET_IMPLIED.to_string(),
        fair_yes: fair_yes.round_dp(6),
        fair_no: fair_no.round_dp(6),
        market_midpoint_yes: market_midpoint_yes.map(|value| value.round_dp(6)),
        confidence,
        uncertainty_cents: uncertainty_cents.round_dp(4),
        midpoint_deviation_cents,
        sample_count: history_sample_count,
        components,
        do_not_quote_reason,
        observed_at: now,
        expires_at: now + TimeDuration::milliseconds(config.stale_book_ms as i64),
    })
}

fn build_reward_fair_value_decision(
    plan: &RewardQuotePlan,
    estimate: RewardFairValueEstimate,
    config: &RewardBotConfig,
) -> RewardFairValueDecision {
    let rebate_cents = reward_fair_value_rebate_cents(plan, config);
    let mut edges = Vec::new();
    for leg in &plan.legs {
        let fair_price = if leg.outcome.trim().eq_ignore_ascii_case("no") {
            estimate.fair_no
        } else {
            estimate.fair_yes
        };
        let raw_edge_cents = match leg.side {
            RewardOrderSide::Buy => (fair_price - leg.price) * decimal("100"),
            RewardOrderSide::Sell => (leg.price - fair_price) * decimal("100"),
        }
        .round_dp(4);
        // Market Maker V2: LP rewards are secondary income. They are reported
        // below but never subsidize quote admission. A quote must preserve its
        // trading edge after uncertainty on its own.
        let effective_edge_cents =
            (raw_edge_cents - estimate.uncertainty_cents).round_dp(4);
        let reward_adjusted_edge_cents = (effective_edge_cents + rebate_cents).round_dp(4);
        let mut reason = "fair-value edge accepted".to_string();
        let mut passed = true;
        if raw_edge_cents < config.fair_value_min_raw_edge_cents {
            passed = false;
            reason = format!(
                "raw edge {raw_edge_cents}c below {}c",
                config.fair_value_min_raw_edge_cents
            );
        } else if effective_edge_cents < config.fair_value_min_effective_edge_cents {
            passed = false;
            reason = format!(
                "effective trading edge {effective_edge_cents}c below {}c after uncertainty",
                config.fair_value_min_effective_edge_cents
            );
        }
        edges.push(RewardQuoteEdge {
            token_id: leg.token_id.clone(),
            outcome: leg.outcome.clone(),
            side: leg.side,
            quote_price: leg.price,
            fair_price,
            raw_edge_cents,
            expected_reward_rebate_cents: rebate_cents,
            uncertainty_cents: estimate.uncertainty_cents,
            effective_edge_cents,
            reward_adjusted_edge_cents,
            min_raw_edge_cents: config.fair_value_min_raw_edge_cents,
            min_effective_edge_cents: config.fair_value_min_effective_edge_cents,
            passed,
            reason,
        });
    }

    let mut passed = estimate.do_not_quote_reason.is_none() && !edges.is_empty();
    let mut reason = estimate
        .do_not_quote_reason
        .clone()
        .unwrap_or_else(|| "fair-value gate accepted".to_string());
    if passed
        && let Some(edge) = edges.iter().find(|edge| !edge.passed)
    {
        passed = false;
        reason = format!("{} {}", edge.outcome, edge.reason);
    }

    RewardFairValueDecision {
        estimate,
        edges,
        expected_reward_rebate_cents: rebate_cents,
        passed,
        reason,
    }
}

#[derive(Debug, Clone, Copy)]
struct RewardFairValueTokenState {
    midpoint: Decimal,
    spread_cents: Decimal,
}

fn reward_fair_value_current_token_state(
    token_id: &str,
    books: &HashMap<String, RewardOrderBook>,
    config: &RewardBotConfig,
    now: OffsetDateTime,
) -> Option<RewardFairValueTokenState> {
    let book = books.get(token_id)?;
    if config.stale_book_ms > 0 {
        let age_ms = (now - book.confirmed_at).whole_milliseconds();
        if age_ms < 0 || age_ms > config.stale_book_ms as i128 {
            return None;
        }
    }
    reward_fair_value_state_from_levels(&book.bids, &book.asks)
}

fn reward_fair_value_state_from_levels(
    bids: &[RewardBookLevel],
    asks: &[RewardBookLevel],
) -> Option<RewardFairValueTokenState> {
    let best_bid = bids.first()?.price;
    let best_ask = asks.first()?.price;
    if best_bid <= Decimal::ZERO || best_ask <= best_bid || best_ask >= Decimal::ONE {
        return None;
    }
    Some(RewardFairValueTokenState {
        midpoint: ((best_bid + best_ask) / decimal("2")).round_dp(6),
        spread_cents: ((best_ask - best_bid) * decimal("100")).round_dp(4),
    })
}

fn reward_fair_value_history_components(
    yes_token_id: &str,
    no_token_id: &str,
    book_history: &HashMap<String, VecDeque<BookSnapshot>>,
    config: &RewardBotConfig,
    now: OffsetDateTime,
) -> (Vec<RewardFairValueComponent>, u64, Option<Decimal>) {
    if config.fair_value_history_window_sec == 0 {
        return (Vec::new(), 0, None);
    }

    let cutoff = now - TimeDuration::seconds(config.fair_value_history_window_sec as i64);
    let yes_values = reward_fair_value_history_midpoints(yes_token_id, book_history, cutoff, false);
    let no_values = reward_fair_value_history_midpoints(no_token_id, book_history, cutoff, true);
    let sample_count = (yes_values.len() + no_values.len()) as u64;
    let mut components = Vec::new();
    if let Some(value) = median_decimal(&yes_values) {
        components.push(RewardFairValueComponent {
            source: "history_yes_median".to_string(),
            value,
            weight: decimal("0.5"),
            confidence: decimal("0.7"),
            reason: "YES historical median midpoint".to_string(),
        });
    }
    if let Some(value) = median_decimal(&no_values) {
        components.push(RewardFairValueComponent {
            source: "history_no_inverse_median".to_string(),
            value,
            weight: decimal("0.5"),
            confidence: decimal("0.7"),
            reason: "inverse NO historical median midpoint".to_string(),
        });
    }

    let mut all = yes_values;
    all.extend(no_values);
    let range = if all.len() >= config.fair_value_min_history_samples as usize && !all.is_empty() {
        let (min, max) = all.iter().fold((all[0], all[0]), |(min, max), value| {
            (Decimal::min(min, *value), Decimal::max(max, *value))
        });
        Some(((max - min) * decimal("100")).round_dp(4))
    } else {
        None
    };

    (components, sample_count, range)
}

fn reward_fair_value_history_midpoints(
    token_id: &str,
    book_history: &HashMap<String, VecDeque<BookSnapshot>>,
    cutoff: OffsetDateTime,
    inverse: bool,
) -> Vec<Decimal> {
    book_history
        .get(token_id)
        .into_iter()
        .flat_map(|history| history.iter())
        .filter(|snapshot| snapshot.observed_at >= cutoff)
        .filter_map(|snapshot| reward_fair_value_state_from_levels(&snapshot.bids, &snapshot.asks))
        .map(|state| {
            if inverse {
                Decimal::ONE - state.midpoint
            } else {
                state.midpoint
            }
        })
        .collect()
}

fn reward_fair_value_token_ids(
    plan: &RewardQuotePlan,
) -> std::result::Result<(String, String), String> {
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

    match (yes, no) {
        (Some(yes), Some(no)) => Ok((yes, no)),
        (Some(yes), None) => plan
            .orderbook_token_ids
            .iter()
            .find(|token_id| token_id.as_str() != yes.as_str())
            .cloned()
            .map(|no| (yes, no))
            .ok_or_else(|| "quote plan missing NO token for fair-value estimation".to_string()),
        (None, Some(no)) => plan
            .orderbook_token_ids
            .iter()
            .find(|token_id| token_id.as_str() != no.as_str())
            .cloned()
            .map(|yes| (yes, no))
            .ok_or_else(|| "quote plan missing YES token for fair-value estimation".to_string()),
        (None, None) => Err("quote plan has no legs for fair-value estimation".to_string()),
    }
}

fn weighted_reward_fair_value(components: &[RewardFairValueComponent]) -> std::result::Result<Decimal, String> {
    let total_weight: Decimal = components.iter().map(|component| component.weight).sum();
    if total_weight <= Decimal::ZERO {
        return Err("fair-value components have no positive weight".to_string());
    }
    Ok((components
        .iter()
        .map(|component| component.value * component.weight)
        .sum::<Decimal>()
        / total_weight)
        .max(Decimal::ZERO)
        .min(Decimal::ONE)
        .round_dp(6))
}

fn reward_fair_value_market_midpoint_yes(
    components: &[RewardFairValueComponent],
) -> Option<Decimal> {
    let current = components
        .iter()
        .filter(|component| {
            component.source == "current_yes_midpoint"
                || component.source == "current_no_inverse_midpoint"
        })
        .cloned()
        .collect::<Vec<_>>();
    (!current.is_empty())
        .then(|| weighted_reward_fair_value(&current).ok())
        .flatten()
}

fn reward_fair_value_midpoint_deviation(
    yes: Option<RewardFairValueTokenState>,
    no: Option<RewardFairValueTokenState>,
) -> Option<Decimal> {
    let yes = yes?;
    let no = no?;
    Some(((yes.midpoint + no.midpoint - Decimal::ONE).abs() * decimal("100")).round_dp(4))
}

fn reward_fair_value_rejection_reason(
    config: &RewardBotConfig,
    confidence: Decimal,
    midpoint_deviation_cents: Option<Decimal>,
) -> Option<String> {
    if let Some(deviation) = midpoint_deviation_cents
        && config.fair_value_max_midpoint_deviation_cents > Decimal::ZERO
        && deviation > config.fair_value_max_midpoint_deviation_cents
    {
        return Some(format!(
            "YES/NO midpoint sum deviation {deviation}c exceeds {}c",
            config.fair_value_max_midpoint_deviation_cents
        ));
    }
    if confidence < config.fair_value_min_confidence {
        return Some(format!(
            "fair-value confidence {confidence} below {}",
            config.fair_value_min_confidence
        ));
    }
    None
}

fn reward_fair_value_rebate_cents(plan: &RewardQuotePlan, config: &RewardBotConfig) -> Decimal {
    let reward_density = plan
        .opportunity_metrics
        .as_ref()
        .map(|metrics| metrics.estimated_reward_per_100_usd_day)
        .unwrap_or(Decimal::ZERO);
    (reward_density * config.fair_value_rebate_haircut)
        .max(Decimal::ZERO)
        .min(config.fair_value_max_reward_rebate_cents)
        .round_dp(4)
}

fn empty_reward_fair_value_estimate(
    plan: &RewardQuotePlan,
    reason: String,
    config: &RewardBotConfig,
    now: OffsetDateTime,
) -> RewardFairValueEstimate {
    RewardFairValueEstimate {
        condition_id: plan.condition_id.clone(),
        source: REWARD_FAIR_VALUE_SOURCE_MARKET_IMPLIED.to_string(),
        fair_yes: Decimal::ZERO,
        fair_no: Decimal::ZERO,
        market_midpoint_yes: None,
        confidence: Decimal::ZERO,
        uncertainty_cents: config.fair_value_uncertainty_buffer_cents,
        midpoint_deviation_cents: None,
        sample_count: 0,
        components: Vec::new(),
        do_not_quote_reason: Some(reason),
        observed_at: now,
        expires_at: now + TimeDuration::milliseconds(config.stale_book_ms as i64),
    }
}

fn median_decimal(values: &[Decimal]) -> Option<Decimal> {
    if values.is_empty() {
        return None;
    }
    let mut values = values.to_vec();
    values.sort();
    let mid = values.len() / 2;
    if values.len() % 2 == 0 {
        Some(((values[mid - 1] + values[mid]) / decimal("2")).round_dp(6))
    } else {
        Some(values[mid].round_dp(6))
    }
}
