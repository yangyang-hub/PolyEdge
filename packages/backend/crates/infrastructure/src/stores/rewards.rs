// Reward-bot persistence. An in-memory implementation backs tests and the no-database
// local path; a Postgres implementation backs shared, durable state. Both implement
// `RewardBotStore` and are split by backend; the row mappers and SQL helpers they share
// live in the parent `stores` module.

const REWARD_CONTROL_COMMAND_LEASE: Duration = Duration::minutes(5);

#[derive(Debug)]
struct NormalizedRewardFairValueEstimates {
    latest: Vec<RewardFairValueEstimate>,
    history: Vec<RewardFairValueEstimate>,
}

fn normalize_reward_fair_value_estimates(
    estimates: &[RewardFairValueEstimate],
) -> Result<NormalizedRewardFairValueEstimates> {
    let mut latest_by_condition = BTreeMap::<String, RewardFairValueEstimate>::new();
    let mut history_by_identity =
        BTreeMap::<(String, String, OffsetDateTime), RewardFairValueEstimate>::new();

    for estimate in estimates {
        let identity = (
            estimate.condition_id.clone(),
            estimate.source.clone(),
            estimate.observed_at,
        );
        if let Some(existing) = history_by_identity.get(&identity) {
            if existing != estimate {
                return Err(AppError::invalid_input(
                    "REWARD_FAIR_VALUE_DUPLICATE_CONFLICT",
                    format!(
                        "conflicting reward fair-value estimates share condition_id={}, source={}, observed_at={}",
                        estimate.condition_id, estimate.source, estimate.observed_at
                    ),
                ));
            }
        } else {
            history_by_identity.insert(identity, estimate.clone());
        }

        match latest_by_condition.get(&estimate.condition_id) {
            Some(existing) if existing.observed_at > estimate.observed_at => {}
            Some(existing) if existing.observed_at == estimate.observed_at => {
                if existing != estimate {
                    return Err(AppError::invalid_input(
                        "REWARD_FAIR_VALUE_LATEST_CONFLICT",
                        format!(
                            "conflicting latest reward fair-value estimates share condition_id={} and observed_at={}",
                            estimate.condition_id, estimate.observed_at
                        ),
                    ));
                }
            }
            _ => {
                latest_by_condition.insert(estimate.condition_id.clone(), estimate.clone());
            }
        }
    }

    Ok(NormalizedRewardFairValueEstimates {
        latest: latest_by_condition.into_values().collect(),
        history: history_by_identity.into_values().collect(),
    })
}

include!("rewards/in_memory.rs");
include!("rewards/postgres_control_commands.rs");
include!("rewards/postgres_event_windows.rs");
include!("rewards/postgres_heartbeat.rs");
include!("rewards/postgres_info_risk.rs");
include!("rewards/postgres_orders.rs");
include!("rewards/postgres_plans.rs");
include!("rewards/postgres_run_ledger.rs");
include!("rewards/postgres_writes.rs");
include!("rewards/postgres.rs");
