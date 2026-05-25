#[tokio::test]
async fn arbitrage_routes_return_recorded_opportunities() {
    let signing_key = SigningKey::from_bytes(&[42_u8; 32]);
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
    let started_at = OffsetDateTime::now_utc();
    let scan_id = "scan_api_test_arbitrage";
    state
        .arbitrage_service
        .start_scan(ArbitrageScanView {
            id: scan_id.to_string(),
            started_at,
            finished_at: None,
            market_count: 0,
            snapshot_count: 0,
            opportunity_count: 0,
            scanner_version: "api-test".to_string(),
            metadata: json!({ "mode": "test" }),
            trace_id: "trc_api_test_arbitrage".to_string(),
        })
        .await
        .expect("start scan");
    let observed_at = started_at + time::Duration::seconds(1);
    let snapshot = MarketBookSnapshotView {
        id: "book_api_test_mkt_120".to_string(),
        scan_id: scan_id.to_string(),
        connector_name: "polymarket".to_string(),
        market_id: "mkt_120".to_string(),
        yes_asset_id: Some("asset_yes".to_string()),
        no_asset_id: Some("asset_no".to_string()),
        yes_bid: None,
        yes_ask: Some(Probability::new(Decimal::new(45, 2)).expect("yes ask")),
        yes_bid_size: Quantity::new(Decimal::ZERO).expect("zero yes bid size"),
        yes_ask_size: Quantity::new(Decimal::new(900, 0)).expect("yes ask size"),
        no_bid: None,
        no_ask: Some(Probability::new(Decimal::new(51, 2)).expect("no ask")),
        no_bid_size: Quantity::new(Decimal::ZERO).expect("zero no bid size"),
        no_ask_size: Quantity::new(Decimal::new(850, 0)).expect("no ask size"),
        observed_at,
        raw_payload: json!({ "fixture": true }),
        trace_id: "trc_api_test_arbitrage".to_string(),
    };
    let opportunities = state
        .arbitrage_service
        .record_snapshot_and_detect(snapshot.clone())
        .await
        .expect("record snapshot and detect");
    state
        .arbitrage_service
        .validate_opportunity(
            &opportunities[0],
            &snapshot,
            &ArbitrageValidationConfig {
                max_book_age_ms: 10_000,
                min_gross_edge: Edge::new(Decimal::new(5, 3)).expect("min edge"),
                min_capacity: Quantity::new(Decimal::ONE).expect("min capacity"),
                fee_buffer: Edge::new(Decimal::new(5, 3)).expect("fee buffer"),
                slippage_buffer: Edge::new(Decimal::new(5, 3)).expect("slippage buffer"),
            },
            observed_at + time::Duration::milliseconds(50),
        )
        .await
        .expect("validate opportunity");
    let unvalidated_snapshot = MarketBookSnapshotView {
        id: "book_api_test_mkt_121".to_string(),
        scan_id: scan_id.to_string(),
        connector_name: "polymarket".to_string(),
        market_id: "mkt_121".to_string(),
        yes_asset_id: Some("asset_yes_121".to_string()),
        no_asset_id: Some("asset_no_121".to_string()),
        yes_bid: None,
        yes_ask: Some(Probability::new(Decimal::new(46, 2)).expect("yes ask")),
        yes_bid_size: Quantity::new(Decimal::ZERO).expect("zero yes bid size"),
        yes_ask_size: Quantity::new(Decimal::new(500, 0)).expect("yes ask size"),
        no_bid: None,
        no_ask: Some(Probability::new(Decimal::new(50, 2)).expect("no ask")),
        no_bid_size: Quantity::new(Decimal::ZERO).expect("zero no bid size"),
        no_ask_size: Quantity::new(Decimal::new(450, 0)).expect("no ask size"),
        observed_at: observed_at + time::Duration::milliseconds(100),
        raw_payload: json!({ "fixture": "unvalidated" }),
        trace_id: "trc_api_test_arbitrage".to_string(),
    };
    let unvalidated_opportunities = state
        .arbitrage_service
        .record_snapshot_and_detect(unvalidated_snapshot)
        .await
        .expect("record unvalidated snapshot and detect");
    assert_eq!(unvalidated_opportunities.len(), 1);
    state
        .arbitrage_service
        .complete_scan(scan_id, started_at + time::Duration::seconds(2), 2, 2, 2)
        .await
        .expect("complete scan");
    let summary =
        build_arbitrage_analysis(&opportunities, 24, started_at + time::Duration::seconds(3));
    state
        .arbitrage_service
        .record_analysis_run(ArbitrageAnalysisRunView {
            id: "arb_analysis_api_test".to_string(),
            generated_at: summary.generated_at,
            lookback_hours: summary.lookback_hours,
            opportunity_count: summary.opportunity_count,
            market_count: summary.market_count,
            summary_payload: serde_json::to_value(&summary).expect("serialize summary"),
            trace_id: "trc_api_test_arbitrage_analysis".to_string(),
        })
        .await
        .expect("record analysis");

    let mut emitted_ids = HashSet::new();
    let mut emitted_id_order = VecDeque::new();
    let mut last_arbitrage_sequence = None;
    let stream_chunk = build_stream_chunk(
        &state,
        "arbitrage",
        0,
        &mut emitted_ids,
        &mut emitted_id_order,
        &mut last_arbitrage_sequence,
    )
    .await
    .expect("build arbitrage stream");
    assert!(stream_chunk.contains("event: arbitrage.scan.started"));
    assert!(stream_chunk.contains("event: arbitrage.validation.passed"));
    assert!(last_arbitrage_sequence.is_some());

    let mut resumed_ids = HashSet::new();
    let mut resumed_id_order = VecDeque::new();
    let mut resumed_sequence = Some(1);
    let resumed_stream_chunk = build_stream_chunk(
        &state,
        "arbitrage",
        0,
        &mut resumed_ids,
        &mut resumed_id_order,
        &mut resumed_sequence,
    )
    .await
    .expect("build resumed arbitrage stream");
    assert!(!resumed_stream_chunk.contains("event: arbitrage.scan.started"));
    assert!(resumed_stream_chunk.contains("event: arbitrage.opportunity.observed"));

    let second_stream_chunk = build_stream_chunk(
        &state,
        "arbitrage",
        1,
        &mut emitted_ids,
        &mut emitted_id_order,
        &mut last_arbitrage_sequence,
    )
    .await
    .expect("build arbitrage stream heartbeat");
    assert!(second_stream_chunk.contains("polyedge arbitrage stream heartbeat"));

    let app = build_app(state);
    let request_id = format!("req_{}", Uuid::now_v7());
    let token = issue_token(&signing_key, "test-key", &request_id);

    let opportunities_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/arbitrage/opportunities?market_id=mkt_120&opportunity_type=binary_buy_both&validation_status=valid&active_only=true&min_net_edge=0.01")
                .header("Authorization", format!("Bearer {token}"))
                .header("X-Request-Id", &request_id)
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("opportunities response");

    assert_eq!(opportunities_response.status(), StatusCode::OK);
    let opportunities_body = to_bytes(opportunities_response.into_body(), usize::MAX)
        .await
        .expect("read opportunities body");
    let opportunities_payload: ApiResponse<Vec<ArbitrageOpportunityData>> =
        serde_json::from_slice(&opportunities_body).expect("deserialize opportunities");
    assert_eq!(opportunities_payload.data.len(), 1);
    assert_eq!(
        opportunities_payload.data[0].opportunity_type,
        "binary_buy_both"
    );
    assert_eq!(opportunities_payload.data[0].status, "observed");
    assert_eq!(opportunities_payload.data[0].price_sum, "0.96");
    assert_eq!(
        opportunities_payload.data[0].reason_codes,
        vec!["yes_ask_plus_no_ask_below_one"]
    );
    let validation = opportunities_payload.data[0]
        .validation
        .as_ref()
        .expect("validation");
    assert_eq!(validation.status, "valid");
    assert_eq!(validation.book_age_ms, 50);

    let high_edge_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/arbitrage/opportunities?market_id=mkt_120&validation_status=valid&active_only=true&min_net_edge=0.035")
                .header("Authorization", format!("Bearer {token}"))
                .header("X-Request-Id", &request_id)
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("high edge opportunities response");

    assert_eq!(high_edge_response.status(), StatusCode::OK);
    let high_edge_body = to_bytes(high_edge_response.into_body(), usize::MAX)
        .await
        .expect("read high edge opportunities body");
    let high_edge_payload: ApiResponse<Vec<ArbitrageOpportunityData>> =
        serde_json::from_slice(&high_edge_body).expect("deserialize high edge opportunities");
    assert!(high_edge_payload.data.is_empty());

    let unvalidated_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/arbitrage/opportunities?market_id=mkt_121&validation_status=unvalidated")
                .header("Authorization", format!("Bearer {token}"))
                .header("X-Request-Id", &request_id)
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("unvalidated opportunities response");

    assert_eq!(unvalidated_response.status(), StatusCode::OK);
    let unvalidated_body = to_bytes(unvalidated_response.into_body(), usize::MAX)
        .await
        .expect("read unvalidated opportunities body");
    let unvalidated_payload: ApiResponse<Vec<ArbitrageOpportunityData>> =
        serde_json::from_slice(&unvalidated_body)
            .expect("deserialize unvalidated opportunities");
    assert_eq!(unvalidated_payload.data.len(), 1);
    assert_eq!(unvalidated_payload.data[0].market_id, "mkt_121");
    assert!(unvalidated_payload.data[0].validation.is_none());

    let scans_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/arbitrage/scans?limit=1")
                .header("Authorization", format!("Bearer {token}"))
                .header("X-Request-Id", &request_id)
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("scans response");

    assert_eq!(scans_response.status(), StatusCode::OK);
    let scans_body = to_bytes(scans_response.into_body(), usize::MAX)
        .await
        .expect("read scans body");
    let scans_payload: ApiResponse<Vec<ArbitrageScanData>> =
        serde_json::from_slice(&scans_body).expect("deserialize scans");
    assert_eq!(scans_payload.data[0].id, scan_id);
    assert_eq!(scans_payload.data[0].opportunity_count, 2);

    let analysis_response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/arbitrage/analysis?limit=1")
                .header("Authorization", format!("Bearer {token}"))
                .header("X-Request-Id", &request_id)
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("analysis response");

    assert_eq!(analysis_response.status(), StatusCode::OK);
    let analysis_body = to_bytes(analysis_response.into_body(), usize::MAX)
        .await
        .expect("read analysis body");
    let analysis_payload: ApiResponse<Vec<ArbitrageAnalysisRunData>> =
        serde_json::from_slice(&analysis_body).expect("deserialize analysis");
    assert_eq!(analysis_payload.data[0].opportunity_count, 1);
    assert_eq!(analysis_payload.data[0].market_count, 1);
}
