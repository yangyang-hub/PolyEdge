const REWARD_ORDERBOOK_ELIGIBLE_SOURCE: &str = "rewards_eligible";

async fn register_live_eligible_orderbook_tokens(
    state: &AppState,
    plans: &[RewardQuotePlan],
    trace_id: &str,
) {
    let token_ids =
        live_eligible_orderbook_tokens(plans, state.settings.orderbook_stream.max_tokens);
    if token_ids.is_empty() {
        return;
    }

    match state
        .orderbook_registry
        .register_tokens(REWARD_ORDERBOOK_ELIGIBLE_SOURCE, &token_ids)
        .await
    {
        Ok(()) => {
            debug!(
                trace_id = %trace_id,
                source = REWARD_ORDERBOOK_ELIGIBLE_SOURCE,
                tokens = token_ids.len(),
                "registered live eligible rewards tokens for orderbook subscription"
            );
        }
        Err(error) => {
            warn!(
                trace_id = %trace_id,
                source = REWARD_ORDERBOOK_ELIGIBLE_SOURCE,
                tokens = token_ids.len(),
                error = %error,
                "failed to register live eligible rewards tokens for orderbook subscription"
            );
        }
    }
}

fn live_eligible_orderbook_tokens(plans: &[RewardQuotePlan], max_tokens: usize) -> Vec<String> {
    if max_tokens == 0 {
        return Vec::new();
    }

    let mut token_ids = Vec::new();
    let mut seen = HashSet::new();
    for plan in plans.iter().filter(|plan| plan.eligible) {
        for leg in &plan.legs {
            let token_id = leg.token_id.trim();
            if token_id.is_empty() || !seen.insert(token_id.to_string()) {
                continue;
            }
            token_ids.push(token_id.to_string());
            if token_ids.len() >= max_tokens {
                return token_ids;
            }
        }
    }
    token_ids
}
