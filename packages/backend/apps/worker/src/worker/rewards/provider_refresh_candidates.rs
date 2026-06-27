const REWARD_PROVIDER_STANDARD_CONDITIONS_PER_LOW_COMPETITION: usize = 2;

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
    let plans_by_condition = plans
        .iter()
        .filter_map(|plan| {
            reward_provider_normalized_condition_id(&plan.condition_id)
                .map(|condition_id| (condition_id, plan))
        })
        .collect::<HashMap<_, _>>();

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
    let mut standard_conditions = Vec::new();
    let mut low_competition_conditions = Vec::new();
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
        let Some(plan) = plans_by_condition.get(&condition_id) else {
            continue;
        };
        match reward_provider_pre_llm_candidate_kind(plan, config, false) {
            Some(RewardProviderPreLlmCandidateKind::Standard) => {
                standard_conditions.push(condition_id);
            }
            Some(RewardProviderPreLlmCandidateKind::LowCompetition) => {
                low_competition_conditions.push(condition_id);
            }
            Some(RewardProviderPreLlmCandidateKind::ActiveExposure) | None => {}
        }
    }
    append_reward_provider_condition_mix(
        &mut ordered,
        standard_conditions,
        low_competition_conditions,
    );
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

fn append_reward_provider_condition_mix(
    ordered: &mut Vec<String>,
    standard_conditions: Vec<String>,
    low_competition_conditions: Vec<String>,
) {
    let mut standard_conditions = standard_conditions.into_iter();
    let mut low_competition_conditions = low_competition_conditions.into_iter();
    loop {
        let mut pushed = false;
        for _ in 0..REWARD_PROVIDER_STANDARD_CONDITIONS_PER_LOW_COMPETITION {
            if let Some(condition_id) = standard_conditions.next() {
                ordered.push(condition_id);
                pushed = true;
            } else {
                break;
            }
        }
        if let Some(condition_id) = low_competition_conditions.next() {
            ordered.push(condition_id);
            pushed = true;
        }
        if !pushed {
            break;
        }
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

fn reward_provider_configured_batch_size(value: u16) -> usize {
    usize::from(value.clamp(1, 12))
}
