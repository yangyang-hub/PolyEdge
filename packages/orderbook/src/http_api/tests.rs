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
}
