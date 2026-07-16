#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolymarketSignatureScheme {
    Eoa,
    Proxy,
    GnosisSafe,
    Poly1271,
}

impl From<PolymarketSignatureScheme> for SignatureType {
    fn from(value: PolymarketSignatureScheme) -> Self {
        match value {
            PolymarketSignatureScheme::Eoa => SignatureType::Eoa,
            PolymarketSignatureScheme::Proxy => SignatureType::Proxy,
            PolymarketSignatureScheme::GnosisSafe => SignatureType::GnosisSafe,
            PolymarketSignatureScheme::Poly1271 => SignatureType::Poly1271,
        }
    }
}

#[derive(Clone)]
pub struct LivePolymarketConfig {
    pub account_id: String,
    pub clob_host: String,
    pub chain_id: u64,
    pub signature_type: PolymarketSignatureScheme,
    pub funder: Option<String>,
    pub private_key: String,
    pub api_key: Option<String>,
    pub api_secret: Option<String>,
    pub api_passphrase: Option<String>,
}

impl fmt::Debug for LivePolymarketConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("LivePolymarketConfig")
            .field("account_id", &self.account_id)
            .field("clob_host", &self.clob_host)
            .field("chain_id", &self.chain_id)
            .field("signature_type", &self.signature_type)
            .field("funder", &self.funder)
            .field("private_key", &"[REDACTED]")
            .field("api_key", &self.api_key.as_ref().map(|_| "[REDACTED]"))
            .field("api_secret", &self.api_secret.as_ref().map(|_| "[REDACTED]"))
            .field("api_passphrase", &self.api_passphrase.as_ref().map(|_| "[REDACTED]"))
            .finish()
    }
}

impl Drop for LivePolymarketConfig {
    fn drop(&mut self) {
        self.private_key.zeroize();
        if let Some(value) = self.api_key.as_mut() {
            value.zeroize();
        }
        if let Some(value) = self.api_secret.as_mut() {
            value.zeroize();
        }
        if let Some(value) = self.api_passphrase.as_mut() {
            value.zeroize();
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolymarketTokenOrderSide {
    Buy,
    Sell,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolymarketOrderLifecycleStatus {
    Open,
    PartiallyFilled,
    Filled,
    Cancelled,
    Rejected,
    Expired,
    Unknown,
}

#[derive(Debug, Clone)]
pub struct PolymarketOrderSnapshot {
    pub external_order_id: String,
    pub status: PolymarketOrderLifecycleStatus,
    pub filled_quantity: Decimal,
}

/// An open order returned from the Polymarket CLOB API, converted to a
/// crate-local type so downstream crates do not need to depend on the SDK.
#[derive(Debug, Clone)]
pub struct PolymarketOpenOrder {
    pub id: String,
    pub market: String,
    pub asset_id: String,
    pub side: PolymarketTokenOrderSide,
    pub original_size: Decimal,
    pub size_matched: Decimal,
    pub price: Decimal,
    pub outcome: String,
    pub lifecycle_status: PolymarketOrderLifecycleStatus,
    pub created_at: OffsetDateTime,
}

#[derive(Debug, Clone)]
pub struct LivePolymarketTokenOrderRequest {
    pub client_order_id: String,
    pub connector_name: String,
    pub token_id: String,
    pub side: PolymarketTokenOrderSide,
    pub limit_price: Probability,
    pub quantity: Quantity,
    pub post_only: bool,
}

#[derive(Debug, Clone)]
pub struct LivePolymarketCancelOrderRequest {
    pub connector_name: String,
    pub external_order_id: String,
}

#[derive(Debug, Clone)]
pub struct LivePolymarketOrderAcceptance {
    pub order_id: String,
    pub status: PolymarketAcceptedOrderStatus,
    pub submitted_quantity: Quantity,
    pub accepted_at: OffsetDateTime,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolymarketAcceptedOrderStatus {
    Live,
    Matched,
    Delayed,
    Unmatched,
    Canceled,
    Unknown,
}

impl PolymarketAcceptedOrderStatus {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Live => "live",
            Self::Matched => "matched",
            Self::Delayed => "delayed",
            Self::Unmatched => "unmatched",
            Self::Canceled => "canceled",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone)]
pub struct LivePolymarketCancelAcceptance {
    pub external_order_id: String,
    pub cancelled_at: OffsetDateTime,
}

#[derive(Debug, Clone)]
pub enum LivePolymarketExecutionOutcome {
    Accepted(LivePolymarketOrderAcceptance),
    Rejected(PolymarketOrderRejection),
}

#[derive(Debug, Clone)]
pub enum LivePolymarketCancelOutcome {
    Accepted(LivePolymarketCancelAcceptance),
    Rejected(PolymarketOrderRejection),
}

#[derive(Debug, Clone)]
pub struct PolymarketOrderRejection {
    pub code: String,
    pub message: String,
}

#[derive(Clone)]
pub struct LivePolymarketConnector {
    client: ClobClient<Authenticated<Normal>>,
    private_key: String,
    chain_id: u64,
    signature_type: PolymarketSignatureScheme,
}

impl fmt::Debug for LivePolymarketConnector {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("LivePolymarketConnector")
            .field("client", &"[AUTHENTICATED CLIENT REDACTED]")
            .field("private_key", &"[REDACTED]")
            .field("chain_id", &self.chain_id)
            .field("signature_type", &self.signature_type)
            .finish()
    }
}

impl Drop for LivePolymarketConnector {
    fn drop(&mut self) {
        self.private_key.zeroize();
    }
}

#[cfg(test)]
mod secret_debug_tests {
    use super::*;

    #[test]
    fn live_config_debug_redacts_credentials() {
        let config = LivePolymarketConfig {
            account_id: "account".to_string(),
            clob_host: "https://clob.example".to_string(),
            chain_id: 137,
            signature_type: PolymarketSignatureScheme::Eoa,
            funder: None,
            private_key: "private-key-material".to_string(),
            api_key: Some("api-key-material".to_string()),
            api_secret: Some("api-secret-material".to_string()),
            api_passphrase: Some("api-passphrase-material".to_string()),
        };
        let debug = format!("{config:?}");
        assert!(debug.contains("[REDACTED]"));
        assert!(!debug.contains("private-key-material"));
        assert!(!debug.contains("api-secret-material"));
        assert!(!debug.contains("api-passphrase-material"));
    }
}
