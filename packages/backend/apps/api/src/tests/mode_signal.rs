#[tokio::test]
async fn trigger_kill_switch_requires_specific_scope() {
    let signing_key = SigningKey::from_bytes(&[21_u8; 32]);
    let settings = Settings::for_test(
        SystemMode::ManualConfirm,
        "test",
        vec![AuthKeySettings {
            kid: "test-key".to_string(),
            public_key_base64: general_purpose::STANDARD
                .encode(signing_key.verifying_key().as_bytes()),
        }],
    );
    let app = build_app(Runtime::test_app_state(settings).expect("state"));
    let request_id = format!("req_{}", Uuid::now_v7());
    let token = issue_token_with(
        &signing_key,
        "test-key",
        &request_id,
        vec![UserRole::RiskAdmin],
        vec![StepUpScope::SystemModeSwitch],
    );
    let body = serde_json::to_vec(&TriggerKillSwitchRequest {
        reason: "operator initiated stop".to_string(),
        expected_version: Some(1),
    })
    .expect("serialize body");

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/system/kill-switch/trigger")
                .header("Authorization", format!("Bearer {token}"))
                .header("X-Request-Id", &request_id)
                .header("Idempotency-Key", "idem-kill-trigger-scope")
                .header("Content-Type", "application/json")
                .body(Body::from(body))
                .expect("request"),
        )
        .await
        .expect("response");

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn kill_switch_trigger_and_release_are_idempotent() {
    let signing_key = SigningKey::from_bytes(&[22_u8; 32]);
    let settings = Settings::for_test(
        SystemMode::LiveAuto,
        "test",
        vec![AuthKeySettings {
            kid: "test-key".to_string(),
            public_key_base64: general_purpose::STANDARD
                .encode(signing_key.verifying_key().as_bytes()),
        }],
    );
    let app = build_app(Runtime::test_app_state(settings).expect("state"));

    let trigger_request_id = format!("req_{}", Uuid::now_v7());
    let trigger_token = issue_token_with(
        &signing_key,
        "test-key",
        &trigger_request_id,
        vec![UserRole::RiskAdmin],
        vec![StepUpScope::SystemKillSwitchTrigger],
    );
    let trigger_body = serde_json::to_vec(&TriggerKillSwitchRequest {
        reason: "operator initiated stop".to_string(),
        expected_version: Some(1),
    })
    .expect("serialize body");

    let trigger_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/system/kill-switch/trigger")
                .header("Authorization", format!("Bearer {trigger_token}"))
                .header("X-Request-Id", &trigger_request_id)
                .header("Idempotency-Key", "idem-kill-trigger-1")
                .header("Content-Type", "application/json")
                .body(Body::from(trigger_body.clone()))
                .expect("request"),
        )
        .await
        .expect("response");

    assert_eq!(trigger_response.status(), StatusCode::OK);
    let trigger_response_body = to_bytes(trigger_response.into_body(), usize::MAX)
        .await
        .expect("read body");
    let trigger_payload: ApiResponse<KillSwitchData> =
        serde_json::from_slice(&trigger_response_body).expect("deserialize response");
    assert_eq!(
        trigger_payload.data.risk_state.mode,
        SystemMode::KillSwitchLocked
    );
    assert!(trigger_payload.data.risk_state.kill_switch);
    assert_eq!(trigger_payload.data.risk_state.version, 2);
    assert!(!trigger_payload.data.replayed);

    let trigger_replay = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/system/kill-switch/trigger")
                .header("Authorization", format!("Bearer {trigger_token}"))
                .header("X-Request-Id", &trigger_request_id)
                .header("Idempotency-Key", "idem-kill-trigger-1")
                .header("Content-Type", "application/json")
                .body(Body::from(trigger_body))
                .expect("request"),
        )
        .await
        .expect("response");

    assert_eq!(trigger_replay.status(), StatusCode::OK);
    let trigger_replay_body = to_bytes(trigger_replay.into_body(), usize::MAX)
        .await
        .expect("read body");
    let trigger_replay_payload: ApiResponse<KillSwitchData> =
        serde_json::from_slice(&trigger_replay_body).expect("deserialize response");
    assert!(trigger_replay_payload.data.replayed);

    let release_request_id = format!("req_{}", Uuid::now_v7());
    let release_token = issue_token_with(
        &signing_key,
        "test-key",
        &release_request_id,
        vec![UserRole::RiskAdmin],
        vec![StepUpScope::SystemKillSwitchRelease],
    );
    let release_body = serde_json::to_vec(&ReleaseKillSwitchRequest {
        reason: "resume controlled paper trading".to_string(),
        to_mode: SystemMode::PaperTrade,
        expected_version: Some(2),
    })
    .expect("serialize body");

    let release_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/system/kill-switch/release")
                .header("Authorization", format!("Bearer {release_token}"))
                .header("X-Request-Id", &release_request_id)
                .header("Idempotency-Key", "idem-kill-release-1")
                .header("Content-Type", "application/json")
                .body(Body::from(release_body.clone()))
                .expect("request"),
        )
        .await
        .expect("response");

    assert_eq!(release_response.status(), StatusCode::OK);
    let release_response_body = to_bytes(release_response.into_body(), usize::MAX)
        .await
        .expect("read body");
    let release_payload: ApiResponse<KillSwitchData> =
        serde_json::from_slice(&release_response_body).expect("deserialize response");
    assert_eq!(release_payload.data.risk_state.mode, SystemMode::PaperTrade);
    assert!(!release_payload.data.risk_state.kill_switch);
    assert_eq!(release_payload.data.risk_state.version, 3);
    assert!(!release_payload.data.replayed);

    let release_replay = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/system/kill-switch/release")
                .header("Authorization", format!("Bearer {release_token}"))
                .header("X-Request-Id", &release_request_id)
                .header("Idempotency-Key", "idem-kill-release-1")
                .header("Content-Type", "application/json")
                .body(Body::from(release_body))
                .expect("request"),
        )
        .await
        .expect("response");

    assert_eq!(release_replay.status(), StatusCode::OK);
    let release_replay_body = to_bytes(release_replay.into_body(), usize::MAX)
        .await
        .expect("read body");
    let release_replay_payload: ApiResponse<KillSwitchData> =
        serde_json::from_slice(&release_replay_body).expect("deserialize response");
    assert!(release_replay_payload.data.replayed);
    assert_eq!(release_replay_payload.data.risk_state.version, 3);
}

#[tokio::test]
async fn recompute_signal_route_is_idempotent_and_creates_estimate() {
    let signing_key = SigningKey::from_bytes(&[16_u8; 32]);
    let settings = Settings::for_test(
        SystemMode::ManualConfirm,
        "test",
        vec![AuthKeySettings {
            kid: "test-key".to_string(),
            public_key_base64: general_purpose::STANDARD
                .encode(signing_key.verifying_key().as_bytes()),
        }],
    );
    let state = Runtime::test_app_state(settings).expect("state");
    state
        .market_event_service
        .ingest_fixture_bundle(demo_fixture_bundle(), "trc_seeded_recompute")
        .await
        .expect("seed fixtures");
    let app = build_app(state);
    let request_id = format!("req_{}", Uuid::now_v7());
    let token = issue_token(&signing_key, "test-key", &request_id);
    let body = serde_json::to_vec(&RecomputeSignalRequest {
        reason: "manual pricing refresh after official update".to_string(),
    })
    .expect("serialize body");

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/signals/sig_2412/recompute")
                .header("Authorization", format!("Bearer {token}"))
                .header("X-Request-Id", &request_id)
                .header("Idempotency-Key", "idem-signal-1")
                .header("Content-Type", "application/json")
                .body(Body::from(body.clone()))
                .expect("request"),
        )
        .await
        .expect("response");

    assert_eq!(response.status(), StatusCode::OK);
    let first_body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("read body");
    let first_payload: ApiResponse<RecomputeSignalData> =
        serde_json::from_slice(&first_body).expect("deserialize response");
    assert_eq!(first_payload.data.signal.id, "sig_2412");
    assert_eq!(
        first_payload.data.signal.side,
        polyedge_domain::SignalSide::No
    );
    assert_eq!(
        first_payload.data.signal.lifecycle_state,
        polyedge_domain::SignalLifecycleState::New
    );
    assert!(first_payload.data.transition.is_none());
    assert!(!first_payload.data.replayed);

    let replay = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/signals/sig_2412/recompute")
                .header("Authorization", format!("Bearer {token}"))
                .header("X-Request-Id", &request_id)
                .header("Idempotency-Key", "idem-signal-1")
                .header("Content-Type", "application/json")
                .body(Body::from(body))
                .expect("request"),
        )
        .await
        .expect("response");

    assert_eq!(replay.status(), StatusCode::OK);
    let replay_body = to_bytes(replay.into_body(), usize::MAX)
        .await
        .expect("read body");
    let replay_payload: ApiResponse<RecomputeSignalData> =
        serde_json::from_slice(&replay_body).expect("deserialize response");
    assert!(replay_payload.data.replayed);
    assert_eq!(
        replay_payload.data.estimate.id,
        first_payload.data.estimate.id
    );

    let estimates_request_id = format!("req_{}", Uuid::now_v7());
    let estimates_token = issue_token(&signing_key, "test-key", &estimates_request_id);
    let estimates_response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/pricing/estimates?signal_id=sig_2412")
                .header("Authorization", format!("Bearer {estimates_token}"))
                .header("X-Request-Id", &estimates_request_id)
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");

    assert_eq!(estimates_response.status(), StatusCode::OK);
    let estimates_body = to_bytes(estimates_response.into_body(), usize::MAX)
        .await
        .expect("read body");
    let estimates_payload: ApiResponse<Vec<ProbabilityEstimateData>> =
        serde_json::from_slice(&estimates_body).expect("deserialize response");
    assert_eq!(estimates_payload.data.len(), 1);
    assert_eq!(
        estimates_payload.data[0].signal_id.as_deref(),
        Some("sig_2412")
    );
}

#[tokio::test]
async fn signal_transitions_route_returns_recompute_transition() {
    let signing_key = SigningKey::from_bytes(&[17_u8; 32]);
    let settings = Settings::for_test(
        SystemMode::ManualConfirm,
        "test",
        vec![AuthKeySettings {
            kid: "test-key".to_string(),
            public_key_base64: general_purpose::STANDARD
                .encode(signing_key.verifying_key().as_bytes()),
        }],
    );
    let state = Runtime::test_app_state(settings).expect("state");
    state
        .market_event_service
        .ingest_fixture_bundle(demo_fixture_bundle(), "trc_seeded_transitions")
        .await
        .expect("seed fixtures");
    let app = build_app(state);
    let request_id = format!("req_{}", Uuid::now_v7());
    let token = issue_token(&signing_key, "test-key", &request_id);
    let body = serde_json::to_vec(&RecomputeSignalRequest {
        reason: "refresh transition history".to_string(),
    })
    .expect("serialize body");

    let recompute = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/signals/sig_2411/recompute")
                .header("Authorization", format!("Bearer {token}"))
                .header("X-Request-Id", &request_id)
                .header("Idempotency-Key", "idem-signal-2")
                .header("Content-Type", "application/json")
                .body(Body::from(body))
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(recompute.status(), StatusCode::OK);

    let transitions_request_id = format!("req_{}", Uuid::now_v7());
    let transitions_token = issue_token(&signing_key, "test-key", &transitions_request_id);
    let transitions_response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/signals/sig_2411/transitions?limit=10")
                .header("Authorization", format!("Bearer {transitions_token}"))
                .header("X-Request-Id", &transitions_request_id)
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");

    assert_eq!(transitions_response.status(), StatusCode::OK);
    let transitions_body = to_bytes(transitions_response.into_body(), usize::MAX)
        .await
        .expect("read body");
    let payload: ApiResponse<Vec<SignalTransitionData>> =
        serde_json::from_slice(&transitions_body).expect("deserialize response");
    assert_eq!(payload.data.len(), 1);
    assert_eq!(payload.data[0].signal_id, "sig_2411");
    assert_eq!(
        payload.data[0].from_state,
        polyedge_domain::SignalLifecycleState::Active
    );
    assert_eq!(
        payload.data[0].to_state,
        polyedge_domain::SignalLifecycleState::Weakened
    );
}

#[tokio::test]
async fn mode_transition_is_idempotent() {
    let signing_key = SigningKey::from_bytes(&[11_u8; 32]);
    let settings = Settings::for_test(
        SystemMode::ManualConfirm,
        "test",
        vec![AuthKeySettings {
            kid: "test-key".to_string(),
            public_key_base64: general_purpose::STANDARD
                .encode(signing_key.verifying_key().as_bytes()),
        }],
    );
    let app = build_app(Runtime::test_app_state(settings).expect("state"));
    let request_id = "req_test_2";
    let token = issue_token(&signing_key, "test-key", request_id);
    let body = serde_json::to_vec(&TransitionSystemModeRequest {
        to_mode: SystemMode::Research,
        reason: "operator switched to research mode".to_string(),
    })
    .expect("serialize body");

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/system/mode")
                .header("Authorization", format!("Bearer {token}"))
                .header("X-Request-Id", request_id)
                .header("Idempotency-Key", "idem-1")
                .header("Content-Type", "application/json")
                .body(Body::from(body.clone()))
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::OK);

    let replay = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/system/mode")
                .header("Authorization", format!("Bearer {token}"))
                .header("X-Request-Id", request_id)
                .header("Idempotency-Key", "idem-1")
                .header("Content-Type", "application/json")
                .body(Body::from(body))
                .expect("request"),
        )
        .await
        .expect("response");

    assert_eq!(replay.status(), StatusCode::OK);

    let replay_body = to_bytes(replay.into_body(), usize::MAX)
        .await
        .expect("read body");
    let payload: ApiResponse<SystemModeData> =
        serde_json::from_slice(&replay_body).expect("deserialize response");
    assert!(payload.data.replayed);
}
