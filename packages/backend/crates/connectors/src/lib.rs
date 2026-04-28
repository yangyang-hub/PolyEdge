#![allow(clippy::missing_const_for_fn)]

mod news;
mod polymarket;

use polyedge_domain::{
    AppError, OrderStatus, Probability, Quantity, Result, SignalSide, UsdAmount,
};
use rust_decimal::Decimal;
use time::OffsetDateTime;

pub use news::{ConnectorNewsItem, NewsSource, RssNewsConnector, RssNewsSourceConfig};
pub use polymarket::{
    ConnectorOrderStatusUpdate, ConnectorTradeFillUpdate, LivePolymarketConfig,
    LivePolymarketConnector, LivePolymarketExecutionOutcome, LivePolymarketOrderAcceptance,
    LivePolymarketOrderRequest, LivePolymarketOrderStatusRequest, LivePolymarketTradeSyncRequest,
    MockPolymarketConnector, MockPolymarketExecutionOutcome, MockPolymarketFillRequest,
    MockPolymarketOrderRequest, MockPolymarketOrderStatusPayload, MockPolymarketOrderStatusRequest,
    MockPolymarketTradePayload, POLYMARKET_ACCOUNT_ID, POLYMARKET_CONNECTOR_NAME,
    PolymarketMarketRefs, PolymarketSignatureScheme, normalize_polymarket_order_status_update,
    normalize_polymarket_trade_fill_update, normalize_polymarket_ws_order_message,
    normalize_polymarket_ws_trade_message,
};

pub const PAPER_EXECUTOR_NAME: &str = "paper_executor";
pub const PAPER_ACCOUNT_ID: &str = "paper_account";
const PAPER_MIN_NOTIONAL_USD: Decimal = Decimal::ONE;

#[derive(Debug, Clone)]
pub struct PaperOrderRequest {
    pub execution_request_id: String,
    pub connector_name: String,
    pub market_id: String,
    pub side: SignalSide,
    pub limit_price: Probability,
    pub quantity: Quantity,
    pub notional: UsdAmount,
}

#[derive(Debug, Clone)]
pub struct PaperOrderAcceptance {
    pub external_order_id: String,
    pub submitted_at: OffsetDateTime,
}

#[derive(Debug, Clone)]
pub struct PaperOrderRejection {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct PaperOrderStatusRequest {
    pub connector_name: String,
    pub external_order_id: String,
    pub current_status: OrderStatus,
}

#[derive(Debug, Clone)]
pub struct PaperOrderStatusSnapshot {
    pub external_order_id: String,
    pub status: OrderStatus,
    pub observed_at: OffsetDateTime,
}

#[derive(Debug, Clone)]
pub struct PaperFillRequest {
    pub execution_request_id: String,
    pub connector_name: String,
    pub account_id: String,
    pub external_order_id: String,
    pub market_id: String,
    pub side: SignalSide,
    pub fill_price: Probability,
    pub total_quantity: Quantity,
    pub already_filled_quantity: Quantity,
}

#[derive(Debug, Clone)]
pub struct PaperFillReceipt {
    pub account_id: String,
    pub external_trade_id: String,
    pub fill_price: Probability,
    pub filled_quantity: Quantity,
    pub total_filled_quantity: Quantity,
    pub fee: UsdAmount,
    pub executed_at: OffsetDateTime,
}

#[derive(Debug, Clone)]
pub enum PaperExecutionOutcome {
    Submitted(PaperOrderAcceptance),
    Rejected(PaperOrderRejection),
}

#[derive(Debug, Default, Clone, Copy)]
pub struct PaperExecutor;

impl PaperExecutor {
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    pub fn submit(&self, request: &PaperOrderRequest) -> Result<PaperExecutionOutcome> {
        if request.execution_request_id.trim().is_empty() {
            return Err(AppError::invalid_input(
                "PAPER_EXECUTION_REQUEST_ID_REQUIRED",
                "execution_request_id must not be empty",
            ));
        }

        if request.connector_name != PAPER_EXECUTOR_NAME {
            return Ok(PaperExecutionOutcome::Rejected(PaperOrderRejection {
                code: "PAPER_CONNECTOR_UNSUPPORTED".to_string(),
                message: format!(
                    "paper executor only handles connector_name={PAPER_EXECUTOR_NAME}, got {}",
                    request.connector_name
                ),
            }));
        }

        if request.market_id.trim().is_empty() {
            return Err(AppError::invalid_input(
                "PAPER_MARKET_ID_REQUIRED",
                "market_id must not be empty",
            ));
        }

        if request.limit_price.value() <= Decimal::ZERO {
            return Ok(PaperExecutionOutcome::Rejected(PaperOrderRejection {
                code: "PAPER_LIMIT_PRICE_INVALID".to_string(),
                message: "paper executor requires a positive limit price".to_string(),
            }));
        }

        if request.quantity.value() <= Decimal::ZERO {
            return Ok(PaperExecutionOutcome::Rejected(PaperOrderRejection {
                code: "PAPER_QUANTITY_INVALID".to_string(),
                message: "paper executor requires a positive quantity".to_string(),
            }));
        }

        if request.notional.value() < PAPER_MIN_NOTIONAL_USD {
            return Ok(PaperExecutionOutcome::Rejected(PaperOrderRejection {
                code: "PAPER_MIN_NOTIONAL_NOT_MET".to_string(),
                message: format!(
                    "paper executor requires notional >= 1.00 USD, got {}",
                    request.notional
                ),
            }));
        }

        Ok(PaperExecutionOutcome::Submitted(PaperOrderAcceptance {
            external_order_id: format!(
                "paper:{}:{}:{}",
                request.market_id,
                request.side.as_str(),
                request.execution_request_id
            ),
            submitted_at: OffsetDateTime::now_utc(),
        }))
    }

    pub fn reconcile_fill(&self, request: &PaperFillRequest) -> Result<PaperFillReceipt> {
        if request.execution_request_id.trim().is_empty() {
            return Err(AppError::invalid_input(
                "PAPER_EXECUTION_REQUEST_ID_REQUIRED",
                "execution_request_id must not be empty",
            ));
        }

        if request.connector_name != PAPER_EXECUTOR_NAME {
            return Err(AppError::invalid_input(
                "PAPER_CONNECTOR_UNSUPPORTED",
                format!(
                    "paper executor only handles connector_name={PAPER_EXECUTOR_NAME}, got {}",
                    request.connector_name
                ),
            ));
        }

        if request.account_id.trim().is_empty() {
            return Err(AppError::invalid_input(
                "PAPER_ACCOUNT_ID_REQUIRED",
                "account_id must not be empty",
            ));
        }

        if request.external_order_id.trim().is_empty() {
            return Err(AppError::invalid_input(
                "PAPER_EXTERNAL_ORDER_ID_REQUIRED",
                "external_order_id must not be empty",
            ));
        }

        if request.market_id.trim().is_empty() {
            return Err(AppError::invalid_input(
                "PAPER_MARKET_ID_REQUIRED",
                "market_id must not be empty",
            ));
        }

        if request.fill_price.value() <= Decimal::ZERO {
            return Err(AppError::invalid_input(
                "PAPER_FILL_PRICE_INVALID",
                "paper fill price must be positive",
            ));
        }

        if request.total_quantity.value() <= Decimal::ZERO {
            return Err(AppError::invalid_input(
                "PAPER_TOTAL_QUANTITY_INVALID",
                "paper total_quantity must be positive",
            ));
        }

        if request.already_filled_quantity.value() < Decimal::ZERO {
            return Err(AppError::invalid_input(
                "PAPER_ALREADY_FILLED_QUANTITY_INVALID",
                "paper already_filled_quantity must be non-negative",
            ));
        }

        if request.already_filled_quantity.value() >= request.total_quantity.value() {
            return Err(AppError::conflict(
                "PAPER_ORDER_ALREADY_FILLED",
                "paper order is already fully filled",
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

        Ok(PaperFillReceipt {
            account_id: request.account_id.clone(),
            external_trade_id: format!(
                "paper-trade:{}:{}:{}:{}",
                request.market_id,
                request.side.as_str(),
                request.external_order_id,
                next_total_filled_quantity.normalize()
            ),
            fill_price: request.fill_price,
            filled_quantity: Quantity::new(next_fill_quantity).map_err(|error| {
                AppError::internal(
                    "PAPER_FILL_QUANTITY_INVALID",
                    format!("failed to build paper fill quantity: {error}"),
                )
            })?,
            total_filled_quantity: Quantity::new(next_total_filled_quantity).map_err(|error| {
                AppError::internal(
                    "PAPER_TOTAL_FILLED_QUANTITY_INVALID",
                    format!("failed to build paper total filled quantity: {error}"),
                )
            })?,
            fee: UsdAmount::new(Decimal::ZERO).map_err(|error| {
                AppError::internal(
                    "PAPER_FILL_FEE_INVALID",
                    format!("failed to build paper fee amount: {error}"),
                )
            })?,
            executed_at: OffsetDateTime::now_utc(),
        })
    }

    pub fn poll_order_status(
        &self,
        request: &PaperOrderStatusRequest,
    ) -> Result<PaperOrderStatusSnapshot> {
        if request.connector_name != PAPER_EXECUTOR_NAME {
            return Err(AppError::invalid_input(
                "PAPER_CONNECTOR_UNSUPPORTED",
                format!(
                    "paper executor only handles connector_name={PAPER_EXECUTOR_NAME}, got {}",
                    request.connector_name
                ),
            ));
        }

        if request.external_order_id.trim().is_empty() {
            return Err(AppError::invalid_input(
                "PAPER_EXTERNAL_ORDER_ID_REQUIRED",
                "external_order_id must not be empty",
            ));
        }

        let status = match request.current_status {
            OrderStatus::Submitted => OrderStatus::Open,
            current_status => current_status,
        };

        Ok(PaperOrderStatusSnapshot {
            external_order_id: request.external_order_id.clone(),
            status,
            observed_at: OffsetDateTime::now_utc(),
        })
    }
}
