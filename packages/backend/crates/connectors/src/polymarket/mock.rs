impl MockPolymarketConnector {
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    pub fn submit(
        &self,
        request: &MockPolymarketOrderRequest,
    ) -> Result<MockPolymarketExecutionOutcome> {
        if request.execution_request_id.trim().is_empty() {
            return Err(AppError::invalid_input(
                "POLYMARKET_EXECUTION_REQUEST_ID_REQUIRED",
                "execution_request_id must not be empty",
            ));
        }

        if request.connector_name != POLYMARKET_CONNECTOR_NAME {
            return Ok(MockPolymarketExecutionOutcome::Rejected(
                MockPolymarketOrderRejection {
                    code: "POLYMARKET_CONNECTOR_UNSUPPORTED".to_string(),
                    message: format!(
                        "mock polymarket connector only handles connector_name={POLYMARKET_CONNECTOR_NAME}, got {}",
                        request.connector_name
                    ),
                },
            ));
        }

        if request.market_id.trim().is_empty() {
            return Err(AppError::invalid_input(
                "POLYMARKET_MARKET_ID_REQUIRED",
                "market_id must not be empty",
            ));
        }

        if request.limit_price.value() <= Decimal::ZERO {
            return Ok(MockPolymarketExecutionOutcome::Rejected(
                MockPolymarketOrderRejection {
                    code: "POLYMARKET_LIMIT_PRICE_INVALID".to_string(),
                    message: "mock polymarket connector requires a positive limit price"
                        .to_string(),
                },
            ));
        }

        if request.quantity.value() <= Decimal::ZERO {
            return Ok(MockPolymarketExecutionOutcome::Rejected(
                MockPolymarketOrderRejection {
                    code: "POLYMARKET_QUANTITY_INVALID".to_string(),
                    message: "mock polymarket connector requires a positive quantity".to_string(),
                },
            ));
        }

        if request.notional.value() < POLYMARKET_MIN_NOTIONAL_USD {
            return Ok(MockPolymarketExecutionOutcome::Rejected(
                MockPolymarketOrderRejection {
                    code: "POLYMARKET_MIN_NOTIONAL_NOT_MET".to_string(),
                    message: format!(
                        "mock polymarket connector requires notional >= 1.00 USD, got {}",
                        request.notional
                    ),
                },
            ));
        }

        Ok(MockPolymarketExecutionOutcome::Accepted(
            MockPolymarketOrderAcceptance {
                order_id: format!(
                    "pm:{}:{}:{}",
                    request.market_id,
                    request.side.as_str(),
                    request.execution_request_id
                ),
                accepted_at: OffsetDateTime::now_utc(),
            },
        ))
    }

    pub fn poll_order_status(
        &self,
        request: &MockPolymarketOrderStatusRequest,
    ) -> Result<MockPolymarketOrderStatusPayload> {
        if request.connector_name != POLYMARKET_CONNECTOR_NAME {
            return Err(AppError::invalid_input(
                "POLYMARKET_CONNECTOR_UNSUPPORTED",
                format!(
                    "mock polymarket connector only handles connector_name={POLYMARKET_CONNECTOR_NAME}, got {}",
                    request.connector_name
                ),
            ));
        }

        if request.external_order_id.trim().is_empty() {
            return Err(AppError::invalid_input(
                "POLYMARKET_ORDER_ID_REQUIRED",
                "external_order_id must not be empty",
            ));
        }

        let status = match request.current_status {
            OrderStatus::Submitted => "live",
            OrderStatus::Open | OrderStatus::PartiallyFilled => "live",
            OrderStatus::Canceled => "canceled",
            current_status => current_status.as_str(),
        };

        Ok(MockPolymarketOrderStatusPayload {
            event_id: format!("evt_pm_order_status:{}", request.external_order_id),
            order_id: request.external_order_id.clone(),
            status: status.to_string(),
            observed_at: OffsetDateTime::now_utc(),
        })
    }

    pub fn reconcile_fill(
        &self,
        request: &MockPolymarketFillRequest,
    ) -> Result<MockPolymarketTradePayload> {
        if request.execution_request_id.trim().is_empty() {
            return Err(AppError::invalid_input(
                "POLYMARKET_EXECUTION_REQUEST_ID_REQUIRED",
                "execution_request_id must not be empty",
            ));
        }

        if request.connector_name != POLYMARKET_CONNECTOR_NAME {
            return Err(AppError::invalid_input(
                "POLYMARKET_CONNECTOR_UNSUPPORTED",
                format!(
                    "mock polymarket connector only handles connector_name={POLYMARKET_CONNECTOR_NAME}, got {}",
                    request.connector_name
                ),
            ));
        }

        if request.account_id.trim().is_empty() {
            return Err(AppError::invalid_input(
                "POLYMARKET_ACCOUNT_ID_REQUIRED",
                "account_id must not be empty",
            ));
        }

        if request.external_order_id.trim().is_empty() {
            return Err(AppError::invalid_input(
                "POLYMARKET_ORDER_ID_REQUIRED",
                "external_order_id must not be empty",
            ));
        }

        if request.market_id.trim().is_empty() {
            return Err(AppError::invalid_input(
                "POLYMARKET_MARKET_ID_REQUIRED",
                "market_id must not be empty",
            ));
        }

        if request.fill_price.value() <= Decimal::ZERO {
            return Err(AppError::invalid_input(
                "POLYMARKET_FILL_PRICE_INVALID",
                "mock polymarket fill price must be positive",
            ));
        }

        if request.total_quantity.value() <= Decimal::ZERO {
            return Err(AppError::invalid_input(
                "POLYMARKET_TOTAL_QUANTITY_INVALID",
                "mock polymarket total_quantity must be positive",
            ));
        }

        if request.already_filled_quantity.value() < Decimal::ZERO {
            return Err(AppError::invalid_input(
                "POLYMARKET_ALREADY_FILLED_QUANTITY_INVALID",
                "mock polymarket already_filled_quantity must be non-negative",
            ));
        }

        if request.already_filled_quantity.value() >= request.total_quantity.value() {
            return Err(AppError::conflict(
                "POLYMARKET_ORDER_ALREADY_FILLED",
                "mock polymarket order is already fully filled",
            ));
        }

        let remaining_quantity =
            request.total_quantity.value() - request.already_filled_quantity.value();
        let next_fill_quantity = if request.already_filled_quantity.value().is_zero()
            && remaining_quantity > Decimal::ONE
        {
            Decimal::ONE
        } else {
            remaining_quantity
        };
        let next_total_filled_quantity =
            request.already_filled_quantity.value() + next_fill_quantity;
        let size = Quantity::new(next_fill_quantity).map_err(|error| {
            AppError::internal(
                "POLYMARKET_FILL_QUANTITY_INVALID",
                format!("failed to build mock polymarket fill quantity: {error}"),
            )
        })?;

        Ok(MockPolymarketTradePayload {
            event_id: format!(
                "evt_pm_trade_fill:{}:{}",
                request.external_order_id,
                next_total_filled_quantity.normalize()
            ),
            order_id: request.external_order_id.clone(),
            account_id: request.account_id.clone(),
            trade_id: format!(
                "pm-trade:{}:{}:{}:{}",
                request.market_id,
                request.side.as_str(),
                request.external_order_id,
                next_total_filled_quantity.normalize()
            ),
            price: request.fill_price,
            size,
            fee: UsdAmount::new(Decimal::ZERO).map_err(|error| {
                AppError::internal(
                    "POLYMARKET_FILL_FEE_INVALID",
                    format!("failed to build mock polymarket fee amount: {error}"),
                )
            })?,
            executed_at: OffsetDateTime::now_utc(),
        })
    }
}
