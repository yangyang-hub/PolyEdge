// Environment source builder and typed-value parsers for static configuration defaults.

fn environment_source() -> Environment {
    Environment::with_prefix("POLYEDGE")
        .prefix_separator("_")
        .separator("__")
        .ignore_empty(true)
        .try_parsing(true)
        .list_separator(",")
        .with_list_parse_key("auth.revoked_sessions")
        .with_list_parse_key("cors.allowed_origins")
}

fn decimal(value: &str) -> rust_decimal::Decimal {
    rust_decimal::Decimal::from_str_exact(value)
        .expect("static backend configuration default must be a valid decimal")
}

fn probability(value: &str) -> Probability {
    Probability::new(decimal(value)).expect("static backend configuration default must be valid")
}

fn exposure_ratio(value: &str) -> ExposureRatio {
    ExposureRatio::new(decimal(value)).expect("static backend configuration default must be valid")
}

fn usd_amount(value: &str) -> UsdAmount {
    UsdAmount::new(decimal(value)).expect("static backend configuration default must be valid")
}

fn signed_usd_amount(value: &str) -> SignedUsdAmount {
    SignedUsdAmount::new(decimal(value))
        .expect("static backend configuration default must be valid")
}
