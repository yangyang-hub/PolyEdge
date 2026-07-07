#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HighProbabilityMode {
    Observe,
    Paper,
    LiveGuarded,
}

impl HighProbabilityMode {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Observe => "observe",
            Self::Paper => "paper",
            Self::LiveGuarded => "live_guarded",
        }
    }
}

impl FromStr for HighProbabilityMode {
    type Err = AppError;

    fn from_str(value: &str) -> Result<Self> {
        match value {
            "observe" => Ok(Self::Observe),
            "paper" => Ok(Self::Paper),
            "live_guarded" => Ok(Self::LiveGuarded),
            other => Err(AppError::invalid_input(
                "HIGH_PROBABILITY_MODE_INVALID",
                format!("unknown high probability mode: {other}"),
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HighProbabilitySampleOutcome {
    Win,
    Loss,
    Voided,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HighProbabilityMarketOutcomeStatus {
    Unresolved,
    Resolved,
    Voided,
    Ambiguous,
}

impl HighProbabilityMarketOutcomeStatus {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Unresolved => "unresolved",
            Self::Resolved => "resolved",
            Self::Voided => "voided",
            Self::Ambiguous => "ambiguous",
        }
    }
}

impl FromStr for HighProbabilityMarketOutcomeStatus {
    type Err = AppError;

    fn from_str(value: &str) -> Result<Self> {
        match value {
            "unresolved" => Ok(Self::Unresolved),
            "resolved" => Ok(Self::Resolved),
            "voided" => Ok(Self::Voided),
            "ambiguous" => Ok(Self::Ambiguous),
            other => Err(AppError::invalid_input(
                "HIGH_PROBABILITY_MARKET_OUTCOME_STATUS_INVALID",
                format!("unknown high probability market outcome status: {other}"),
            )),
        }
    }
}

impl HighProbabilitySampleOutcome {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Win => "win",
            Self::Loss => "loss",
            Self::Voided => "voided",
            Self::Unknown => "unknown",
        }
    }

    #[must_use]
    pub const fn is_settled_for_stats(self) -> bool {
        matches!(self, Self::Win | Self::Loss)
    }
}

impl FromStr for HighProbabilitySampleOutcome {
    type Err = AppError;

    fn from_str(value: &str) -> Result<Self> {
        match value {
            "win" => Ok(Self::Win),
            "loss" => Ok(Self::Loss),
            "voided" => Ok(Self::Voided),
            "unknown" => Ok(Self::Unknown),
            other => Err(AppError::invalid_input(
                "HIGH_PROBABILITY_SAMPLE_OUTCOME_INVALID",
                format!("unknown high probability sample outcome: {other}"),
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HighProbabilityTriggerKind {
    FirstTouch,
    Sustained,
    ReEntry,
}

impl HighProbabilityTriggerKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::FirstTouch => "first_touch",
            Self::Sustained => "sustained",
            Self::ReEntry => "re_entry",
        }
    }
}

impl FromStr for HighProbabilityTriggerKind {
    type Err = AppError;

    fn from_str(value: &str) -> Result<Self> {
        match value {
            "first_touch" => Ok(Self::FirstTouch),
            "sustained" => Ok(Self::Sustained),
            "re_entry" => Ok(Self::ReEntry),
            other => Err(AppError::invalid_input(
                "HIGH_PROBABILITY_TRIGGER_KIND_INVALID",
                format!("unknown high probability trigger kind: {other}"),
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HighProbabilityDecision {
    Allow,
    Reject,
    Skip,
}

impl HighProbabilityDecision {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Allow => "allow",
            Self::Reject => "reject",
            Self::Skip => "skip",
        }
    }
}

impl FromStr for HighProbabilityDecision {
    type Err = AppError;

    fn from_str(value: &str) -> Result<Self> {
        match value {
            "allow" => Ok(Self::Allow),
            "reject" => Ok(Self::Reject),
            "skip" => Ok(Self::Skip),
            other => Err(AppError::invalid_input(
                "HIGH_PROBABILITY_DECISION_INVALID",
                format!("unknown high probability decision: {other}"),
            )),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HighProbabilityConfig {
    pub enabled: bool,
    pub mode: HighProbabilityMode,
    pub market_scope: String,
    pub model_version: String,
    pub min_required_edge: Decimal,
    pub fee_buffer: Decimal,
    pub default_risk_margin: Decimal,
    pub min_confidence: Decimal,
    pub min_bucket_samples: u64,
    pub max_spread_cents: Decimal,
    pub min_depth_usd: Decimal,
    pub max_single_trade_usd: Decimal,
    pub max_single_market_exposure_usd: Decimal,
    pub max_daily_new_notional_usd: Decimal,
    pub conservative_kelly_multiplier: Decimal,
    pub excluded_risk_tags: Vec<String>,
    // Fair value provider configuration. The provider turns bucket stats + the
    // current orderbook into a conservative `fair_yes_low/mid/high` snapshot for
    // the (future) Rewards market maker. It never quotes, sizes or trades.
    pub fair_value_enabled: bool,
    pub fair_value_ttl_sec: i64,
    pub fair_value_market_weight: Decimal,
    pub fair_value_base_rate_weight: Decimal,
    pub fair_value_target_sample_count: u64,
    pub fair_value_max_uncertainty_cents: Decimal,
    pub fair_value_stale_book_ms: i64,
}

impl Default for HighProbabilityConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            mode: HighProbabilityMode::Observe,
            market_scope: "rewards_only".to_string(),
            model_version: HIGH_PROBABILITY_BUCKET_MODEL_VERSION.to_string(),
            min_required_edge: Decimal::new(3, 2),
            fee_buffer: Decimal::new(5, 3),
            default_risk_margin: Decimal::new(2, 2),
            min_confidence: Decimal::new(60, 2),
            min_bucket_samples: HIGH_PROBABILITY_MIN_BUCKET_SAMPLES,
            max_spread_cents: Decimal::new(3, 0),
            min_depth_usd: Decimal::new(50, 0),
            max_single_trade_usd: Decimal::new(25, 0),
            max_single_market_exposure_usd: Decimal::new(50, 0),
            max_daily_new_notional_usd: Decimal::new(100, 0),
            conservative_kelly_multiplier: Decimal::new(10, 2),
            excluded_risk_tags: vec![
                "ambiguous_rules".to_string(),
                "subjective_resolution".to_string(),
            ],
            fair_value_enabled: false,
            fair_value_ttl_sec: 300,
            fair_value_market_weight: Decimal::new(25, 2),
            fair_value_base_rate_weight: Decimal::new(75, 2),
            fair_value_target_sample_count: 200,
            fair_value_max_uncertainty_cents: Decimal::new(8, 0),
            fair_value_stale_book_ms: 60_000,
        }
    }
}

impl HighProbabilityConfig {
    #[must_use]
    pub fn normalized(mut self) -> Self {
        self.market_scope = non_empty_or(self.market_scope, "rewards_only");
        self.model_version =
            non_empty_or(self.model_version, HIGH_PROBABILITY_BUCKET_MODEL_VERSION);
        self.min_required_edge = clamp_decimal(self.min_required_edge, Decimal::ZERO, Decimal::ONE);
        self.fee_buffer = clamp_decimal(self.fee_buffer, Decimal::ZERO, Decimal::ONE);
        self.default_risk_margin =
            clamp_decimal(self.default_risk_margin, Decimal::ZERO, Decimal::ONE);
        self.min_confidence = clamp_decimal(self.min_confidence, Decimal::ZERO, Decimal::ONE);
        self.min_bucket_samples = self.min_bucket_samples.max(1);
        self.max_spread_cents = self.max_spread_cents.max(Decimal::ZERO);
        self.min_depth_usd = self.min_depth_usd.max(Decimal::ZERO);
        self.max_single_trade_usd = self.max_single_trade_usd.max(Decimal::ZERO);
        self.max_single_market_exposure_usd =
            self.max_single_market_exposure_usd.max(Decimal::ZERO);
        self.max_daily_new_notional_usd = self.max_daily_new_notional_usd.max(Decimal::ZERO);
        self.conservative_kelly_multiplier = clamp_decimal(
            self.conservative_kelly_multiplier,
            Decimal::ZERO,
            Decimal::ONE,
        );
        self.excluded_risk_tags = normalize_string_list(self.excluded_risk_tags);
        self.fair_value_ttl_sec = self.fair_value_ttl_sec.max(0);
        self.fair_value_market_weight =
            clamp_decimal(self.fair_value_market_weight, Decimal::ZERO, Decimal::ONE);
        self.fair_value_base_rate_weight =
            clamp_decimal(self.fair_value_base_rate_weight, Decimal::ZERO, Decimal::ONE);
        self.fair_value_target_sample_count = self.fair_value_target_sample_count.max(1);
        self.fair_value_max_uncertainty_cents = self
            .fair_value_max_uncertainty_cents
            .max(Decimal::ZERO)
            .min(Decimal::new(100, 0));
        self.fair_value_stale_book_ms = self.fair_value_stale_book_ms.max(0);
        self
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HighProbabilitySample {
    pub id: i64,
    pub condition_id: String,
    pub token_id: String,
    pub side: String,
    pub sampled_at: OffsetDateTime,
    pub trigger_kind: HighProbabilityTriggerKind,
    pub executable_price: Decimal,
    pub price_bucket: String,
    pub market_type: String,
    pub time_to_resolution_bucket: Option<String>,
    pub liquidity_bucket: Option<String>,
    pub spread_bucket: Option<String>,
    pub path_features: Value,
    pub risk_tags: Vec<String>,
    pub outcome: HighProbabilitySampleOutcome,
    pub settlement_pnl: Option<Decimal>,
    pub max_drawdown_cents: Option<Decimal>,
    pub hold_seconds: Option<i64>,
    pub created_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HighProbabilityMarketOutcome {
    pub condition_id: String,
    pub status: HighProbabilityMarketOutcomeStatus,
    pub winning_token_id: Option<String>,
    pub resolved_at: Option<OffsetDateTime>,
    pub market_type: String,
    pub risk_tags: Vec<String>,
    pub label_source: String,
    pub raw: Value,
    pub updated_at: OffsetDateTime,
}

impl HighProbabilityMarketOutcome {
    #[must_use]
    pub fn normalized(mut self) -> Self {
        self.condition_id = self.condition_id.trim().to_string();
        self.winning_token_id = normalize_optional_string(self.winning_token_id);
        self.market_type = non_empty_or(self.market_type, "unknown");
        self.risk_tags = normalize_string_list(self.risk_tags);
        self.label_source = non_empty_or(self.label_source, "manual");
        self
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HighProbabilityRewardCandleSampleInput {
    pub condition_id: String,
    pub token_id: String,
    pub outcome: String,
    pub bucket_start: OffsetDateTime,
    pub close: Decimal,
    pub spread_cents_close: Decimal,
    pub market_type: String,
    pub liquidity_usd: Option<Decimal>,
    pub resolved_at: Option<OffsetDateTime>,
    pub outcome_status: HighProbabilityMarketOutcomeStatus,
    pub winning_token_id: Option<String>,
    pub risk_tags: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HighProbabilityObserveCandidate {
    pub condition_id: String,
    pub token_id: String,
    pub outcome: String,
    pub observed_at: OffsetDateTime,
    pub reference_price: Decimal,
    pub reference_spread_cents: Decimal,
    pub market_type: String,
    pub liquidity_usd: Option<Decimal>,
    pub end_at: Option<OffsetDateTime>,
    pub risk_tags: Vec<String>,
}

impl HighProbabilityObserveCandidate {
    #[must_use]
    pub fn normalized(mut self) -> Self {
        self.condition_id = self.condition_id.trim().to_string();
        self.token_id = self.token_id.trim().to_string();
        self.outcome = self.outcome.trim().to_ascii_lowercase();
        self.reference_price = clamp_decimal(self.reference_price, Decimal::ZERO, Decimal::ONE);
        self.reference_spread_cents = self.reference_spread_cents.max(Decimal::ZERO);
        self.market_type = non_empty_or(self.market_type, "unknown");
        self.risk_tags = normalize_string_list(self.risk_tags);
        self
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HighProbabilityOrderbookQuote {
    pub token_id: String,
    pub best_bid: Option<Decimal>,
    pub best_ask: Option<Decimal>,
    pub ask_depth_usd: Option<Decimal>,
    pub confirmed_at_ms: Option<i64>,
}

impl HighProbabilityOrderbookQuote {
    #[must_use]
    pub fn normalized(mut self) -> Self {
        self.token_id = self.token_id.trim().to_string();
        self.best_bid = self
            .best_bid
            .map(|value| clamp_decimal(value, Decimal::ZERO, Decimal::ONE));
        self.best_ask = self
            .best_ask
            .map(|value| clamp_decimal(value, Decimal::ZERO, Decimal::ONE));
        self.ask_depth_usd = self.ask_depth_usd.map(|value| value.max(Decimal::ZERO));
        self
    }
}

impl HighProbabilityRewardCandleSampleInput {
    #[must_use]
    pub fn normalized(mut self) -> Self {
        self.condition_id = self.condition_id.trim().to_string();
        self.token_id = self.token_id.trim().to_string();
        self.outcome = self.outcome.trim().to_ascii_lowercase();
        self.close = clamp_decimal(self.close, Decimal::ZERO, Decimal::ONE);
        self.spread_cents_close = self.spread_cents_close.max(Decimal::ZERO);
        self.market_type = non_empty_or(self.market_type, "unknown");
        self.winning_token_id = normalize_optional_string(self.winning_token_id);
        self.risk_tags = normalize_string_list(self.risk_tags);
        self
    }
}

impl HighProbabilitySample {
    #[must_use]
    pub fn normalized(mut self) -> Self {
        self.condition_id = self.condition_id.trim().to_string();
        self.token_id = self.token_id.trim().to_string();
        self.side = self.side.trim().to_ascii_lowercase();
        self.price_bucket = non_empty_or(self.price_bucket, "unknown");
        self.market_type = non_empty_or(self.market_type, "unknown");
        self.time_to_resolution_bucket = normalize_optional_string(self.time_to_resolution_bucket);
        self.liquidity_bucket = normalize_optional_string(self.liquidity_bucket);
        self.spread_bucket = normalize_optional_string(self.spread_bucket);
        self.risk_tags = normalize_string_list(self.risk_tags);
        self.executable_price = clamp_decimal(self.executable_price, Decimal::ZERO, Decimal::ONE);
        self
    }

    #[must_use]
    pub fn is_settled_for_stats(&self) -> bool {
        self.outcome.is_settled_for_stats()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HighProbabilitySampleQuery {
    pub outcome: Option<HighProbabilitySampleOutcome>,
    pub market_type: Option<String>,
    pub limit: u16,
}

impl Default for HighProbabilitySampleQuery {
    fn default() -> Self {
        Self {
            outcome: None,
            market_type: None,
            limit: DEFAULT_HIGH_PROBABILITY_LIST_LIMIT,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HighProbabilityBucketStats {
    pub id: i64,
    pub model_version: String,
    pub bucket_key: String,
    pub bucket_dimensions: Value,
    pub sample_count: u64,
    pub win_count: u64,
    pub win_rate: Decimal,
    pub fair_probability: Decimal,
    pub confidence_low: Option<Decimal>,
    pub confidence_high: Option<Decimal>,
    pub expected_pnl: Option<Decimal>,
    pub avg_max_drawdown_cents: Option<Decimal>,
    pub break_70_rate: Option<Decimal>,
    pub break_60_rate: Option<Decimal>,
    pub break_50_rate: Option<Decimal>,
    pub avg_hold_seconds: Option<i64>,
    pub recommended_max_entry_price: Option<Decimal>,
    pub computed_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HighProbabilityObservation {
    pub id: i64,
    pub observed_at: OffsetDateTime,
    pub condition_id: String,
    pub token_id: String,
    pub mode: HighProbabilityMode,
    pub executable_price: Decimal,
    pub fair_probability: Option<Decimal>,
    pub net_edge: Option<Decimal>,
    pub recommended_size_usd: Option<Decimal>,
    pub decision: HighProbabilityDecision,
    pub reasons: Vec<String>,
    pub model_version: Option<String>,
    pub created_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HighProbabilitySnapshot {
    pub config: HighProbabilityConfig,
    pub bucket_stats: Vec<HighProbabilityBucketStats>,
    pub observations: Vec<HighProbabilityObservation>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HighProbabilityResearchReport {
    pub generated_at: OffsetDateTime,
    pub model_version: String,
    pub market_scope: String,
    pub sample_limit: u16,
    pub samples_scanned: usize,
    pub settled_samples: usize,
    pub win_samples: usize,
    pub loss_samples: usize,
    pub voided_samples: usize,
    pub unknown_samples: usize,
    pub bucket_count: usize,
    pub qualified_bucket_count: usize,
    pub positive_expected_pnl_bucket_count: usize,
    pub weighted_win_rate: Option<Decimal>,
    pub weighted_expected_pnl: Option<Decimal>,
    pub weighted_break_70_rate: Option<Decimal>,
    pub best_bucket: Option<HighProbabilityBucketStats>,
    pub worst_bucket: Option<HighProbabilityBucketStats>,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HighProbabilityBacktestReport {
    pub generated_at: OffsetDateTime,
    pub model_version: String,
    pub market_scope: String,
    pub sample_limit: u16,
    pub train_sample_count: usize,
    pub test_sample_count: usize,
    pub candidate_count: usize,
    pub trade_count: usize,
    pub skipped_no_bucket_count: usize,
    pub skipped_no_edge_count: usize,
    pub win_trades: usize,
    pub loss_trades: usize,
    pub win_rate: Option<Decimal>,
    pub total_pnl: Decimal,
    pub average_pnl: Option<Decimal>,
    pub total_entry_cost: Decimal,
    pub roi: Option<Decimal>,
    pub max_drawdown: Decimal,
    pub average_entry_price: Option<Decimal>,
    pub train_start_at: Option<OffsetDateTime>,
    pub train_end_at: Option<OffsetDateTime>,
    pub test_start_at: Option<OffsetDateTime>,
    pub test_end_at: Option<OffsetDateTime>,
    pub exit_rule_reports: Vec<HighProbabilityBacktestExitRuleReport>,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HighProbabilityBacktestExitRuleReport {
    pub rule_key: String,
    pub trade_count: usize,
    pub win_rate: Option<Decimal>,
    pub total_pnl: Decimal,
    pub average_pnl: Option<Decimal>,
    pub total_entry_cost: Decimal,
    pub roi: Option<Decimal>,
    pub max_drawdown: Decimal,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HighProbabilityBacktestRun {
    pub id: i64,
    pub run_at: OffsetDateTime,
    pub report: HighProbabilityBacktestReport,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HighProbabilityBacktestTrade {
    pub id: i64,
    pub run_id: i64,
    pub sample_id: i64,
    pub condition_id: String,
    pub token_id: String,
    pub sampled_at: OffsetDateTime,
    pub bucket_key: String,
    pub executable_price: Decimal,
    pub fair_probability: Decimal,
    pub net_edge: Decimal,
    pub recommended_max_entry_price: Option<Decimal>,
    pub outcome: HighProbabilitySampleOutcome,
    pub settlement_pnl: Decimal,
    pub cumulative_pnl: Decimal,
    pub drawdown: Decimal,
    pub reasons: Vec<String>,
    pub created_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HighProbabilityBacktestResult {
    pub run: HighProbabilityBacktestRun,
    pub trades: Vec<HighProbabilityBacktestTrade>,
    pub config: HighProbabilityConfig,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct HighProbabilityBacktestPersistReport {
    pub run_id: i64,
    pub trades_saved: usize,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct HighProbabilityBucketRefreshReport {
    pub samples_scanned: usize,
    pub settled_samples: usize,
    pub buckets_computed: usize,
    pub buckets_saved: usize,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct HighProbabilitySampleBuildReport {
    pub candle_inputs_scanned: usize,
    pub samples_built: usize,
    pub samples_inserted: usize,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct HighProbabilityObserveReport {
    pub candidates_scanned: usize,
    pub observations_recorded: usize,
    pub allow_count: usize,
    pub reject_count: usize,
    pub skip_count: usize,
    pub missing_quote_count: usize,
    pub missing_bucket_count: usize,
}

/// Which side of a binary market was used to derive the YES-scale fair value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FairValueSide {
    /// Estimated directly from the YES token price bucket.
    Yes,
    /// Estimated from the NO token price bucket and complemented to YES scale.
    NoComplement,
}

impl FairValueSide {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Yes => "yes",
            Self::NoComplement => "no_complement",
        }
    }
}

impl FromStr for FairValueSide {
    type Err = AppError;

    fn from_str(value: &str) -> Result<Self> {
        match value {
            "yes" => Ok(Self::Yes),
            "no_complement" => Ok(Self::NoComplement),
            other => Err(AppError::invalid_input(
                "HIGH_PROBABILITY_FAIR_VALUE_SIDE_INVALID",
                format!("unknown high probability fair value side: {other}"),
            )),
        }
    }
}

/// Per-token pricing input used to build a fair value estimate. The worker
/// assembles this from an observe candidate plus its current orderbook quote.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FairValuePricingInput {
    pub condition_id: String,
    pub token_id: String,
    /// "yes" or "no" — which outcome this token represents.
    pub outcome: String,
    pub reference_price: Decimal,
    pub reference_spread_cents: Decimal,
    pub best_bid: Option<Decimal>,
    pub best_ask: Option<Decimal>,
    pub ask_depth_usd: Option<Decimal>,
    pub market_type: String,
    pub liquidity_usd: Option<Decimal>,
    pub end_at: Option<OffsetDateTime>,
    pub observed_at: OffsetDateTime,
    pub risk_tags: Vec<String>,
    pub confirmed_at_ms: Option<i64>,
}

impl FairValuePricingInput {
    #[must_use]
    pub fn normalized(mut self) -> Self {
        self.condition_id = self.condition_id.trim().to_string();
        self.token_id = self.token_id.trim().to_string();
        self.outcome = self.outcome.trim().to_ascii_lowercase();
        self.reference_price = clamp_decimal(self.reference_price, Decimal::ZERO, Decimal::ONE);
        self.reference_spread_cents = self.reference_spread_cents.max(Decimal::ZERO);
        self.best_bid = self
            .best_bid
            .map(|value| clamp_decimal(value, Decimal::ZERO, Decimal::ONE));
        self.best_ask = self
            .best_ask
            .map(|value| clamp_decimal(value, Decimal::ZERO, Decimal::ONE));
        self.ask_depth_usd = self.ask_depth_usd.map(|value| value.max(Decimal::ZERO));
        self.market_type = non_empty_or(self.market_type, "unknown");
        self.risk_tags = normalize_string_list(self.risk_tags);
        self
    }

    /// The executable price used as the reference for fair value (best ask,
    /// clamped, falling back to the candle reference price when the book is
    /// missing — mirroring the observe path's `executable_price`).
    #[must_use]
    pub fn executable_price(&self) -> Decimal {
        self.best_ask
            .filter(|price| *price > Decimal::ZERO)
            .unwrap_or(self.reference_price)
    }
}

/// A conservative, auditable fair value snapshot for one condition. This is the
/// provider output consumed (read-only) by the Rewards market maker.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FairValueEstimate {
    pub id: i64,
    pub condition_id: String,
    pub token_id: String,
    pub side_used: FairValueSide,
    pub price_used: Decimal,
    pub fair_yes_low: Decimal,
    pub fair_yes_mid: Decimal,
    pub fair_yes_high: Decimal,
    pub market_implied: Decimal,
    pub base_rate: Decimal,
    pub confidence: Decimal,
    pub uncertainty_cents: Decimal,
    pub sample_count: u64,
    pub bucket_key: String,
    /// 0 = exact bucket, increasing as the resolution falls back to coarser
    /// buckets, up to 5 for the global prior.
    pub fallback_level: u8,
    pub model_version: String,
    pub input_hash: String,
    pub reason_codes: Vec<String>,
    pub live_eligible: bool,
    pub computed_at: OffsetDateTime,
    pub expires_at: OffsetDateTime,
    pub created_at: OffsetDateTime,
}

impl FairValueEstimate {
    #[must_use]
    pub fn normalized(mut self) -> Self {
        self.condition_id = self.condition_id.trim().to_string();
        self.token_id = self.token_id.trim().to_string();
        self.price_used = clamp_decimal(self.price_used, Decimal::ZERO, Decimal::ONE);
        self.fair_yes_low = clamp_decimal(self.fair_yes_low, Decimal::ZERO, Decimal::ONE);
        self.fair_yes_mid = clamp_decimal(self.fair_yes_mid, Decimal::ZERO, Decimal::ONE);
        self.fair_yes_high = clamp_decimal(self.fair_yes_high, Decimal::ZERO, Decimal::ONE);
        self.market_implied = clamp_decimal(self.market_implied, Decimal::ZERO, Decimal::ONE);
        self.base_rate = clamp_decimal(self.base_rate, Decimal::ZERO, Decimal::ONE);
        self.confidence = clamp_decimal(self.confidence, Decimal::ZERO, Decimal::ONE);
        self.uncertainty_cents = self.uncertainty_cents.max(Decimal::ZERO);
        self.reason_codes = normalize_string_list(self.reason_codes);
        self
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FairValueRefreshReport {
    pub conditions_scanned: usize,
    pub estimates_computed: usize,
    pub live_eligible_count: usize,
    pub unavailable_count: usize,
    pub missing_bucket_count: usize,
    pub missing_quote_count: usize,
}

// ── Research feature vector (Phase 2 ML preparation) ───────────────────────

/// Price-path features computed from the candle window *before* the sample
/// point (at-sample-time information only). Forward-looking labels such as
/// `min_future_close` / `max_future_close` are kept separately by the sample
/// builder for exit-rule backtesting.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PricePathFeatures {
    pub return_5m: Option<Decimal>,
    pub return_1h: Option<Decimal>,
    pub return_6h: Option<Decimal>,
    pub return_24h: Option<Decimal>,
    pub realized_volatility: Option<Decimal>,
    pub max_run_up_cents: Option<Decimal>,
    pub largest_prior_drawdown_cents: Option<Decimal>,
    pub prior_bucket_crossings: Option<i64>,
    pub time_above_70_sec: Option<i64>,
    pub time_above_80_sec: Option<i64>,
    pub time_above_90_sec: Option<i64>,
    pub monotonic_trend_score: Option<Decimal>,
}

/// Point-in-time liquidity / book features.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct LiquidityFeatures {
    pub spread_cents: Option<Decimal>,
    pub top_ask_depth_usd: Option<Decimal>,
    pub top_bid_depth_usd: Option<Decimal>,
    pub book_fresh_ms: Option<i64>,
    pub liquidity_bucket: Option<String>,
}

/// Time-to-resolution and market-age features.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct TimeFeatures {
    pub time_to_resolution_bucket: Option<String>,
    pub market_age_bucket: Option<String>,
}

/// Risk-tag presence flags over the documented taxonomy.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct RiskFeatures {
    pub ambiguous_rules: bool,
    pub subjective_resolution: bool,
    pub regulatory_or_court_dependency: bool,
    pub official_confirmation_pending: bool,
    pub single_source_news: bool,
    pub high_news_velocity: bool,
    pub source_conflict: bool,
    pub long_horizon: bool,
}

impl RiskFeatures {
    #[must_use]
    pub fn active_count(&self) -> u8 {
        u8::from(self.ambiguous_rules)
            + u8::from(self.subjective_resolution)
            + u8::from(self.regulatory_or_court_dependency)
            + u8::from(self.official_confirmation_pending)
            + u8::from(self.single_source_news)
            + u8::from(self.high_news_velocity)
            + u8::from(self.source_conflict)
            + u8::from(self.long_horizon)
    }
}

/// Aggregated research feature vector. Serialized into the sample
/// `path_features` JSONB so future ML models and diagnostics can consume it.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct HighProbabilityFeatureVector {
    pub version: String,
    pub path: PricePathFeatures,
    pub liquidity: LiquidityFeatures,
    pub time: TimeFeatures,
    pub risk: RiskFeatures,
}
