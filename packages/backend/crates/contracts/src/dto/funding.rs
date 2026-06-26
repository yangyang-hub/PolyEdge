#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FundingTokenData {
    pub id: String,
    pub symbol: String,
    pub name: String,
    pub address: String,
    pub decimals: u8,
    pub min_transfer_amount: Decimal,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub balance: Option<Decimal>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FundingStatusData {
    pub enabled: bool,
    pub source_address: Option<String>,
    pub polymarket_wallet_address: Option<String>,
    pub chain_id: u64,
    pub max_transfer_amount: Decimal,
    pub tokens: Vec<FundingTokenData>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub configuration_error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub balance_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FundingTransferRequest {
    pub token_id: String,
    pub amount: Decimal,
    pub confirmed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FundingTransferData {
    pub tx_hash: String,
    pub source_address: String,
    pub polymarket_wallet_address: String,
    pub bridge_deposit_address: String,
    pub token_id: String,
    pub token_symbol: String,
    pub token_address: String,
    pub amount: Decimal,
    pub amount_units: String,
    pub chain_id: u64,
    pub replayed: bool,
}
