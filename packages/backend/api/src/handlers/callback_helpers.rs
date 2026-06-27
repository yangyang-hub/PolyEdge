fn authenticated_actor(auth: &AuthContext) -> AuthenticatedActor {
    AuthenticatedActor {
        user_id: auth.user_id.clone(),
        session_id: auth.session_id.clone(),
        roles: auth.roles.clone(),
        request_id: auth.request_id.clone(),
        ip: auth.ip.clone(),
        user_agent: auth.user_agent.clone(),
    }
}

fn normalize_callback_connector_name(connector_name: &str) -> polyedge_domain::Result<String> {
    let normalized = connector_name.trim().to_ascii_lowercase();

    if normalized.is_empty() {
        return Err(AppError::invalid_input(
            "CONNECTOR_NAME_REQUIRED",
            "connector_name must not be empty",
        ));
    }

    Ok(normalized)
}

fn validate_callback_event_id(event_id: &str) -> polyedge_domain::Result<String> {
    let normalized = event_id.trim().to_string();

    if normalized.is_empty() {
        return Err(AppError::invalid_input(
            "EXTERNAL_EVENT_ID_REQUIRED",
            "event_id must not be empty",
        ));
    }

    Ok(normalized)
}

fn callback_source(prefix: &str, connector_name: &str) -> String {
    format!("{prefix}.{connector_name}")
}

async fn build_trade_fill_callback_response(
    state: &AppState,
    connector_name: &str,
    external_order_id: &str,
    account_id: &str,
    external_trade_id: &str,
    replayed: bool,
) -> polyedge_domain::Result<ConnectorTradeFillCallbackData> {
    let order = state
        .market_event_service
        .get_order_by_external_ref(connector_name.to_string(), external_order_id.to_string())
        .await?;
    let trades = state
        .execution_service
        .list_trades(TradeListFilters::new(
            Some(order.id.clone()),
            Some(order.signal_id.clone()),
            Some(order.market_id.clone()),
            Some(order.connector_name.clone()),
            Some(100),
        )?)
        .await?;
    let trade = trades
        .into_iter()
        .find(|trade| trade.external_trade_id == external_trade_id)
        .ok_or_else(|| {
            AppError::not_found(
                "EXTERNAL_TRADE_NOT_FOUND",
                "external trade callback replay could not find a matching trade",
            )
        })?;
    let positions = state
        .execution_service
        .list_positions(PositionListFilters::new(
            Some(order.market_id.clone()),
            Some(order.connector_name.clone()),
            Some(order.side),
            Some(100),
        )?)
        .await?;
    let position = positions
        .into_iter()
        .find(|position| position.account_id == account_id)
        .ok_or_else(|| {
            AppError::not_found(
                "POSITION_NOT_FOUND",
                "external trade callback replay could not find a matching position",
            )
        })?;
    let risk_state = state.risk_service.read_state().await?;

    Ok(ConnectorTradeFillCallbackData {
        order: order_to_contract(order),
        trade: trade_to_contract(trade),
        position: position_to_contract(position),
        risk_state: risk_state_to_contract_for_state(state, risk_state)?,
        replayed,
    })
}
