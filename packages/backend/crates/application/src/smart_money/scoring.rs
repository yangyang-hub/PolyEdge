pub fn build_smart_wallet_score(
    config: &SmartMoneyConfig,
    profile: &SmartWalletProfile,
) -> SmartWalletScore {
    let profit_score = clamp_unit_decimal((profile.roi + Decimal::new(50, 2)) / Decimal::from(2));
    let sample_score = ratio_score(profile.trade_count, config.min_trade_count.max(1));
    let settled_sample_score = ratio_score(
        profile.settled_trade_count,
        config.min_settled_trade_count.max(1),
    );
    let volume_score = if config.min_total_volume_usd <= Decimal::ZERO {
        Decimal::ONE
    } else {
        clamp_unit_decimal(profile.total_volume_usd / config.min_total_volume_usd)
    };
    let consistency_score = clamp_unit_decimal(
        (profile.win_rate + sample_score + settled_sample_score + volume_score) / Decimal::from(4),
    );
    let concentration_penalty = profile
        .market_concentration_score
        .max(profile.category_concentration_score);
    let risk_score = clamp_unit_decimal(Decimal::ONE - concentration_penalty);
    let liquidity_score = clamp_unit_decimal(Decimal::ONE - profile.low_liquidity_trade_ratio);
    let recency_score = if profile.last_trade_at.is_some() {
        Decimal::ONE
    } else {
        Decimal::ZERO
    };
    let copyability_score = clamp_unit_decimal(Decimal::ONE - profile.stale_copy_window_ratio);

    let total_score = clamp_unit_decimal(
        profit_score * Decimal::new(25, 2)
            + consistency_score * Decimal::new(20, 2)
            + copyability_score * Decimal::new(20, 2)
            + risk_score * Decimal::new(15, 2)
            + liquidity_score * Decimal::new(10, 2)
            + recency_score * Decimal::new(10, 2),
    );

    let tier = if profile.trade_count < config.min_trade_count
        || profile.settled_trade_count < config.min_settled_trade_count
        || profile.total_volume_usd < config.min_total_volume_usd
    {
        SmartWalletTier::Candidate
    } else if copyability_score < config.min_copyability_score {
        SmartWalletTier::Candidate
    } else if total_score >= Decimal::new(80, 2) {
        SmartWalletTier::Approved
    } else if total_score >= Decimal::new(60, 2) {
        SmartWalletTier::Watch
    } else {
        SmartWalletTier::Candidate
    };

    SmartWalletScore {
        wallet_address: profile.wallet_address.clone(),
        total_score,
        profit_score,
        consistency_score,
        risk_score,
        liquidity_score,
        recency_score,
        copyability_score,
        tier,
        explanation: json!({
            "scoring_version": SMART_MONEY_SCORING_VERSION,
            "hard_filters": {
                "min_trade_count": config.min_trade_count,
                "min_settled_trade_count": config.min_settled_trade_count,
                "min_total_volume_usd": config.min_total_volume_usd.to_string(),
                "min_copyability_score": config.min_copyability_score.to_string()
            }
        }),
        scoring_version: SMART_MONEY_SCORING_VERSION.to_string(),
        updated_at: OffsetDateTime::now_utc(),
    }
}

fn ratio_score(value: i64, threshold: i64) -> Decimal {
    if threshold <= 0 {
        return Decimal::ONE;
    }
    clamp_unit_decimal(Decimal::from(value.max(0)) / Decimal::from(threshold))
}
