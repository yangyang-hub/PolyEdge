fn worker_actor(request_id: &str) -> AuthenticatedActor {
    AuthenticatedActor {
        user_id: "system:worker".to_string(),
        session_id: "worker-runtime".to_string(),
        roles: vec![UserRole::Admin],
        request_id: request_id.to_string(),
        ip: None,
        user_agent: Some("polyedge-worker/0.1".to_string()),
    }
}

fn parse_limit_arg(raw: Option<String>) -> Result<Option<u16>> {
    raw.map(|value| {
        value.parse::<u16>().map_err(|error| {
            AppError::invalid_input(
                "WORKER_LIMIT_INVALID",
                format!("worker limit must be a valid u16: {error}"),
            )
        })
    })
    .transpose()
}

fn polymarket_account_id(state: &AppState) -> &str {
    let configured = state.settings.polymarket.account_id.trim();
    if configured.is_empty() {
        POLYMARKET_ACCOUNT_ID
    } else {
        configured
    }
}

fn polymarket_order_status_limit(state: &AppState, cli_limit: Option<u16>) -> Option<u16> {
    cli_limit.or(Some(state.settings.polymarket.order_status_poll_limit))
}

fn polymarket_fill_limit(state: &AppState, cli_limit: Option<u16>) -> Option<u16> {
    cli_limit.or(Some(state.settings.polymarket.fill_poll_limit))
}

fn polymarket_signature_scheme(
    signature_type: PolymarketSignatureType,
) -> PolymarketSignatureScheme {
    match signature_type {
        PolymarketSignatureType::Eoa => PolymarketSignatureScheme::Eoa,
        PolymarketSignatureType::Proxy => PolymarketSignatureScheme::Proxy,
        PolymarketSignatureType::GnosisSafe => PolymarketSignatureScheme::GnosisSafe,
    }
}

fn polymarket_market_refs(market: &MarketView) -> Result<PolymarketMarketRefs> {
    let condition_id = market.polymarket_condition_id.clone().ok_or_else(|| {
        AppError::invalid_input(
            "POLYMARKET_CONDITION_ID_MISSING",
            format!("market {} is missing polymarket_condition_id", market.id),
        )
    })?;
    let yes_asset_id = market.polymarket_yes_asset_id.clone().ok_or_else(|| {
        AppError::invalid_input(
            "POLYMARKET_YES_ASSET_ID_MISSING",
            format!("market {} is missing polymarket_yes_asset_id", market.id),
        )
    })?;
    let no_asset_id = market.polymarket_no_asset_id.clone().ok_or_else(|| {
        AppError::invalid_input(
            "POLYMARKET_NO_ASSET_ID_MISSING",
            format!("market {} is missing polymarket_no_asset_id", market.id),
        )
    })?;

    Ok(PolymarketMarketRefs {
        condition_id,
        yes_asset_id,
        no_asset_id,
    })
}

fn ensure_polymarket_enabled(state: &AppState) -> Result<()> {
    match state.settings.polymarket.mode {
        PolymarketConnectorMode::Mock | PolymarketConnectorMode::Live => Ok(()),
        PolymarketConnectorMode::Disabled => Err(AppError::invalid_input(
            "POLYMARKET_CONNECTOR_DISABLED",
            "polymarket connector is disabled in configuration",
        )),
    }
}
