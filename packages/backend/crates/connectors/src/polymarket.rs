use polyedge_domain::{
    AmbiguityLevel, AppError, MarketStatus, OrderStatus, Probability, Quantity, Result, SignalSide,
    TradabilityStatus, UsdAmount,
};
use polymarket_client_sdk::auth::state::{Authenticated, Unauthenticated};
use polymarket_client_sdk::auth::{Credentials, LocalSigner, Normal, Signer, Uuid};
use polymarket_client_sdk::clob::types::request::{
    OrderBookSummaryRequest, OrdersRequest, TradesRequest,
};
use polymarket_client_sdk::clob::types::response::OrderSummary;
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
const POLYMARKET_MIN_NOTIONAL_USD: Decimal = Decimal::ONE;

include!("polymarket/models.rs");
include!("polymarket/gamma.rs");
include!("polymarket/data_api.rs");
include!("polymarket/book.rs");
include!("polymarket/live.rs");
include!("polymarket/normalizers.rs");
include!("polymarket/helpers.rs");
include!("polymarket/tests.rs");
