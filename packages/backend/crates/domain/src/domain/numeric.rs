#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Probability(Decimal);

impl Probability {
    pub const SCALE: u32 = 6;

    pub fn new(value: Decimal) -> Result<Self> {
        if value < Decimal::ZERO || value > Decimal::ONE {
            return Err(AppError::invalid_input(
                "DOMAIN_PROBABILITY_OUT_OF_RANGE",
                "probability must be within [0, 1]",
            ));
        }

        Ok(Self(value))
    }

    #[must_use]
    pub fn value(self) -> Decimal {
        self.0
    }

    #[must_use]
    pub fn api_string(self) -> String {
        format_decimal(self.0, Self::SCALE)
    }
}

impl Serialize for Probability {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.api_string())
    }
}

impl<'de> Deserialize<'de> for Probability {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::new(deserialize_decimal_str(deserializer)?).map_err(serde::de::Error::custom)
    }
}

impl fmt::Display for Probability {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.api_string())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Edge(Decimal);

impl Edge {
    pub const SCALE: u32 = 6;

    pub fn new(value: Decimal) -> Result<Self> {
        if value < -Decimal::ONE || value > Decimal::ONE {
            return Err(AppError::invalid_input(
                "DOMAIN_EDGE_OUT_OF_RANGE",
                "edge must be within [-1, 1]",
            ));
        }

        Ok(Self(value))
    }

    #[must_use]
    pub fn value(self) -> Decimal {
        self.0
    }

    #[must_use]
    pub fn api_string(self) -> String {
        format_decimal(self.0, Self::SCALE)
    }
}

impl Serialize for Edge {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.api_string())
    }
}

impl<'de> Deserialize<'de> for Edge {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::new(deserialize_decimal_str(deserializer)?).map_err(serde::de::Error::custom)
    }
}

impl fmt::Display for Edge {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.api_string())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ExposureRatio(Decimal);

impl ExposureRatio {
    pub const SCALE: u32 = 6;
    const MAX: Decimal = Decimal::from_parts(10, 0, 0, false, 0);

    pub fn new(value: Decimal) -> Result<Self> {
        if value < Decimal::ZERO || value > Self::MAX {
            return Err(AppError::invalid_input(
                "DOMAIN_EXPOSURE_RATIO_OUT_OF_RANGE",
                "exposure ratio must be within [0, 10]",
            ));
        }

        Ok(Self(value))
    }

    #[must_use]
    pub fn value(self) -> Decimal {
        self.0
    }

    #[must_use]
    pub fn api_string(self) -> String {
        format_decimal(self.0, Self::SCALE)
    }
}

impl Serialize for ExposureRatio {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.api_string())
    }
}

impl<'de> Deserialize<'de> for ExposureRatio {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::new(deserialize_decimal_str(deserializer)?).map_err(serde::de::Error::custom)
    }
}

impl fmt::Display for ExposureRatio {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.api_string())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Quantity(Decimal);

impl Quantity {
    pub const SCALE: u32 = 8;

    pub fn new(value: Decimal) -> Result<Self> {
        if value < Decimal::ZERO {
            return Err(AppError::invalid_input(
                "DOMAIN_QUANTITY_OUT_OF_RANGE",
                "quantity must be non-negative",
            ));
        }

        Ok(Self(value))
    }

    #[must_use]
    pub fn value(self) -> Decimal {
        self.0
    }

    #[must_use]
    pub fn api_string(self) -> String {
        format_decimal(self.0, Self::SCALE)
    }
}

impl Serialize for Quantity {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.api_string())
    }
}

impl<'de> Deserialize<'de> for Quantity {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::new(deserialize_decimal_str(deserializer)?).map_err(serde::de::Error::custom)
    }
}

impl fmt::Display for Quantity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.api_string())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct UsdAmount(Decimal);

impl UsdAmount {
    pub const SCALE: u32 = 2;

    pub fn new(value: Decimal) -> Result<Self> {
        if value < Decimal::ZERO {
            return Err(AppError::invalid_input(
                "DOMAIN_USD_AMOUNT_OUT_OF_RANGE",
                "usd amount must be non-negative",
            ));
        }

        Ok(Self(value))
    }

    #[must_use]
    pub fn value(self) -> Decimal {
        self.0
    }

    #[must_use]
    pub fn api_string(self) -> String {
        format_decimal(self.0, Self::SCALE)
    }
}

impl Serialize for UsdAmount {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.api_string())
    }
}

impl<'de> Deserialize<'de> for UsdAmount {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::new(deserialize_decimal_str(deserializer)?).map_err(serde::de::Error::custom)
    }
}

impl fmt::Display for UsdAmount {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.api_string())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct SignedUsdAmount(Decimal);

impl SignedUsdAmount {
    pub const SCALE: u32 = 2;

    pub fn new(value: Decimal) -> Result<Self> {
        Ok(Self(value))
    }

    #[must_use]
    pub fn value(self) -> Decimal {
        self.0
    }

    #[must_use]
    pub fn api_string(self) -> String {
        format_decimal(self.0, Self::SCALE)
    }
}

impl Serialize for SignedUsdAmount {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.api_string())
    }
}

impl<'de> Deserialize<'de> for SignedUsdAmount {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::new(deserialize_decimal_str(deserializer)?).map_err(serde::de::Error::custom)
    }
}

impl fmt::Display for SignedUsdAmount {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.api_string())
    }
}
