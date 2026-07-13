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

#[derive(Debug)]
struct ValidatedRewardEventWindowSnapshot {
    covered_condition_ids: Vec<String>,
    condition_source_updated_at: HashMap<String, Option<OffsetDateTime>>,
    incoming_identities: HashSet<(String, String)>,
    condition_hashes: HashMap<String, String>,
}

fn validate_reward_event_window_snapshot(
    snapshot: &RewardEventWindowSourceSnapshot,
) -> Result<ValidatedRewardEventWindowSnapshot> {
    if snapshot.source.trim().is_empty() || snapshot.source.trim() != snapshot.source {
        return Err(AppError::invalid_input(
            "REWARD_EVENT_WINDOW_SOURCE_INVALID",
            "reward event-window snapshot source must be non-empty and normalized",
        ));
    }
    if snapshot.producer_version == 0 {
        return Err(AppError::invalid_input(
            "REWARD_EVENT_WINDOW_PRODUCER_VERSION_INVALID",
            "reward event-window snapshot producer_version must be greater than zero",
        ));
    }

    let mut covered = HashSet::new();
    let mut covered_condition_ids = Vec::new();
    let mut condition_source_updated_at = HashMap::new();
    for coverage in &snapshot.coverage {
        let condition_id = &coverage.condition_id;
        let normalized = condition_id.trim();
        if normalized.is_empty() || normalized != condition_id {
            return Err(AppError::invalid_input(
                "REWARD_EVENT_WINDOW_CONDITION_ID_INVALID",
                "reward event-window covered condition ids must be non-empty and normalized",
            ));
        }
        if covered.insert(condition_id.clone()) {
            covered_condition_ids.push(condition_id.clone());
            condition_source_updated_at.insert(condition_id.clone(), coverage.source_updated_at);
        } else if condition_source_updated_at.get(condition_id) != Some(&coverage.source_updated_at) {
            return Err(AppError::invalid_input(
                "REWARD_EVENT_WINDOW_COVERAGE_CONFLICT",
                format!(
                    "conflicting event-window coverage versions for condition {condition_id}"
                ),
            ));
        }
    }

    let mut incoming_identities = HashSet::new();
    for window in &snapshot.windows {
        if window.source != snapshot.source {
            return Err(AppError::invalid_input(
                "REWARD_EVENT_WINDOW_SOURCE_MISMATCH",
                format!(
                    "event-window source {} does not match snapshot source {}",
                    window.source, snapshot.source
                ),
            ));
        }
        if window.producer_version != snapshot.producer_version {
            return Err(AppError::invalid_input(
                "REWARD_EVENT_WINDOW_PRODUCER_VERSION_MISMATCH",
                format!(
                    "event-window producer version {} does not match snapshot version {}",
                    window.producer_version, snapshot.producer_version
                ),
            ));
        }
        if !covered.contains(&window.condition_id) {
            return Err(AppError::invalid_input(
                "REWARD_EVENT_WINDOW_OUTSIDE_COVERAGE",
                format!(
                    "event-window condition {} is not included in snapshot coverage",
                    window.condition_id
                ),
            ));
        }
        if window.event_key.trim().is_empty() || window.event_key.trim() != window.event_key {
            return Err(AppError::invalid_input(
                "REWARD_EVENT_WINDOW_KEY_INVALID",
                "reward event-window keys must be non-empty and normalized",
            ));
        }
        if window.producer_version == 0 {
            return Err(AppError::invalid_input(
                "REWARD_EVENT_WINDOW_PRODUCER_VERSION_INVALID",
                "reward event-window producer_version must be greater than zero",
            ));
        }
        if window
            .expires_at
            .is_some_and(|expires_at| expires_at < window.observed_at.unwrap_or(snapshot.observed_at))
        {
            return Err(AppError::invalid_input(
                "REWARD_EVENT_WINDOW_EXPIRY_INVALID",
                "reward event-window expires_at cannot precede observed_at",
            ));
        }
        if window.hard_gate_eligible && !reward_event_window_has_valid_hard_gate_shape(window) {
            return Err(AppError::invalid_input(
                "REWARD_EVENT_WINDOW_HARD_GATE_SHAPE_INVALID",
                format!(
                    "event-window condition {} key {} is not valid for hard gating",
                    window.condition_id, window.event_key
                ),
            ));
        }

        let identity = (window.condition_id.clone(), window.event_key.clone());
        if !incoming_identities.insert(identity) {
            return Err(AppError::invalid_input(
                "REWARD_EVENT_WINDOW_DUPLICATE_IDENTITY",
                format!(
                    "duplicate event-window identity for condition {} key {}",
                    window.condition_id, window.event_key
                ),
            ));
        }
    }

    let mut condition_hashes = HashMap::new();
    for condition_id in &covered_condition_ids {
        condition_hashes.insert(
            condition_id.clone(),
            reward_event_window_condition_snapshot_hash(snapshot, condition_id)?,
        );
    }

    Ok(ValidatedRewardEventWindowSnapshot {
        covered_condition_ids,
        condition_source_updated_at,
        incoming_identities,
        condition_hashes,
    })
}

fn reward_event_window_condition_snapshot_hash(
    snapshot: &RewardEventWindowSourceSnapshot,
    condition_id: &str,
) -> Result<String> {
    let mut windows = snapshot
        .windows
        .iter()
        .filter(|window| window.condition_id == condition_id)
        .cloned()
        .collect::<Vec<_>>();
    windows.sort_by(|left, right| left.event_key.cmp(&right.event_key));
    for window in &mut windows {
        window.observed_at = Some(window.observed_at.unwrap_or(snapshot.observed_at));
    }
    let payload = serde_json::to_vec(&json!({
        "source": snapshot.source,
        "producer_version": snapshot.producer_version,
        "observed_at": snapshot.observed_at,
        "condition_id": condition_id,
        "source_updated_at": snapshot
            .coverage
            .iter()
            .find(|coverage| coverage.condition_id == condition_id)
            .and_then(|coverage| coverage.source_updated_at),
        "windows": windows,
    }))
    .map_err(|error| {
        AppError::internal(
            "REWARD_EVENT_WINDOW_SNAPSHOT_HASH_FAILED",
            format!("failed to hash reward event-window snapshot: {error}"),
        )
    })?;
    Ok(format!("{:x}", Sha256::digest(payload)))
}

fn reward_event_window_source_version_cmp(
    producer_version: u32,
    source_updated_at: Option<OffsetDateTime>,
    observed_at: OffsetDateTime,
    existing_producer_version: u32,
    existing_source_updated_at: Option<OffsetDateTime>,
    existing_observed_at: OffsetDateTime,
) -> std::cmp::Ordering {
    producer_version
        .cmp(&existing_producer_version)
        .then_with(|| source_updated_at.cmp(&existing_source_updated_at))
        .then_with(|| observed_at.cmp(&existing_observed_at))
}

fn reward_event_window_has_valid_hard_gate_shape(window: &RewardMarketEventWindow) -> bool {
    let Some(start_at) = window.event_start_at else {
        return false;
    };
    let end_shape_valid = match window.end_policy {
        RewardEventEndPolicy::Point => true,
        RewardEventEndPolicy::Explicit => window.event_end_at.is_some(),
        RewardEventEndPolicy::UntilMarketClosed => true,
        RewardEventEndPolicy::Unknown => false,
    };

    window.active
        && window.event_time_role == RewardEventTimeRole::EventOccurrence
        && window.schedule_status == RewardEventScheduleStatus::Scheduled
        && window.time_precision == RewardEventTimePrecision::Exact
        && window
            .start_source_field
            .as_deref()
            .is_some_and(|field| !field.trim().is_empty() && field.trim() == field)
        && end_shape_valid
        && window.event_end_at.is_none_or(|end_at| end_at >= start_at)
}

fn reward_event_window_version_cmp(
    candidate: &RewardMarketEventWindow,
    candidate_observed_at: OffsetDateTime,
    existing: &RewardMarketEventWindow,
) -> std::cmp::Ordering {
    candidate
        .producer_version
        .cmp(&existing.producer_version)
        .then_with(|| candidate.source_updated_at.cmp(&existing.source_updated_at))
        .then_with(|| {
            candidate_observed_at.cmp(&existing.observed_at.unwrap_or(existing.updated_at))
        })
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

include!("rewards/in_memory_event_windows.rs");
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
