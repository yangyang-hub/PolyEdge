#[tokio::test]
async fn connector_order_status_callback_is_deduplicated_without_idempotency_key() {
    let signing_key = SigningKey::from_bytes(&[27_u8; 32]);
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
        .ingest_fixture_bundle(demo_fixture_bundle(), "trc_seeded_connector_callbacks")
        .await
        .expect("seed fixtures");
    let app = build_app(state.clone());
    let submission =
        submit_execution_for_test(app.clone(), &signing_key, "sig_2412", "paper_executor")
            .await;
    let external_order_id = "paper_ord_callback_001";

    dispatch_execution(
        &state,
        &submission.execution_request.id,
        "acct_paper_main",
        external_order_id,
    )
    .await;

    let callback_request_id = format!("req_{}", Uuid::now_v7());
    let callback_token = issue_token_with(
        &signing_key,
        "test-key",
        &callback_request_id,
        vec![UserRole::RiskAdmin],
        Vec::new(),
    );
    let callback_body = serde_json::to_vec(&serde_json::json!({
        "event_id": "evt_connector_order_open_1",
        "connector_name": "paper_executor",
        "external_order_id": external_order_id,
        "status": "open"
    }))
    .expect("serialize callback body");

    let first_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/connectors/callbacks/orders/status")
                .header("Authorization", format!("Bearer {callback_token}"))
                .header("X-Request-Id", &callback_request_id)
                .header("Content-Type", "application/json")
                .body(Body::from(callback_body.clone()))
                .expect("request"),
        )
        .await
        .expect("response");

    assert_eq!(first_response.status(), StatusCode::OK);
    let first_response_body = to_bytes(first_response.into_body(), usize::MAX)
        .await
        .expect("read body");
    let first_payload: ApiResponse<ConnectorOrderStatusCallbackData> =
        serde_json::from_slice(&first_response_body).expect("deserialize response");
    assert_eq!(
        first_payload.data.order.external_order_id,
        external_order_id
    );
    assert_eq!(first_payload.data.order.status.as_str(), "open");
    assert!(!first_payload.data.replayed);

    let replay_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/connectors/callbacks/orders/status")
                .header("Authorization", format!("Bearer {callback_token}"))
                .header("X-Request-Id", &callback_request_id)
                .header("Content-Type", "application/json")
                .body(Body::from(callback_body))
                .expect("request"),
        )
        .await
        .expect("response");

    assert_eq!(replay_response.status(), StatusCode::OK);
    let replay_response_body = to_bytes(replay_response.into_body(), usize::MAX)
        .await
        .expect("read body");
    let replay_payload: ApiResponse<ConnectorOrderStatusCallbackData> =
        serde_json::from_slice(&replay_response_body).expect("deserialize response");
    assert!(replay_payload.data.replayed);
    assert_eq!(
        replay_payload.data.order.external_order_id,
        external_order_id
    );
    assert_eq!(replay_payload.data.order.status.as_str(), "open");
}

#[tokio::test]
async fn connector_trade_fill_callback_is_deduplicated_without_duplicate_trades() {
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
        .ingest_fixture_bundle(demo_fixture_bundle(), "trc_seeded_connector_trade_callback")
        .await
        .expect("seed fixtures");
    let app = build_app(state.clone());
    let submission =
        submit_execution_for_test(app.clone(), &signing_key, "sig_2412", "paper_executor")
            .await;
    let external_order_id = "paper_ord_callback_002";

    dispatch_execution(
        &state,
        &submission.execution_request.id,
        "acct_paper_main",
        external_order_id,
    )
    .await;

    let open_request_id = format!("req_{}", Uuid::now_v7());
    let open_token = issue_token_with(
        &signing_key,
        "test-key",
        &open_request_id,
        vec![UserRole::RiskAdmin],
        Vec::new(),
    );
    let open_body = serde_json::to_vec(&serde_json::json!({
        "event_id": "evt_connector_order_open_2",
        "connector_name": "paper_executor",
        "external_order_id": external_order_id,
        "status": "open"
    }))
    .expect("serialize order open body");
    let open_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/connectors/callbacks/orders/status")
                .header("Authorization", format!("Bearer {open_token}"))
                .header("X-Request-Id", &open_request_id)
                .header("Content-Type", "application/json")
                .body(Body::from(open_body))
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(open_response.status(), StatusCode::OK);

    let trade_request_id = format!("req_{}", Uuid::now_v7());
    let trade_token = issue_token_with(
        &signing_key,
        "test-key",
        &trade_request_id,
        vec![UserRole::RiskAdmin],
        Vec::new(),
    );
    let trade_body = serde_json::to_vec(&serde_json::json!({
        "event_id": "evt_connector_trade_fill_1",
        "connector_name": "paper_executor",
        "external_order_id": external_order_id,
        "account_id": "acct_paper_main",
        "external_trade_id": "paper_trade_callback_001",
        "fill_price": "0.48",
        "filled_quantity": "1",
        "fee": "0.00"
    }))
    .expect("serialize trade body");

    let first_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/connectors/callbacks/trades/fill")
                .header("Authorization", format!("Bearer {trade_token}"))
                .header("X-Request-Id", &trade_request_id)
                .header("Content-Type", "application/json")
                .body(Body::from(trade_body.clone()))
                .expect("request"),
        )
        .await
        .expect("response");

    assert_eq!(first_response.status(), StatusCode::OK);
    let first_response_body = to_bytes(first_response.into_body(), usize::MAX)
        .await
        .expect("read body");
    let first_payload: ApiResponse<ConnectorTradeFillCallbackData> =
        serde_json::from_slice(&first_response_body).expect("deserialize response");
    assert_eq!(
        first_payload.data.trade.external_trade_id,
        "paper_trade_callback_001"
    );
    assert_eq!(
        first_payload.data.order.external_order_id,
        external_order_id
    );
    assert_eq!(first_payload.data.position.account_id, "acct_paper_main");
    assert!(!first_payload.data.replayed);

    let replay_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/connectors/callbacks/trades/fill")
                .header("Authorization", format!("Bearer {trade_token}"))
                .header("X-Request-Id", &trade_request_id)
                .header("Content-Type", "application/json")
                .body(Body::from(trade_body))
                .expect("request"),
        )
        .await
        .expect("response");

    assert_eq!(replay_response.status(), StatusCode::OK);
    let replay_response_body = to_bytes(replay_response.into_body(), usize::MAX)
        .await
        .expect("read body");
    let replay_payload: ApiResponse<ConnectorTradeFillCallbackData> =
        serde_json::from_slice(&replay_response_body).expect("deserialize response");
    assert!(replay_payload.data.replayed);
    assert_eq!(
        replay_payload.data.trade.external_trade_id,
        "paper_trade_callback_001"
    );

    let trades_request_id = format!("req_{}", Uuid::now_v7());
    let trades_token = issue_token(&signing_key, "test-key", &trades_request_id);
    let trades_response = app
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/v1/trades?order_id={}",
                    first_payload.data.order.id
                ))
                .header("Authorization", format!("Bearer {trades_token}"))
                .header("X-Request-Id", &trades_request_id)
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");

    assert_eq!(trades_response.status(), StatusCode::OK);
    let trades_response_body = to_bytes(trades_response.into_body(), usize::MAX)
        .await
        .expect("read body");
    let trades_payload: ApiResponse<Paginated<TradeData>> =
        serde_json::from_slice(&trades_response_body).expect("deserialize response");
    assert_eq!(trades_payload.data.data.len(), 1);
    assert_eq!(
        trades_payload.data.data[0].external_trade_id,
        "paper_trade_callback_001"
    );
}

#[tokio::test]
async fn polymarket_order_status_callback_normalizes_live_to_open() {
    let signing_key = SigningKey::from_bytes(&[29_u8; 32]);
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
        .ingest_fixture_bundle(demo_fixture_bundle(), "trc_seeded_polymarket_status")
        .await
        .expect("seed fixtures");
    let app = build_app(state.clone());
    let submission = submit_execution_for_test(
        app.clone(),
        &signing_key,
        "sig_2412",
        polyedge_connectors::POLYMARKET_CONNECTOR_NAME,
    )
    .await;
    let external_order_id = "pm_ord_callback_001";

    dispatch_execution(
        &state,
        &submission.execution_request.id,
        "acct_poly_main",
        external_order_id,
    )
    .await;

    let request_id = format!("req_{}", Uuid::now_v7());
    let token = issue_token_with(
        &signing_key,
        "test-key",
        &request_id,
        vec![UserRole::RiskAdmin],
        Vec::new(),
    );
    let body = serde_json::to_vec(&serde_json::json!({
        "event_id": "evt_pm_order_open_1",
        "order_id": external_order_id,
        "status": "live"
    }))
    .expect("serialize body");

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/connectors/polymarket/callbacks/orders/status")
                .header("Authorization", format!("Bearer {token}"))
                .header("X-Request-Id", &request_id)
                .header("Content-Type", "application/json")
                .body(Body::from(body))
                .expect("request"),
        )
        .await
        .expect("response");

    assert_eq!(response.status(), StatusCode::OK);
    let response_body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("read body");
    let payload: ApiResponse<ConnectorOrderStatusCallbackData> =
        serde_json::from_slice(&response_body).expect("deserialize response");
    assert_eq!(
        payload.data.order.connector_name,
        polyedge_connectors::POLYMARKET_CONNECTOR_NAME
    );
    assert_eq!(payload.data.order.status.as_str(), "open");
    assert!(!payload.data.replayed);
}

#[tokio::test]
async fn polymarket_trade_fill_callback_normalizes_trade_payload() {
    let signing_key = SigningKey::from_bytes(&[30_u8; 32]);
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
        .ingest_fixture_bundle(demo_fixture_bundle(), "trc_seeded_polymarket_fill")
        .await
        .expect("seed fixtures");
    let app = build_app(state.clone());
    let submission = submit_execution_for_test(
        app.clone(),
        &signing_key,
        "sig_2412",
        polyedge_connectors::POLYMARKET_CONNECTOR_NAME,
    )
    .await;
    let external_order_id = "pm_ord_callback_002";

    dispatch_execution(
        &state,
        &submission.execution_request.id,
        "acct_poly_main",
        external_order_id,
    )
    .await;

    let open_request_id = format!("req_{}", Uuid::now_v7());
    let open_token = issue_token_with(
        &signing_key,
        "test-key",
        &open_request_id,
        vec![UserRole::RiskAdmin],
        Vec::new(),
    );
    let open_body = serde_json::to_vec(&serde_json::json!({
        "event_id": "evt_pm_order_open_2",
        "order_id": external_order_id,
        "status": "live"
    }))
    .expect("serialize open body");
    let open_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/connectors/polymarket/callbacks/orders/status")
                .header("Authorization", format!("Bearer {open_token}"))
                .header("X-Request-Id", &open_request_id)
                .header("Content-Type", "application/json")
                .body(Body::from(open_body))
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(open_response.status(), StatusCode::OK);

    let trade_request_id = format!("req_{}", Uuid::now_v7());
    let trade_token = issue_token_with(
        &signing_key,
        "test-key",
        &trade_request_id,
        vec![UserRole::RiskAdmin],
        Vec::new(),
    );
    let trade_body = serde_json::to_vec(&serde_json::json!({
        "event_id": "evt_pm_trade_fill_1",
        "order_id": external_order_id,
        "account_id": "acct_poly_main",
        "trade_id": "pm_trade_callback_001",
        "price": "0.48",
        "size": "1",
        "fee": "0.00"
    }))
    .expect("serialize trade body");

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/connectors/polymarket/callbacks/trades/fill")
                .header("Authorization", format!("Bearer {trade_token}"))
                .header("X-Request-Id", &trade_request_id)
                .header("Content-Type", "application/json")
                .body(Body::from(trade_body))
                .expect("request"),
        )
        .await
        .expect("response");

    assert_eq!(response.status(), StatusCode::OK);
    let response_body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("read body");
    let payload: ApiResponse<ConnectorTradeFillCallbackData> =
        serde_json::from_slice(&response_body).expect("deserialize response");
    assert_eq!(
        payload.data.order.connector_name,
        polyedge_connectors::POLYMARKET_CONNECTOR_NAME
    );
    assert_eq!(
        payload.data.trade.external_trade_id,
        "pm_trade_callback_001"
    );
    assert_eq!(payload.data.position.account_id, "acct_poly_main");
    assert!(!payload.data.replayed);
}
