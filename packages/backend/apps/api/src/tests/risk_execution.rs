#[tokio::test]
async fn risk_state_route_returns_current_snapshot() {
    let signing_key = SigningKey::from_bytes(&[18_u8; 32]);
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
    let token = issue_token(&signing_key, "test-key", &request_id);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/risk/state")
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
    let payload: ApiResponse<RiskStateData> =
        serde_json::from_slice(&body).expect("deserialize response");
    assert_eq!(payload.data.mode, SystemMode::PaperTrade);
    assert_eq!(payload.data.id, "risk_state_global");
    assert_eq!(payload.data.environment, "test");
    assert!(!payload.data.kill_switch);
    assert_eq!(payload.data.open_alerts, 0);
}

#[tokio::test]
async fn console_risk_routes_return_derived_resources() {
    let signing_key = SigningKey::from_bytes(&[28_u8; 32]);
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
        .ingest_fixture_bundle(demo_fixture_bundle(), "trc_seeded_console_risk")
        .await
        .expect("seed fixtures");
    let app = build_app(state);

    let alerts_request_id = format!("req_{}", Uuid::now_v7());
    let alerts_token = issue_token(&signing_key, "test-key", &alerts_request_id);
    let alerts_response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/risk/alerts")
                .header("Authorization", format!("Bearer {alerts_token}"))
                .header("X-Request-Id", &alerts_request_id)
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(alerts_response.status(), StatusCode::OK);
    let alerts_body = to_bytes(alerts_response.into_body(), usize::MAX)
        .await
        .expect("read alerts body");
    let alerts_payload: ApiResponse<Vec<RiskAlertData>> =
        serde_json::from_slice(&alerts_body).expect("deserialize alerts response");
    assert!(
        alerts_payload
            .data
            .iter()
            .all(|alert| alert.id != "alt_pending_signal_approvals")
    );
}

#[tokio::test]
async fn submit_execution_request_requires_execution_submit_scope() {
    let signing_key = SigningKey::from_bytes(&[25_u8; 32]);
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
        .ingest_fixture_bundle(demo_fixture_bundle(), "trc_seeded_execution_scope")
        .await
        .expect("seed fixtures");
    let app = build_app(state);
    let request_id = format!("req_{}", Uuid::now_v7());
    let token = issue_token(&signing_key, "test-key", &request_id);
    let body = serde_json::to_vec(&serde_json::json!({
        "limit_price": "0.48",
        "quantity": "25",
        "reason": "scope check",
        "expected_signal_version": 9,
        "connector_name": "paper_executor"
    }))
    .expect("serialize body");

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/signals/sig_2412/execution-requests")
                .header("Authorization", format!("Bearer {token}"))
                .header("X-Request-Id", &request_id)
                .header("Idempotency-Key", "idem-execution-scope")
                .header("Content-Type", "application/json")
                .body(Body::from(body))
                .expect("request"),
        )
        .await
        .expect("response");

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn submit_execution_request_is_idempotent_and_lists_created_records() {
    let signing_key = SigningKey::from_bytes(&[26_u8; 32]);
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
        .ingest_fixture_bundle(demo_fixture_bundle(), "trc_seeded_execution")
        .await
        .expect("seed fixtures");
    let app = build_app(state);

    let submit_request_id = format!("req_{}", Uuid::now_v7());
    let submit_token = issue_token_with(
        &signing_key,
        "test-key",
        &submit_request_id,
        vec![UserRole::RiskAdmin],
        vec![StepUpScope::ExecutionSubmit],
    );
    let submit_body = serde_json::to_vec(&serde_json::json!({
        "limit_price": "0.48",
        "quantity": "25",
        "reason": "queue manual execution request",
        "expected_signal_version": 9,
        "connector_name": "paper_executor"
    }))
    .expect("serialize body");

    let submit_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/signals/sig_2412/execution-requests")
                .header("Authorization", format!("Bearer {submit_token}"))
                .header("X-Request-Id", &submit_request_id)
                .header("Idempotency-Key", "idem-execution-submit")
                .header("Content-Type", "application/json")
                .body(Body::from(submit_body.clone()))
                .expect("request"),
        )
        .await
        .expect("response");

    assert_eq!(submit_response.status(), StatusCode::OK);
    let submit_response_body = to_bytes(submit_response.into_body(), usize::MAX)
        .await
        .expect("read body");
    let submit_payload: ApiResponse<SubmitExecutionData> =
        serde_json::from_slice(&submit_response_body).expect("deserialize response");
    assert_eq!(submit_payload.data.order_draft.signal_id, "sig_2412");
    assert_eq!(submit_payload.data.order_draft.status.as_str(), "queued");
    assert_eq!(
        submit_payload.data.execution_request.status.as_str(),
        "queued"
    );
    assert_eq!(
        submit_payload.data.execution_request.mode,
        SystemMode::PaperTrade
    );
    assert_eq!(
        submit_payload.data.execution_request.connector_name,
        "paper_executor"
    );
    assert!(!submit_payload.data.replayed);

    let submit_replay = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/signals/sig_2412/execution-requests")
                .header("Authorization", format!("Bearer {submit_token}"))
                .header("X-Request-Id", &submit_request_id)
                .header("Idempotency-Key", "idem-execution-submit")
                .header("Content-Type", "application/json")
                .body(Body::from(submit_body))
                .expect("request"),
        )
        .await
        .expect("response");

    assert_eq!(submit_replay.status(), StatusCode::OK);
    let submit_replay_body = to_bytes(submit_replay.into_body(), usize::MAX)
        .await
        .expect("read body");
    let submit_replay_payload: ApiResponse<SubmitExecutionData> =
        serde_json::from_slice(&submit_replay_body).expect("deserialize response");
    assert!(submit_replay_payload.data.replayed);

    let list_request_id = format!("req_{}", Uuid::now_v7());
    let list_token = issue_token(&signing_key, "test-key", &list_request_id);

    let order_drafts_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/orders/drafts?signal_id=sig_2412")
                .header("Authorization", format!("Bearer {list_token}"))
                .header("X-Request-Id", &list_request_id)
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(order_drafts_response.status(), StatusCode::OK);
    let order_drafts_body = to_bytes(order_drafts_response.into_body(), usize::MAX)
        .await
        .expect("read body");
    let order_drafts_payload: ApiResponse<Vec<OrderDraftData>> =
        serde_json::from_slice(&order_drafts_body).expect("deserialize response");
    assert_eq!(order_drafts_payload.data.len(), 1);
    assert_eq!(order_drafts_payload.data[0].signal_id, "sig_2412");

    let execution_requests_response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/execution/requests?signal_id=sig_2412")
                .header("Authorization", format!("Bearer {list_token}"))
                .header("X-Request-Id", &list_request_id)
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(execution_requests_response.status(), StatusCode::OK);
    let execution_requests_body = to_bytes(execution_requests_response.into_body(), usize::MAX)
        .await
        .expect("read body");
    let execution_requests_payload: ApiResponse<Vec<ExecutionRequestData>> =
        serde_json::from_slice(&execution_requests_body).expect("deserialize response");
    assert_eq!(execution_requests_payload.data.len(), 1);
    assert_eq!(execution_requests_payload.data[0].signal_id, "sig_2412");
}
