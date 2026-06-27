fn smart_wallet_candidate_from_row(row: &sqlx::postgres::PgRow) -> Result<SmartWalletCandidate> {
    let status_raw: String = row.try_get("status").map_err(postgres_decode_error)?;
    let raw: Json<Value> = row.try_get("raw").map_err(postgres_decode_error)?;
    Ok(SmartWalletCandidate {
        id: row.try_get("id").map_err(postgres_decode_error)?,
        wallet_address: row.try_get("wallet_address").map_err(postgres_decode_error)?,
        source: row.try_get("source").map_err(postgres_decode_error)?,
        status: SmartWalletCandidateStatus::from_str(&status_raw)?,
        first_seen_at: row.try_get("first_seen_at").map_err(postgres_decode_error)?,
        last_seen_at: row.try_get("last_seen_at").map_err(postgres_decode_error)?,
        last_analyzed_at: row
            .try_get("last_analyzed_at")
            .map_err(postgres_decode_error)?,
        promoted_at: row.try_get("promoted_at").map_err(postgres_decode_error)?,
        rejected_at: row.try_get("rejected_at").map_err(postgres_decode_error)?,
        reason: row.try_get("reason").map_err(postgres_decode_error)?,
        raw: raw.0,
    })
}

fn smart_wallet_profile_from_row(row: &sqlx::postgres::PgRow) -> Result<SmartWalletProfile> {
    Ok(SmartWalletProfile {
        wallet_address: row.try_get("wallet_address").map_err(postgres_decode_error)?,
        trade_count: row.try_get("trade_count").map_err(postgres_decode_error)?,
        settled_trade_count: row
            .try_get("settled_trade_count")
            .map_err(postgres_decode_error)?,
        total_volume_usd: row
            .try_get("total_volume_usd")
            .map_err(postgres_decode_error)?,
        realized_pnl_usd: row
            .try_get("realized_pnl_usd")
            .map_err(postgres_decode_error)?,
        roi: row.try_get("roi").map_err(postgres_decode_error)?,
        win_rate: row.try_get("win_rate").map_err(postgres_decode_error)?,
        max_drawdown_usd: row
            .try_get("max_drawdown_usd")
            .map_err(postgres_decode_error)?,
        avg_trade_usd: row.try_get("avg_trade_usd").map_err(postgres_decode_error)?,
        median_trade_usd: row
            .try_get("median_trade_usd")
            .map_err(postgres_decode_error)?,
        avg_hold_secs: row.try_get("avg_hold_secs").map_err(postgres_decode_error)?,
        active_days: row.try_get("active_days").map_err(postgres_decode_error)?,
        markets_traded: row
            .try_get("markets_traded")
            .map_err(postgres_decode_error)?,
        category_concentration_score: row
            .try_get("category_concentration_score")
            .map_err(postgres_decode_error)?,
        market_concentration_score: row
            .try_get("market_concentration_score")
            .map_err(postgres_decode_error)?,
        low_liquidity_trade_ratio: row
            .try_get("low_liquidity_trade_ratio")
            .map_err(postgres_decode_error)?,
        stale_copy_window_ratio: row
            .try_get("stale_copy_window_ratio")
            .map_err(postgres_decode_error)?,
        last_trade_at: row.try_get("last_trade_at").map_err(postgres_decode_error)?,
        updated_at: row.try_get("updated_at").map_err(postgres_decode_error)?,
    })
}

fn smart_wallet_score_from_row(row: &sqlx::postgres::PgRow) -> Result<SmartWalletScore> {
    let tier_raw: String = row.try_get("tier").map_err(postgres_decode_error)?;
    let explanation: Json<Value> = row
        .try_get("explanation")
        .map_err(postgres_decode_error)?;
    Ok(SmartWalletScore {
        wallet_address: row.try_get("wallet_address").map_err(postgres_decode_error)?,
        total_score: row.try_get("total_score").map_err(postgres_decode_error)?,
        profit_score: row.try_get("profit_score").map_err(postgres_decode_error)?,
        consistency_score: row
            .try_get("consistency_score")
            .map_err(postgres_decode_error)?,
        risk_score: row.try_get("risk_score").map_err(postgres_decode_error)?,
        liquidity_score: row
            .try_get("liquidity_score")
            .map_err(postgres_decode_error)?,
        recency_score: row.try_get("recency_score").map_err(postgres_decode_error)?,
        copyability_score: row
            .try_get("copyability_score")
            .map_err(postgres_decode_error)?,
        tier: SmartWalletTier::from_str(&tier_raw)?,
        explanation: explanation.0,
        scoring_version: row
            .try_get("scoring_version")
            .map_err(postgres_decode_error)?,
        updated_at: row.try_get("updated_at").map_err(postgres_decode_error)?,
    })
}

fn smart_wallet_trade_from_row(row: &sqlx::postgres::PgRow) -> Result<SmartWalletTrade> {
    let side_raw: String = row.try_get("side").map_err(postgres_decode_error)?;
    let raw: Json<Value> = row.try_get("raw").map_err(postgres_decode_error)?;
    Ok(SmartWalletTrade {
        id: row.try_get("id").map_err(postgres_decode_error)?,
        wallet_address: row.try_get("wallet_address").map_err(postgres_decode_error)?,
        source: row.try_get("source").map_err(postgres_decode_error)?,
        condition_id: row.try_get("condition_id").map_err(postgres_decode_error)?,
        token_id: row.try_get("token_id").map_err(postgres_decode_error)?,
        side: SmartMoneySide::from_str(&side_raw)?,
        outcome: row.try_get("outcome").map_err(postgres_decode_error)?,
        price: row.try_get("price").map_err(postgres_decode_error)?,
        size: row.try_get("size").map_err(postgres_decode_error)?,
        notional_usd: row.try_get("notional_usd").map_err(postgres_decode_error)?,
        tx_hash: row.try_get("tx_hash").map_err(postgres_decode_error)?,
        source_timestamp: row
            .try_get("source_timestamp")
            .map_err(postgres_decode_error)?,
        discovered_at: row.try_get("discovered_at").map_err(postgres_decode_error)?,
        raw: raw.0,
    })
}

fn smart_signal_from_row(row: &sqlx::postgres::PgRow) -> Result<SmartSignal> {
    let side_raw: String = row.try_get("side").map_err(postgres_decode_error)?;
    let status_raw: String = row.try_get("status").map_err(postgres_decode_error)?;
    Ok(SmartSignal {
        id: row.try_get("id").map_err(postgres_decode_error)?,
        source_trade_id: row
            .try_get("source_trade_id")
            .map_err(postgres_decode_error)?,
        wallet_address: row.try_get("wallet_address").map_err(postgres_decode_error)?,
        condition_id: row.try_get("condition_id").map_err(postgres_decode_error)?,
        token_id: row.try_get("token_id").map_err(postgres_decode_error)?,
        side: SmartMoneySide::from_str(&side_raw)?,
        source_price: row.try_get("source_price").map_err(postgres_decode_error)?,
        current_price: row.try_get("current_price").map_err(postgres_decode_error)?,
        price_slippage_cents: row
            .try_get("price_slippage_cents")
            .map_err(postgres_decode_error)?,
        latency_ms: row.try_get("latency_ms").map_err(postgres_decode_error)?,
        source_notional_usd: row
            .try_get("source_notional_usd")
            .map_err(postgres_decode_error)?,
        consensus_wallet_count: row
            .try_get("consensus_wallet_count")
            .map_err(postgres_decode_error)?,
        score: row.try_get("score").map_err(postgres_decode_error)?,
        status: SmartSignalStatus::from_str(&status_raw)?,
        reason: row.try_get("reason").map_err(postgres_decode_error)?,
        created_at: row.try_get("created_at").map_err(postgres_decode_error)?,
        updated_at: row.try_get("updated_at").map_err(postgres_decode_error)?,
    })
}

fn smart_signal_decision_from_row(row: &sqlx::postgres::PgRow) -> Result<SmartSignalDecision> {
    let decision_raw: String = row.try_get("decision").map_err(postgres_decode_error)?;
    let mode_raw: String = row.try_get("mode").map_err(postgres_decode_error)?;
    let risk_checks: Json<Value> = row
        .try_get("risk_checks")
        .map_err(postgres_decode_error)?;
    Ok(SmartSignalDecision {
        id: row.try_get("id").map_err(postgres_decode_error)?,
        signal_id: row.try_get("signal_id").map_err(postgres_decode_error)?,
        decision: SmartSignalDecisionValue::from_str(&decision_raw)?,
        stage: row.try_get("stage").map_err(postgres_decode_error)?,
        mode: SmartMoneyMode::from_str(&mode_raw)?,
        rejection_reason: row
            .try_get("rejection_reason")
            .map_err(postgres_decode_error)?,
        risk_checks: risk_checks.0,
        decided_at: row.try_get("decided_at").map_err(postgres_decode_error)?,
    })
}

fn smart_signal_advisory_from_row(row: &sqlx::postgres::PgRow) -> Result<SmartSignalAdvisory> {
    let recommendation_raw: String = row.try_get("recommendation").map_err(postgres_decode_error)?;
    let risk_tags: Json<Vec<String>> = row.try_get("risk_tags").map_err(postgres_decode_error)?;
    let reasons: Json<Vec<String>> = row.try_get("reasons").map_err(postgres_decode_error)?;
    let raw_output: Json<Value> = row
        .try_get("raw_output")
        .map_err(postgres_decode_error)?;
    Ok(SmartSignalAdvisory {
        id: row.try_get("id").map_err(postgres_decode_error)?,
        signal_id: row.try_get("signal_id").map_err(postgres_decode_error)?,
        provider: row.try_get("provider").map_err(postgres_decode_error)?,
        request_format: row
            .try_get("request_format")
            .map_err(postgres_decode_error)?,
        model: row.try_get("model").map_err(postgres_decode_error)?,
        input_hash: row.try_get("input_hash").map_err(postgres_decode_error)?,
        recommendation: SmartSignalDecisionValue::from_str(&recommendation_raw)?,
        confidence: row.try_get("confidence").map_err(postgres_decode_error)?,
        risk_tags: risk_tags.0,
        summary: row.try_get("summary").map_err(postgres_decode_error)?,
        reasons: reasons.0,
        raw_output: raw_output.0,
        expires_at: row.try_get("expires_at").map_err(postgres_decode_error)?,
        created_at: row.try_get("created_at").map_err(postgres_decode_error)?,
    })
}
