use rust_decimal::{Decimal, RoundingStrategy};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::{fmt, str::FromStr};

include!("domain/error.rs");
include!("domain/numeric.rs");
include!("domain/market_enums.rs");
include!("domain/auth.rs");
include!("domain/tests.rs");
