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
    let app = build_app(Runtime::test_app_state(settings).expect("state"));

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
