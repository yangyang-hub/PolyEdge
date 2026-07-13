/// Compact historical snapshot introduced by replay schema V2 and retained by
/// V3. Decision history consumers read only the first valid bid/ask, so deeper
/// levels are omitted.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RewardReplayTopOfBookPoint {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub best_bid: Option<RewardBookLevel>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub best_ask: Option<RewardBookLevel>,
    #[serde(with = "time::serde::rfc3339")]
    pub observed_at: OffsetDateTime,
}

/// State changes between the original input and the final decision refresh.
/// History upserts replace one token's compact series, avoiding a second copy
/// of every unchanged book and account collection.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct RewardReplayFinalDelta {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account: Option<RewardAccountState>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub open_orders: Option<Vec<ManagedRewardOrder>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub positions: Option<Vec<RewardPosition>>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub book_upserts: HashMap<String, RewardOrderBook>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub book_removals: Vec<String>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub history_upserts: HashMap<String, Vec<RewardReplayTopOfBookPoint>>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub history_removals: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RewardReplayExpectedPlanHash {
    pub condition_id: String,
    pub strategy_profile: RewardStrategyProfile,
    pub normalized_sha256: String,
    pub reason_code: String,
}

#[allow(clippy::too_many_arguments)]
pub fn build_reward_decision_replay_fixture_v2(
    input: RewardStrategyInput,
    providers: RewardReplayProviderSnapshot,
    final_account: &RewardAccountState,
    final_open_orders: &[ManagedRewardOrder],
    final_positions: &[RewardPosition],
    final_books: &HashMap<String, RewardOrderBook>,
    final_book_history: &HashMap<String, VecDeque<BookSnapshot>>,
    expected_plans: &[RewardQuotePlan],
) -> Result<RewardDecisionReplayFixture> {
    let mut fixture = build_reward_decision_replay_fixture_pending_expectations(
        REWARD_DECISION_REPLAY_V2_SCHEMA_VERSION,
        input,
        providers,
        final_account,
        final_open_orders,
        final_positions,
        final_books,
        final_book_history,
    );
    set_reward_replay_expected_plan_hashes(&mut fixture, expected_plans)?;
    Ok(fixture)
}

#[allow(clippy::too_many_arguments)]
pub fn build_reward_decision_replay_fixture_v3(
    input: RewardStrategyInput,
    providers: RewardReplayProviderSnapshot,
    final_account: &RewardAccountState,
    final_open_orders: &[ManagedRewardOrder],
    final_positions: &[RewardPosition],
    final_books: &HashMap<String, RewardOrderBook>,
    final_book_history: &HashMap<String, VecDeque<BookSnapshot>>,
    expected_plans: &[RewardQuotePlan],
) -> Result<RewardDecisionReplayFixture> {
    let mut fixture = build_reward_decision_replay_fixture_v3_pending_expectations(
        input,
        providers,
        final_account,
        final_open_orders,
        final_positions,
        final_books,
        final_book_history,
    );
    set_reward_replay_expected_plan_hashes(&mut fixture, expected_plans)?;
    Ok(fixture)
}

#[allow(clippy::too_many_arguments)]
pub fn build_reward_decision_replay_fixture_v2_pending_expectations(
    input: RewardStrategyInput,
    providers: RewardReplayProviderSnapshot,
    final_account: &RewardAccountState,
    final_open_orders: &[ManagedRewardOrder],
    final_positions: &[RewardPosition],
    final_books: &HashMap<String, RewardOrderBook>,
    final_book_history: &HashMap<String, VecDeque<BookSnapshot>>,
) -> RewardDecisionReplayFixture {
    build_reward_decision_replay_fixture_pending_expectations(
        REWARD_DECISION_REPLAY_V2_SCHEMA_VERSION,
        input,
        providers,
        final_account,
        final_open_orders,
        final_positions,
        final_books,
        final_book_history,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn build_reward_decision_replay_fixture_v3_pending_expectations(
    input: RewardStrategyInput,
    providers: RewardReplayProviderSnapshot,
    final_account: &RewardAccountState,
    final_open_orders: &[ManagedRewardOrder],
    final_positions: &[RewardPosition],
    final_books: &HashMap<String, RewardOrderBook>,
    final_book_history: &HashMap<String, VecDeque<BookSnapshot>>,
) -> RewardDecisionReplayFixture {
    build_reward_decision_replay_fixture_pending_expectations(
        REWARD_DECISION_REPLAY_SCHEMA_VERSION,
        input,
        providers,
        final_account,
        final_open_orders,
        final_positions,
        final_books,
        final_book_history,
    )
}

#[allow(clippy::too_many_arguments)]
fn build_reward_decision_replay_fixture_pending_expectations(
    schema_version: u16,
    mut input: RewardStrategyInput,
    providers: RewardReplayProviderSnapshot,
    final_account: &RewardAccountState,
    final_open_orders: &[ManagedRewardOrder],
    final_positions: &[RewardPosition],
    final_books: &HashMap<String, RewardOrderBook>,
    final_book_history: &HashMap<String, VecDeque<BookSnapshot>>,
) -> RewardDecisionReplayFixture {
    let history_cutoff = reward_replay_history_cutoff(&input);
    let compact_book_history = compact_reward_replay_history(
        &std::mem::take(&mut input.book_history),
        history_cutoff,
    );
    let final_compact_history = final_book_history
        .iter()
        .map(|(token_id, snapshots)| {
            (
                token_id.clone(),
                snapshots
                    .iter()
                    .filter(|snapshot| snapshot.observed_at >= history_cutoff)
                    .map(compact_reward_replay_snapshot)
                    .collect::<Vec<_>>(),
            )
        })
        .collect::<HashMap<_, _>>();

    let final_delta = RewardReplayFinalDelta {
        account: (input.account != *final_account).then(|| final_account.clone()),
        open_orders: (input.open_orders != final_open_orders).then(|| final_open_orders.to_vec()),
        positions: (input.positions != final_positions).then(|| final_positions.to_vec()),
        book_upserts: final_books
            .iter()
            .filter(|(token_id, book)| input.books.get(*token_id) != Some(*book))
            .map(|(token_id, book)| (token_id.clone(), book.clone()))
            .collect(),
        book_removals: input
            .books
            .keys()
            .filter(|token_id| !final_books.contains_key(*token_id))
            .cloned()
            .collect(),
        history_upserts: final_compact_history
            .iter()
            .filter(|(token_id, history)| compact_book_history.get(*token_id) != Some(*history))
            .map(|(token_id, history)| (token_id.clone(), history.clone()))
            .collect(),
        history_removals: compact_book_history
            .keys()
            .filter(|token_id| !final_compact_history.contains_key(*token_id))
            .cloned()
            .collect(),
    };

    RewardDecisionReplayFixture {
        schema_version,
        input,
        providers,
        compact_book_history,
        final_state: None,
        final_delta: Some(final_delta),
        expected_plans: None,
        expected_plan_hashes: None,
    }
}

pub fn set_reward_replay_expected_plan_hashes(
    fixture: &mut RewardDecisionReplayFixture,
    expected_plans: &[RewardQuotePlan],
) -> Result<()> {
    fixture.expected_plans = None;
    fixture.expected_plan_hashes = Some(reward_replay_expected_plan_hashes(expected_plans)?);
    Ok(())
}

fn reward_replay_history_cutoff(input: &RewardStrategyInput) -> OffsetDateTime {
    let window_secs = input
        .config
        .fair_value_history_window_sec
        .max(input.config.opportunity_observation_window_sec);
    input.now - TimeDuration::seconds(i64::try_from(window_secs).unwrap_or(i64::MAX))
}

fn compact_reward_replay_history(
    history: &HashMap<String, Vec<BookSnapshot>>,
    cutoff: OffsetDateTime,
) -> HashMap<String, Vec<RewardReplayTopOfBookPoint>> {
    history
        .iter()
        .map(|(token_id, snapshots)| {
            (
                token_id.clone(),
                snapshots
                    .iter()
                    .filter(|snapshot| snapshot.observed_at >= cutoff)
                    .map(compact_reward_replay_snapshot)
                    .collect(),
            )
        })
        .collect()
}

fn compact_reward_replay_snapshot(snapshot: &BookSnapshot) -> RewardReplayTopOfBookPoint {
    RewardReplayTopOfBookPoint {
        best_bid: snapshot.bids.first().cloned(),
        best_ask: snapshot.asks.first().cloned(),
        observed_at: snapshot.observed_at,
    }
}

fn replay_compact_book_history(
    history: &HashMap<String, Vec<RewardReplayTopOfBookPoint>>,
) -> HashMap<String, VecDeque<BookSnapshot>> {
    history
        .iter()
        .map(|(token_id, points)| {
            (
                token_id.clone(),
                points
                    .iter()
                    .map(|point| BookSnapshot {
                        bids: point.best_bid.clone().into_iter().collect(),
                        asks: point.best_ask.clone().into_iter().collect(),
                        observed_at: point.observed_at,
                    })
                    .collect(),
            )
        })
        .collect()
}

fn apply_reward_replay_final_delta(
    delta: &RewardReplayFinalDelta,
    cycle: &mut RewardLiveCycle,
    books: &mut HashMap<String, RewardOrderBook>,
    history: &mut HashMap<String, VecDeque<BookSnapshot>>,
) {
    if let Some(account) = &delta.account {
        cycle.account = account.clone();
    }
    if let Some(open_orders) = &delta.open_orders {
        cycle.open_orders = open_orders.clone();
    }
    if let Some(positions) = &delta.positions {
        cycle.positions = positions.clone();
    }
    for token_id in &delta.book_removals {
        books.remove(token_id);
    }
    books.extend(delta.book_upserts.clone());
    for token_id in &delta.history_removals {
        history.remove(token_id);
    }
    history.extend(replay_compact_book_history(&delta.history_upserts));
}

fn reward_replay_expected_plan_hashes(
    plans: &[RewardQuotePlan],
) -> Result<Vec<RewardReplayExpectedPlanHash>> {
    plans
        .iter()
        .map(|plan| {
            Ok(RewardReplayExpectedPlanHash {
                condition_id: plan.condition_id.clone(),
                strategy_profile: plan.strategy_profile,
                normalized_sha256: normalized_reward_replay_plan_sha256(plan)?,
                reason_code: reward_quote_plan_reason_code(plan),
            })
        })
        .collect()
}

fn normalized_reward_replay_plan_sha256(plan: &RewardQuotePlan) -> Result<String> {
    let value = serde_json::to_value(normalized_reward_replay_plan(plan)).map_err(|error| {
        AppError::invalid_input(
            "REWARD_REPLAY_SERIALIZATION_FAILED",
            format!("failed to serialize normalized reward plan: {error}"),
        )
    })?;
    let bytes = serde_json::to_vec(&canonicalize_reward_replay_json_value(value)).map_err(
        |error| {
            AppError::invalid_input(
                "REWARD_REPLAY_SERIALIZATION_FAILED",
                format!("failed to serialize canonical normalized reward plan: {error}"),
            )
        },
    )?;
    Ok(format!("{:x}", Sha256::digest(&bytes)))
}

fn canonicalize_reward_replay_json_value(value: Value) -> Value {
    match value {
        Value::Array(values) => Value::Array(
            values
                .into_iter()
                .map(canonicalize_reward_replay_json_value)
                .collect(),
        ),
        Value::Object(values) => {
            let mut entries = values.into_iter().collect::<Vec<_>>();
            entries.sort_by(|left, right| left.0.cmp(&right.0));
            Value::Object(
                entries
                    .into_iter()
                    .map(|(key, value)| (key, canonicalize_reward_replay_json_value(value)))
                    .collect(),
            )
        }
        value => value,
    }
}

fn compare_reward_replay_plan_hashes(
    expected: &[RewardReplayExpectedPlanHash],
    actual: &[RewardQuotePlan],
) -> Result<RewardDecisionReplayComparison> {
    let expected_by_key = expected
        .iter()
        .map(|plan| ((plan.condition_id.clone(), plan.strategy_profile), plan))
        .collect::<HashMap<_, _>>();
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
                let actual_hash = normalized_reward_replay_plan_sha256(actual_plan)?;
                if expected_plan.normalized_sha256 != actual_hash {
                    differences.push(RewardDecisionReplayDifference {
                        condition_id,
                        strategy_profile,
                        kind: RewardDecisionReplayDifferenceKind::Changed,
                        expected_reason_code: Some(expected_plan.reason_code.clone()),
                        actual_reason_code: Some(reward_quote_plan_reason_code(actual_plan)),
                    });
                }
            }
            (Some(expected_plan), None) => differences.push(RewardDecisionReplayDifference {
                condition_id,
                strategy_profile,
                kind: RewardDecisionReplayDifferenceKind::MissingActual,
                expected_reason_code: Some(expected_plan.reason_code.clone()),
                actual_reason_code: None,
            }),
            (None, Some(actual_plan)) => differences.push(RewardDecisionReplayDifference {
                condition_id,
                strategy_profile,
                kind: RewardDecisionReplayDifferenceKind::UnexpectedActual,
                expected_reason_code: None,
                actual_reason_code: Some(reward_quote_plan_reason_code(actual_plan)),
            }),
            (None, None) => {}
        }
    }
    Ok(RewardDecisionReplayComparison {
        matches: differences.is_empty(),
        expected_plan_count: expected.len(),
        actual_plan_count: actual.len(),
        differences,
    })
}
