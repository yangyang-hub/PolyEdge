use rust_decimal::{Decimal, RoundingStrategy};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::{fmt, str::FromStr};
use time::OffsetDateTime;

include!("domain/error.rs");
include!("domain/numeric.rs");
include!("manual_trading.rs");
