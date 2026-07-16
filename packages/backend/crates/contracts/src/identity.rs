#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActivateUserRequest {
    pub token: String,
    pub password: String,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReauthenticateRequest {
    pub password: String,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct AuthSessionData {
    pub user: polyedge_domain::UserAccount,
    pub csrf_token: String,
    #[serde(with = "time::serde::rfc3339")]
    pub expires_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339::option")]
    pub recent_auth_at: Option<OffsetDateTime>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CurrentUserData {
    pub user: polyedge_domain::UserAccount,
    #[serde(with = "time::serde::rfc3339::option")]
    pub recent_auth_at: Option<OffsetDateTime>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateUserRequest {
    pub username: String,
    pub display_name: String,
    pub role: polyedge_domain::UserRole,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct UpdateUserRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<polyedge_domain::UserRole>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<polyedge_domain::UserStatus>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct CreatedUserData {
    pub user: polyedge_domain::UserAccount,
    pub activation_token: String,
    #[serde(with = "time::serde::rfc3339")]
    pub activation_expires_at: OffsetDateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct ReissueActivationTokenRequest {}

#[derive(Clone, Serialize, Deserialize)]
pub struct ActivationTokenData {
    pub user_id: i64,
    pub activation_token: String,
    #[serde(with = "time::serde::rfc3339")]
    pub activation_expires_at: OffsetDateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdminFinanceSummaryData {
    pub user_id: i64,
    pub username: String,
    pub display_name: String,
    pub wallet_count: i64,
    pub available_collateral: Decimal,
    pub reserved_collateral: Decimal,
    pub position_market_value: Decimal,
    pub equity: Decimal,
    pub realized_pnl: Decimal,
    pub unrealized_pnl: Decimal,
    pub total_pnl: Decimal,
    pub valuation_complete: bool,
    #[serde(with = "time::serde::rfc3339::option")]
    pub observed_at: Option<OffsetDateTime>,
}
