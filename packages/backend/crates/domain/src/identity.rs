macro_rules! identity_enum {
    ($name:ident { $($variant:ident => $wire:literal),+ $(,)? }) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[serde(rename_all = "snake_case")]
        pub enum $name { $($variant),+ }

        impl $name {
            #[must_use]
            pub fn as_str(self) -> &'static str {
                match self { $(Self::$variant => $wire),+ }
            }
        }

        impl FromStr for $name {
            type Err = AppError;
            fn from_str(value: &str) -> Result<Self> {
                match value {
                    $($wire => Ok(Self::$variant),)+
                    _ => Err(AppError::invalid_input(
                        "DOMAIN_IDENTITY_ENUM_INVALID",
                        format!("unknown {} value: {value}", stringify!($name)),
                    )),
                }
            }
        }
    };
}

identity_enum!(UserRole {
    Admin => "admin",
    MarketEditor => "market_editor",
    ReadOnly => "read_only",
});
identity_enum!(UserStatus {
    Pending => "pending",
    Active => "active",
    Disabled => "disabled",
    Locked => "locked",
});
identity_enum!(UserAuthSource {
    EnvironmentAdmin => "environment_admin",
    Local => "local",
});

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserAccount {
    pub id: i64,
    pub username: String,
    pub display_name: String,
    pub role: UserRole,
    pub status: UserStatus,
    pub auth_source: UserAuthSource,
    pub created_by_user_id: Option<i64>,
    pub credential_version: i64,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActorScope {
    pub user_id: i64,
    pub role: UserRole,
}

impl ActorScope {
    #[must_use]
    pub fn is_admin(self) -> bool { self.role == UserRole::Admin }

    #[must_use]
    pub fn can_write_markets(self) -> bool {
        matches!(self.role, UserRole::Admin | UserRole::MarketEditor)
    }
}
