use super::{NewsSourceSettings, PolymarketConnectorMode, PolymarketSignatureType, Settings};
use polyedge_domain::{AppError, Edge, ExposureRatio, Probability, Quantity, UsdAmount};
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

impl PolymarketConnectorMode {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::Mock => "mock",
            Self::Live => "live",
        }
    }
}

impl FromStr for PolymarketConnectorMode {
    type Err = AppError;

    fn from_str(value: &str) -> polyedge_domain::Result<Self> {
        match value {
            "disabled" => Ok(Self::Disabled),
            "mock" => Ok(Self::Mock),
            "live" => Ok(Self::Live),
            other => Err(AppError::invalid_input(
                "CONFIG_POLYMARKET_MODE_INVALID",
                format!("unsupported polymarket mode: {other}"),
            )),
        }
    }
}

impl PolymarketSignatureType {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Eoa => "eoa",
            Self::Proxy => "proxy",
            Self::GnosisSafe => "gnosis_safe",
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
