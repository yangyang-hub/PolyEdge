/// Collect token IDs from active rewards state, execution orders, final eligible
/// plans, and reward candidates, then register them with the orderbook service
/// via HTTP. Provider risk refresh is independent of live orderbook subscriptions.
async fn register_orderbook_tokens(
    state: &AppState,
    registration_state: &mut OrderbookRegistrationState,
) {
    let max_tokens = state.settings.orderbook_stream.max_tokens;
    let reward_candidate_token_cap = state.settings.orderbook_stream.reward_candidate_token_cap;
    if reward_candidate_token_cap == 0 && !registration_state.candidate_cap_zero_warned {
        warn!(
            "reward candidate orderbook prewarm is disabled because reward_candidate_token_cap is zero"
        );
        registration_state.candidate_cap_zero_warned = true;
    }
    let mut exec_candidates = Vec::new();
    let mut exec_candidate_seen = HashSet::new();
    let mut exec_query_complete = true;

    // Source 1: every distinct market with a potentially live execution order.
    // This specialized query is not constrained by the 200-row console list
    // limit, so older active orders cannot silently lose their subscriptions.
    let active_market_ids = match state
        .market_event_service
        .list_active_order_market_ids(POLYMARKET_CONNECTOR_NAME, max_tokens.max(1))
        .await
    {
        Ok(market_ids) => market_ids,
        Err(error) => {
            warn!(error = %error, "failed to list active order markets for token registration");
            exec_query_complete = false;
            Vec::new()
        }
    };
    for market_id in active_market_ids {
        if exec_candidates.len() >= max_tokens {
            break;
        }
        let market = match state.market_event_service.get_market(&market_id).await {
                Ok(m) => m,
                Err(_) => continue,
            };
        let refs = match polymarket_market_refs(&market) {
            Ok(r) => r,
            Err(_) => continue,
        };
        for token_id in [refs.yes_asset_id, refs.no_asset_id] {
            if exec_candidate_seen.insert(token_id.clone()) {
                exec_candidates.push(token_id);
            }
        }
    }

    // Collect raw token lists per source together with a completeness flag.
    // On query failure a source is marked incomplete and skipped below, so the
    // orderbook registry keeps its previous value for that source.
    let (active_tokens, reward_active_complete) =
        match state.reward_bot_service.list_active_reward_book_token_ids().await {
            Ok(tokens) => (tokens, true),
            Err(error) => {
                warn!(error = %error, "failed to list active rewards tokens for registration");
                (Vec::new(), false)
            }
        };
    let (eligible_tokens, reward_eligible_complete) =
        match state.reward_bot_service.list_eligible_reward_book_token_ids().await {
            Ok(tokens) => (tokens, true),
            Err(error) => {
                warn!(error = %error, "failed to list eligible rewards tokens for registration");
                (Vec::new(), false)
            }
        };
    let (candidate_tokens, reward_candidates_complete) = if reward_candidate_token_cap == 0 {
        (Vec::new(), true)
    } else {
        match state.reward_bot_service.list_all_reward_candidate_token_ids().await {
            Ok(tokens) => (tokens, true),
            Err(error) => {
                warn!(error = %error, "failed to list reward candidate tokens for registration");
                (Vec::new(), false)
            }
        }
    };

    let reward_active_raw_tokens = active_tokens.len();
    let reward_eligible_raw_tokens = eligible_tokens.len();
    let reward_candidate_raw_tokens = candidate_tokens.len();
    let exec_raw_tokens = exec_candidates.len();
    let buckets = allocate_registration_buckets(
        active_tokens,
        exec_candidates,
        eligible_tokens,
        candidate_tokens,
        max_tokens,
        reward_candidate_token_cap,
    );

    for (source, source_tokens, complete, empty_clear_after) in [
        (
            "rewards_active",
            buckets.active.as_slice(),
            reward_active_complete,
            REWARDS_ACTIVE_EMPTY_CLEAR_AFTER,
        ),
        (
            "exec_orders",
            buckets.exec.as_slice(),
            exec_query_complete,
            EXEC_ORDERS_EMPTY_CLEAR_AFTER,
        ),
        (
            "rewards_eligible",
            buckets.eligible.as_slice(),
            reward_eligible_complete,
            REWARDS_ELIGIBLE_EMPTY_CLEAR_AFTER,
        ),
        (
            "rewards_candidates",
            buckets.candidate.as_slice(),
            reward_candidates_complete,
            REWARDS_CANDIDATES_EMPTY_CLEAR_AFTER,
        ),
        ("rewards", &[], true, 0),
    ] {
        if !should_replace_orderbook_source(
            registration_state,
            source,
            source_tokens,
            complete,
            empty_clear_after,
        ) {
            continue;
        }
        if let Err(error) = state
            .orderbook_registry
            .register_tokens(source, source_tokens)
            .await
        {
            warn!(source, error = %error, "failed to replace orderbook token registration");
        }
    }

    info!(
        reward_active_tokens = buckets.active.len(),
        reward_active_raw_tokens,
        exec_tokens = buckets.exec.len(),
        exec_raw_tokens,
        reward_eligible_tokens = buckets.eligible.len(),
        reward_eligible_raw_tokens,
        reward_candidate_tokens = buckets.candidate.len(),
        reward_candidate_raw_tokens,
        reward_candidate_token_cap,
        max_tokens,
        "registered orderbook tokens with orderbook service"
    );
}

#[derive(Debug, Default)]
struct OrderbookRegistrationState {
    rewards_active_empty_streak: usize,
    exec_orders_empty_streak: usize,
    rewards_eligible_empty_streak: usize,
    rewards_candidates_empty_streak: usize,
    candidate_cap_zero_warned: bool,
}

const REWARDS_ACTIVE_EMPTY_CLEAR_AFTER: usize = 2;
const EXEC_ORDERS_EMPTY_CLEAR_AFTER: usize = 2;
const REWARDS_ELIGIBLE_EMPTY_CLEAR_AFTER: usize = 3;
const REWARDS_CANDIDATES_EMPTY_CLEAR_AFTER: usize = 3;

fn should_replace_orderbook_source(
    state: &mut OrderbookRegistrationState,
    source: &str,
    source_tokens: &[String],
    complete: bool,
    empty_clear_after: usize,
) -> bool {
    if !complete {
        return false;
    }
    if !source_tokens.is_empty() {
        reset_empty_streak(state, source);
        return true;
    }
    if empty_clear_after == 0 {
        return true;
    }

    let empty_streak = increment_empty_streak(state, source);
    if empty_streak >= empty_clear_after {
        return true;
    }

    info!(
        source,
        empty_streak,
        empty_clear_after,
        "orderbook token registration source returned empty; preserving previous registration"
    );
    false
}

fn increment_empty_streak(state: &mut OrderbookRegistrationState, source: &str) -> usize {
    let streak = empty_streak_mut(state, source);
    *streak += 1;
    *streak
}

fn reset_empty_streak(state: &mut OrderbookRegistrationState, source: &str) {
    *empty_streak_mut(state, source) = 0;
}

fn empty_streak_mut<'a>(
    state: &'a mut OrderbookRegistrationState,
    source: &str,
) -> &'a mut usize {
    match source {
        "rewards_active" => &mut state.rewards_active_empty_streak,
        "exec_orders" => &mut state.exec_orders_empty_streak,
        "rewards_eligible" => &mut state.rewards_eligible_empty_streak,
        "rewards_candidates" => &mut state.rewards_candidates_empty_streak,
        _ => &mut state.rewards_candidates_empty_streak,
    }
}

async fn register_reward_active_orderbook_tokens(state: &AppState, trace_id: &str) {
    let max_tokens = state.settings.orderbook_stream.max_tokens;
    let active_tokens = match state.reward_bot_service.list_active_reward_book_token_ids().await {
        Ok(tokens) => tokens,
        Err(error) => {
            warn!(
                trace_id = %trace_id,
                error = %error,
                "failed to list active rewards tokens for immediate orderbook registration"
            );
            return;
        }
    };

    let mut source_tokens = Vec::new();
    let mut seen = HashSet::new();
    push_unique_tokens(&mut source_tokens, &mut seen, active_tokens, max_tokens);
    if source_tokens.is_empty() {
        debug!(
            trace_id = %trace_id,
            "immediate active rewards orderbook registration returned empty; preserving previous source"
        );
        return;
    }

    if let Err(error) = state
        .orderbook_registry
        .register_tokens("rewards_active", &source_tokens)
        .await
    {
        warn!(
            trace_id = %trace_id,
            error = %error,
            "failed to immediately replace active rewards orderbook token registration"
        );
        return;
    }

    debug!(
        trace_id = %trace_id,
        reward_active_tokens = source_tokens.len(),
        max_tokens,
        "immediately registered active rewards orderbook tokens"
    );
}

async fn register_reward_eligible_orderbook_tokens_from_plans(
    state: &AppState,
    plans: &[RewardQuotePlan],
    trace_id: &str,
) {
    let max_tokens = state.settings.orderbook_stream.max_tokens;
    let (source_tokens, diagnostics) =
        reward_eligible_orderbook_tokens_with_diagnostics(plans, max_tokens);
    if source_tokens.is_empty() {
        if diagnostics.eligible_plans > 0 {
            warn!(
                trace_id = %trace_id,
                eligible_plans = diagnostics.eligible_plans,
                plans_missing_orderbook_tokens = diagnostics.plans_missing_orderbook_tokens,
                plans_using_leg_fallback = diagnostics.plans_using_leg_fallback,
                max_tokens,
                "eligible reward plans produced no orderbook registration tokens; preserving previous source"
            );
        } else {
            debug!(
                trace_id = %trace_id,
                "immediate eligible rewards orderbook registration returned empty; preserving previous source"
            );
        }
        return;
    }

    if let Err(error) = state
        .orderbook_registry
        .register_tokens("rewards_eligible", &source_tokens)
        .await
    {
        warn!(
            trace_id = %trace_id,
            error = %error,
            "failed to immediately replace eligible rewards orderbook token registration"
        );
        return;
    }

    debug!(
        trace_id = %trace_id,
        reward_eligible_tokens = source_tokens.len(),
        eligible_plans = diagnostics.eligible_plans,
        plans_missing_orderbook_tokens = diagnostics.plans_missing_orderbook_tokens,
        plans_using_leg_fallback = diagnostics.plans_using_leg_fallback,
        raw_tokens = diagnostics.raw_tokens,
        duplicate_or_empty_tokens = diagnostics.raw_tokens.saturating_sub(source_tokens.len()),
        truncated = diagnostics.truncated,
        max_tokens,
        "immediately registered eligible rewards orderbook tokens"
    );
}

#[derive(Debug, Default, PartialEq, Eq)]
struct EligibleRegistrationDiagnostics {
    eligible_plans: usize,
    plans_missing_orderbook_tokens: usize,
    plans_using_leg_fallback: usize,
    raw_tokens: usize,
    truncated: bool,
}

fn reward_eligible_orderbook_tokens_with_diagnostics(
    plans: &[RewardQuotePlan],
    max_tokens: usize,
) -> (Vec<String>, EligibleRegistrationDiagnostics) {
    let mut source_tokens = Vec::new();
    let mut seen = HashSet::new();
    let mut diagnostics = EligibleRegistrationDiagnostics::default();
    for plan in plans.iter().filter(|plan| plan.eligible) {
        diagnostics.eligible_plans += 1;
        if source_tokens.len() >= max_tokens {
            diagnostics.truncated = true;
            break;
        }
        if plan.orderbook_token_ids.is_empty() {
            diagnostics.plans_using_leg_fallback += 1;
            if plan.legs.is_empty() {
                diagnostics.plans_missing_orderbook_tokens += 1;
            }
            diagnostics.raw_tokens += plan.legs.len();
            for leg in &plan.legs {
                push_unique_token(&mut source_tokens, &mut seen, &leg.token_id, max_tokens);
            }
        } else {
            diagnostics.raw_tokens += plan.orderbook_token_ids.len();
            for token_id in &plan.orderbook_token_ids {
                push_unique_token(&mut source_tokens, &mut seen, token_id, max_tokens);
            }
        }
    }
    diagnostics.truncated |= source_tokens.len() >= max_tokens
        && diagnostics.raw_tokens > source_tokens.len();
    (source_tokens, diagnostics)
}

fn push_unique_token(
    target: &mut Vec<String>,
    seen: &mut HashSet<String>,
    token: &str,
    limit: usize,
) {
    if target.len() >= limit {
        return;
    }
    let token = token.trim();
    if token.is_empty() || !seen.insert(token.to_string()) {
        return;
    }
    target.push(token.to_string());
}

fn push_unique_tokens(
    target: &mut Vec<String>,
    seen: &mut HashSet<String>,
    tokens: Vec<String>,
    limit: usize,
) {
    for token in tokens {
        push_unique_token(target, seen, &token, limit);
    }
}

struct RegistrationBuckets {
    active: Vec<String>,
    exec: Vec<String>,
    eligible: Vec<String>,
    candidate: Vec<String>,
}

/// Dedup and cap each orderbook registration source independently.
///
/// Cross-source deduplication and the global `max_tokens` cap are applied by
/// the orderbook registry aggregation layer, so each source registers its own
/// full set here. `rewards_eligible` is limited to final orderable plans; pre-AI
/// provider markets are registered separately as a bounded temporary source.
/// `candidate` is capped by `candidate_cap` to preserve the cold-start prewarm
/// budget.
fn allocate_registration_buckets(
    active: Vec<String>,
    exec: Vec<String>,
    eligible: Vec<String>,
    candidate: Vec<String>,
    max_tokens: usize,
    candidate_cap: usize,
) -> RegistrationBuckets {
    let mut buckets = RegistrationBuckets {
        active: Vec::new(),
        exec: Vec::new(),
        eligible: Vec::new(),
        candidate: Vec::new(),
    };
    let mut active_seen = HashSet::new();
    let mut exec_seen = HashSet::new();
    let mut eligible_seen = HashSet::new();
    let mut candidate_seen = HashSet::new();
    push_unique_tokens(&mut buckets.active, &mut active_seen, active, max_tokens);
    push_unique_tokens(&mut buckets.exec, &mut exec_seen, exec, max_tokens);
    push_unique_tokens(&mut buckets.eligible, &mut eligible_seen, eligible, max_tokens);
    push_unique_tokens(
        &mut buckets.candidate,
        &mut candidate_seen,
        candidate,
        candidate_cap,
    );
    buckets
}

#[cfg(test)]
mod orderbook_registration_tests {
    use super::*;

    #[test]
    fn empty_eligible_source_is_debounced_until_threshold() {
        let mut state = OrderbookRegistrationState::default();
        let empty: Vec<String> = Vec::new();

        assert!(!should_replace_orderbook_source(
            &mut state,
            "rewards_eligible",
            &empty,
            true,
            REWARDS_ELIGIBLE_EMPTY_CLEAR_AFTER,
        ));
        assert!(!should_replace_orderbook_source(
            &mut state,
            "rewards_eligible",
            &empty,
            true,
            REWARDS_ELIGIBLE_EMPTY_CLEAR_AFTER,
        ));
        assert!(should_replace_orderbook_source(
            &mut state,
            "rewards_eligible",
            &empty,
            true,
            REWARDS_ELIGIBLE_EMPTY_CLEAR_AFTER,
        ));
    }

    #[test]
    fn non_empty_source_resets_empty_streak() {
        let mut state = OrderbookRegistrationState::default();
        let empty: Vec<String> = Vec::new();
        let non_empty = vec!["123".to_string()];

        assert!(!should_replace_orderbook_source(
            &mut state,
            "rewards_candidates",
            &empty,
            true,
            REWARDS_CANDIDATES_EMPTY_CLEAR_AFTER,
        ));
        assert!(should_replace_orderbook_source(
            &mut state,
            "rewards_candidates",
            &non_empty,
            true,
            REWARDS_CANDIDATES_EMPTY_CLEAR_AFTER,
        ));
        assert!(!should_replace_orderbook_source(
            &mut state,
            "rewards_candidates",
            &empty,
            true,
            REWARDS_CANDIDATES_EMPTY_CLEAR_AFTER,
        ));
    }

    #[test]
    fn immediate_eligible_registration_collects_only_eligible_plan_tokens() {
        let now = OffsetDateTime::now_utc();
        let plans = vec![
            RewardQuotePlan {
                condition_id: "cond_a".to_string(),
                market_slug: "a".to_string(),
                question: "A?".to_string(),
                score: Decimal::ONE,
                selection_score: Decimal::ZERO,
                eligible: true,
                pre_ai_eligible: true,
                quote_readiness: polyedge_application::RewardQuoteReadiness::ReadyToQuote,
                reason: "eligible".to_string(),
                strategy_bucket: RewardStrategyBucket::None,
                strategy_profile: RewardStrategyProfile::Standard,
                latest_run_id: None,
                quote_mode: RewardPlanQuoteMode::Double,
                recommended_quote_mode: None,
                book_metrics: None,
                opportunity_metrics: None,
                selection_metrics: None,
                fair_value: None,
                ai_advisory: None,
                info_risk: None,
                event_window: None,
                midpoint: None,
                live_skip_until: None,
                live_skip_reason: None,
                first_quote_observed_at: None,
                ai_advisory_pending_since: None,
                info_risk_pending_since: None,
                total_daily_rate: Decimal::ZERO,
                rewards_max_spread: Decimal::ZERO,
                rewards_min_size: Decimal::ZERO,
                orderbook_token_ids: vec!["yes_a".to_string(), "no_a".to_string()],
                legs: Vec::new(),
                updated_at: now,
            },
            RewardQuotePlan {
                condition_id: "cond_b".to_string(),
                market_slug: "b".to_string(),
                question: "B?".to_string(),
                score: Decimal::ONE,
                selection_score: Decimal::ZERO,
                eligible: false,
                pre_ai_eligible: true,
                quote_readiness: polyedge_application::RewardQuoteReadiness::ProviderPending,
                reason: "provider pending".to_string(),
                strategy_bucket: RewardStrategyBucket::None,
                strategy_profile: RewardStrategyProfile::Standard,
                latest_run_id: None,
                quote_mode: RewardPlanQuoteMode::None,
                recommended_quote_mode: None,
                book_metrics: None,
                opportunity_metrics: None,
                selection_metrics: None,
                fair_value: None,
                ai_advisory: None,
                info_risk: None,
                event_window: None,
                midpoint: None,
                live_skip_until: None,
                live_skip_reason: None,
                first_quote_observed_at: None,
                ai_advisory_pending_since: None,
                info_risk_pending_since: None,
                total_daily_rate: Decimal::ZERO,
                rewards_max_spread: Decimal::ZERO,
                rewards_min_size: Decimal::ZERO,
                orderbook_token_ids: vec!["yes_b".to_string(), "no_b".to_string()],
                legs: Vec::new(),
                updated_at: now,
            },
        ];

        let (tokens, diagnostics) =
            reward_eligible_orderbook_tokens_with_diagnostics(&plans, 10);
        assert_eq!(tokens, vec!["yes_a".to_string(), "no_a".to_string()]);
        assert_eq!(diagnostics.eligible_plans, 1);
        assert_eq!(diagnostics.raw_tokens, 2);
        assert_eq!(diagnostics.plans_missing_orderbook_tokens, 0);
    }
}
