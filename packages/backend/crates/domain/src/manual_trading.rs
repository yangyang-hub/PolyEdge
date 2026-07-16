macro_rules! manual_trading_enum {
    ($name:ident { $($variant:ident => $wire:literal),+ $(,)? }) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[serde(rename_all = "snake_case")]
        pub enum $name {
            $($variant),+
        }

        impl $name {
            #[must_use]
            pub fn as_str(self) -> &'static str {
                match self {
                    $(Self::$variant => $wire),+
                }
            }
        }

        impl FromStr for $name {
            type Err = AppError;

            fn from_str(value: &str) -> Result<Self> {
                match value {
                    $($wire => Ok(Self::$variant),)+
                    _ => Err(AppError::invalid_input(
                        "DOMAIN_MANUAL_TRADING_ENUM_INVALID",
                        format!("unknown {} value: {value}", stringify!($name)),
                    )),
                }
            }
        }
    };
}

manual_trading_enum!(WalletAccountStatus {
    Active => "active",
    Paused => "paused",
    Disabled => "disabled",
    Error => "error",
});
manual_trading_enum!(CredentialProvider {
    Environment => "environment",
    Vault => "vault",
    Kms => "kms",
});
manual_trading_enum!(MarketStatus {
    Open => "open",
    Closed => "closed",
    Resolved => "resolved",
});
manual_trading_enum!(StrategyStatus {
    Draft => "draft",
    Active => "active",
    Paused => "paused",
    Expired => "expired",
    Archived => "archived",
});
manual_trading_enum!(StrategyVisibility {
    Private => "private",
    Followable => "followable",
});
manual_trading_enum!(StrategyVersionStatus {
    Draft => "draft",
    Published => "published",
    Retired => "retired",
});
manual_trading_enum!(QuoteOutcome {
    Yes => "yes",
    No => "no",
});
manual_trading_enum!(QuotePricingMode {
    Fixed => "fixed",
    BookRank => "book_rank",
});
manual_trading_enum!(StrategySubscriptionKind {
    Owner => "owner",
    Follower => "follower",
});
manual_trading_enum!(StrategySubscriptionStatus {
    Active => "active",
    Paused => "paused",
    Stopped => "stopped",
    Expired => "expired",
});
manual_trading_enum!(StrategyCommandType {
    Publish => "publish",
    Activate => "activate",
    Pause => "pause",
    Resume => "resume",
    Expire => "expire",
    Archive => "archive",
    ForceCancel => "force_cancel",
});
manual_trading_enum!(StrategyCommandStatus {
    Pending => "pending",
    Running => "running",
    Completed => "completed",
    Failed => "failed",
});
manual_trading_enum!(TradingOrderSide {
    Buy => "buy",
    Sell => "sell",
});
manual_trading_enum!(ExecutionBatchStatus {
    Pending => "pending",
    Running => "running",
    PartiallySucceeded => "partially_succeeded",
    Succeeded => "succeeded",
    Failed => "failed",
    Cancelled => "cancelled",
});
manual_trading_enum!(ExecutionBatchType {
    Execute => "execute",
    Cancel => "cancel",
});
manual_trading_enum!(ExecutionRequestSource {
    Operator => "operator",
    Runtime => "runtime",
    StrategyCommand => "strategy_command",
    ExpirySupervisor => "expiry_supervisor",
});
manual_trading_enum!(WalletExecutionJobStatus {
    Pending => "pending",
    Running => "running",
    Succeeded => "succeeded",
    Failed => "failed",
    Cancelled => "cancelled",
});
manual_trading_enum!(ExecutionActionType {
    PlaceOrder => "place_order",
    CancelOrder => "cancel_order",
    ReplaceOrder => "replace_order",
    ReconcileOrder => "reconcile_order",
});
manual_trading_enum!(ExecutionActionStatus {
    Planned => "planned",
    Executing => "executing",
    Succeeded => "succeeded",
    Failed => "failed",
    Unknown => "unknown",
    Cancelled => "cancelled",
});
manual_trading_enum!(ManagedOrderStatus {
    Planned => "planned",
    Submitting => "submitting",
    Open => "open",
    PartiallyFilled => "partially_filled",
    CancelPending => "cancel_pending",
    Cancelled => "cancelled",
    Filled => "filled",
    Expired => "expired",
    Rejected => "rejected",
    Unknown => "unknown",
});

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WalletAccount {
    pub id: i64,
    pub owner_user_id: i64,
    pub name: String,
    pub signer_address: String,
    pub funder_address: String,
    pub signature_type: i32,
    pub status: WalletAccountStatus,
    pub trading_enabled: bool,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WalletSecretMetadata {
    pub wallet_id: i64,
    pub key_id: String,
    pub secret_version: i64,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WalletRiskPolicy {
    pub wallet_id: i64,
    pub max_open_orders: i64,
    pub max_open_buy_notional: Decimal,
    pub max_total_position_notional: Decimal,
    pub max_market_position_notional: Decimal,
    pub max_order_notional: Decimal,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WalletAccountState {
    pub wallet_id: i64,
    pub available_collateral: Decimal,
    pub reserved_collateral: Decimal,
    pub open_buy_notional: Decimal,
    pub total_position_notional: Decimal,
    #[serde(with = "time::serde::rfc3339::option")]
    pub last_synced_at: Option<OffsetDateTime>,
    pub last_error: Option<String>,
    pub version: i64,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ManagedMarket {
    pub id: i64,
    pub created_by_user_id: i64,
    pub condition_id: String,
    pub slug: String,
    pub question: String,
    pub polymarket_url: Option<String>,
    pub status: MarketStatus,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ManagedMarketOutcome {
    pub id: i64,
    pub market_id: i64,
    pub outcome: QuoteOutcome,
    pub token_id: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MarketStrategy {
    pub id: i64,
    pub owner_user_id: i64,
    pub owner_display_name: String,
    pub market_id: i64,
    pub name: String,
    pub status: StrategyStatus,
    pub visibility: StrategyVisibility,
    #[serde(with = "time::serde::rfc3339")]
    pub active_from: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub active_until: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339::option")]
    pub expired_at: Option<OffsetDateTime>,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StrategyRewardTerms {
    pub strategy_version_id: i64,
    pub minimum_size: Decimal,
    pub maximum_spread: Decimal,
    pub daily_rate: Option<Decimal>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StrategyVersion {
    pub id: i64,
    pub strategy_id: i64,
    pub version_number: i64,
    pub status: StrategyVersionStatus,
    pub book_freshness_ms: i64,
    pub downward_reprice_confirm_ms: i64,
    pub upward_reprice_confirm_ms: i64,
    pub reprice_cooldown_ms: i64,
    pub max_replaces_per_cycle: i64,
    #[serde(with = "time::serde::rfc3339::option")]
    pub published_at: Option<OffsetDateTime>,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StrategyQuoteSlot {
    pub id: i64,
    pub strategy_version_id: i64,
    pub slot_key: String,
    pub outcome: QuoteOutcome,
    pub quantity: Decimal,
    pub pricing_mode: QuotePricingMode,
    pub fixed_price: Option<Decimal>,
    pub book_rank: Option<i64>,
    pub price_offset: Decimal,
    pub minimum_price: Decimal,
    pub maximum_price: Decimal,
    pub post_only: bool,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StrategySubscription {
    pub id: i64,
    pub follower_user_id: i64,
    pub source_strategy_id: i64,
    pub source_strategy_name: String,
    pub source_user_id: i64,
    pub source_display_name: String,
    pub kind: StrategySubscriptionKind,
    pub status: StrategySubscriptionStatus,
    #[serde(with = "time::serde::rfc3339::option")]
    pub active_until: Option<OffsetDateTime>,
    #[serde(with = "time::serde::rfc3339")]
    pub effective_active_until: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339::option")]
    pub stopped_at: Option<OffsetDateTime>,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StrategySubscriptionWallet {
    pub subscription_id: i64,
    pub follower_user_id: i64,
    pub wallet_id: i64,
    pub enabled: bool,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StrategyCommand {
    pub id: i64,
    pub source_strategy_id: i64,
    pub source_user_id: i64,
    pub strategy_version_id: Option<i64>,
    pub sequence: i64,
    pub command_type: StrategyCommandType,
    pub status: StrategyCommandStatus,
    pub lease_owner: Option<String>,
    pub lease_epoch: i64,
    #[serde(with = "time::serde::rfc3339::option")]
    pub lease_expires_at: Option<OffsetDateTime>,
    pub last_error: Option<String>,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339::option")]
    pub completed_at: Option<OffsetDateTime>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExecutionBatch {
    pub id: i64,
    pub subscriber_user_id: i64,
    pub subscription_id: i64,
    pub source_strategy_id: i64,
    pub strategy_version_id: i64,
    pub strategy_command_id: Option<i64>,
    pub batch_type: ExecutionBatchType,
    pub status: ExecutionBatchStatus,
    pub requested_by_user_id: Option<i64>,
    pub request_source: ExecutionRequestSource,
    pub operator_note: Option<String>,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339::option")]
    pub started_at: Option<OffsetDateTime>,
    #[serde(with = "time::serde::rfc3339::option")]
    pub completed_at: Option<OffsetDateTime>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WalletExecutionJob {
    pub id: i64,
    pub batch_id: i64,
    pub owner_user_id: i64,
    pub wallet_id: i64,
    pub status: WalletExecutionJobStatus,
    pub attempt_count: i64,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    pub lease_epoch: i64,
    pub lease_owner: Option<String>,
    #[serde(with = "time::serde::rfc3339::option")]
    pub lease_expires_at: Option<OffsetDateTime>,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExecutionAction {
    pub id: i64,
    pub job_id: i64,
    pub quote_slot_id: Option<i64>,
    pub managed_order_id: Option<i64>,
    pub action_type: ExecutionActionType,
    pub status: ExecutionActionStatus,
    pub idempotency_key: String,
    pub reason_code: String,
    pub attempt_count: i64,
    pub lease_epoch: i64,
    pub lease_owner: Option<String>,
    #[serde(with = "time::serde::rfc3339::option")]
    pub lease_expires_at: Option<OffsetDateTime>,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ManagedOrder {
    pub id: i64,
    pub owner_user_id: i64,
    pub wallet_id: i64,
    pub subscription_id: i64,
    pub market_id: i64,
    pub strategy_version_id: i64,
    pub quote_slot_id: Option<i64>,
    pub token_id: String,
    pub outcome: QuoteOutcome,
    pub side: TradingOrderSide,
    pub price: Decimal,
    pub quantity: Decimal,
    pub filled_quantity: Decimal,
    pub status: ManagedOrderStatus,
    pub external_order_id: Option<String>,
    pub generation: i64,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ManagedPosition {
    pub id: i64,
    pub owner_user_id: i64,
    pub wallet_id: i64,
    pub market_id: i64,
    pub token_id: String,
    pub outcome: QuoteOutcome,
    pub quantity: Decimal,
    pub average_price: Decimal,
    pub realized_pnl: Decimal,
    pub version: i64,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
}
