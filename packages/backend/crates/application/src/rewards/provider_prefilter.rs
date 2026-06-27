/// Hard gates applied before AI advisory / info-risk provider HTTP calls.
/// Non-eligible plans are excluded unless the condition already has open orders
/// or inventory.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RewardProviderPreLlmCandidateKind {
    ActiveExposure,
    Standard,
}

#[must_use]
pub fn reward_condition_has_active_exposure(
    condition_id: &str,
    open_orders: &[ManagedRewardOrder],
    positions: &[RewardPosition],
) -> bool {
    let condition_id = condition_id.trim();
    if condition_id.is_empty() {
        return false;
    }
    open_orders
        .iter()
        .any(|order| order.condition_id.trim() == condition_id)
        || positions.iter().any(|position| {
            position.condition_id.trim() == condition_id && position.size > Decimal::ZERO
        })
}

#[must_use]
pub fn reward_provider_plan_passes_pre_llm_gate(
    plan: &RewardQuotePlan,
    config: &RewardBotConfig,
    has_active_exposure: bool,
) -> bool {
    reward_provider_pre_llm_candidate_kind(plan, config, has_active_exposure).is_some()
}

#[must_use]
pub fn reward_provider_pre_llm_candidate_kind(
    plan: &RewardQuotePlan,
    _config: &RewardBotConfig,
    has_active_exposure: bool,
) -> Option<RewardProviderPreLlmCandidateKind> {
    if has_active_exposure {
        return Some(RewardProviderPreLlmCandidateKind::ActiveExposure);
    }

    match plan.strategy_bucket {
        RewardStrategyBucket::None => None,
        RewardStrategyBucket::Standard | RewardStrategyBucket::LowCompetition => {
            if plan.pre_ai_eligible || plan.eligible {
                Some(RewardProviderPreLlmCandidateKind::Standard)
            } else {
                None
            }
        }
    }
}
