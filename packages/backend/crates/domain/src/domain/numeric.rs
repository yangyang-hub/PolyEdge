macro_rules! non_negative_decimal_type {
    ($name:ident, $scale:expr, $code:literal, $message:literal) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
        pub struct $name(Decimal);

        impl $name {
            pub const SCALE: u32 = $scale;

            pub fn new(value: Decimal) -> Result<Self> {
                if value < Decimal::ZERO {
                    return Err(AppError::invalid_input($code, $message));
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

        impl Serialize for $name {
            fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
            where
                S: Serializer,
            {
                serializer.serialize_str(&self.api_string())
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
            where
                D: Deserializer<'de>,
            {
                Self::new(deserialize_decimal_str(deserializer)?).map_err(serde::de::Error::custom)
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(&self.api_string())
            }
        }
    };
}

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
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.api_string())
    }
}

non_negative_decimal_type!(
    Quantity,
    8,
    "DOMAIN_QUANTITY_OUT_OF_RANGE",
    "quantity must be non-negative"
);
