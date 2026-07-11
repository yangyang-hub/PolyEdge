use super::*;

fn replay_test_fixture(now: OffsetDateTime) -> RewardDecisionReplayFixture {
    RewardDecisionReplayFixture {
        schema_version: REWARD_DECISION_REPLAY_SCHEMA_VERSION,
        input: super::strategy_input_tests::strategy_test_snapshot(now, false),
        providers: RewardReplayProviderSnapshot::default(),
        final_state: None,
        expected_plans: None,
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
