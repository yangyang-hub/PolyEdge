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

    pub async fn submit(
        &self,
        request: &LivePolymarketOrderRequest,
    ) -> Result<LivePolymarketExecutionOutcome> {
        validate_live_order_request(request)?;
        let _ = request.market_refs.condition_id()?;
        let asset_id = request.market_refs.asset_id_for_side(request.side)?;
        let adjusted_quantity = adjusted_order_quantity(request.limit_price, request.quantity)?;
        let adjusted_notional = request.limit_price.value() * adjusted_quantity.value();
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

        let signable = self
            .client
            .limit_order()
            .token_id(asset_id)
            .side(Side::Buy)
            .price(request.limit_price.value())
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

        if !response.success {
            return Ok(LivePolymarketExecutionOutcome::Rejected(
                PolymarketOrderRejection {
                    code: "POLYMARKET_ORDER_REJECTED".to_string(),
                    message: response
                        .error_msg
                        .unwrap_or_else(|| "Polymarket order was rejected".to_string()),
                },
            ));
        }

        let accepted_status = accepted_order_status(&response.status);
        match response.status {
            SdkOrderStatusType::Live
            | SdkOrderStatusType::Matched
            | SdkOrderStatusType::Delayed => Ok(LivePolymarketExecutionOutcome::Accepted(
                LivePolymarketOrderAcceptance {
                    order_id: response.order_id,
                    status: accepted_status,
                    accepted_at: OffsetDateTime::now_utc(),
                },
            )),
            other => Ok(LivePolymarketExecutionOutcome::Rejected(
                PolymarketOrderRejection {
                    code: "POLYMARKET_ORDER_STATUS_UNSUPPORTED".to_string(),
                    message: format!(
                        "Polymarket returned unsupported post_order status={other} for execution_request_id={}",
                        request.execution_request_id
                    ),
                },
            )),
        }
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
        let adjusted_notional = request.limit_price.value() * adjusted_quantity.value();
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

        let signable = self
            .client
            .limit_order()
            .token_id(token_id)
            .side(token_order_side(request.side))
            .price(request.limit_price.value())
            .size(adjusted_quantity.value())
            .order_type(OrderType::GTC)
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

        if !response.success {
            return Ok(LivePolymarketExecutionOutcome::Rejected(
                PolymarketOrderRejection {
                    code: "POLYMARKET_ORDER_REJECTED".to_string(),
                    message: response
                        .error_msg
                        .unwrap_or_else(|| "Polymarket order was rejected".to_string()),
                },
            ));
        }

        let accepted_status = accepted_order_status(&response.status);
        match response.status {
            SdkOrderStatusType::Live
            | SdkOrderStatusType::Matched
            | SdkOrderStatusType::Delayed => Ok(LivePolymarketExecutionOutcome::Accepted(
                LivePolymarketOrderAcceptance {
                    order_id: response.order_id,
                    status: accepted_status,
                    accepted_at: OffsetDateTime::now_utc(),
                },
            )),
            other => Ok(LivePolymarketExecutionOutcome::Rejected(
                PolymarketOrderRejection {
                    code: "POLYMARKET_ORDER_STATUS_UNSUPPORTED".to_string(),
                    message: format!(
                        "Polymarket returned unsupported post_order status={other} for client_order_id={}",
                        request.client_order_id
                    ),
                },
            )),
        }
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
            SdkOrderStatusType::Live => Ok(Some(ConnectorOrderStatusUpdate {
                event_id: format!("evt_pm_order_poll:{}:live", order.id),
                connector_name: POLYMARKET_CONNECTOR_NAME.to_string(),
                external_order_id: order.id,
                status: OrderStatus::Open,
            })),
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
    ) -> Result<Vec<ConnectorTradeFillUpdate>> {
        validate_live_trade_sync_request(request)?;
        let order = self.fetch_order(&request.external_order_id).await?;
        let mut updates = Vec::new();

        for trade_id in order.associate_trades {
            if let Some(update) = self
                .fetch_trade_update(&trade_id, &request.external_order_id, &request.account_id)
                .await?
            {
                updates.push(update);
            }
        }

        Ok(updates)
    }

    async fn fetch_order(
        &self,
        external_order_id: &str,
    ) -> Result<polymarket_client_sdk::clob::types::response::OpenOrderResponse> {
        let request = OrdersRequest::builder()
            .order_id(external_order_id.to_string())
            .build();
        let page = self.client.orders(&request, None).await.map_err(|error| {
            AppError::internal(
                "POLYMARKET_ORDER_QUERY_FAILED",
                format!("failed to query polymarket order {external_order_id}: {error}"),
            )
        })?;

        page.data
            .into_iter()
            .find(|order| order.id == external_order_id)
            .ok_or_else(|| {
                AppError::not_found(
                    "POLYMARKET_ORDER_NOT_FOUND",
                    format!("Polymarket order {external_order_id} was not found"),
                )
            })
    }

    async fn fetch_trade_update(
        &self,
        external_trade_id: &str,
        external_order_id: &str,
        account_id: &str,
    ) -> Result<Option<ConnectorTradeFillUpdate>> {
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
            return Ok(None);
        };

        if matches!(trade.status, SdkTradeStatusType::Failed) {
            return Ok(None);
        }

        let fill_price = Probability::new(trade.price).map_err(|error| {
            AppError::internal(
                "POLYMARKET_TRADE_PRICE_INVALID",
                format!("failed to decode trade price for {external_trade_id}: {error}"),
            )
        })?;
        let filled_quantity = Quantity::new(trade.size).map_err(|error| {
            AppError::internal(
                "POLYMARKET_TRADE_SIZE_INVALID",
                format!("failed to decode trade size for {external_trade_id}: {error}"),
            )
        })?;
        let fee = UsdAmount::new(
            trade.price * trade.size * trade.fee_rate_bps / Decimal::from(10_000_u64),
        )
        .map_err(|error| {
            AppError::internal(
                "POLYMARKET_TRADE_FEE_INVALID",
                format!("failed to decode trade fee for {external_trade_id}: {error}"),
            )
        })?;

        Ok(Some(ConnectorTradeFillUpdate {
            event_id: format!("evt_pm_trade_poll:{}:{}", external_order_id, trade.id),
            connector_name: POLYMARKET_CONNECTOR_NAME.to_string(),
            external_order_id: external_order_id.to_string(),
            account_id: account_id.to_string(),
            external_trade_id: trade.id,
            fill_price,
            filled_quantity,
            fee,
        }))
    }
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
        _ => PolymarketAcceptedOrderStatus::Delayed,
    }
}
