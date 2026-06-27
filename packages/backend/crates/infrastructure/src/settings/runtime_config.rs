use super::{NewsSourceSettings, PolymarketSignatureType, Settings};
use polyedge_domain::AppError;
use serde::Serialize;
use std::{collections::BTreeMap, str::FromStr};

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeConfigValueType {
    Boolean,
    Integer,
    Decimal,
    Text,
    Url,
    Json,
    Enum,
}

impl RuntimeConfigValueType {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Boolean => "boolean",
            Self::Integer => "integer",
            Self::Decimal => "decimal",
            Self::Text => "text",
            Self::Url => "url",
            Self::Json => "json",
            Self::Enum => "enum",
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct RuntimeConfigEntry {
    pub key: String,
    pub section: String,
    pub field: String,
    pub label: String,
    pub env_name: String,
    pub value: String,
    pub default_value: String,
    pub value_type: RuntimeConfigValueType,
    pub options: Vec<String>,
    pub restart_required: bool,
}

impl PolymarketSignatureType {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Eoa => "eoa",
            Self::Proxy => "proxy",
            Self::GnosisSafe => "gnosis_safe",
            Self::Poly1271 => "poly_1271",
        }
    }
}

impl FromStr for PolymarketSignatureType {
    type Err = AppError;

    fn from_str(value: &str) -> polyedge_domain::Result<Self> {
        match value {
            "eoa" => Ok(Self::Eoa),
            "proxy" => Ok(Self::Proxy),
            "gnosis_safe" => Ok(Self::GnosisSafe),
            "poly_1271" | "poly1271" | "POLY_1271" | "deposit_wallet" => Ok(Self::Poly1271),
            other => Err(AppError::invalid_input(
                "CONFIG_POLYMARKET_SIGNATURE_TYPE_INVALID",
                format!("unsupported polymarket signature type: {other}"),
            )),
        }
    }
}

include!("runtime_config/entries.rs");
include!("runtime_config/apply.rs");
include!("runtime_config/helpers.rs");
