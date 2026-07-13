use super::*;

fn replay_test_fixture(now: OffsetDateTime) -> RewardDecisionReplayFixture {
    RewardDecisionReplayFixture {
        schema_version: REWARD_DECISION_REPLAY_SCHEMA_VERSION,
        input: super::strategy_input_tests::strategy_test_snapshot(now, false),
        providers: RewardReplayProviderSnapshot::default(),
        compact_book_history: HashMap::new(),
        final_state: None,
        final_delta: None,
        expected_plans: None,
        expected_plan_hashes: None,
    }
}

#[test]
fn reward_decision_replay_is_deterministic_and_compares_expected_plans() {
    let now = OffsetDateTime::from_unix_timestamp(1_750_000_000).expect("fixed timestamp");
    let mut fixture = replay_test_fixture(now);

    let first = replay_reward_decision_engine(&fixture).expect("first replay");
    let second = replay_reward_decision_engine(&fixture).expect("second replay");
    assert_eq!(first, second);

    fixture.expected_plans = Some(first.plans.clone());
    let compared = replay_reward_decision_engine(&fixture).expect("compared replay");
    assert_eq!(compared.comparison.as_ref().map(|value| value.matches), Some(true));

    let serialized = serde_json::to_value(&fixture).expect("serialize fixture");
    let deserialized: RewardDecisionReplayFixture =
        serde_json::from_value(serialized).expect("deserialize fixture");
    assert_eq!(deserialized, fixture);
}

#[test]
fn reward_decision_replay_applies_captured_provider_snapshot_at_input_time() {
    let now = OffsetDateTime::from_unix_timestamp(1_750_000_000).expect("fixed timestamp");
    let mut fixture = replay_test_fixture(now);
    fixture.input.config.ai_advisory_enabled = true;
    fixture.input.config.ai_advisory_provider_pending_grace_sec = 0;
    fixture.providers.advisories.insert(
        "cond_strategy".to_string(),
        RewardMarketAdvisory {
            condition_id: "cond_strategy".to_string(),
            provider: RewardAiProvider::OpenAi,
            request_format: RewardAiRequestFormat::OpenAiResponses,
            model: "replay-test".to_string(),
            input_hash: "replay-input".to_string(),
            action: RewardProviderAction::StopNew,
            size_multiplier: Decimal::ZERO,
            edge_buffer_cents: decimal("2"),
            confidence: Decimal::ONE,
            reasons: vec!["captured structural risk".to_string()],
            metrics: json!({}),
            created_at: now - TimeDuration::minutes(5),
            expires_at: now + TimeDuration::hours(1),
        },
    );

    let result = replay_reward_decision_engine(&fixture).expect("provider replay");
    let plan = result.plans.first().expect("replayed plan");
    assert_eq!(
        plan.ai_advisory.as_ref().map(|advisory| advisory.action),
        Some(RewardProviderAction::StopNew)
    );
    assert!(!plan.eligible);
    assert_eq!(result.summary.provider_advisory_count, 1);
}

#[test]
fn reward_decision_replay_rejects_provider_key_mismatch() {
    let now = OffsetDateTime::from_unix_timestamp(1_750_000_000).expect("fixed timestamp");
    let mut fixture = replay_test_fixture(now);
    fixture.providers.advisories.insert(
        "wrong_condition".to_string(),
        RewardMarketAdvisory {
            condition_id: "cond_strategy".to_string(),
            provider: RewardAiProvider::OpenAi,
            request_format: RewardAiRequestFormat::OpenAiResponses,
            model: "replay-test".to_string(),
            input_hash: "replay-input".to_string(),
            action: RewardProviderAction::Allow,
            size_multiplier: Decimal::ONE,
            edge_buffer_cents: Decimal::ZERO,
            confidence: Decimal::ONE,
            reasons: Vec::new(),
            metrics: json!({}),
            created_at: now,
            expires_at: now + TimeDuration::hours(1),
        },
    );

    let error = replay_reward_decision_engine(&fixture).expect_err("mismatch must fail");
    assert_eq!(error.code(), "REWARD_REPLAY_ADVISORY_KEY_MISMATCH");
}

#[test]
fn reward_decision_replay_comparison_ignores_run_linkage_only() {
    let now = OffsetDateTime::from_unix_timestamp(1_750_000_000).expect("fixed timestamp");
    let mut fixture = replay_test_fixture(now);
    let first = replay_reward_decision_engine(&fixture).expect("first replay");
    let mut expected = first.plans.clone();
    expected[0].latest_run_id = Some(42);
    fixture.expected_plans = Some(expected);

    let result = replay_reward_decision_engine(&fixture).expect("comparison replay");
    assert_eq!(result.comparison.as_ref().map(|value| value.matches), Some(true));
}

#[test]
fn persisted_replay_fixture_has_stable_integrity_metadata() {
    let now = OffsetDateTime::from_unix_timestamp(1_750_000_000).expect("fixed timestamp");
    let fixture = replay_test_fixture(now);

    let captured = RewardStrategyReplayFixture::capture(42, fixture.clone(), now)
        .expect("capture replay fixture");
    let recaptured =
        RewardStrategyReplayFixture::capture(42, fixture, now).expect("recapture replay fixture");

    assert_eq!(captured.schema_version, REWARD_DECISION_REPLAY_SCHEMA_VERSION);
    assert!(captured.json_bytes > 0);
    assert_eq!(captured.sha256.len(), 64);
    assert_eq!(captured, recaptured);
    captured.validate_integrity().expect("fixture integrity");
}

#[test]
fn persisted_replay_fixture_rejects_sensitive_nested_fields() {
    let now = OffsetDateTime::from_unix_timestamp(1_750_000_000).expect("fixed timestamp");
    let mut fixture = replay_test_fixture(now);
    fixture.input.plans[0].ai_advisory = Some(RewardMarketAdvisory {
        condition_id: "cond_strategy".to_string(),
        provider: RewardAiProvider::OpenAi,
        request_format: RewardAiRequestFormat::OpenAiResponses,
        model: "replay-test".to_string(),
        input_hash: "replay-input".to_string(),
        action: RewardProviderAction::Allow,
        size_multiplier: Decimal::ONE,
        edge_buffer_cents: Decimal::ZERO,
        confidence: Decimal::ONE,
        reasons: Vec::new(),
        metrics: json!({"nested": {"private_key": "must-not-persist"}}),
        created_at: now,
        expires_at: now + TimeDuration::hours(1),
    });

    let error = RewardStrategyReplayFixture::capture(42, fixture, now)
        .expect_err("sensitive fixture must be rejected");
    assert_eq!(error.code(), "REWARD_REPLAY_SENSITIVE_FIELD_REJECTED");
}

#[test]
fn persisted_replay_fixture_enforces_json_size_limit() {
    let now = OffsetDateTime::from_unix_timestamp(1_750_000_000).expect("fixed timestamp");
    let mut fixture = replay_test_fixture(now);
    fixture.input.plans[0].question = "x".repeat(REWARD_DECISION_REPLAY_MAX_JSON_BYTES);

    let error = RewardStrategyReplayFixture::capture(42, fixture, now)
        .expect_err("oversized fixture must be rejected");
    assert_eq!(error.code(), "REWARD_REPLAY_FIXTURE_TOO_LARGE");
}

#[test]
fn reward_decision_replay_reads_v1_without_v2_fields() {
    let now = OffsetDateTime::from_unix_timestamp(1_750_000_000).expect("fixed timestamp");
    let mut value = serde_json::to_value(replay_test_fixture(now)).expect("serialize fixture");
    let object = value.as_object_mut().expect("fixture object");
    object.insert("schema_version".to_string(), json!(1));
    object.remove("compact_book_history");
    object.remove("final_delta");
    object.remove("expected_plan_hashes");

    let fixture: RewardDecisionReplayFixture =
        serde_json::from_value(value).expect("decode v1 fixture");
    assert_eq!(fixture.schema_version, 1);
    replay_reward_decision_engine(&fixture).expect("replay v1 fixture");
}

#[test]
fn reward_decision_replay_treats_missing_schema_version_as_v1() {
    let now = OffsetDateTime::from_unix_timestamp(1_750_000_000).expect("fixed timestamp");
    let mut value = serde_json::to_value(replay_test_fixture(now)).expect("serialize fixture");
    value
        .as_object_mut()
        .expect("fixture object")
        .remove("schema_version");
    let fixture: RewardDecisionReplayFixture =
        serde_json::from_value(value).expect("decode legacy fixture");
    assert_eq!(fixture.schema_version, 1);
}

#[test]
fn reward_decision_replay_v2_compacts_history_and_plan_expectations() {
    let now = OffsetDateTime::from_unix_timestamp(1_750_000_000).expect("fixed timestamp");
    let mut input = super::strategy_input_tests::strategy_test_snapshot(now, false);
    let deep_levels = (1..=100)
        .map(|index| RewardBookLevel {
            price: decimal("0.50"),
            size: Decimal::from(index),
        })
        .collect::<Vec<_>>();
    for history in input.book_history.values_mut() {
        *history = (0..120)
            .map(|index| BookSnapshot {
                bids: deep_levels.clone(),
                asks: deep_levels.clone(),
                observed_at: if index < 100 {
                    now - TimeDuration::hours(2) + TimeDuration::seconds(index)
                } else {
                    now - TimeDuration::seconds(120 - index)
                },
            })
            .collect();
    }
    let final_history = input
        .book_history
        .iter()
        .map(|(token_id, history)| (token_id.clone(), history.iter().cloned().collect()))
        .collect::<HashMap<String, VecDeque<BookSnapshot>>>();
    let expected = replay_reward_decision_engine(&RewardDecisionReplayFixture {
        schema_version: 1,
        input: input.clone(),
        providers: RewardReplayProviderSnapshot::default(),
        compact_book_history: HashMap::new(),
        final_state: None,
        final_delta: None,
        expected_plans: None,
        expected_plan_hashes: None,
    })
    .expect("baseline replay");
    let final_account = input.account.clone();
    let final_open_orders = input.open_orders.clone();
    let final_positions = input.positions.clone();
    let final_books = input.books.clone();
    let legacy_input = input.clone();
    let fixture = build_reward_decision_replay_fixture_v2(
        input,
        RewardReplayProviderSnapshot::default(),
        &final_account,
        &final_open_orders,
        &final_positions,
        &final_books,
        &final_history,
        &expected.plans,
    );
    let fixture = fixture.expect("build v2 fixture");
    assert_eq!(fixture.schema_version, REWARD_DECISION_REPLAY_V2_SCHEMA_VERSION);
    assert!(fixture.input.book_history.is_empty());
    assert!(fixture.expected_plans.is_none());
    assert!(fixture.expected_plan_hashes.is_some());
    for history in fixture.compact_book_history.values() {
        assert_eq!(history.len(), 20);
        assert!(history.iter().all(|point| point.observed_at >= now - TimeDuration::minutes(30)));
        assert!(history.iter().all(|point| point.best_bid.is_some()));
    }
    let replayed = replay_reward_decision_engine(&fixture).expect("replay v2 fixture");
    assert_eq!(
        replayed.comparison.as_ref().map(|comparison| comparison.matches),
        Some(true)
    );
    let json = serde_json::to_vec(&fixture).expect("serialize v2 fixture");
    let legacy_json = serde_json::to_vec(&RewardDecisionReplayFixture {
        schema_version: 1,
        input: legacy_input,
        providers: RewardReplayProviderSnapshot::default(),
        compact_book_history: HashMap::new(),
        final_state: Some(RewardReplayFinalState {
            account: Some(final_account),
            open_orders: Some(final_open_orders),
            positions: Some(final_positions),
            books: Some(final_books),
            book_history: Some(
                final_history
                    .iter()
                    .map(|(token_id, history)| {
                        (token_id.clone(), history.iter().cloned().collect())
                    })
                    .collect(),
            ),
        }),
        final_delta: None,
        expected_plans: Some(expected.plans),
        expected_plan_hashes: None,
    })
    .expect("serialize v1 fixture");
    assert!(json.len() < 500_000, "compact fixture was {} bytes", json.len());
    assert!(
        json.len() * 10 < legacy_json.len(),
        "v2={} bytes v1={} bytes",
        json.len(),
        legacy_json.len()
    );
}

#[test]
fn reward_decision_replay_v3_is_the_current_capture_schema() {
    let now = OffsetDateTime::from_unix_timestamp(1_750_000_000).expect("fixed timestamp");
    let input = super::strategy_input_tests::strategy_test_snapshot(now, false);
    let final_history = input
        .book_history
        .iter()
        .map(|(token_id, history)| (token_id.clone(), history.iter().cloned().collect()))
        .collect::<HashMap<String, VecDeque<BookSnapshot>>>();
    let expected = replay_reward_decision_engine(&RewardDecisionReplayFixture {
        schema_version: REWARD_DECISION_REPLAY_V2_SCHEMA_VERSION,
        input: input.clone(),
        providers: RewardReplayProviderSnapshot::default(),
        compact_book_history: HashMap::new(),
        final_state: None,
        final_delta: None,
        expected_plans: None,
        expected_plan_hashes: None,
    })
    .expect("baseline replay");
    let fixture = build_reward_decision_replay_fixture_v3(
        input.clone(),
        RewardReplayProviderSnapshot::default(),
        &input.account,
        &input.open_orders,
        &input.positions,
        &input.books,
        &final_history,
        &expected.plans,
    )
    .expect("build v3 fixture");

    assert_eq!(fixture.schema_version, REWARD_DECISION_REPLAY_SCHEMA_VERSION);
    let replayed = replay_reward_decision_engine(&fixture).expect("replay v3 fixture");
    assert_eq!(
        replayed.comparison.as_ref().map(|comparison| comparison.matches),
        Some(true)
    );
}
