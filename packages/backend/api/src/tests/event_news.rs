#[tokio::test]
async fn events_route_filters_by_status() {
    let signing_key = SigningKey::from_bytes(&[13_u8; 32]);
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
        .ingest_fixture_bundle(demo_fixture_bundle(), "trc_seeded_events")
        .await
        .expect("seed fixtures");
    let app = build_app(state);
    let request_id = format!("req_{}", Uuid::now_v7());
    let token = issue_token(&signing_key, "test-key", &request_id);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/events?status=active")
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
    let payload: ApiResponse<Paginated<EventData>> =
        serde_json::from_slice(&body).expect("deserialize response");
    assert_eq!(payload.data.data.len(), 1);
    assert_eq!(payload.data.data[0].id, "evt_9001");
}

#[tokio::test]
async fn news_source_health_route_filters_by_source_type() {
    let signing_key = SigningKey::from_bytes(&[17_u8; 32]);
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
    let reliability = Probability::new(Decimal::new(95, 2)).expect("probability");
    state
        .news_ingestion_service
        .ingest_source_items(NewsIngestSourceCommand {
            source: "sec_feed".to_string(),
            source_type: "official".to_string(),
            reliability,
            items: vec![NewsIngestionItem {
                source: "sec_feed".to_string(),
                source_type: "official".to_string(),
                external_id: Some("entry-1".to_string()),
                title: "SEC publishes ETF calendar update".to_string(),
                url: Some("https://example.com/sec-calendar".to_string()),
                author: None,
                published_at: None,
                content_snippet: Some("Window narrowed".to_string()),
                raw_payload: serde_json::json!({"id": "entry-1"}),
            }],
            trace_id: "trc_seed_news_health".to_string(),
        })
        .await
        .expect("seed official source health");
    state
        .news_ingestion_service
        .ingest_source_items(NewsIngestSourceCommand {
            source: "wire_feed".to_string(),
            source_type: "news".to_string(),
            reliability,
            items: vec![NewsIngestionItem {
                source: "wire_feed".to_string(),
                source_type: "news".to_string(),
                external_id: Some("wire-1".to_string()),
                title: "Wire reports crypto policy hearing".to_string(),
                url: Some("https://example.com/wire-policy".to_string()),
                author: None,
                published_at: None,
                content_snippet: Some("Hearing scheduled".to_string()),
                raw_payload: serde_json::json!({"id": "wire-1"}),
            }],
            trace_id: "trc_seed_wire_health".to_string(),
        })
        .await
        .expect("seed news source health");

    let app = build_app(state);
    let request_id = format!("req_{}", Uuid::now_v7());
    let token = issue_token(&signing_key, "test-key", &request_id);

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/news/source-health?source_type=official")
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
    let payload: ApiResponse<Paginated<NewsSourceHealthData>> =
        serde_json::from_slice(&body).expect("deserialize response");
    assert_eq!(payload.data.data.len(), 1);
    assert_eq!(payload.data.data[0].source, "sec_feed");
    assert_eq!(payload.data.data[0].source_type, "official");
    assert_eq!(payload.data.data[0].items_fetched, 1);
    assert_eq!(payload.data.data[0].items_inserted, 1);
    assert_eq!(payload.data.data[0].consecutive_failures, 0);

    let raw_request_id = format!("req_{}", Uuid::now_v7());
    let raw_token = issue_token(&signing_key, "test-key", &raw_request_id);
    let raw_response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/news/raw-events?source_type=official")
                .header("Authorization", format!("Bearer {raw_token}"))
                .header("X-Request-Id", &raw_request_id)
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");

    assert_eq!(raw_response.status(), StatusCode::OK);
    let raw_body = to_bytes(raw_response.into_body(), usize::MAX)
        .await
        .expect("read raw body");
    let raw_payload: ApiResponse<Paginated<NewsRawEventData>> =
        serde_json::from_slice(&raw_body).expect("deserialize raw response");
    assert_eq!(raw_payload.data.data.len(), 1);
    assert_eq!(raw_payload.data.data[0].source, "sec_feed");
    assert_eq!(
        raw_payload.data.data[0].title,
        "SEC publishes ETF calendar update"
    );
    assert_eq!(raw_payload.data.data[0].external_id.as_deref(), Some("entry-1"));
}

#[tokio::test]
async fn evidences_route_filters_by_market() {
    let signing_key = SigningKey::from_bytes(&[14_u8; 32]);
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
        .ingest_fixture_bundle(demo_fixture_bundle(), "trc_seeded_evidences")
        .await
        .expect("seed fixtures");
    let app = build_app(state);
    let request_id = format!("req_{}", Uuid::now_v7());
    let token = issue_token(&signing_key, "test-key", &request_id);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/evidences?market_id=mkt_121")
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
    let payload: ApiResponse<Paginated<EvidenceData>> =
        serde_json::from_slice(&body).expect("deserialize response");
    assert_eq!(payload.data.data.len(), 2);
    assert!(payload.data.data.iter().all(|item| item.market_id == "mkt_121"));
}
