use alloy_network::TransactionBuilder;
use alloy_primitives::{Address as AlloyAddress, Bytes as AlloyBytes, U256 as AlloyU256};
use alloy_provider::{Provider, ProviderBuilder};
use alloy_rpc_types_eth::TransactionRequest as AlloyTransactionRequest;
use alloy_signer::Signer as AlloySigner;
use alloy_signer_local::PrivateKeySigner;
use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE;
use hmac::{Hmac, Mac as _};
use polyedge_domain::{
    AmbiguityLevel, AppError, MarketStatus, OrderStatus, Probability, Quantity, Result, SignalSide,
    TradabilityStatus, UsdAmount,
};
use polymarket_client_sdk::auth::state::{Authenticated, Unauthenticated};
use polymarket_client_sdk::auth::{Credentials, ExposeSecret, LocalSigner, Normal, Uuid};
use polymarket_client_sdk::clob::types::request::{
    BalanceAllowanceRequest, OrderBookSummaryRequest, OrdersRequest, TradesRequest,
    UpdateBalanceAllowanceRequest,
};
use polymarket_client_sdk::clob::types::response::{
    BalanceAllowanceResponse, OrderSummary, TradeResponse,
};
use polymarket_client_sdk::clob::types::{
    AssetType, OrderStatusType as SdkOrderStatusType, OrderType, Side, SignatureType,
    TradeStatusType as SdkTradeStatusType,
};
use polymarket_client_sdk::clob::ws::Client as ClobWsClient;
use polymarket_client_sdk::clob::ws::types::response::{
    MakerOrder, OrderMessage as PolymarketWsOrderMessage,
    OrderMessageType as PolymarketWsOrderMessageType, TradeMessage as PolymarketWsTradeMessage,
    TradeMessageStatus as PolymarketWsTradeMessageStatus,
};
use polymarket_client_sdk::clob::{Client as ClobClient, Config as ClobConfig};
use polymarket_client_sdk::error::{Error as PolymarketSdkError, Status as PolymarketSdkStatus};
use polymarket_client_sdk::types::{Address, B256, U256};
use polymarket_client_sdk::ws::config::Config as WsConfig;
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
use sha2::Sha256;
use std::str::FromStr;
use std::time::Duration;
use time::OffsetDateTime;
use tracing::warn;

pub const POLYMARKET_CONNECTOR_NAME: &str = "polymarket";
const POLYMARKET_MIN_NOTIONAL_USD: Decimal = Decimal::ONE;
const CLOB_TERMINAL_CURSOR: &str = "LTE=";
const CLOB_MAX_PAGES: usize = 1000;

include!("polymarket/models.rs");
include!("polymarket/gamma.rs");
include!("polymarket/data_api.rs");
include!("polymarket/chain.rs");
include!("polymarket/book.rs");
include!("polymarket/live.rs");
include!("polymarket/normalizers.rs");
include!("polymarket/helpers.rs");
include!("polymarket/tests.rs");
