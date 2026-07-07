fn high_probability_sample_from_row(row: &sqlx::postgres::PgRow) -> Result<HighProbabilitySample> {
    let trigger_kind_raw: String = row.try_get("trigger_kind").map_err(postgres_decode_error)?;
    let outcome_raw: String = row.try_get("outcome").map_err(postgres_decode_error)?;
    let path_features: Json<Value> = row
        .try_get("path_features")
        .map_err(postgres_decode_error)?;
    let risk_tags: Json<Vec<String>> = row.try_get("risk_tags").map_err(postgres_decode_error)?;
    Ok(HighProbabilitySample {
        id: row.try_get("id").map_err(postgres_decode_error)?,
        condition_id: row.try_get("condition_id").map_err(postgres_decode_error)?,
        token_id: row.try_get("token_id").map_err(postgres_decode_error)?,
        side: row.try_get("side").map_err(postgres_decode_error)?,
        sampled_at: row.try_get("sampled_at").map_err(postgres_decode_error)?,
        trigger_kind: HighProbabilityTriggerKind::from_str(&trigger_kind_raw)?,
        executable_price: row
            .try_get("executable_price")
            .map_err(postgres_decode_error)?,
        price_bucket: row.try_get("price_bucket").map_err(postgres_decode_error)?,
        market_type: row.try_get("market_type").map_err(postgres_decode_error)?,
        time_to_resolution_bucket: row
            .try_get("time_to_resolution_bucket")
            .map_err(postgres_decode_error)?,
        liquidity_bucket: row
            .try_get("liquidity_bucket")
            .map_err(postgres_decode_error)?,
        spread_bucket: row.try_get("spread_bucket").map_err(postgres_decode_error)?,
        path_features: path_features.0,
        risk_tags: risk_tags.0,
        outcome: HighProbabilitySampleOutcome::from_str(&outcome_raw)?,
        settlement_pnl: row.try_get("settlement_pnl").map_err(postgres_decode_error)?,
        max_drawdown_cents: row
            .try_get("max_drawdown_cents")
            .map_err(postgres_decode_error)?,
        hold_seconds: row.try_get("hold_seconds").map_err(postgres_decode_error)?,
        created_at: row.try_get("created_at").map_err(postgres_decode_error)?,
    })
}

fn high_probability_bucket_stats_from_row(
    row: &sqlx::postgres::PgRow,
) -> Result<HighProbabilityBucketStats> {
    let bucket_dimensions: Json<Value> = row
        .try_get("bucket_dimensions")
        .map_err(postgres_decode_error)?;
    Ok(HighProbabilityBucketStats {
        id: row.try_get("id").map_err(postgres_decode_error)?,
        model_version: row.try_get("model_version").map_err(postgres_decode_error)?,
        bucket_key: row.try_get("bucket_key").map_err(postgres_decode_error)?,
        bucket_dimensions: bucket_dimensions.0,
        sample_count: i64_count_to_u64(row.try_get("sample_count").map_err(postgres_decode_error)?),
        win_count: i64_count_to_u64(row.try_get("win_count").map_err(postgres_decode_error)?),
        win_rate: row.try_get("win_rate").map_err(postgres_decode_error)?,
        fair_probability: row
            .try_get("fair_probability")
            .map_err(postgres_decode_error)?,
        confidence_low: row.try_get("confidence_low").map_err(postgres_decode_error)?,
        confidence_high: row.try_get("confidence_high").map_err(postgres_decode_error)?,
        expected_pnl: row.try_get("expected_pnl").map_err(postgres_decode_error)?,
        avg_max_drawdown_cents: row
            .try_get("avg_max_drawdown_cents")
            .map_err(postgres_decode_error)?,
        break_70_rate: row.try_get("break_70_rate").map_err(postgres_decode_error)?,
        break_60_rate: row.try_get("break_60_rate").map_err(postgres_decode_error)?,
        break_50_rate: row.try_get("break_50_rate").map_err(postgres_decode_error)?,
        avg_hold_seconds: row
            .try_get("avg_hold_seconds")
            .map_err(postgres_decode_error)?,
        recommended_max_entry_price: row
            .try_get("recommended_max_entry_price")
            .map_err(postgres_decode_error)?,
        computed_at: row.try_get("computed_at").map_err(postgres_decode_error)?,
    })
}

fn high_probability_observation_from_row(
    row: &sqlx::postgres::PgRow,
) -> Result<HighProbabilityObservation> {
    let mode_raw: String = row.try_get("mode").map_err(postgres_decode_error)?;
    let decision_raw: String = row.try_get("decision").map_err(postgres_decode_error)?;
    let reasons: Json<Vec<String>> = row.try_get("reasons").map_err(postgres_decode_error)?;
    Ok(HighProbabilityObservation {
        id: row.try_get("id").map_err(postgres_decode_error)?,
        observed_at: row.try_get("observed_at").map_err(postgres_decode_error)?,
        condition_id: row.try_get("condition_id").map_err(postgres_decode_error)?,
        token_id: row.try_get("token_id").map_err(postgres_decode_error)?,
        mode: HighProbabilityMode::from_str(&mode_raw)?,
        executable_price: row
            .try_get("executable_price")
            .map_err(postgres_decode_error)?,
        fair_probability: row
            .try_get("fair_probability")
            .map_err(postgres_decode_error)?,
        net_edge: row.try_get("net_edge").map_err(postgres_decode_error)?,
        recommended_size_usd: row
            .try_get("recommended_size_usd")
            .map_err(postgres_decode_error)?,
        decision: HighProbabilityDecision::from_str(&decision_raw)?,
        reasons: reasons.0,
        model_version: row.try_get("model_version").map_err(postgres_decode_error)?,
        created_at: row.try_get("created_at").map_err(postgres_decode_error)?,
    })
}

fn high_probability_reward_candle_sample_input_from_row(
    row: &sqlx::postgres::PgRow,
) -> Result<HighProbabilityRewardCandleSampleInput> {
    let outcome_status_raw: String = row
        .try_get("outcome_status")
        .map_err(postgres_decode_error)?;
    let risk_tags: Json<Vec<String>> = row.try_get("risk_tags").map_err(postgres_decode_error)?;
    Ok(HighProbabilityRewardCandleSampleInput {
        condition_id: row.try_get("condition_id").map_err(postgres_decode_error)?,
        token_id: row.try_get("token_id").map_err(postgres_decode_error)?,
        outcome: row.try_get("outcome").map_err(postgres_decode_error)?,
        bucket_start: row.try_get("bucket_start").map_err(postgres_decode_error)?,
        close: row.try_get("close").map_err(postgres_decode_error)?,
        spread_cents_close: row
            .try_get("spread_cents_close")
            .map_err(postgres_decode_error)?,
        market_type: row.try_get("market_type").map_err(postgres_decode_error)?,
        liquidity_usd: row.try_get("liquidity_usd").map_err(postgres_decode_error)?,
        resolved_at: row.try_get("resolved_at").map_err(postgres_decode_error)?,
        outcome_status: HighProbabilityMarketOutcomeStatus::from_str(&outcome_status_raw)?,
        winning_token_id: row
            .try_get("winning_token_id")
            .map_err(postgres_decode_error)?,
        risk_tags: risk_tags.0,
    })
}

fn high_probability_observe_candidate_from_row(
    row: &sqlx::postgres::PgRow,
) -> Result<HighProbabilityObserveCandidate> {
    let risk_tags: Json<Vec<String>> = row.try_get("risk_tags").map_err(postgres_decode_error)?;
    Ok(HighProbabilityObserveCandidate {
        condition_id: row.try_get("condition_id").map_err(postgres_decode_error)?,
        token_id: row.try_get("token_id").map_err(postgres_decode_error)?,
        outcome: row.try_get("outcome").map_err(postgres_decode_error)?,
        observed_at: row.try_get("observed_at").map_err(postgres_decode_error)?,
        reference_price: row
            .try_get("reference_price")
            .map_err(postgres_decode_error)?,
        reference_spread_cents: row
            .try_get("reference_spread_cents")
            .map_err(postgres_decode_error)?,
        market_type: row.try_get("market_type").map_err(postgres_decode_error)?,
        liquidity_usd: row.try_get("liquidity_usd").map_err(postgres_decode_error)?,
        end_at: row.try_get("end_at").map_err(postgres_decode_error)?,
        risk_tags: risk_tags.0,
    })
}

fn high_probability_backtest_run_from_row(
    row: &sqlx::postgres::PgRow,
) -> Result<HighProbabilityBacktestRun> {
    let notes: Json<Vec<String>> = row.try_get("notes").map_err(postgres_decode_error)?;
    let exit_rule_reports: Json<Vec<HighProbabilityBacktestExitRuleReport>> = row
        .try_get("exit_rule_reports")
        .map_err(postgres_decode_error)?;
    let report = HighProbabilityBacktestReport {
        generated_at: row.try_get("run_at").map_err(postgres_decode_error)?,
        model_version: row.try_get("model_version").map_err(postgres_decode_error)?,
        market_scope: row.try_get("market_scope").map_err(postgres_decode_error)?,
        sample_limit: i64_count_to_u16(row.try_get("sample_limit").map_err(postgres_decode_error)?),
        train_sample_count: i64_count_to_usize(
            row.try_get("train_sample_count")
                .map_err(postgres_decode_error)?,
        ),
        test_sample_count: i64_count_to_usize(
            row.try_get("test_sample_count")
                .map_err(postgres_decode_error)?,
        ),
        candidate_count: i64_count_to_usize(
            row.try_get("candidate_count")
                .map_err(postgres_decode_error)?,
        ),
        trade_count: i64_count_to_usize(row.try_get("trade_count").map_err(postgres_decode_error)?),
        skipped_no_bucket_count: i64_count_to_usize(
            row.try_get("skipped_no_bucket_count")
                .map_err(postgres_decode_error)?,
        ),
        skipped_no_edge_count: i64_count_to_usize(
            row.try_get("skipped_no_edge_count")
                .map_err(postgres_decode_error)?,
        ),
        win_trades: i64_count_to_usize(row.try_get("win_trades").map_err(postgres_decode_error)?),
        loss_trades: i64_count_to_usize(row.try_get("loss_trades").map_err(postgres_decode_error)?),
        win_rate: row.try_get("win_rate").map_err(postgres_decode_error)?,
        total_pnl: row.try_get("total_pnl").map_err(postgres_decode_error)?,
        average_pnl: row.try_get("average_pnl").map_err(postgres_decode_error)?,
        total_entry_cost: row
            .try_get("total_entry_cost")
            .map_err(postgres_decode_error)?,
        roi: row.try_get("roi").map_err(postgres_decode_error)?,
        max_drawdown: row.try_get("max_drawdown").map_err(postgres_decode_error)?,
        average_entry_price: row
            .try_get("average_entry_price")
            .map_err(postgres_decode_error)?,
        train_start_at: row.try_get("train_start_at").map_err(postgres_decode_error)?,
        train_end_at: row.try_get("train_end_at").map_err(postgres_decode_error)?,
        test_start_at: row.try_get("test_start_at").map_err(postgres_decode_error)?,
        test_end_at: row.try_get("test_end_at").map_err(postgres_decode_error)?,
        exit_rule_reports: exit_rule_reports.0,
        notes: notes.0,
    };
    Ok(HighProbabilityBacktestRun {
        id: row.try_get("id").map_err(postgres_decode_error)?,
        run_at: row.try_get("run_at").map_err(postgres_decode_error)?,
        report,
    })
}

fn high_probability_backtest_trade_from_row(
    row: &sqlx::postgres::PgRow,
) -> Result<HighProbabilityBacktestTrade> {
    let outcome_raw: String = row.try_get("outcome").map_err(postgres_decode_error)?;
    let reasons: Json<Vec<String>> = row.try_get("reasons").map_err(postgres_decode_error)?;
    Ok(HighProbabilityBacktestTrade {
        id: row.try_get("id").map_err(postgres_decode_error)?,
        run_id: row.try_get("run_id").map_err(postgres_decode_error)?,
        sample_id: row.try_get("sample_id").map_err(postgres_decode_error)?,
        condition_id: row.try_get("condition_id").map_err(postgres_decode_error)?,
        token_id: row.try_get("token_id").map_err(postgres_decode_error)?,
        sampled_at: row.try_get("sampled_at").map_err(postgres_decode_error)?,
        bucket_key: row.try_get("bucket_key").map_err(postgres_decode_error)?,
        executable_price: row
            .try_get("executable_price")
            .map_err(postgres_decode_error)?,
        fair_probability: row
            .try_get("fair_probability")
            .map_err(postgres_decode_error)?,
        net_edge: row.try_get("net_edge").map_err(postgres_decode_error)?,
        recommended_max_entry_price: row
            .try_get("recommended_max_entry_price")
            .map_err(postgres_decode_error)?,
        outcome: HighProbabilitySampleOutcome::from_str(&outcome_raw)?,
        settlement_pnl: row.try_get("settlement_pnl").map_err(postgres_decode_error)?,
        cumulative_pnl: row.try_get("cumulative_pnl").map_err(postgres_decode_error)?,
        drawdown: row.try_get("drawdown").map_err(postgres_decode_error)?,
        reasons: reasons.0,
        created_at: row.try_get("created_at").map_err(postgres_decode_error)?,
    })
}

fn high_probability_fair_value_from_row(
    row: &sqlx::postgres::PgRow,
) -> Result<FairValueEstimate> {
    let side_raw: String = row.try_get("side_used").map_err(postgres_decode_error)?;
    let reason_codes: Json<Vec<String>> = row.try_get("reason_codes").map_err(postgres_decode_error)?;
    let fallback_level_raw: i16 = row
        .try_get("fallback_level")
        .map_err(postgres_decode_error)?;
    Ok(FairValueEstimate {
        id: row.try_get("id").map_err(postgres_decode_error)?,
        condition_id: row.try_get("condition_id").map_err(postgres_decode_error)?,
        token_id: row.try_get("token_id").map_err(postgres_decode_error)?,
        side_used: FairValueSide::from_str(&side_raw)?,
        price_used: row.try_get("price_used").map_err(postgres_decode_error)?,
        fair_yes_low: row.try_get("fair_yes_low").map_err(postgres_decode_error)?,
        fair_yes_mid: row.try_get("fair_yes_mid").map_err(postgres_decode_error)?,
        fair_yes_high: row.try_get("fair_yes_high").map_err(postgres_decode_error)?,
        market_implied: row.try_get("market_implied").map_err(postgres_decode_error)?,
        base_rate: row.try_get("base_rate").map_err(postgres_decode_error)?,
        confidence: row.try_get("confidence").map_err(postgres_decode_error)?,
        uncertainty_cents: row
            .try_get("uncertainty_cents")
            .map_err(postgres_decode_error)?,
        sample_count: i64_count_to_u64(row.try_get("sample_count").map_err(postgres_decode_error)?),
        bucket_key: row.try_get("bucket_key").map_err(postgres_decode_error)?,
        fallback_level: u8::try_from(fallback_level_raw).unwrap_or(0),
        model_version: row.try_get("model_version").map_err(postgres_decode_error)?,
        input_hash: row.try_get("input_hash").map_err(postgres_decode_error)?,
        reason_codes: reason_codes.0,
        live_eligible: row.try_get("live_eligible").map_err(postgres_decode_error)?,
        computed_at: row.try_get("computed_at").map_err(postgres_decode_error)?,
        expires_at: row.try_get("expires_at").map_err(postgres_decode_error)?,
        created_at: row.try_get("created_at").map_err(postgres_decode_error)?,
    })
}

fn apply_high_probability_config_value(
    config: &mut HighProbabilityConfig,
    key: &str,
    value: &str,
) -> Result<()> {
    match key {
        "enabled" => config.enabled = value.parse::<bool>().unwrap_or(false),
        "mode" => config.mode = HighProbabilityMode::from_str(value)?,
        "market_scope" => config.market_scope = value.to_string(),
        "model_version" => config.model_version = value.to_string(),
        "min_required_edge" => {
            config.min_required_edge = parse_high_probability_decimal_config(key, value)?;
        }
        "fee_buffer" => config.fee_buffer = parse_high_probability_decimal_config(key, value)?,
        "default_risk_margin" => {
            config.default_risk_margin = parse_high_probability_decimal_config(key, value)?;
        }
        "min_confidence" => {
            config.min_confidence = parse_high_probability_decimal_config(key, value)?;
        }
        "min_bucket_samples" => {
            config.min_bucket_samples = value.parse::<u64>().unwrap_or(config.min_bucket_samples);
        }
        "max_spread_cents" => {
            config.max_spread_cents = parse_high_probability_decimal_config(key, value)?;
        }
        "min_depth_usd" => config.min_depth_usd = parse_high_probability_decimal_config(key, value)?,
        "max_single_trade_usd" => {
            config.max_single_trade_usd = parse_high_probability_decimal_config(key, value)?;
        }
        "max_single_market_exposure_usd" => {
            config.max_single_market_exposure_usd =
                parse_high_probability_decimal_config(key, value)?;
        }
        "max_daily_new_notional_usd" => {
            config.max_daily_new_notional_usd =
                parse_high_probability_decimal_config(key, value)?;
        }
        "conservative_kelly_multiplier" => {
            config.conservative_kelly_multiplier =
                parse_high_probability_decimal_config(key, value)?;
        }
        "excluded_risk_tags" => {
            config.excluded_risk_tags = serde_json::from_str(value).unwrap_or_default();
        }
        "fair_value_enabled" => config.fair_value_enabled = value.parse::<bool>().unwrap_or(false),
        "fair_value_ttl_sec" => {
            config.fair_value_ttl_sec = value.parse::<i64>().unwrap_or(config.fair_value_ttl_sec);
        }
        "fair_value_market_weight" => {
            config.fair_value_market_weight = parse_high_probability_decimal_config(key, value)?;
        }
        "fair_value_base_rate_weight" => {
            config.fair_value_base_rate_weight =
                parse_high_probability_decimal_config(key, value)?;
        }
        "fair_value_target_sample_count" => {
            config.fair_value_target_sample_count =
                value.parse::<u64>().unwrap_or(config.fair_value_target_sample_count);
        }
        "fair_value_max_uncertainty_cents" => {
            config.fair_value_max_uncertainty_cents =
                parse_high_probability_decimal_config(key, value)?;
        }
        "fair_value_stale_book_ms" => {
            config.fair_value_stale_book_ms =
                value.parse::<i64>().unwrap_or(config.fair_value_stale_book_ms);
        }
        _ => {}
    }
    Ok(())
}

fn high_probability_config_entries(config: &HighProbabilityConfig) -> Vec<(&'static str, String)> {
    let config = config.clone().normalized();
    vec![
        ("enabled", config.enabled.to_string()),
        ("mode", config.mode.as_str().to_string()),
        ("market_scope", config.market_scope),
        ("model_version", config.model_version),
        ("min_required_edge", config.min_required_edge.to_string()),
        ("fee_buffer", config.fee_buffer.to_string()),
        ("default_risk_margin", config.default_risk_margin.to_string()),
        ("min_confidence", config.min_confidence.to_string()),
        ("min_bucket_samples", config.min_bucket_samples.to_string()),
        ("max_spread_cents", config.max_spread_cents.to_string()),
        ("min_depth_usd", config.min_depth_usd.to_string()),
        ("max_single_trade_usd", config.max_single_trade_usd.to_string()),
        (
            "max_single_market_exposure_usd",
            config.max_single_market_exposure_usd.to_string(),
        ),
        (
            "max_daily_new_notional_usd",
            config.max_daily_new_notional_usd.to_string(),
        ),
        (
            "conservative_kelly_multiplier",
            config.conservative_kelly_multiplier.to_string(),
        ),
        (
            "excluded_risk_tags",
            serde_json::to_string(&config.excluded_risk_tags).unwrap_or_else(|_| "[]".to_string()),
        ),
        ("fair_value_enabled", config.fair_value_enabled.to_string()),
        ("fair_value_ttl_sec", config.fair_value_ttl_sec.to_string()),
        (
            "fair_value_market_weight",
            config.fair_value_market_weight.to_string(),
        ),
        (
            "fair_value_base_rate_weight",
            config.fair_value_base_rate_weight.to_string(),
        ),
        (
            "fair_value_target_sample_count",
            config.fair_value_target_sample_count.to_string(),
        ),
        (
            "fair_value_max_uncertainty_cents",
            config.fair_value_max_uncertainty_cents.to_string(),
        ),
        (
            "fair_value_stale_book_ms",
            config.fair_value_stale_book_ms.to_string(),
        ),
    ]
}

fn parse_high_probability_decimal_config(key: &str, value: &str) -> Result<Decimal> {
    Decimal::from_str(value).map_err(|error| {
        AppError::invalid_input(
            "HIGH_PROBABILITY_CONFIG_INVALID",
            format!("invalid high probability config value for {key}: {error}"),
        )
    })
}

fn i64_count_to_usize(value: i64) -> usize {
    usize::try_from(value.max(0)).unwrap_or(usize::MAX)
}

fn i64_count_to_u16(value: i64) -> u16 {
    u16::try_from(value.max(0)).unwrap_or(u16::MAX)
}
