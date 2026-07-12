#[tokio::test]
async fn healthz_is_available_without_authentication() {
    let signing_key = SigningKey::from_bytes(&[11_u8; 32]);
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

    let response = app
        .oneshot(
            Request::builder()
                .uri("/healthz")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn protected_read_route_requires_valid_token() {
    let signing_key = SigningKey::from_bytes(&[11_u8; 32]);
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
    let request_id = "req_test_1";
    let token = issue_token(&signing_key, "test-key", request_id);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/system/mode")
                .header("Authorization", format!("Bearer {token}"))
                .header("X-Request-Id", request_id)
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn auth_disabled_allows_protected_read_route_without_headers() {
    let mut settings = Settings::for_test(SystemMode::LiveAuto, "intranet", Vec::new());
    settings.auth.disabled = true;
    let state = Runtime::test_app_state(settings).expect("state");
    let app = build_app(state.clone());

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/system/mode")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn markets_route_returns_seeded_market_list() {
    let signing_key = SigningKey::from_bytes(&[12_u8; 32]);
    let settings = Settings::for_test(
        SystemMode::LiveAuto,
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
        .ingest_fixture_bundle(demo_fixture_bundle(), "trc_seeded_markets")
        .await
        .expect("seed fixtures");
    let app = build_app(state);
    let request_id = format!("req_{}", Uuid::now_v7());
    let token = issue_token(&signing_key, "test-key", &request_id);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/markets?tradability_status=manual_review")
                .header("Authorization", format!("Bearer {token}"))
                .header("X-Request-Id", &request_id)
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("read body");
    let payload: ApiResponse<Vec<MarketData>> =
        serde_json::from_slice(&body).expect("deserialize response");
    assert_eq!(payload.data.len(), 1);
    assert_eq!(payload.data[0].id, "mkt_121");
}

#[tokio::test]
async fn rewards_control_idempotency_replays_the_original_full_response() {
    let mut settings = Settings::for_test(SystemMode::LiveAuto, "intranet", Vec::new());
    settings.auth.disabled = true;
    let state = Runtime::test_app_state(settings).expect("state");
    let app = build_app(state.clone());
    let idempotency_key = format!("reward-run-{}", Uuid::now_v7());

    let first = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/rewards-bot/run")
                .header("Content-Type", "application/json")
                .header("X-Request-Id", "req_reward_first")
                .header("Idempotency-Key", &idempotency_key)
                .body(Body::from(r#"{"operator_note":"ops ticket 42"}"#))
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(first.status(), StatusCode::OK);
    let first_body = to_bytes(first.into_body(), usize::MAX)
        .await
        .expect("read first body");
    let first_json: serde_json::Value =
        serde_json::from_slice(&first_body).expect("deserialize first response");

    let replay = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/rewards-bot/run")
                .header("Content-Type", "application/json")
                .header("X-Request-Id", "req_reward_replay")
                .header("Idempotency-Key", &idempotency_key)
                .body(Body::from(r#"{"operator_note":"ops ticket 42"}"#))
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(replay.status(), StatusCode::OK);
    let replay_body = to_bytes(replay.into_body(), usize::MAX)
        .await
        .expect("read replay body");
    let replay_json: serde_json::Value =
        serde_json::from_slice(&replay_body).expect("deserialize replay response");

    assert_eq!(replay_json, first_json);
    assert_eq!(
        replay_json["meta"]["request_id"],
        serde_json::Value::String("req_reward_first".to_string())
    );
    let command = state
        .reward_bot_service
        .claim_next_control_command("trc_operator_note_test")
        .await
        .expect("claim command")
        .expect("queued command");
    assert!(command.reason.contains("ops ticket 42"));
}

#[tokio::test]
async fn rewards_run_requires_its_step_up_scope() {
    let signing_key = SigningKey::from_bytes(&[23_u8; 32]);
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
    let request_id = "req_reward_missing_step_up";
    let token = issue_token_with(
        &signing_key,
        "test-key",
        request_id,
        vec![UserRole::Operator],
        Vec::new(),
    );

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/rewards-bot/run")
                .header("Authorization", format!("Bearer {token}"))
                .header("X-Request-Id", request_id)
                .header("Idempotency-Key", format!("reward-run-{}", Uuid::now_v7()))
                .header("Content-Type", "application/json")
                .body(Body::from("{}"))
                .expect("request"),
        )
        .await
        .expect("response");

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn rewards_config_requires_every_dangerous_enabled_scope() {
    let signing_key = SigningKey::from_bytes(&[24_u8; 32]);
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
    let request_id = "req_reward_config_step_up";
    let token = issue_token_with(
        &signing_key,
        "test-key",
        request_id,
        vec![UserRole::Operator],
        vec![StepUpScope::RewardsLiveTradingEnable],
    );

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/rewards-bot/config")
                .header("Authorization", format!("Bearer {token}"))
                .header("X-Request-Id", request_id)
                .header(
                    "Idempotency-Key",
                    format!("reward-config-{}", Uuid::now_v7()),
                )
                .header("Content-Type", "application/json")
                .body(Body::from(
                    r#"{"enabled":true,"balanced_merge_auto_execute_enabled":true}"#,
                ))
                .expect("request"),
        )
        .await
        .expect("response");

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn cors_only_echoes_configured_origins() {
    let mut settings = Settings::for_test(SystemMode::LiveAuto, "intranet", Vec::new());
    settings.auth.disabled = true;
    settings.cors.allowed_origins = vec!["https://console.example".to_string()];
    let app = build_app(Runtime::test_app_state(settings).expect("state"));

    let allowed = app
        .clone()
        .oneshot(
            Request::builder()
                .method("OPTIONS")
                .uri("/api/v1/rewards-bot/run")
                .header("Origin", "https://console.example")
                .header("Access-Control-Request-Method", "POST")
                .header("Access-Control-Request-Headers", "idempotency-key,content-type")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(
        allowed
            .headers()
            .get("access-control-allow-origin")
            .and_then(|value| value.to_str().ok()),
        Some("https://console.example")
    );

    let denied = app
        .oneshot(
            Request::builder()
                .method("OPTIONS")
                .uri("/api/v1/rewards-bot/run")
                .header("Origin", "https://evil.example")
                .header("Access-Control-Request-Method", "POST")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    assert!(denied.headers().get("access-control-allow-origin").is_none());
}
