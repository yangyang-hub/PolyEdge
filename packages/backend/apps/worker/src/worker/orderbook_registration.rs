/// Collect token IDs from active rewards state, execution orders, eligible plans,
/// and reward candidates, then register them with the orderbook service via HTTP.
async fn register_orderbook_tokens(
    state: &AppState,
    registration_state: &mut OrderbookRegistrationState,
) {
    let max_tokens = state.settings.orderbook_stream.max_tokens;
    let reward_candidate_token_cap = state.settings.orderbook_stream.reward_candidate_token_cap;
    let mut exec_candidates = Vec::new();
    let mut exec_candidate_seen = HashSet::new();
    let mut exec_query_complete = true;

    // Source 1: Active execution orders -> market YES/NO asset IDs.
    for status in [
        OrderStatus::Submitted,
        OrderStatus::Open,
        OrderStatus::PartiallyFilled,
    ] {
        // Cap at 200 to satisfy OrderListFilters validation (execution MAX_LIST_LIMIT).
        let fetch_limit = max_tokens.saturating_mul(2).min(200) as u16;
        let filters = match OrderListFilters::new(
            None,
            None,
            Some(POLYMARKET_CONNECTOR_NAME.to_string()),
            Some(status),
            Some(fetch_limit),
        ) {
            Ok(f) => f,
            Err(error) => {
                warn!(error = %error, "failed to build order filters for token registration");
                exec_query_complete = false;
                continue;
            }
        };
        let orders = match state.execution_service.list_orders(filters).await {
            Ok(orders) => orders,
            Err(error) => {
                warn!(error = %error, "failed to list orders for token registration");
                exec_query_complete = false;
                continue;
            }
        };

        for order in orders {
            if exec_candidates.len() >= max_tokens {
                break;
            }
            let market = match state.market_event_service.get_market(&order.market_id).await {
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
        exec_tokens = buckets.exec.len(),
        reward_eligible_tokens = buckets.eligible.len(),
        reward_candidate_tokens = buckets.candidate.len(),
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

fn push_unique_tokens(
    target: &mut Vec<String>,
    seen: &mut HashSet<String>,
    tokens: Vec<String>,
    limit: usize,
) {
    for token in tokens {
        if target.len() >= limit {
            break;
        }
        if token.trim().is_empty() || !seen.insert(token.clone()) {
            continue;
        }
        target.push(token);
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
/// full set here. This keeps `rewards_eligible` stable (no longer emptied when
/// active positions overlap eligible tokens), avoiding the WS subscription
/// rebuild oscillation that the previous shared-seen budget allocation caused.
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
}
