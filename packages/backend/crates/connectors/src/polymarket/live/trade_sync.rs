impl LivePolymarketConnector {
    pub async fn collect_trade_updates(
        &self,
        request: &LivePolymarketTradeSyncRequest,
    ) -> Result<LivePolymarketTradeSyncOutcome> {
        validate_live_trade_sync_request(request)?;
        let order = match self.fetch_order(&request.external_order_id).await {
            Ok(order) => order,
            Err(error)
                if error.code() == "POLYMARKET_ORDER_NOT_FOUND"
                    && request.fallback_token_id.is_some() =>
            {
                return self.collect_missing_order_trade_updates(request).await;
            }
            Err(error) => return Err(error),
        };
        let mut updates = Vec::new();
        let mut associated_trades_terminal =
            !order.associate_trades.is_empty() || order.size_matched <= Decimal::ZERO;

        for trade_id in &order.associate_trades {
            let reconciliation = match self
                .fetch_trade_update(trade_id, &request.external_order_id, &request.account_id)
                .await
            {
                Ok(reconciliation) => reconciliation,
                Err(error) => {
                    warn!(
                        external_order_id = request.external_order_id,
                        external_trade_id = trade_id,
                        error = %error,
                        "associated trade lookup failed; scanning account trades for the order"
                    );
                    return self
                        .collect_existing_order_trade_updates(request, &order)
                        .await;
                }
            };
            match reconciliation {
                LivePolymarketTradeReconciliation::Confirmed(update) => updates.push(update),
                LivePolymarketTradeReconciliation::SettledWithoutFill => {}
                LivePolymarketTradeReconciliation::Pending => {
                    associated_trades_terminal = false;
                }
            }
        }

        Ok(LivePolymarketTradeSyncOutcome {
            updates,
            order_status: reconciled_order_status_update(&order, associated_trades_terminal),
            order_not_found: false,
        })
    }

    async fn collect_missing_order_trade_updates(
        &self,
        request: &LivePolymarketTradeSyncRequest,
    ) -> Result<LivePolymarketTradeSyncOutcome> {
        let scan = self
            .scan_order_trade_history(
                request,
                &[],
                "POLYMARKET_MISSING_ORDER_TRADE_QUERY_FAILED",
            )
            .await?;
        Ok(LivePolymarketTradeSyncOutcome {
            updates: scan.updates,
            order_status: None,
            order_not_found: true,
        })
    }

    async fn collect_existing_order_trade_updates(
        &self,
        request: &LivePolymarketTradeSyncRequest,
        order: &polymarket_client_sdk::clob::types::response::OpenOrderResponse,
    ) -> Result<LivePolymarketTradeSyncOutcome> {
        let scan = self
            .scan_order_trade_history(
                request,
                &order.associate_trades,
                "POLYMARKET_ASSOCIATED_TRADE_FALLBACK_FAILED",
            )
            .await?;
        Ok(LivePolymarketTradeSyncOutcome {
            updates: scan.updates,
            order_status: reconciled_order_status_update(order, scan.expected_trades_terminal),
            order_not_found: false,
        })
    }

    async fn scan_order_trade_history(
        &self,
        request: &LivePolymarketTradeSyncRequest,
        expected_trade_ids: &[String],
        error_code: &'static str,
    ) -> Result<OrderTradeHistoryScan> {
        let token_id = parse_u256(
            "fallback_token_id",
            request.fallback_token_id.as_deref().unwrap_or_default(),
            "POLYMARKET_ASSET_ID_INVALID",
        )?;
        let trades_request = match request.fallback_after {
            Some(after) => TradesRequest::builder()
                .asset_id(token_id)
                .after(after)
                .build(),
            None => TradesRequest::builder().asset_id(token_id).build(),
        };
        let mut updates = Vec::new();
        let mut terminal_trade_ids = std::collections::HashSet::new();
        let mut emitted_trade_ids = std::collections::HashSet::new();
        let mut next_cursor: Option<String> = None;

        for _ in 0..CLOB_MAX_PAGES {
            let page = self
                .client
                .trades(&trades_request, next_cursor.clone())
                .await
                .map_err(|error| {
                    AppError::internal(
                        error_code,
                        format!(
                            "failed to scan fallback trades for order {}: {error}",
                            request.external_order_id
                        ),
                    )
                })?;

            for trade in page
                .data
                .iter()
                .filter(|trade| trade_matches_order(trade, &request.external_order_id))
            {
                match reconcile_live_trade(trade, &request.external_order_id, &request.account_id)?
                {
                    LivePolymarketTradeReconciliation::Confirmed(update) => {
                        terminal_trade_ids.insert(trade.id.clone());
                        if emitted_trade_ids.insert(update.external_trade_id.clone()) {
                            updates.push(update);
                        }
                    }
                    LivePolymarketTradeReconciliation::SettledWithoutFill => {
                        terminal_trade_ids.insert(trade.id.clone());
                    }
                    LivePolymarketTradeReconciliation::Pending => {}
                }
            }

            if clob_page_is_terminal(&page.next_cursor, page.count, next_cursor.as_deref()) {
                break;
            }
            next_cursor = Some(page.next_cursor);
        }

        Ok(OrderTradeHistoryScan {
            updates,
            expected_trades_terminal: expected_trade_ids_are_terminal(
                expected_trade_ids,
                &terminal_trade_ids,
            ),
        })
    }

    async fn fetch_order(
        &self,
        external_order_id: &str,
    ) -> Result<polymarket_client_sdk::clob::types::response::OpenOrderResponse> {
        match self.client.order(external_order_id).await {
            Ok(order) => Ok(order),
            Err(error)
                if error
                    .downcast_ref::<polymarket_client_sdk::error::Status>()
                    .is_some_and(|status| status.status_code.as_u16() == 404) =>
            {
                Err(AppError::not_found(
                    "POLYMARKET_ORDER_NOT_FOUND",
                    format!("Polymarket order {external_order_id} was not found"),
                ))
            }
            Err(error) => Err(AppError::internal(
                "POLYMARKET_ORDER_QUERY_FAILED",
                format!("failed to query polymarket order {external_order_id}: {error}"),
            )),
        }
    }

    async fn fetch_trade_update(
        &self,
        external_trade_id: &str,
        external_order_id: &str,
        account_id: &str,
    ) -> Result<LivePolymarketTradeReconciliation> {
        let request = TradesRequest::builder()
            .id(external_trade_id.to_string())
            .build();
        let page = self.client.trades(&request, None).await.map_err(|error| {
            AppError::internal(
                "POLYMARKET_TRADE_QUERY_FAILED",
                format!("failed to query polymarket trade {external_trade_id}: {error}"),
            )
        })?;

        let Some(trade) = page
            .data
            .into_iter()
            .find(|trade| trade_matches_order(trade, external_order_id))
        else {
            warn!(
                external_trade_id,
                external_order_id,
                "polymarket trade response did not map back to the requested order"
            );
            return Ok(LivePolymarketTradeReconciliation::Pending);
        };

        reconcile_live_trade(&trade, external_order_id, account_id)
    }
}

struct OrderTradeHistoryScan {
    updates: Vec<ConnectorTradeFillUpdate>,
    expected_trades_terminal: bool,
}

fn expected_trade_ids_are_terminal(
    expected_trade_ids: &[String],
    terminal_trade_ids: &std::collections::HashSet<String>,
) -> bool {
    !expected_trade_ids.is_empty()
        && expected_trade_ids
            .iter()
            .all(|trade_id| terminal_trade_ids.contains(trade_id))
}
