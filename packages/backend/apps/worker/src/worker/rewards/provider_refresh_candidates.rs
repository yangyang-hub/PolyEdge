fn reward_provider_refresh_candidate_condition_ids(
    condition_ids: &[String],
    plans: &[RewardQuotePlan],
    open_orders: &[ManagedRewardOrder],
    positions: &[RewardPosition],
    config: &RewardBotConfig,
) -> Vec<String> {
    let available_conditions = condition_ids
        .iter()
        .filter_map(|condition_id| reward_provider_normalized_condition_id(condition_id))
        .collect::<HashSet<_>>();
    let mut seen = HashSet::new();
    let mut ordered = Vec::with_capacity(condition_ids.len());
    let plans_by_condition = reward_provider_plans_by_condition(plans);

    for order in open_orders {
        push_reward_provider_available_condition(
            &mut ordered,
            &mut seen,
            &available_conditions,
            &order.condition_id,
        );
    }
    for position in positions {
        push_reward_provider_available_condition(
            &mut ordered,
            &mut seen,
            &available_conditions,
            &position.condition_id,
        );
    }

    let mut queued = seen.clone();
    for condition_id in condition_ids {
        let Some(condition_id) = reward_provider_normalized_condition_id(condition_id) else {
            continue;
        };
        if !available_conditions.contains(&condition_id) {
            continue;
        }
        if !queued.insert(condition_id.clone()) {
            continue;
        }
        let Some(condition_plans) = plans_by_condition.get(condition_id.as_str()) else {
            continue;
        };
        if condition_plans.iter().any(|plan| {
            matches!(
                reward_provider_pre_llm_candidate_kind(plan, config, false),
                Some(RewardProviderPreLlmCandidateKind::Standard)
            )
        }) {
            ordered.push(condition_id);
        }
    }
    ordered
}

fn push_reward_provider_available_condition(
    ordered: &mut Vec<String>,
    seen: &mut HashSet<String>,
    available_conditions: &HashSet<String>,
    condition_id: &str,
) {
    let Some(condition_id) = reward_provider_normalized_condition_id(condition_id) else {
        return;
    };
    if !available_conditions.contains(&condition_id) {
        return;
    }
    if seen.insert(condition_id.clone()) {
        ordered.push(condition_id);
    }
}

fn reward_provider_normalized_condition_id(condition_id: &str) -> Option<String> {
    let condition_id = condition_id.trim();
    if condition_id.is_empty() {
        return None;
    }
    Some(condition_id.to_string())
}

fn reward_provider_max_conditions_per_cycle(state: &AppState) -> usize {
    usize::from(state.settings.rewards.info_risk_max_markets_per_cycle)
}
