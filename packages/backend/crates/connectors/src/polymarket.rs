use polyedge_domain::{
    AppError, OrderStatus, Probability, Quantity, Result, SignalSide, UsdAmount,
};
use polymarket_client_sdk::auth::state::Authenticated;
use polymarket_client_sdk::auth::{Credentials, LocalSigner, Normal, Signer, Uuid};
use polymarket_client_sdk::clob::types::request::{OrdersRequest, TradesRequest};
use polymarket_client_sdk::clob::types::{
    OrderStatusType as SdkOrderStatusType, OrderType, Side, SignatureType,
    TradeStatusType as SdkTradeStatusType,
};
use polymarket_client_sdk::clob::ws::Client as ClobWsClient;
use polymarket_client_sdk::clob::ws::types::response::{
    MakerOrder, OrderMessage as PolymarketWsOrderMessage,
    OrderMessageType as PolymarketWsOrderMessageType, TradeMessage as PolymarketWsTradeMessage,
    TradeMessageStatus as PolymarketWsTradeMessageStatus,
};
use polymarket_client_sdk::clob::{Client as ClobClient, Config as ClobConfig};
use polymarket_client_sdk::types::{Address, B256, U256};
use polymarket_client_sdk::ws::config::Config as WsConfig;
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
use std::str::FromStr;
use time::OffsetDateTime;
use tracing::warn;

pub const POLYMARKET_CONNECTOR_NAME: &str = "polymarket";
pub const POLYMARKET_ACCOUNT_ID: &str = "polymarket_account";
const POLYMARKET_MIN_NOTIONAL_USD: Decimal = Decimal::ONE;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolymarketSignatureScheme {
    Eoa,
    Proxy,
    GnosisSafe,
}

impl From<PolymarketSignatureScheme> for SignatureType {
    fn from(value: PolymarketSignatureScheme) -> Self {
        match value {
            PolymarketSignatureScheme::Eoa => SignatureType::Eoa,
            PolymarketSignatureScheme::Proxy => SignatureType::Proxy,
            PolymarketSignatureScheme::GnosisSafe => SignatureType::GnosisSafe,
        }
    }
}

#[derive(Debug, Clone)]
pub struct LivePolymarketConfig {
    pub account_id: String,
    pub clob_host: String,
    pub ws_host: String,
    pub chain_id: u64,
    pub signature_type: PolymarketSignatureScheme,
    pub funder: Option<String>,
    pub private_key: String,
    pub api_key: Option<String>,
    pub api_secret: Option<String>,
    pub api_passphrase: Option<String>,
}

#[derive(Debug, Clone)]
pub struct PolymarketMarketRefs {
    pub condition_id: String,
    pub yes_asset_id: String,
    pub no_asset_id: String,
}

impl PolymarketMarketRefs {
    pub fn asset_id_for_side(&self, side: SignalSide) -> Result<U256> {
        let raw = match side {
            SignalSide::Yes => &self.yes_asset_id,
            SignalSide::No => &self.no_asset_id,
        };

        parse_u256("polymarket_asset_id", raw, "POLYMARKET_ASSET_ID_INVALID")
    }

    pub fn condition_id(&self) -> Result<B256> {
        parse_b256(
            "polymarket_condition_id",
            &self.condition_id,
            "POLYMARKET_CONDITION_ID_INVALID",
        )
    }
}

#[derive(Debug, Clone)]
pub struct LivePolymarketOrderRequest {
    pub execution_request_id: String,
    pub connector_name: String,
    pub market_id: String,
    pub side: SignalSide,
    pub limit_price: Probability,
    pub quantity: Quantity,
    pub notional: UsdAmount,
    pub market_refs: PolymarketMarketRefs,
}

#[derive(Debug, Clone)]
pub struct LivePolymarketOrderStatusRequest {
    pub connector_name: String,
    pub external_order_id: String,
}

#[derive(Debug, Clone)]
pub struct LivePolymarketTradeSyncRequest {
    pub connector_name: String,
    pub account_id: String,
    pub external_order_id: String,
}

#[derive(Debug, Clone)]
pub struct LivePolymarketOrderAcceptance {
    pub order_id: String,
    pub accepted_at: OffsetDateTime,
}

#[derive(Debug, Clone)]
pub enum LivePolymarketExecutionOutcome {
    Accepted(LivePolymarketOrderAcceptance),
    Rejected(MockPolymarketOrderRejection),
}

#[derive(Debug, Clone)]
pub struct LivePolymarketConnector {
    client: ClobClient<Authenticated<Normal>>,
    private_key: String,
    chain_id: u64,
    account_id: String,
    ws_host: String,
}

#[derive(Debug, Clone)]
pub struct MockPolymarketOrderRequest {
    pub execution_request_id: String,
    pub connector_name: String,
    pub market_id: String,
    pub side: SignalSide,
    pub limit_price: Probability,
    pub quantity: Quantity,
    pub notional: UsdAmount,
}

#[derive(Debug, Clone)]
pub struct MockPolymarketOrderAcceptance {
    pub order_id: String,
    pub accepted_at: OffsetDateTime,
}

#[derive(Debug, Clone)]
pub struct MockPolymarketOrderRejection {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone)]
pub enum MockPolymarketExecutionOutcome {
    Accepted(MockPolymarketOrderAcceptance),
    Rejected(MockPolymarketOrderRejection),
}

#[derive(Debug, Clone)]
pub struct MockPolymarketOrderStatusRequest {
    pub connector_name: String,
    pub external_order_id: String,
    pub current_status: OrderStatus,
}

#[derive(Debug, Clone)]
pub struct MockPolymarketOrderStatusPayload {
    pub event_id: String,
    pub order_id: String,
    pub status: String,
    pub observed_at: OffsetDateTime,
}

#[derive(Debug, Clone)]
pub struct MockPolymarketFillRequest {
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
pub struct MockPolymarketTradePayload {
    pub event_id: String,
    pub order_id: String,
    pub account_id: String,
    pub trade_id: String,
    pub price: Probability,
    pub size: Quantity,
    pub fee: UsdAmount,
    pub executed_at: OffsetDateTime,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct MockPolymarketConnector;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectorOrderStatusUpdate {
    pub event_id: String,
    pub connector_name: String,
    pub external_order_id: String,
    pub status: OrderStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectorTradeFillUpdate {
    pub event_id: String,
    pub connector_name: String,
    pub external_order_id: String,
    pub account_id: String,
    pub external_trade_id: String,
    pub fill_price: Probability,
    pub filled_quantity: Quantity,
    pub fee: UsdAmount,
}

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
                MockPolymarketOrderRejection {
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
                MockPolymarketOrderRejection {
                    code: "POLYMARKET_ORDER_REJECTED".to_string(),
                    message: response
                        .error_msg
                        .unwrap_or_else(|| "Polymarket order was rejected".to_string()),
                },
            ));
        }

        match response.status {
            SdkOrderStatusType::Live
            | SdkOrderStatusType::Matched
            | SdkOrderStatusType::Delayed => Ok(LivePolymarketExecutionOutcome::Accepted(
                LivePolymarketOrderAcceptance {
                    order_id: response.order_id,
                    accepted_at: OffsetDateTime::now_utc(),
                },
            )),
            other => Ok(LivePolymarketExecutionOutcome::Rejected(
                MockPolymarketOrderRejection {
                    code: "POLYMARKET_ORDER_STATUS_UNSUPPORTED".to_string(),
                    message: format!(
                        "Polymarket returned unsupported post_order status={other} for execution_request_id={}",
                        request.execution_request_id
                    ),
                },
            )),
        }
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

pub fn normalize_polymarket_order_status_update(
    event_id: &str,
    external_order_id: &str,
    status: &str,
) -> Result<ConnectorOrderStatusUpdate> {
    let event_id = normalize_required("event_id", event_id, "POLYMARKET_EVENT_ID_REQUIRED")?;
    let external_order_id = normalize_required(
        "order_id",
        external_order_id,
        "POLYMARKET_ORDER_ID_REQUIRED",
    )?;
    let status = match status.trim().to_ascii_lowercase().as_str() {
        "live" => OrderStatus::Open,
        "canceled" | "cancelled" => OrderStatus::Canceled,
        "matched" | "delayed" => {
            return Err(AppError::invalid_input(
                "POLYMARKET_ORDER_STATUS_REQUIRES_TRADE_CALLBACK",
                "matched or delayed Polymarket order updates must be handled via the trade fill callback",
            ));
        }
        other => {
            return Err(AppError::invalid_input(
                "POLYMARKET_ORDER_STATUS_INVALID",
                format!("unsupported Polymarket order status: {other}"),
            ));
        }
    };

    Ok(ConnectorOrderStatusUpdate {
        event_id,
        connector_name: POLYMARKET_CONNECTOR_NAME.to_string(),
        external_order_id,
        status,
    })
}

pub fn normalize_polymarket_trade_fill_update(
    event_id: &str,
    external_order_id: &str,
    account_id: &str,
    external_trade_id: &str,
    fill_price: Probability,
    filled_quantity: Quantity,
    fee: UsdAmount,
) -> Result<ConnectorTradeFillUpdate> {
    let event_id = normalize_required("event_id", event_id, "POLYMARKET_EVENT_ID_REQUIRED")?;
    let external_order_id = normalize_required(
        "order_id",
        external_order_id,
        "POLYMARKET_ORDER_ID_REQUIRED",
    )?;
    let account_id =
        normalize_required("account_id", account_id, "POLYMARKET_ACCOUNT_ID_REQUIRED")?;
    let external_trade_id = normalize_required(
        "trade_id",
        external_trade_id,
        "POLYMARKET_TRADE_ID_REQUIRED",
    )?;

    if filled_quantity.value().is_zero() {
        return Err(AppError::invalid_input(
            "POLYMARKET_FILLED_QUANTITY_REQUIRED",
            "size must be greater than zero",
        ));
    }

    Ok(ConnectorTradeFillUpdate {
        event_id,
        connector_name: POLYMARKET_CONNECTOR_NAME.to_string(),
        external_order_id,
        account_id,
        external_trade_id,
        fill_price,
        filled_quantity,
        fee,
    })
}

pub fn normalize_polymarket_ws_order_message(
    message: &PolymarketWsOrderMessage,
) -> Result<Option<ConnectorOrderStatusUpdate>> {
    let external_order_id =
        normalize_required("order_id", &message.id, "POLYMARKET_ORDER_ID_REQUIRED")?;
    let mapped_status = match message.status.as_ref() {
        Some(
            SdkOrderStatusType::Live
            | SdkOrderStatusType::Unmatched
            | SdkOrderStatusType::Matched
            | SdkOrderStatusType::Delayed,
        ) => Some(OrderStatus::Open),
        Some(SdkOrderStatusType::Canceled) => Some(OrderStatus::Canceled),
        Some(SdkOrderStatusType::Unknown(_)) | Some(_) => None,
        None => match message.msg_type.as_ref() {
            Some(
                PolymarketWsOrderMessageType::Placement | PolymarketWsOrderMessageType::Update,
            ) => Some(OrderStatus::Open),
            Some(PolymarketWsOrderMessageType::Cancellation) => Some(OrderStatus::Canceled),
            Some(PolymarketWsOrderMessageType::Unknown(_)) | Some(_) | None => None,
        },
    };

    let Some(status) = mapped_status else {
        return Ok(None);
    };

    let status_marker = match status {
        OrderStatus::Open => "open",
        OrderStatus::Canceled => "canceled",
        _ => unreachable!("websocket order updates only map to open/canceled"),
    };
    let event_time = message
        .timestamp
        .map_or_else(|| "na".to_string(), |timestamp| timestamp.to_string());

    Ok(Some(ConnectorOrderStatusUpdate {
        event_id: format!("evt_pm_ws_order:{external_order_id}:{status_marker}:{event_time}"),
        connector_name: POLYMARKET_CONNECTOR_NAME.to_string(),
        external_order_id,
        status,
    }))
}

pub fn normalize_polymarket_ws_trade_message(
    message: &PolymarketWsTradeMessage,
    account_id: &str,
) -> Result<Vec<ConnectorTradeFillUpdate>> {
    if matches!(message.status, PolymarketWsTradeMessageStatus::Unknown(_)) {
        return Ok(Vec::new());
    }

    let trade_id = normalize_required("trade_id", &message.id, "POLYMARKET_TRADE_ID_REQUIRED")?;
    let account_id =
        normalize_required("account_id", account_id, "POLYMARKET_ACCOUNT_ID_REQUIRED")?;
    let fill_price = Probability::new(message.price).map_err(|error| {
        AppError::internal(
            "POLYMARKET_TRADE_PRICE_INVALID",
            format!("failed to decode websocket trade price for {trade_id}: {error}"),
        )
    })?;
    let filled_quantity = Quantity::new(message.size).map_err(|error| {
        AppError::internal(
            "POLYMARKET_TRADE_SIZE_INVALID",
            format!("failed to decode websocket trade size for {trade_id}: {error}"),
        )
    })?;
    let fee = UsdAmount::new(
        message.price * message.size * message.fee_rate_bps.unwrap_or(Decimal::ZERO)
            / Decimal::from(10_000_u64),
    )
    .map_err(|error| {
        AppError::internal(
            "POLYMARKET_TRADE_FEE_INVALID",
            format!("failed to decode websocket trade fee for {trade_id}: {error}"),
        )
    })?;

    let order_ids = candidate_order_ids_from_trade_message(
        message.taker_order_id.as_deref(),
        &message.maker_orders,
    );
    if order_ids.is_empty() {
        return Ok(Vec::new());
    }

    let multiple_orders = order_ids.len() > 1;
    let mut updates = Vec::with_capacity(order_ids.len());
    for order_id in order_ids {
        let external_trade_id = if multiple_orders {
            format!("{trade_id}:{order_id}")
        } else {
            trade_id.clone()
        };
        updates.push(normalize_polymarket_trade_fill_update(
            &format!("evt_pm_ws_trade:{trade_id}:{order_id}"),
            &order_id,
            &account_id,
            &external_trade_id,
            fill_price,
            filled_quantity,
            fee,
        )?);
    }

    Ok(updates)
}

fn normalize_required(field_name: &str, value: &str, error_code: &'static str) -> Result<String> {
    let normalized = value.trim().to_string();

    if normalized.is_empty() {
        return Err(AppError::invalid_input(
            error_code,
            format!("{field_name} must not be empty"),
        ));
    }

    Ok(normalized)
}

fn candidate_order_ids_from_trade_message(
    taker_order_id: Option<&str>,
    maker_orders: &[MakerOrder],
) -> Vec<String> {
    let mut order_ids = Vec::new();

    if let Some(order_id) = normalize_optional(taker_order_id) {
        order_ids.push(order_id);
    }

    for maker_order in maker_orders {
        let Some(order_id) = normalize_optional(Some(maker_order.order_id.as_str())) else {
            continue;
        };
        if !order_ids.iter().any(|candidate| candidate == &order_id) {
            order_ids.push(order_id);
        }
    }

    order_ids
}

fn normalize_optional(value: Option<&str>) -> Option<String> {
    value.and_then(|value| {
        let normalized = value.trim();
        if normalized.is_empty() {
            None
        } else {
            Some(normalized.to_string())
        }
    })
}

fn maybe_credentials(config: &LivePolymarketConfig) -> Result<Option<Credentials>> {
    let api_key = normalize_optional(config.api_key.as_deref());
    let api_secret = normalize_optional(config.api_secret.as_deref());
    let api_passphrase = normalize_optional(config.api_passphrase.as_deref());

    match (api_key, api_secret, api_passphrase) {
        (None, None, None) => Ok(None),
        (Some(key), Some(secret), Some(passphrase)) => {
            let key = Uuid::parse_str(&key).map_err(|error| {
                AppError::invalid_input(
                    "POLYMARKET_API_KEY_INVALID",
                    format!("invalid polymarket api_key: {error}"),
                )
            })?;
            Ok(Some(Credentials::new(key, secret, passphrase)))
        }
        _ => Err(AppError::invalid_input(
            "POLYMARKET_CREDENTIALS_INCOMPLETE",
            "api_key, api_secret, and api_passphrase must all be set together for live mode",
        )),
    }
}

fn parse_address(field_name: &str, value: &str, error_code: &'static str) -> Result<Address> {
    Address::from_str(value.trim()).map_err(|error| {
        AppError::invalid_input(error_code, format!("invalid {field_name}: {error}"))
    })
}

fn parse_b256(field_name: &str, value: &str, error_code: &'static str) -> Result<B256> {
    B256::from_str(value.trim()).map_err(|error| {
        AppError::invalid_input(error_code, format!("invalid {field_name}: {error}"))
    })
}

fn parse_u256(field_name: &str, value: &str, error_code: &'static str) -> Result<U256> {
    U256::from_str(value.trim()).map_err(|error| {
        AppError::invalid_input(error_code, format!("invalid {field_name}: {error}"))
    })
}

fn validate_live_order_request(request: &LivePolymarketOrderRequest) -> Result<()> {
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
                "polymarket live connector only handles connector_name={POLYMARKET_CONNECTOR_NAME}, got {}",
                request.connector_name
            ),
        ));
    }

    if request.market_id.trim().is_empty() {
        return Err(AppError::invalid_input(
            "POLYMARKET_MARKET_ID_REQUIRED",
            "market_id must not be empty",
        ));
    }

    if request.limit_price.value() <= Decimal::ZERO {
        return Err(AppError::invalid_input(
            "POLYMARKET_LIMIT_PRICE_INVALID",
            "polymarket live connector requires a positive limit price",
        ));
    }

    if request.quantity.value() <= Decimal::ZERO {
        return Err(AppError::invalid_input(
            "POLYMARKET_QUANTITY_INVALID",
            "polymarket live connector requires a positive quantity",
        ));
    }

    if request.notional.value() < POLYMARKET_MIN_NOTIONAL_USD {
        return Err(AppError::invalid_input(
            "POLYMARKET_NOTIONAL_INVALID",
            "polymarket live connector requires notional >= 1.00 USD",
        ));
    }

    Ok(())
}

fn validate_live_order_status_request(request: &LivePolymarketOrderStatusRequest) -> Result<()> {
    if request.connector_name != POLYMARKET_CONNECTOR_NAME {
        return Err(AppError::invalid_input(
            "POLYMARKET_CONNECTOR_UNSUPPORTED",
            format!(
                "polymarket live connector only handles connector_name={POLYMARKET_CONNECTOR_NAME}, got {}",
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

    Ok(())
}

fn validate_live_trade_sync_request(request: &LivePolymarketTradeSyncRequest) -> Result<()> {
    validate_live_order_status_request(&LivePolymarketOrderStatusRequest {
        connector_name: request.connector_name.clone(),
        external_order_id: request.external_order_id.clone(),
    })?;

    if request.account_id.trim().is_empty() {
        return Err(AppError::invalid_input(
            "POLYMARKET_ACCOUNT_ID_REQUIRED",
            "account_id must not be empty",
        ));
    }

    Ok(())
}

fn adjusted_order_quantity(limit_price: Probability, quantity: Quantity) -> Result<Quantity> {
    let rounded = quantity.value().round_dp(2);
    let adjusted = adjust_size_for_cost_precision(limit_price.value(), rounded);
    Quantity::new(adjusted).map_err(|error| {
        AppError::invalid_input(
            "POLYMARKET_QUANTITY_INVALID",
            format!("adjusted polymarket quantity is invalid: {error}"),
        )
    })
}

fn cost_precision_step(price: Decimal) -> (u64, u64, u64) {
    let scale = price.scale();
    let denom = 10_u64.pow(scale);
    let numer = (price * Decimal::from(denom)).round().to_u64().unwrap_or(1);

    if numer == 0 {
        return (1, 0, denom);
    }

    let gcd = greatest_common_divisor(numer, denom);
    (denom / gcd, numer, denom)
}

fn adjust_size_for_cost_precision(price: Decimal, size: Decimal) -> Decimal {
    let cost = price * size;
    if cost == cost.round_dp(2) {
        return size;
    }

    let (step, numer, _) = cost_precision_step(price);
    if numer == 0 {
        return size;
    }

    let size_as_hundredths = (size * Decimal::from(100_u64))
        .round()
        .to_u64()
        .unwrap_or(0);
    if step == 0 || size_as_hundredths < step {
        return Decimal::ZERO;
    }

    let rounded = (size_as_hundredths / step) * step;
    Decimal::new(rounded as i64, 2)
}

fn greatest_common_divisor(mut left: u64, mut right: u64) -> u64 {
    while right != 0 {
        let remainder = left % right;
        left = right;
        right = remainder;
    }

    left
}

fn trade_matches_order(
    trade: &polymarket_client_sdk::clob::types::response::TradeResponse,
    external_order_id: &str,
) -> bool {
    if trade.taker_order_id == external_order_id {
        return true;
    }

    trade
        .maker_orders
        .iter()
        .any(|maker_order| maker_order.order_id == external_order_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::Decimal;

    #[test]
    fn live_status_maps_to_open() {
        let update =
            normalize_polymarket_order_status_update("evt_1", "pm_ord_1", "live").expect("map");

        assert_eq!(update.connector_name, POLYMARKET_CONNECTOR_NAME);
        assert_eq!(update.status, OrderStatus::Open);
    }

    #[test]
    fn canceled_alias_maps_to_canceled() {
        let update = normalize_polymarket_order_status_update("evt_1", "pm_ord_1", "cancelled")
            .expect("map");

        assert_eq!(update.status, OrderStatus::Canceled);
    }

    #[test]
    fn matched_status_requires_trade_callback() {
        let error = normalize_polymarket_order_status_update("evt_1", "pm_ord_1", "matched")
            .expect_err("matched should be rejected");

        assert_eq!(
            error.code(),
            "POLYMARKET_ORDER_STATUS_REQUIRES_TRADE_CALLBACK"
        );
    }

    #[test]
    fn trade_fill_normalization_preserves_trade_fields() {
        let update = normalize_polymarket_trade_fill_update(
            "evt_trade_1",
            "pm_ord_1",
            "acct_1",
            "pm_trade_1",
            Probability::new(Decimal::new(48, 2)).expect("price"),
            Quantity::new(Decimal::ONE).expect("quantity"),
            UsdAmount::new(Decimal::ZERO).expect("fee"),
        )
        .expect("trade fill");

        assert_eq!(update.connector_name, POLYMARKET_CONNECTOR_NAME);
        assert_eq!(update.external_trade_id, "pm_trade_1");
        assert_eq!(update.filled_quantity.value(), Decimal::ONE);
    }

    #[test]
    fn websocket_cancellation_message_maps_to_canceled() {
        let message = PolymarketWsOrderMessage::builder()
            .id("pm_ord_1".to_string())
            .market(B256::ZERO)
            .asset_id(U256::ZERO)
            .side(Side::Buy)
            .price(Decimal::new(57, 2))
            .msg_type(PolymarketWsOrderMessageType::Cancellation)
            .outcome("YES".to_string())
            .original_size(Decimal::ONE)
            .size_matched(Decimal::ZERO)
            .timestamp(1_717_171_717_000)
            .build();
        let update = normalize_polymarket_ws_order_message(&message)
            .expect("normalize")
            .expect("mapped update");

        assert_eq!(update.status, OrderStatus::Canceled);
        assert_eq!(update.external_order_id, "pm_ord_1");
    }

    #[test]
    fn websocket_trade_message_generates_distinct_updates_per_order() {
        let maker_order = MakerOrder::builder()
            .asset_id(U256::ZERO)
            .matched_amount(Decimal::ONE)
            .order_id("pm_ord_maker".to_string())
            .outcome("YES".to_string())
            .owner(Uuid::nil())
            .price(Decimal::new(57, 2))
            .build();
        let message = PolymarketWsTradeMessage::builder()
            .id("pm_trade_1".to_string())
            .market(B256::ZERO)
            .asset_id(U256::ZERO)
            .side(Side::Buy)
            .size(Decimal::ONE)
            .price(Decimal::new(57, 2))
            .status(PolymarketWsTradeMessageStatus::Matched)
            .last_update(1_717_171_717_100)
            .matchtime(1_717_171_717_100)
            .timestamp(1_717_171_717_100)
            .outcome("YES".to_string())
            .taker_order_id("pm_ord_taker".to_string())
            .maker_orders(vec![maker_order])
            .fee_rate_bps(Decimal::new(25, 0))
            .build();
        let updates =
            normalize_polymarket_ws_trade_message(&message, "acct_polymarket").expect("normalize");

        assert_eq!(updates.len(), 2);
        assert_eq!(updates[0].external_order_id, "pm_ord_taker");
        assert_eq!(updates[1].external_order_id, "pm_ord_maker");
        assert_eq!(updates[0].external_trade_id, "pm_trade_1:pm_ord_taker");
        assert_eq!(updates[1].external_trade_id, "pm_trade_1:pm_ord_maker");
    }

    #[test]
    fn mock_connector_accepts_valid_order() {
        let connector = MockPolymarketConnector::new();
        let outcome = connector
            .submit(&MockPolymarketOrderRequest {
                execution_request_id: "exec_1".to_string(),
                connector_name: POLYMARKET_CONNECTOR_NAME.to_string(),
                market_id: "mkt_1".to_string(),
                side: SignalSide::Yes,
                limit_price: Probability::new(Decimal::new(48, 2)).expect("price"),
                quantity: Quantity::new(Decimal::new(3, 0)).expect("quantity"),
                notional: UsdAmount::new(Decimal::new(144, 2)).expect("notional"),
            })
            .expect("submit");

        match outcome {
            MockPolymarketExecutionOutcome::Accepted(acceptance) => {
                assert!(acceptance.order_id.starts_with("pm:mkt_1:yes:"));
            }
            MockPolymarketExecutionOutcome::Rejected(_) => panic!("expected acceptance"),
        }
    }
}
