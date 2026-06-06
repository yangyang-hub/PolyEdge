impl LivePolymarketConnector {
    pub async fn connect(config: &LivePolymarketConfig) -> Result<Self> {
        let private_key = normalize_required(
            "private_key",
            &config.private_key,
            "POLYMARKET_PRIVATE_KEY_REQUIRED",
        )?;
        let account_id = normalize_required(
            "account_id",
            &config.account_id,
            "POLYMARKET_ACCOUNT_ID_REQUIRED",
        )?;
        let ws_host =
            normalize_required("ws_host", &config.ws_host, "POLYMARKET_WS_HOST_REQUIRED")?;
        let signer = LocalSigner::from_str(&private_key)
            .map_err(|error| {
                AppError::invalid_input(
                    "POLYMARKET_PRIVATE_KEY_INVALID",
                    format!("invalid polymarket private_key: {error}"),
                )
            })?
            .with_chain_id(Some(config.chain_id));

        let client =
            ClobClient::new(&config.clob_host, ClobConfig::default()).map_err(|error| {
                AppError::internal(
                    "POLYMARKET_CLIENT_INIT_FAILED",
                    format!("failed to initialize Polymarket CLOB client: {error}"),
                )
            })?;

        let credentials = maybe_credentials(config)?;
        let mut auth_builder = client
            .authentication_builder(&signer)
            .signature_type(config.signature_type.into());

        if let Some(funder) = normalize_optional(config.funder.as_deref()) {
            auth_builder = auth_builder.funder(parse_address(
                "funder",
                &funder,
                "POLYMARKET_FUNDER_INVALID",
            )?);
        }

        if let Some(credentials) = credentials {
            auth_builder = auth_builder.credentials(credentials);
        }

        let client = auth_builder.authenticate().await.map_err(|error| {
            AppError::internal(
                "POLYMARKET_AUTHENTICATION_FAILED",
                format!("failed to authenticate Polymarket client: {error}"),
            )
        })?;

        Ok(Self {
            client,
            private_key,
            chain_id: config.chain_id,
            account_id,
            ws_host,
            signature_type: config.signature_type,
        })
    }

    #[must_use]
    pub fn account_id(&self) -> &str {
        &self.account_id
    }

    pub fn connect_user_ws(&self) -> Result<ClobWsClient<Authenticated<Normal>>> {
        let client = ClobWsClient::new(&self.ws_host, WsConfig::default()).map_err(|error| {
            AppError::internal(
                "POLYMARKET_WS_CLIENT_INIT_FAILED",
                format!("failed to initialize Polymarket user websocket client: {error}"),
            )
        })?;

        client
            .authenticate(self.client.credentials().clone(), self.client.address())
            .map_err(|error| {
                AppError::internal(
                    "POLYMARKET_WS_AUTHENTICATION_FAILED",
                    format!("failed to authenticate Polymarket user websocket client: {error}"),
                )
            })
    }

    /// Query the authenticated account's USDC balance from Polymarket.
    pub async fn balance(&self) -> Result<BalanceAllowanceResponse> {
        let request = BalanceAllowanceRequest::builder()
            .asset_type(AssetType::Collateral)
            .signature_type(self.signature_type.into())
            .build();
        self.client.balance_allowance(request).await.map_err(|error| {
            AppError::dependency_unavailable(
                "POLYMARKET_BALANCE_QUERY_FAILED",
                format!("failed to query Polymarket balance: {error}"),
            )
        })
    }

    /// Force the CLOB to refresh its cached collateral balance, then query it.
    /// Required for `poly_1271` deposit wallets where the cached balance may be
    /// stale after a deposit; for EOA wallets this is a no-op update + query.
    pub async fn refresh_balance(&self) -> Result<BalanceAllowanceResponse> {
        if self.signature_type == PolymarketSignatureScheme::Poly1271 {
            self.update_deposit_wallet_balance_allowance_if_needed(Side::Buy, None)
                .await?;
        }
        self.balance().await
    }

    /// List all open orders for the authenticated account, paginating
    /// through all available pages.
    pub async fn list_open_orders(&self) -> Result<Vec<PolymarketOpenOrder>> {
        // Polymarket's CLOB signals end-of-results with the terminal cursor
        // "LTE=" (base64 of -1); it is non-empty and the final page can still
        // carry rows, so we must break on it (and on a repeated cursor) to
        // avoid re-requesting past the end forever.
        const TERMINAL_CURSOR: &str = "LTE=";
        const MAX_PAGES: usize = 1000;

        let request = OrdersRequest::builder().build();
        let mut all_orders = Vec::new();
        let mut next_cursor: Option<String> = None;

        for _ in 0..MAX_PAGES {
            let page = self
                .client
                .orders(&request, next_cursor.clone())
                .await
                .map_err(|error| {
                    AppError::dependency_unavailable(
                        "POLYMARKET_ORDERS_QUERY_FAILED",
                        format!("failed to query Polymarket orders: {error}"),
                    )
                })?;

            for o in page.data {
                all_orders.push(PolymarketOpenOrder {
                    id: o.id,
                    market: format!("0x{:064x}", o.market),
                    asset_id: format!("{}", o.asset_id),
                    side: match o.side {
                        Side::Buy => PolymarketTokenOrderSide::Buy,
                        Side::Sell => PolymarketTokenOrderSide::Sell,
                        _ => PolymarketTokenOrderSide::Buy,
                    },
                    original_size: o.original_size,
                    size_matched: o.size_matched,
                    price: o.price,
                    outcome: o.outcome,
                    status: format!("{:?}", o.status),
                });
            }

            if page.next_cursor.is_empty()
                || page.next_cursor == TERMINAL_CURSOR
                || page.count == 0
                || next_cursor.as_deref() == Some(page.next_cursor.as_str())
            {
                break;
            }
            next_cursor = Some(page.next_cursor);
        }

        Ok(all_orders)
    }

    /// Recover a previously-submitted token order after the caller lost the
    /// `post_order` response. Matching is deliberately strict so a managed
    /// rewards order never adopts an unrelated account order.
    pub async fn find_matching_open_token_order(
        &self,
        request: &LivePolymarketTokenOrderRequest,
    ) -> Result<Option<LivePolymarketOrderAcceptance>> {
        validate_live_token_order_request(request)?;
        let adjusted_quantity = adjusted_order_quantity(request.limit_price, request.quantity)?;
        let tick_price = request.limit_price.value().round_dp(2);
        let expected_size = adjusted_quantity.value().round_dp(4);
        let mut matches = self
            .list_open_orders()
            .await?
            .into_iter()
            .filter(|order| {
                order.asset_id == request.token_id
                    && order.side == request.side
                    && order.price.round_dp(2) == tick_price
                    && order.original_size.round_dp(4) == expected_size
            });

        let Some(order) = matches.next() else {
            return Ok(None);
        };
        if matches.next().is_some() {
            return Err(AppError::conflict(
                "POLYMARKET_ORDER_RECOVERY_AMBIGUOUS",
                format!(
                    "multiple open Polymarket orders match client_order_id={}",
                    request.client_order_id
                ),
            ));
        }

        Ok(Some(LivePolymarketOrderAcceptance {
            order_id: order.id,
            status: PolymarketAcceptedOrderStatus::Live,
            submitted_quantity: adjusted_quantity,
            accepted_at: OffsetDateTime::now_utc(),
        }))
    }

    pub async fn submit(
        &self,
        request: &LivePolymarketOrderRequest,
    ) -> Result<LivePolymarketExecutionOutcome> {
        validate_live_order_request(request)?;
        let _ = request.market_refs.condition_id()?;
        let asset_id = request.market_refs.asset_id_for_side(request.side)?;
        let adjusted_quantity = adjusted_order_quantity(request.limit_price, request.quantity)?;
        // Snap to a 0.01-tick-safe price (<= 2 dp); see submit_token_order.
        let tick_price = request.limit_price.value().round_dp(2);
        let adjusted_notional = tick_price * adjusted_quantity.value();
        let signer = LocalSigner::from_str(&self.private_key)
            .map_err(|error| {
                AppError::invalid_input(
                    "POLYMARKET_PRIVATE_KEY_INVALID",
                    format!("invalid polymarket private_key: {error}"),
                )
            })?
            .with_chain_id(Some(self.chain_id));

        if adjusted_notional < POLYMARKET_MIN_NOTIONAL_USD {
            return Ok(LivePolymarketExecutionOutcome::Rejected(
                PolymarketOrderRejection {
                    code: "POLYMARKET_MIN_NOTIONAL_NOT_MET".to_string(),
                    message: format!(
                        "polymarket live connector requires adjusted notional >= 1.00 USD, got {}",
                        adjusted_notional
                    ),
                },
            ));
        }

        self.update_deposit_wallet_balance_allowance_if_needed(Side::Buy, Some(asset_id))
            .await?;

        let signable = self
            .client
            .limit_order()
            .token_id(asset_id)
            .side(Side::Buy)
            .price(tick_price)
            .size(adjusted_quantity.value())
            .order_type(OrderType::GTC)
            .build()
            .await
            .map_err(|error| {
                AppError::internal(
                    "POLYMARKET_ORDER_BUILD_FAILED",
                    format!(
                        "failed to build live polymarket order for execution_request_id={}: {error}",
                        request.execution_request_id
                    ),
                )
            })?;

        let signed = self.client.sign(&signer, signable).await.map_err(|error| {
            AppError::internal(
                "POLYMARKET_ORDER_SIGN_FAILED",
                format!(
                    "failed to sign live polymarket order for execution_request_id={}: {error}",
                    request.execution_request_id
                ),
            )
        })?;

        let response = self.client.post_order(signed).await.map_err(|error| {
            AppError::internal(
                "POLYMARKET_ORDER_POST_FAILED",
                format!(
                    "failed to submit live polymarket order for execution_request_id={}: {error}",
                    request.execution_request_id
                ),
            )
        })?;

        if !response.success && response.order_id.trim().is_empty() {
            return Ok(LivePolymarketExecutionOutcome::Rejected(
                PolymarketOrderRejection {
                    code: "POLYMARKET_ORDER_REJECTED".to_string(),
                    message: response
                        .error_msg
                        .unwrap_or_else(|| "Polymarket order was rejected".to_string()),
                },
            ));
        }
        if response.order_id.trim().is_empty() {
            return Err(AppError::internal(
                "POLYMARKET_ORDER_POST_FAILED",
                format!(
                    "Polymarket accepted live order without order_id for execution_request_id={}",
                    request.execution_request_id
                ),
            ));
        }

        Ok(LivePolymarketExecutionOutcome::Accepted(
            LivePolymarketOrderAcceptance {
                order_id: response.order_id,
                status: accepted_order_status(&response.status),
                submitted_quantity: adjusted_quantity,
                accepted_at: OffsetDateTime::now_utc(),
            },
        ))
    }

    pub async fn submit_token_order(
        &self,
        request: &LivePolymarketTokenOrderRequest,
    ) -> Result<LivePolymarketExecutionOutcome> {
        validate_live_token_order_request(request)?;
        let token_id = parse_u256(
            "polymarket_token_id",
            &request.token_id,
            "POLYMARKET_TOKEN_ID_INVALID",
        )?;
        let adjusted_quantity = adjusted_order_quantity(request.limit_price, request.quantity)?;
        // Snap to a 0.01-tick-safe price (<= 2 dp) so the CLOB order builder never
        // rejects for over-precision. The rewards planner already floors to 0.01,
        // so this is identity for the live caller; markets with a coarser tick
        // than 0.01 would need per-market tick-size plumbing (not yet wired).
        let tick_price = request.limit_price.value().round_dp(2);
        let adjusted_notional = tick_price * adjusted_quantity.value();
        let signer = LocalSigner::from_str(&self.private_key)
            .map_err(|error| {
                AppError::invalid_input(
                    "POLYMARKET_PRIVATE_KEY_INVALID",
                    format!("invalid polymarket private_key: {error}"),
                )
            })?
            .with_chain_id(Some(self.chain_id));

        if adjusted_notional < POLYMARKET_MIN_NOTIONAL_USD {
            return Ok(LivePolymarketExecutionOutcome::Rejected(
                PolymarketOrderRejection {
                    code: "POLYMARKET_MIN_NOTIONAL_NOT_MET".to_string(),
                    message: format!(
                        "polymarket live connector requires adjusted notional >= 1.00 USD, got {}",
                        adjusted_notional
                    ),
                },
            ));
        }

        self.update_deposit_wallet_balance_allowance_if_needed(
            token_order_side(request.side),
            Some(token_id),
        )
        .await?;

        let signable = self
            .client
            .limit_order()
            .token_id(token_id)
            .side(token_order_side(request.side))
            .price(tick_price)
            .size(adjusted_quantity.value())
            .order_type(if request.post_only {
                OrderType::GTC
            } else {
                OrderType::FAK
            })
            .post_only(request.post_only)
            .build()
            .await
            .map_err(|error| {
                AppError::internal(
                    "POLYMARKET_ORDER_BUILD_FAILED",
                    format!(
                        "failed to build live polymarket rewards order for client_order_id={}: {error}",
                        request.client_order_id
                    ),
                )
            })?;

        let signed = self.client.sign(&signer, signable).await.map_err(|error| {
            AppError::internal(
                "POLYMARKET_ORDER_SIGN_FAILED",
                format!(
                    "failed to sign live polymarket rewards order for client_order_id={}: {error}",
                    request.client_order_id
                ),
            )
        })?;

        let response = self.client.post_order(signed).await.map_err(|error| {
            AppError::internal(
                "POLYMARKET_ORDER_POST_FAILED",
                format!(
                    "failed to submit live polymarket rewards order for client_order_id={}: {error}",
                    request.client_order_id
                ),
            )
        })?;

        if !response.success && response.order_id.trim().is_empty() {
            return Ok(LivePolymarketExecutionOutcome::Rejected(
                PolymarketOrderRejection {
                    code: "POLYMARKET_ORDER_REJECTED".to_string(),
                    message: response
                        .error_msg
                        .unwrap_or_else(|| "Polymarket order was rejected".to_string()),
                },
            ));
        }
        if response.order_id.trim().is_empty() {
            return Err(AppError::internal(
                "POLYMARKET_ORDER_POST_FAILED",
                format!(
                    "Polymarket accepted live rewards order without order_id for client_order_id={}",
                    request.client_order_id
                ),
            ));
        }

        Ok(LivePolymarketExecutionOutcome::Accepted(
            LivePolymarketOrderAcceptance {
                order_id: response.order_id,
                status: accepted_order_status(&response.status),
                submitted_quantity: adjusted_quantity,
                accepted_at: OffsetDateTime::now_utc(),
            },
        ))
    }

    async fn update_deposit_wallet_balance_allowance_if_needed(
        &self,
        side: Side,
        token_id: Option<U256>,
    ) -> Result<()> {
        if self.signature_type != PolymarketSignatureScheme::Poly1271 {
            return Ok(());
        }

        let mut request = UpdateBalanceAllowanceRequest::default();
        request.signature_type = Some(SignatureType::Poly1271);
        match side {
            Side::Buy => {
                request.asset_type = AssetType::Collateral;
                request.token_id = None;
            }
            Side::Sell => {
                request.asset_type = AssetType::Conditional;
                request.token_id = token_id;
            }
            Side::Unknown => {
                return Err(AppError::invalid_input(
                    "POLYMARKET_ORDER_SIDE_INVALID",
                    "cannot update balance allowance for unknown Polymarket side",
                ));
            }
            _ => {
                return Err(AppError::invalid_input(
                    "POLYMARKET_ORDER_SIDE_UNSUPPORTED",
                    "unsupported Polymarket side for balance allowance update",
                ));
            }
        };

        self.client.update_balance_allowance(request).await.map_err(|error| {
            AppError::internal(
                "POLYMARKET_BALANCE_ALLOWANCE_UPDATE_FAILED",
                format!("failed to update Polymarket deposit wallet balance allowance: {error}"),
            )
        })
    }

    pub async fn cancel_order(
        &self,
        request: &LivePolymarketCancelOrderRequest,
    ) -> Result<LivePolymarketCancelOutcome> {
        validate_live_cancel_order_request(request)?;
        let response = self
            .client
            .cancel_order(&request.external_order_id)
            .await
            .map_err(|error| {
                AppError::internal(
                    "POLYMARKET_ORDER_CANCEL_FAILED",
                    format!(
                        "failed to cancel live polymarket order {}: {error}",
                        request.external_order_id
                    ),
                )
            })?;

        if response
            .canceled
            .iter()
            .any(|order_id| order_id == &request.external_order_id)
        {
            return Ok(LivePolymarketCancelOutcome::Accepted(
                LivePolymarketCancelAcceptance {
                    external_order_id: request.external_order_id.clone(),
                    cancelled_at: OffsetDateTime::now_utc(),
                },
            ));
        }

        let message = response
            .not_canceled
            .get(&request.external_order_id)
            .cloned()
            .unwrap_or_else(|| "Polymarket did not confirm order cancellation".to_string());
        Ok(LivePolymarketCancelOutcome::Rejected(
            PolymarketOrderRejection {
                code: "POLYMARKET_ORDER_CANCEL_REJECTED".to_string(),
                message,
            },
        ))
    }

    pub async fn poll_order_status(
        &self,
        request: &LivePolymarketOrderStatusRequest,
    ) -> Result<Option<ConnectorOrderStatusUpdate>> {
        validate_live_order_status_request(request)?;
        let order = self.fetch_order(&request.external_order_id).await?;

        match order.status {
            SdkOrderStatusType::Live
            | SdkOrderStatusType::Unmatched
                if !matches!(order.order_type, OrderType::FAK | OrderType::FOK) =>
            {
                Ok(Some(ConnectorOrderStatusUpdate {
                event_id: format!("evt_pm_order_poll:{}:live", order.id),
                connector_name: POLYMARKET_CONNECTOR_NAME.to_string(),
                external_order_id: order.id,
                status: OrderStatus::Open,
                }))
            }
            SdkOrderStatusType::Canceled => Ok(Some(ConnectorOrderStatusUpdate {
                event_id: format!("evt_pm_order_poll:{}:canceled", order.id),
                connector_name: POLYMARKET_CONNECTOR_NAME.to_string(),
                external_order_id: order.id,
                status: OrderStatus::Canceled,
            })),
            SdkOrderStatusType::Matched
            | SdkOrderStatusType::Delayed
            | SdkOrderStatusType::Unmatched
            | SdkOrderStatusType::Unknown(_)
            | _ => Ok(None),
        }
    }

    pub async fn collect_trade_updates(
        &self,
        request: &LivePolymarketTradeSyncRequest,
    ) -> Result<LivePolymarketTradeSyncOutcome> {
        validate_live_trade_sync_request(request)?;
        let order = self.fetch_order(&request.external_order_id).await?;
        let mut updates = Vec::new();
        let mut associated_trades_terminal = !order.associate_trades.is_empty()
            || order.size_matched <= Decimal::ZERO;

        for trade_id in &order.associate_trades {
            match self
                .fetch_trade_update(trade_id, &request.external_order_id, &request.account_id)
                .await?
            {
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

        match live_trade_settlement(&trade.status) {
            LivePolymarketTradeSettlement::Confirmed => {}
            LivePolymarketTradeSettlement::SettledWithoutFill => {
                return Ok(LivePolymarketTradeReconciliation::SettledWithoutFill);
            }
            LivePolymarketTradeSettlement::Pending => {
                return Ok(LivePolymarketTradeReconciliation::Pending);
            }
        }

        let Some(fill) = trade_order_fill(&trade, external_order_id) else {
            warn!(
                external_trade_id,
                external_order_id,
                "polymarket trade response did not include order-specific fill details"
            );
            return Ok(LivePolymarketTradeReconciliation::Pending);
        };

        let fill_price = Probability::new(fill.price).map_err(|error| {
            AppError::internal(
                "POLYMARKET_TRADE_PRICE_INVALID",
                format!("failed to decode trade price for {external_trade_id}: {error}"),
            )
        })?;
        let filled_quantity = Quantity::new(fill.size).map_err(|error| {
            AppError::internal(
                "POLYMARKET_TRADE_SIZE_INVALID",
                format!("failed to decode trade size for {external_trade_id}: {error}"),
            )
        })?;
        let fee = UsdAmount::new(
            fill.price * fill.size * fill.fee_rate_bps / Decimal::from(10_000_u64),
        )
        .map_err(|error| {
            AppError::internal(
                "POLYMARKET_TRADE_FEE_INVALID",
                format!("failed to decode trade fee for {external_trade_id}: {error}"),
            )
        })?;

        Ok(LivePolymarketTradeReconciliation::Confirmed(
            ConnectorTradeFillUpdate {
            event_id: format!("evt_pm_trade_poll:{}:{}", external_order_id, trade.id),
            connector_name: POLYMARKET_CONNECTOR_NAME.to_string(),
            external_order_id: external_order_id.to_string(),
            account_id: account_id.to_string(),
            external_trade_id: trade.id,
            fill_price,
            filled_quantity,
            fee,
            },
        ))
    }
}

enum LivePolymarketTradeReconciliation {
    Confirmed(ConnectorTradeFillUpdate),
    SettledWithoutFill,
    Pending,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LivePolymarketTradeSettlement {
    Confirmed,
    SettledWithoutFill,
    Pending,
}

fn live_trade_settlement(status: &SdkTradeStatusType) -> LivePolymarketTradeSettlement {
    match status {
        SdkTradeStatusType::Confirmed => LivePolymarketTradeSettlement::Confirmed,
        SdkTradeStatusType::Failed => LivePolymarketTradeSettlement::SettledWithoutFill,
        _ => LivePolymarketTradeSettlement::Pending,
    }
}

fn reconciled_order_status_update(
    order: &polymarket_client_sdk::clob::types::response::OpenOrderResponse,
    associated_trades_terminal: bool,
) -> Option<ConnectorOrderStatusUpdate> {
    let status = match order.status {
        SdkOrderStatusType::Live => OrderStatus::Open,
        SdkOrderStatusType::Canceled if associated_trades_terminal => OrderStatus::Canceled,
        SdkOrderStatusType::Matched if associated_trades_terminal => OrderStatus::Filled,
        SdkOrderStatusType::Unmatched if matches!(order.order_type, OrderType::FAK | OrderType::FOK) => {
            if !associated_trades_terminal {
                return None;
            }
            OrderStatus::Canceled
        }
        SdkOrderStatusType::Unmatched => OrderStatus::Open,
        _ => return None,
    };
    Some(ConnectorOrderStatusUpdate {
        event_id: format!("evt_pm_order_reconcile:{}:{}", order.id, status.as_str()),
        connector_name: POLYMARKET_CONNECTOR_NAME.to_string(),
        external_order_id: order.id.clone(),
        status,
    })
}

fn token_order_side(side: PolymarketTokenOrderSide) -> Side {
    match side {
        PolymarketTokenOrderSide::Buy => Side::Buy,
        PolymarketTokenOrderSide::Sell => Side::Sell,
    }
}

fn accepted_order_status(status: &SdkOrderStatusType) -> PolymarketAcceptedOrderStatus {
    match status {
        SdkOrderStatusType::Live => PolymarketAcceptedOrderStatus::Live,
        SdkOrderStatusType::Matched => PolymarketAcceptedOrderStatus::Matched,
        SdkOrderStatusType::Delayed => PolymarketAcceptedOrderStatus::Delayed,
        SdkOrderStatusType::Unmatched => PolymarketAcceptedOrderStatus::Unmatched,
        SdkOrderStatusType::Canceled => PolymarketAcceptedOrderStatus::Canceled,
        _ => PolymarketAcceptedOrderStatus::Unknown,
    }
}
