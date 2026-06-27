const SMART_SIGNAL_ADVISORY_SCHEMA_VERSION: u16 = 1;

pub fn build_smart_signal_advisory_request(
    provider: &str,
    request_format: &str,
    model: &str,
    config: &SmartMoneyConfig,
    signal: &SmartSignal,
    context: SmartSignalAdvisoryContext<'_>,
) -> Result<SmartSignalAdvisoryRequest> {
    let provider = provider.trim().to_string();
    let request_format = request_format.trim().to_string();
    let model = model.trim().to_string();
    if provider.is_empty() || request_format.is_empty() || model.is_empty() {
        return Err(AppError::invalid_input(
            "SMART_SIGNAL_ADVISORY_PROVIDER_INVALID",
            "provider, request_format and model are required",
        ));
    }

    let cache_key_payload =
        smart_signal_advisory_cache_key_payload(config, signal, &context);
    let payload = json!({
        "schema_version": SMART_SIGNAL_ADVISORY_SCHEMA_VERSION,
        "task": "Assess whether this Smart Money source trade remains copyable. Return allow, observe, or reject with risk tags and concise reasons. Do not propose order placement; hard trading rules are enforced outside the model.",
        "provider_contract": {
            "recommendation_values": ["allow", "observe", "reject"],
            "confidence_range": [0, 1],
            "fail_closed_policy": "Malformed or low-confidence provider output must be treated as observe/reject by the caller.",
        },
        "signal": smart_signal_advisory_signal_payload(signal),
        "source_trade": context.source_trade.map(smart_signal_advisory_trade_payload),
        "wallet_profile": context.profile.map(smart_signal_advisory_profile_payload),
        "wallet_score": context.score.map(smart_signal_advisory_score_payload),
        "deterministic_config": smart_signal_advisory_config_payload(config),
        "provider_cache_policy": smart_signal_advisory_cache_policy_payload(context.ttl_sec, context.now),
    });

    Ok(SmartSignalAdvisoryRequest {
        signal_id: signal.id,
        provider,
        request_format,
        model,
        input_hash: smart_signal_advisory_input_hash(&cache_key_payload)?,
        payload,
    })
}

#[derive(Debug, Clone, Copy)]
pub struct SmartSignalAdvisoryContext<'a> {
    pub source_trade: Option<&'a SmartWalletTrade>,
    pub profile: Option<&'a SmartWalletProfile>,
    pub score: Option<&'a SmartWalletScore>,
    pub now: OffsetDateTime,
    pub ttl_sec: u64,
}

fn smart_signal_advisory_signal_payload(signal: &SmartSignal) -> Value {
    json!({
        "id": signal.id,
        "source_trade_id": signal.source_trade_id,
        "wallet_address": signal.wallet_address,
        "condition_id": signal.condition_id,
        "token_id": signal.token_id,
        "side": signal.side,
        "source_price": signal.source_price,
        "current_price": signal.current_price,
        "price_slippage_cents": signal.price_slippage_cents,
        "latency_ms": signal.latency_ms,
        "source_notional_usd": signal.source_notional_usd,
        "consensus_wallet_count": signal.consensus_wallet_count,
        "score": signal.score,
        "status": signal.status,
        "reason": signal.reason,
        "created_at": signal.created_at,
        "updated_at": signal.updated_at,
    })
}

fn smart_signal_advisory_trade_payload(trade: &SmartWalletTrade) -> Value {
    json!({
        "id": trade.id,
        "wallet_address": trade.wallet_address,
        "source": trade.source,
        "condition_id": trade.condition_id,
        "token_id": trade.token_id,
        "side": trade.side,
        "outcome": trade.outcome,
        "price": trade.price,
        "size": trade.size,
        "notional_usd": trade.notional_usd,
        "tx_hash": trade.tx_hash,
        "source_timestamp": trade.source_timestamp,
        "discovered_at": trade.discovered_at,
    })
}

fn smart_signal_advisory_profile_payload(profile: &SmartWalletProfile) -> Value {
    json!({
        "wallet_address": profile.wallet_address,
        "trade_count": profile.trade_count,
        "settled_trade_count": profile.settled_trade_count,
        "total_volume_usd": profile.total_volume_usd,
        "realized_pnl_usd": profile.realized_pnl_usd,
        "roi": profile.roi,
        "win_rate": profile.win_rate,
        "max_drawdown_usd": profile.max_drawdown_usd,
        "avg_trade_usd": profile.avg_trade_usd,
        "median_trade_usd": profile.median_trade_usd,
        "avg_hold_secs": profile.avg_hold_secs,
        "active_days": profile.active_days,
        "markets_traded": profile.markets_traded,
        "category_concentration_score": profile.category_concentration_score,
        "market_concentration_score": profile.market_concentration_score,
        "low_liquidity_trade_ratio": profile.low_liquidity_trade_ratio,
        "stale_copy_window_ratio": profile.stale_copy_window_ratio,
        "last_trade_at": profile.last_trade_at,
    })
}

fn smart_signal_advisory_score_payload(score: &SmartWalletScore) -> Value {
    json!({
        "wallet_address": score.wallet_address,
        "total_score": score.total_score,
        "profit_score": score.profit_score,
        "consistency_score": score.consistency_score,
        "risk_score": score.risk_score,
        "liquidity_score": score.liquidity_score,
        "recency_score": score.recency_score,
        "copyability_score": score.copyability_score,
        "tier": score.tier,
        "scoring_version": score.scoring_version,
        "explanation": score.explanation,
    })
}

fn smart_signal_advisory_config_payload(config: &SmartMoneyConfig) -> Value {
    json!({
        "mode": config.mode,
        "min_trade_count": config.min_trade_count,
        "min_settled_trade_count": config.min_settled_trade_count,
        "min_total_volume_usd": config.min_total_volume_usd,
        "min_copyability_score": config.min_copyability_score,
        "max_signal_age_ms": config.max_signal_age_ms,
        "max_price_slippage_cents": config.max_price_slippage_cents,
        "min_orderbook_depth_usd": config.min_orderbook_depth_usd,
        "max_wallet_exposure_usd": config.max_wallet_exposure_usd,
        "max_market_exposure_usd": config.max_market_exposure_usd,
        "max_daily_notional_usd": config.max_daily_notional_usd,
    })
}

fn smart_signal_advisory_cache_policy_payload(ttl_sec: u64, now: OffsetDateTime) -> Value {
    let ttl_sec = ttl_sec.min(i64::MAX as u64);
    json!({
        "ttl_sec": ttl_sec,
        "requested_at_utc": now,
        "base_expires_at_utc": now + time::Duration::seconds(ttl_sec as i64),
        "decision_reuse_policy": "Provider output may be reused only for this exact signal cache key until expires_at.",
    })
}

fn smart_signal_advisory_cache_key_payload(
    config: &SmartMoneyConfig,
    signal: &SmartSignal,
    context: &SmartSignalAdvisoryContext<'_>,
) -> Value {
    json!({
        "schema_version": SMART_SIGNAL_ADVISORY_SCHEMA_VERSION,
        "signal": smart_signal_advisory_signal_payload(signal),
        "source_trade": context.source_trade.map(smart_signal_advisory_trade_payload),
        "wallet_profile": context.profile.map(smart_signal_advisory_profile_payload),
        "wallet_score": context.score.map(smart_signal_advisory_score_payload),
        "deterministic_config": smart_signal_advisory_config_payload(config),
    })
}

fn smart_signal_advisory_input_hash(payload: &Value) -> Result<String> {
    let bytes = serde_json::to_vec(payload).map_err(|error| {
        AppError::internal(
            "SMART_SIGNAL_ADVISORY_INPUT_HASH_FAILED",
            format!("failed to serialize smart signal advisory input: {error}"),
        )
    })?;
    let digest = Sha256::digest(bytes);
    Ok(format!("{digest:x}"))
}
