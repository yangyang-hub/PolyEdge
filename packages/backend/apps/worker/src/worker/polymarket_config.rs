async fn build_live_polymarket_connector(state: &AppState) -> Result<LivePolymarketConnector> {
    let settings = &state.settings.polymarket;
    let config = LivePolymarketConfig {
        account_id: polymarket_account_id(state).to_string(),
        clob_host: settings.clob_host.clone(),
        ws_host: settings.ws_host.clone(),
        chain_id: settings.chain_id,
        signature_type: polymarket_signature_scheme(settings.signature_type),
        funder: normalize_optional_config_string(settings.funder.as_deref()),
        private_key: normalize_optional_config_string(settings.private_key.as_deref())
            .unwrap_or_default(),
        api_key: normalize_optional_config_string(settings.api_key.as_deref()),
        api_secret: normalize_optional_config_string(settings.api_secret.as_deref()),
        api_passphrase: normalize_optional_config_string(settings.api_passphrase.as_deref()),
    };

    LivePolymarketConnector::connect(&config).await
}

fn normalize_optional_config_string(value: Option<&str>) -> Option<String> {
    value.and_then(|value| {
        let normalized = value.trim();
        if normalized.is_empty() {
            None
        } else {
            Some(normalized.to_string())
        }
    })
}
