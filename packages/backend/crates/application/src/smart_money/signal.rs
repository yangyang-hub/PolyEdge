fn build_smart_signal_from_trade(
    config: &SmartMoneyConfig,
    trade: &SmartWalletTrade,
    quote: Option<&SmartSignalBookQuote>,
    wallet_score: Option<Decimal>,
    now: OffsetDateTime,
) -> SmartSignal {
    let latency_ms = signal_latency_ms(trade.source_timestamp, now);
    let mut status = SmartSignalStatus::Observe;
    let mut reason = "deterministic gate passed; execution not implemented".to_string();
    let mut current_price = None;
    let mut price_slippage_cents = None;

    if latency_ms > config.max_signal_age_ms {
        status = SmartSignalStatus::Rejected;
        reason = "signal age exceeded".to_string();
    } else if trade.token_id.as_deref().is_none_or(str::is_empty) {
        status = SmartSignalStatus::Rejected;
        reason = "missing token id".to_string();
    } else if let Some(quote) = quote {
        let side_price = smart_signal_current_price(trade.side, quote);
        let side_depth = smart_signal_depth_usd(trade.side, quote);
        current_price = side_price;
        if let Some(price) = side_price {
            let adverse_slippage = smart_signal_adverse_slippage_cents(trade.side, trade.price, price);
            price_slippage_cents = Some(adverse_slippage);
            if adverse_slippage > config.max_price_slippage_cents {
                status = SmartSignalStatus::Rejected;
                reason = "price slippage exceeded".to_string();
            } else if side_depth < config.min_orderbook_depth_usd {
                status = SmartSignalStatus::Rejected;
                reason = "orderbook depth below threshold".to_string();
            }
        } else {
            status = SmartSignalStatus::Rejected;
            reason = "missing side price".to_string();
        }
    } else {
        status = SmartSignalStatus::Rejected;
        reason = "orderbook unavailable".to_string();
    }

    let score = wallet_score.unwrap_or(Decimal::ZERO);
    SmartSignal {
        id: 0,
        source_trade_id: trade.id.clone(),
        wallet_address: trade.wallet_address.clone(),
        condition_id: trade.condition_id.clone(),
        token_id: trade.token_id.clone(),
        side: trade.side,
        source_price: trade.price,
        current_price,
        price_slippage_cents,
        latency_ms: Some(latency_ms),
        source_notional_usd: trade.notional_usd,
        consensus_wallet_count: 1,
        score,
        status,
        reason: Some(reason),
        created_at: now,
        updated_at: now,
    }
}

fn build_smart_signal_decision_for_gate(
    signal: &SmartSignal,
    now: OffsetDateTime,
) -> Option<SmartSignalDecision> {
    let decision = match signal.status {
        SmartSignalStatus::Observe => SmartSignalDecisionValue::Observe,
        SmartSignalStatus::Rejected => SmartSignalDecisionValue::Reject,
        _ => return None,
    };
    Some(SmartSignalDecision {
        id: 0,
        signal_id: signal.id,
        decision,
        stage: "deterministic_gate".to_string(),
        mode: SmartMoneyMode::Observe,
        rejection_reason: (decision == SmartSignalDecisionValue::Reject)
            .then(|| signal.reason.clone())
            .flatten(),
        risk_checks: json!({
            "source_trade_id": signal.source_trade_id.clone(),
            "status": signal.status.as_str(),
            "reason": signal.reason.clone(),
            "latency_ms": signal.latency_ms,
            "source_price": signal.source_price,
            "current_price": signal.current_price,
            "price_slippage_cents": signal.price_slippage_cents,
            "source_notional_usd": signal.source_notional_usd,
            "consensus_wallet_count": signal.consensus_wallet_count,
            "score": signal.score
        }),
        decided_at: now,
    })
}

fn signal_latency_ms(source_timestamp: OffsetDateTime, now: OffsetDateTime) -> i64 {
    let millis = (now - source_timestamp).whole_milliseconds();
    i64::try_from(millis.max(0)).unwrap_or(i64::MAX)
}

fn smart_signal_current_price(
    side: SmartMoneySide,
    quote: &SmartSignalBookQuote,
) -> Option<Decimal> {
    match side {
        SmartMoneySide::Buy => quote.best_ask,
        SmartMoneySide::Sell => quote.best_bid,
    }
}

fn smart_signal_depth_usd(side: SmartMoneySide, quote: &SmartSignalBookQuote) -> Decimal {
    match side {
        SmartMoneySide::Buy => quote.ask_depth_usd,
        SmartMoneySide::Sell => quote.bid_depth_usd,
    }
}

fn smart_signal_adverse_slippage_cents(
    side: SmartMoneySide,
    source_price: Decimal,
    current_price: Decimal,
) -> Decimal {
    let raw = match side {
        SmartMoneySide::Buy => current_price - source_price,
        SmartMoneySide::Sell => source_price - current_price,
    };
    (raw * Decimal::from(100)).max(Decimal::ZERO).round_dp(8)
}
