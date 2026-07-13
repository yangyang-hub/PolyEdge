use std::collections::BTreeMap;

pub const REWARD_DECISION_REPLAY_SCHEMA_VERSION: u16 = 3;
const REWARD_DECISION_REPLAY_V1_SCHEMA_VERSION: u16 = 1;
const REWARD_DECISION_REPLAY_V2_SCHEMA_VERSION: u16 = 2;
/// Hard ceiling for the canonical JSON representation persisted for one tick.
///
/// The database applies an additional defensive bound. Keeping this limit in
/// application code prevents an unexpectedly large book snapshot from
/// becoming a multi-megabyte write before it reaches the store implementation.
pub const REWARD_DECISION_REPLAY_MAX_JSON_BYTES: usize = 8 * 1024 * 1024;

/// Resolved provider rows that affected a historical decision tick.
///
/// This deliberately stores parsed cache values rather than provider settings
/// or raw LLM output. Replay never calls an external provider.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct RewardReplayProviderSnapshot {
    #[serde(default)]
    pub advisories: HashMap<String, RewardMarketAdvisory>,
    #[serde(default)]
    pub info_risks: HashMap<String, RewardMarketInfoRisk>,
}

/// Optional state observed immediately before the live tick's final snapshot
/// refresh. It lets a captured fixture include account/order reconciliation
/// without replaying any external side effect.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct RewardReplayFinalState {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account: Option<RewardAccountState>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub open_orders: Option<Vec<ManagedRewardOrder>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub positions: Option<Vec<RewardPosition>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub books: Option<HashMap<String, RewardOrderBook>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub book_history: Option<HashMap<String, Vec<BookSnapshot>>>,
}

/// Portable input for a deterministic Rewards decision replay.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RewardDecisionReplayFixture {
    #[serde(default = "default_reward_decision_replay_schema_version")]
    pub schema_version: u16,
    pub input: RewardStrategyInput,
    #[serde(default)]
    pub providers: RewardReplayProviderSnapshot,
    /// V2 stores only the top bid/ask used by historical decision metrics.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub compact_book_history: HashMap<String, Vec<RewardReplayTopOfBookPoint>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub final_state: Option<RewardReplayFinalState>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub final_delta: Option<RewardReplayFinalDelta>,
    /// Expected final plans captured from the original run. Omit this field to
    /// use the fixture only for counterfactual output generation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_plans: Option<Vec<RewardQuotePlan>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_plan_hashes: Option<Vec<RewardReplayExpectedPlanHash>>,
}

/// Integrity metadata and typed payload for one persisted full-tick fixture.
///
/// Only `RewardDecisionReplayFixture` can enter this record. That type excludes
/// connector credentials, provider settings and raw LLM responses. The
/// constructor additionally scans serialized field names to fail closed if a
/// future model accidentally introduces a credential-like field.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RewardStrategyReplayFixture {
    pub run_id: i64,
    pub schema_version: u16,
    pub fixture: RewardDecisionReplayFixture,
    pub json_bytes: u32,
    pub sha256: String,
    #[serde(with = "time::serde::rfc3339")]
    pub captured_at: OffsetDateTime,
}

impl RewardStrategyReplayFixture {
    pub fn capture(
        run_id: i64,
        fixture: RewardDecisionReplayFixture,
        captured_at: OffsetDateTime,
    ) -> Result<Self> {
        if run_id <= 0 {
            return Err(AppError::invalid_input(
                "REWARD_REPLAY_RUN_ID_INVALID",
                "rewards replay fixture requires a positive run id",
            ));
        }
        validate_reward_replay_fixture(&fixture)?;
        let canonical_json = canonical_reward_replay_fixture_json(&fixture)?;
        reject_sensitive_reward_json_fields(
            &canonical_json,
            "REWARD_REPLAY_SENSITIVE_FIELD_REJECTED",
        )?;
        let bytes = serde_json::to_vec(&canonical_json).map_err(|error| {
            AppError::invalid_input(
                "REWARD_REPLAY_SERIALIZATION_FAILED",
                format!("failed to serialize rewards replay fixture: {error}"),
            )
        })?;
        if bytes.len() > REWARD_DECISION_REPLAY_MAX_JSON_BYTES {
            return Err(AppError::invalid_input(
                "REWARD_REPLAY_FIXTURE_TOO_LARGE",
                format!(
                    "rewards replay fixture is {} bytes; maximum is {} bytes",
                    bytes.len(),
                    REWARD_DECISION_REPLAY_MAX_JSON_BYTES
                ),
            ));
        }
        let json_bytes = u32::try_from(bytes.len()).map_err(|_| {
            AppError::invalid_input(
                "REWARD_REPLAY_FIXTURE_TOO_LARGE",
                "rewards replay fixture size cannot be represented",
            )
        })?;
        let sha256 = format!("{:x}", Sha256::digest(&bytes));
        Ok(Self {
            run_id,
            schema_version: fixture.schema_version,
            fixture,
            json_bytes,
            sha256,
            captured_at,
        })
    }

    pub fn validate_integrity(&self) -> Result<()> {
        let rebuilt = Self::capture(self.run_id, self.fixture.clone(), self.captured_at)?;
        if self.schema_version != rebuilt.schema_version
            || self.json_bytes != rebuilt.json_bytes
            || self.sha256 != rebuilt.sha256
        {
            return Err(AppError::dependency_unavailable(
                "REWARD_REPLAY_FIXTURE_INTEGRITY_FAILED",
                format!("persisted rewards replay fixture {} failed integrity validation", self.run_id),
            ));
        }
        Ok(())
    }
}

const fn default_reward_decision_replay_schema_version() -> u16 {
    REWARD_DECISION_REPLAY_V1_SCHEMA_VERSION
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RewardDecisionReplaySummary {
    pub plan_count: usize,
    pub eligible_count: usize,
    pub ready_to_quote_count: usize,
    pub fair_value_pass_count: usize,
    pub provider_advisory_count: usize,
    pub info_risk_count: usize,
    pub blocker_counts: BTreeMap<String, usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum RewardDecisionReplayDifferenceKind {
    MissingActual,
    UnexpectedActual,
    Changed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RewardDecisionReplayDifference {
    pub condition_id: String,
    pub strategy_profile: RewardStrategyProfile,
    pub kind: RewardDecisionReplayDifferenceKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_reason_code: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actual_reason_code: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RewardDecisionReplayComparison {
    pub matches: bool,
    pub expected_plan_count: usize,
    pub actual_plan_count: usize,
    pub differences: Vec<RewardDecisionReplayDifference>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RewardDecisionReplayResult {
    pub schema_version: u16,
    pub summary: RewardDecisionReplaySummary,
    pub funding_precheck_blocked: usize,
    pub first_quote_entry_changed: bool,
    pub readiness_changed: bool,
    pub fair_value_estimates: Vec<RewardFairValueEstimate>,
    pub plans: Vec<RewardQuotePlan>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub comparison: Option<RewardDecisionReplayComparison>,
}

/// Re-run all three pure decision-engine phases from a portable fixture.
/// External providers, databases and live connectors are never accessed.
pub fn replay_reward_decision_engine(
    fixture: &RewardDecisionReplayFixture,
) -> Result<RewardDecisionReplayResult> {
    validate_reward_replay_fixture(fixture)?;

    let now = fixture.input.now;
    let mut books = fixture.input.books.clone();
    let mut book_history = if fixture.compact_book_history.is_empty() {
        replay_book_history(&fixture.input.book_history)
    } else {
        replay_compact_book_history(&fixture.compact_book_history)
    };
    let pre_provider = RewardDecisionEngine::evaluate_pre_provider(RewardLiveEngineInput {
        cycle: RewardLiveCycle::from_strategy_input(&fixture.input),
        books: &books,
        book_history: &book_history,
        now,
    });
    let funding_precheck_blocked = pre_provider.funding_precheck_blocked;
    let mut cycle = pre_provider.cycle;

    apply_reward_ai_advisories_at(
        &mut cycle.plans,
        &fixture.providers.advisories,
        &cycle.config,
        cycle.config.ai_action_min_confidence,
        now,
    );
    apply_reward_info_risks_at(
        &mut cycle.plans,
        &fixture.providers.info_risks,
        &cycle.config,
        cycle.config.info_risk_min_confidence,
        now,
    );

    let post_provider = RewardDecisionEngine::evaluate_post_provider(cycle, now);
    let first_quote_entry_changed = post_provider.first_quote_entry_changed;
    cycle = post_provider.cycle;

    if let Some(final_state) = &fixture.final_state {
        if let Some(account) = &final_state.account {
            cycle.account = account.clone();
        }
        if let Some(open_orders) = &final_state.open_orders {
            cycle.open_orders = open_orders.clone();
        }
        if let Some(positions) = &final_state.positions {
            cycle.positions = positions.clone();
        }
        if let Some(final_books) = &final_state.books {
            books = final_books.clone();
        }
        if let Some(final_book_history) = &final_state.book_history {
            book_history = replay_book_history(final_book_history);
        }
    }
    if let Some(final_delta) = &fixture.final_delta {
        apply_reward_replay_final_delta(
            final_delta,
            &mut cycle,
            &mut books,
            &mut book_history,
        );
    }

    let final_decisions = RewardDecisionEngine::refresh_snapshot(RewardLiveEngineInput {
        cycle,
        books: &books,
        book_history: &book_history,
        now,
    });
    let fair_value_estimates = final_decisions.fair_value_estimates;
    let readiness_changed = final_decisions.readiness_changed;
    let plans = final_decisions.cycle.plans;
    let summary = reward_replay_summary(&plans);
    let comparison = match (
        fixture.expected_plan_hashes.as_deref(),
        fixture.expected_plans.as_deref(),
    ) {
        (Some(expected), _) => Some(compare_reward_replay_plan_hashes(expected, &plans)?),
        (None, Some(expected)) => Some(compare_reward_replay_plans(expected, &plans)),
        (None, None) => None,
    };

    Ok(RewardDecisionReplayResult {
        schema_version: fixture.schema_version,
        summary,
        funding_precheck_blocked,
        first_quote_entry_changed,
        readiness_changed,
        fair_value_estimates,
        plans,
        comparison,
    })
}

fn validate_reward_replay_fixture(fixture: &RewardDecisionReplayFixture) -> Result<()> {
    if !matches!(
        fixture.schema_version,
        REWARD_DECISION_REPLAY_V1_SCHEMA_VERSION
            | REWARD_DECISION_REPLAY_V2_SCHEMA_VERSION
            | REWARD_DECISION_REPLAY_SCHEMA_VERSION
    ) {
        return Err(AppError::invalid_input(
            "REWARD_REPLAY_SCHEMA_VERSION_UNSUPPORTED",
            format!(
                "unsupported rewards replay schema version {}; supported versions are {}, {} and {}",
                fixture.schema_version,
                REWARD_DECISION_REPLAY_V1_SCHEMA_VERSION,
                REWARD_DECISION_REPLAY_V2_SCHEMA_VERSION,
                REWARD_DECISION_REPLAY_SCHEMA_VERSION
            ),
        ));
    }
    for (condition_id, advisory) in &fixture.providers.advisories {
        if condition_id != &advisory.condition_id {
            return Err(AppError::invalid_input(
                "REWARD_REPLAY_ADVISORY_KEY_MISMATCH",
                format!(
                    "advisory map key {condition_id} does not match payload condition {}",
                    advisory.condition_id
                ),
            ));
        }
    }
    for (condition_id, risk) in &fixture.providers.info_risks {
        if condition_id != &risk.condition_id {
            return Err(AppError::invalid_input(
                "REWARD_REPLAY_INFO_RISK_KEY_MISMATCH",
                format!(
                    "info-risk map key {condition_id} does not match payload condition {}",
                    risk.condition_id
                ),
            ));
        }
    }
    Ok(())
}

fn canonical_reward_replay_fixture_json(fixture: &RewardDecisionReplayFixture) -> Result<Value> {
    serde_json::to_value(fixture).map_err(|error| {
        AppError::invalid_input(
            "REWARD_REPLAY_SERIALIZATION_FAILED",
            format!("failed to encode rewards replay fixture: {error}"),
        )
    })
}

fn replay_book_history(
    snapshots: &HashMap<String, Vec<BookSnapshot>>,
) -> HashMap<String, VecDeque<BookSnapshot>> {
    snapshots
        .iter()
        .map(|(token_id, snapshots)| {
            (
                token_id.clone(),
                snapshots.iter().cloned().collect::<VecDeque<_>>(),
            )
        })
        .collect()
}

fn reward_replay_summary(plans: &[RewardQuotePlan]) -> RewardDecisionReplaySummary {
    let mut blocker_counts = BTreeMap::new();
    for plan in plans {
        let reason_code = reward_quote_plan_reason_code(plan);
        for blocker in reward_quote_plan_blocker_codes(plan, &reason_code) {
            *blocker_counts.entry(blocker).or_default() += 1;
        }
    }
    RewardDecisionReplaySummary {
        plan_count: plans.len(),
        eligible_count: plans.iter().filter(|plan| plan.eligible).count(),
        ready_to_quote_count: plans
            .iter()
            .filter(|plan| plan.quote_readiness == RewardQuoteReadiness::ReadyToQuote)
            .count(),
        fair_value_pass_count: plans
            .iter()
            .filter(|plan| {
                plan.fair_value
                    .as_ref()
                    .is_some_and(|decision| decision.passed)
            })
            .count(),
        provider_advisory_count: plans
            .iter()
            .filter(|plan| plan.ai_advisory.is_some())
            .count(),
        info_risk_count: plans
            .iter()
            .filter(|plan| plan.info_risk.is_some())
            .count(),
        blocker_counts,
    }
}

fn compare_reward_replay_plans(
    expected: &[RewardQuotePlan],
    actual: &[RewardQuotePlan],
) -> RewardDecisionReplayComparison {
    let expected_by_key = reward_replay_plans_by_key(expected);
    let actual_by_key = reward_replay_plans_by_key(actual);
    let mut keys = expected_by_key
        .keys()
        .chain(actual_by_key.keys())
        .cloned()
        .collect::<Vec<_>>();
    keys.sort_by(|left, right| {
        left.0
            .cmp(&right.0)
            .then_with(|| left.1.as_str().cmp(right.1.as_str()))
    });
    keys.dedup();

    let mut differences = Vec::new();
    for (condition_id, strategy_profile) in keys {
        let key = (condition_id.clone(), strategy_profile);
        match (expected_by_key.get(&key), actual_by_key.get(&key)) {
            (Some(expected_plan), Some(actual_plan)) => {
                if normalized_reward_replay_plan(expected_plan)
                    != normalized_reward_replay_plan(actual_plan)
                {
                    differences.push(RewardDecisionReplayDifference {
                        condition_id,
                        strategy_profile,
                        kind: RewardDecisionReplayDifferenceKind::Changed,
                        expected_reason_code: Some(reward_quote_plan_reason_code(expected_plan)),
                        actual_reason_code: Some(reward_quote_plan_reason_code(actual_plan)),
                    });
                }
            }
            (Some(expected_plan), None) => {
                differences.push(RewardDecisionReplayDifference {
                    condition_id,
                    strategy_profile,
                    kind: RewardDecisionReplayDifferenceKind::MissingActual,
                    expected_reason_code: Some(reward_quote_plan_reason_code(expected_plan)),
                    actual_reason_code: None,
                });
            }
            (None, Some(actual_plan)) => {
                differences.push(RewardDecisionReplayDifference {
                    condition_id,
                    strategy_profile,
                    kind: RewardDecisionReplayDifferenceKind::UnexpectedActual,
                    expected_reason_code: None,
                    actual_reason_code: Some(reward_quote_plan_reason_code(actual_plan)),
                });
            }
            (None, None) => {}
        }
    }

    RewardDecisionReplayComparison {
        matches: differences.is_empty(),
        expected_plan_count: expected.len(),
        actual_plan_count: actual.len(),
        differences,
    }
}

fn reward_replay_plans_by_key(
    plans: &[RewardQuotePlan],
) -> HashMap<(String, RewardStrategyProfile), &RewardQuotePlan> {
    plans
        .iter()
        .map(|plan| {
            (
                (plan.condition_id.clone(), plan.strategy_profile),
                plan,
            )
        })
        .collect()
}

fn normalized_reward_replay_plan(plan: &RewardQuotePlan) -> RewardQuotePlan {
    let mut plan = plan.clone();
    // Run linkage is ledger metadata and does not belong to decision-engine
    // consistency.
    plan.latest_run_id = None;
    plan
}
