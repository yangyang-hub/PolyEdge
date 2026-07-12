mod tests {
    fn test_state(write_token: Option<&str>) -> super::AppState {
        let mut settings = polyedge_infrastructure::Settings::for_test(
            polyedge_domain::SystemMode::LiveAuto,
            "test",
            Vec::new(),
        );
        settings.orderbook.write_token = write_token.map(ToString::to_string);
        polyedge_infrastructure::Runtime::test_app_state(settings).expect("test app state")
    }

    #[tokio::test]
    async fn orderbook_write_auth_is_disabled_without_configured_token() {
        let error = super::authorize_write(&test_state(None), &axum::http::HeaderMap::new())
            .expect_err("write auth must reject missing configuration");

        assert_eq!(error.0, axum::http::StatusCode::SERVICE_UNAVAILABLE);
    }

    #[tokio::test]
    async fn orderbook_write_auth_rejects_wrong_token() {
        let state = test_state(Some("secret"));
        let mut headers = axum::http::HeaderMap::new();
        headers.insert(
            "x-polyedge-orderbook-token",
            "wrong".parse().expect("header"),
        );

        let error = super::authorize_write(&state, &headers)
            .expect_err("write auth must reject wrong token");

        assert_eq!(error.0, axum::http::StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn orderbook_write_auth_accepts_matching_token() {
        let state = test_state(Some("secret"));
        let mut headers = axum::http::HeaderMap::new();
        headers.insert(
            "x-polyedge-orderbook-token",
            "secret".parse().expect("header"),
        );

        assert!(super::authorize_write(&state, &headers).is_ok());
    }

    #[test]
    fn ingest_levels_reject_invalid_values_duplicates_and_crossing() {
        let invalid_price = super::parse_levels(
            vec![super::LevelResponse {
                price: "1".to_string(),
                size: "2".to_string(),
            }],
            10,
            true,
        );
        assert!(invalid_price.is_err());

        let invalid_size = super::parse_levels(
            vec![super::LevelResponse {
                price: "0.5".to_string(),
                size: "0".to_string(),
            }],
            10,
            true,
        );
        assert!(invalid_size.is_err());

        let duplicate = super::parse_levels(
            vec![
                super::LevelResponse {
                    price: "0.5".to_string(),
                    size: "1".to_string(),
                },
                super::LevelResponse {
                    price: "0.5".to_string(),
                    size: "2".to_string(),
                },
            ],
            10,
            true,
        );
        assert!(duplicate.is_err());
    }

    #[test]
    fn ingest_timestamp_requires_recent_service_time() {
        let now = 1_800_000_000_000_i64;
        assert!(super::validate_ingest_observed_at(now, now).is_ok());
        assert!(super::validate_ingest_observed_at(now + 30_001, now).is_err());
        assert!(super::validate_ingest_observed_at(now - 86_400_001, now).is_err());
    }
}
