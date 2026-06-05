async fn consume_polymarket_user_events(
    state: &AppState,
    connector_name: Option<String>,
    max_events: Option<usize>,
) -> Result<PolymarketUserEventReport> {
    let connector_name = connector_name.unwrap_or_else(|| POLYMARKET_CONNECTOR_NAME.to_string());
    if connector_name != POLYMARKET_CONNECTOR_NAME {
        return Err(AppError::invalid_input(
            "WORKER_CONNECTOR_UNSUPPORTED",
            format!("worker does not support connector_name={connector_name}"),
        ));
    }

    let connector = build_live_polymarket_connector(state).await?;
    let subscribed_markets = collect_polymarket_user_event_markets(state, &connector_name).await?;
    let mut report = PolymarketUserEventReport {
        subscribed_markets: subscribed_markets.len(),
        ..PolymarketUserEventReport::default()
    };

    if subscribed_markets.is_empty() {
        info!(
            "skipping polymarket authenticated user websocket because there are no active internal markets to monitor"
        );
        return Ok(report);
    }

    let client = connector.connect_user_ws()?;
    let stream = client
        .subscribe_user_events(subscribed_markets)
        .map_err(|error| {
            AppError::internal(
                "POLYMARKET_USER_WS_SUBSCRIBE_FAILED",
                format!("failed to subscribe to Polymarket user websocket events: {error}"),
            )
        })?;
    let mut stream = Box::pin(stream);

    while let Some(message) = stream.next().await {
        let message = message.map_err(|error| {
            AppError::internal(
                "POLYMARKET_USER_WS_STREAM_FAILED",
                format!("failed to receive Polymarket user websocket event: {error}"),
            )
        })?;
        report.consumed += 1;

        match message {
            WsMessage::Order(order_message) => {
                match apply_polymarket_ws_order_message(state, &order_message).await? {
                    PolymarketOrderEventOutcome::Applied => report.order_updates_applied += 1,
                    PolymarketOrderEventOutcome::UnknownOrder => {
                        report.skipped_unknown_orders += 1;
                    }
                    PolymarketOrderEventOutcome::Ignored => {}
                }
            }
            WsMessage::Trade(trade_message) => {
                let trade_report = apply_polymarket_ws_trade_message(
                    state,
                    connector.account_id(),
                    &trade_message,
                )
                .await?;
                report.trade_updates_applied += trade_report.applied;
                report.skipped_unknown_orders += trade_report.skipped_unknown_orders;
                report.skipped_duplicate_trades += trade_report.skipped_duplicate_trades;
            }
            _ => {}
        }

        if max_events.is_some_and(|limit| report.consumed >= limit) {
            break;
        }
    }

    Ok(report)
}

async fn collect_polymarket_user_event_markets(
    state: &AppState,
    connector_name: &str,
) -> Result<Vec<B256>> {
    if state.settings.polymarket.ws_max_instruments == 0 {
        return Ok(Vec::new());
    }

    // Cap at 200 to match OrderListFilters MAX_LIST_LIMIT.
    let fetch_limit = u16::try_from(
        state
            .settings
            .polymarket
            .ws_max_instruments
            .saturating_mul(4)
            .min(200)
            .min(usize::from(u16::MAX)),
    )
    .expect("bounded polymarket websocket fetch limit");
    let mut seen_condition_ids = HashSet::new();
    let mut markets = Vec::new();

    for status in [
        OrderStatus::Submitted,
        OrderStatus::Open,
        OrderStatus::PartiallyFilled,
    ] {
        let orders = state
            .execution_service
            .list_orders(OrderListFilters::new(
                None,
                None,
                Some(connector_name.to_string()),
                Some(status),
                Some(fetch_limit),
            )?)
            .await?;

        for order in orders {
            if markets.len() >= state.settings.polymarket.ws_max_instruments {
                return Ok(markets);
            }

            let market = state
                .market_event_service
                .get_market(&order.market_id)
                .await?;
            let market_refs = match polymarket_market_refs(&market) {
                Ok(market_refs) => market_refs,
                Err(error) => {
                    warn!(
                        market_id = %market.id,
                        order_id = %order.id,
                        error_code = %error.code(),
                        "skipping polymarket websocket market subscription because market refs are incomplete"
                    );
                    continue;
                }
            };
            let condition_key = market_refs.condition_id.clone();
            if !seen_condition_ids.insert(condition_key.clone()) {
                continue;
            }
            match market_refs.condition_id() {
                Ok(condition_id) => markets.push(condition_id),
                Err(error) => {
                    warn!(
                        market_id = %market.id,
                        order_id = %order.id,
                        condition_id = %condition_key,
                        error_code = %error.code(),
                        "skipping polymarket websocket market subscription because condition id is invalid"
                    );
                }
            }
        }
    }

    Ok(markets)
}

async fn apply_polymarket_ws_order_message(
    state: &AppState,
    order_message: &polymarket_client_sdk::clob::ws::OrderMessage,
) -> Result<PolymarketOrderEventOutcome> {
    let Some(update) = normalize_polymarket_ws_order_message(order_message)? else {
        return Ok(PolymarketOrderEventOutcome::Ignored);
    };

    let request_id = new_trace_id();
    let trace_id = new_trace_id();
    let actor = worker_actor(&request_id);
    match state
        .execution_service
        .sync_external_order_status(SyncExternalOrderStatusCommand {
            connector_name: update.connector_name.clone(),
            external_order_id: update.external_order_id.clone(),
            status: update.status,
            request_id,
            trace_id: trace_id.clone(),
            actor,
        })
        .await
    {
        Ok(order) => {
            info!(
                trace_id = %trace_id,
                order_id = %order.id,
                external_order_id = %update.external_order_id,
                status = %update.status.as_str(),
                event_id = %update.event_id,
                "applied polymarket websocket order update",
            );
            Ok(PolymarketOrderEventOutcome::Applied)
        }
        Err(error) if error.code() == "ORDER_NOT_FOUND" => {
            info!(
                trace_id = %trace_id,
                external_order_id = %update.external_order_id,
                event_id = %update.event_id,
                "skipping polymarket websocket order update because no internal order matches the external order id",
            );
            Ok(PolymarketOrderEventOutcome::UnknownOrder)
        }
        Err(error) => Err(error),
    }
}

async fn apply_polymarket_ws_trade_message(
    state: &AppState,
    account_id: &str,
    trade_message: &polymarket_client_sdk::clob::ws::TradeMessage,
) -> Result<PolymarketTradeEventReport> {
    let updates = normalize_polymarket_ws_trade_message(trade_message, account_id)?;
    let mut report = PolymarketTradeEventReport::default();

    for update in updates {
        let request_id = new_trace_id();
        let trace_id = new_trace_id();
        let actor = worker_actor(&request_id);

        match state
            .execution_service
            .reconcile_external_trade(ReconcileExternalTradeCommand {
                connector_name: update.connector_name.clone(),
                external_order_id: update.external_order_id.clone(),
                account_id: update.account_id.clone(),
                external_trade_id: update.external_trade_id.clone(),
                fill_price: update.fill_price,
                filled_quantity: update.filled_quantity,
                fee: update.fee,
                request_id,
                trace_id: trace_id.clone(),
                actor,
            })
            .await
        {
            Ok(result) => {
                report.applied += 1;
                info!(
                    trace_id = %trace_id,
                    order_id = %result.order.id,
                    external_order_id = %update.external_order_id,
                    external_trade_id = %update.external_trade_id,
                    event_id = %update.event_id,
                    "applied polymarket websocket trade update",
                );
            }
            Err(error) if error.code() == "ORDER_NOT_FOUND" => {
                report.skipped_unknown_orders += 1;
                info!(
                    trace_id = %trace_id,
                    external_order_id = %update.external_order_id,
                    external_trade_id = %update.external_trade_id,
                    event_id = %update.event_id,
                    "skipping polymarket websocket trade update because no internal order matches the external order id",
                );
            }
            Err(error) if error.code() == "STATE_TRADE_ALREADY_RECORDED" => {
                report.skipped_duplicate_trades += 1;
                info!(
                    trace_id = %trace_id,
                    external_order_id = %update.external_order_id,
                    external_trade_id = %update.external_trade_id,
                    event_id = %update.event_id,
                    "skipping polymarket websocket trade update because the external trade id was already reconciled",
                );
            }
            Err(error) => return Err(error),
        }
    }

    Ok(report)
}
