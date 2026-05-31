// API and worker input structs deserialized from POST bodies or connector payloads.

#[derive(Debug, Clone, Deserialize)]
pub struct AddTrackedWalletInput {
    pub address: String,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub sizing_override: Option<CopySizingMode>,
    #[serde(default)]
    pub max_exposure_override: Option<Decimal>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct WalletActionInput {
    pub address: String,
}

#[derive(Debug, Clone)]
pub struct WalletFeedInput {
    pub address: String,
    pub activities: Vec<WalletActivityInput>,
    pub positions: Vec<WalletPositionInput>,
}

#[derive(Debug, Clone)]
pub struct WalletActivityInput {
    pub kind: String,
    pub side: String,
    pub asset: String,
    pub condition_id: String,
    pub outcome: String,
    pub title: String,
    pub slug: String,
    pub price: Decimal,
    pub size: Decimal,
    pub usdc_size: Decimal,
    pub transaction_hash: String,
    pub timestamp: OffsetDateTime,
}

#[derive(Debug, Clone)]
pub struct WalletPositionInput {
    pub asset: String,
    pub condition_id: String,
    pub outcome: String,
    pub title: String,
    pub slug: String,
    pub size: Decimal,
    pub avg_price: Decimal,
    pub cur_price: Decimal,
    pub realized_pnl: Decimal,
    pub percent_pnl: Decimal,
}
