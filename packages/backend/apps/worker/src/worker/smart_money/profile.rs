fn build_smart_money_profile(
    wallet_address: &str,
    inputs: &SmartMoneyWalletInputs,
) -> SmartWalletProfile {
    let trade_samples = smart_money_trade_samples(inputs);
    let trade_count = trade_samples.len() as i64;
    let total_volume_usd = trade_samples
        .iter()
        .map(|sample| sample.notional_usd)
        .sum::<Decimal>();
    let realized_pnl_usd = inputs
        .closed_positions
        .iter()
        .map(|position| position.realized_pnl)
        .chain(inputs.positions.iter().map(|position| position.realized_pnl))
        .sum::<Decimal>();
    let roi = if total_volume_usd > Decimal::ZERO {
        realized_pnl_usd / total_volume_usd
    } else {
        Decimal::ZERO
    };
    let settled_trade_count = inputs.closed_positions.len() as i64;
    let win_rate = if inputs.closed_positions.is_empty() {
        Decimal::ZERO
    } else {
        let wins = inputs
            .closed_positions
            .iter()
            .filter(|position| position.realized_pnl > Decimal::ZERO)
            .count() as i64;
        Decimal::from(wins) / Decimal::from(settled_trade_count)
    };
    let avg_trade_usd = if trade_count > 0 {
        total_volume_usd / Decimal::from(trade_count)
    } else {
        Decimal::ZERO
    };
    let median_trade_usd = median_decimal(
        trade_samples
            .iter()
            .map(|sample| sample.notional_usd)
            .collect(),
    );
    let active_days = trade_samples
        .iter()
        .map(|sample| unix_day(sample.timestamp))
        .collect::<HashSet<_>>()
        .len() as i64;
    let markets_traded = trade_samples
        .iter()
        .filter_map(|sample| non_empty_text(&sample.condition_id))
        .collect::<HashSet<_>>()
        .len() as i64;
    let market_concentration_score = market_concentration_score(&trade_samples, total_volume_usd);
    let last_trade_at = trade_samples.iter().map(|sample| sample.timestamp).max();

    SmartWalletProfile {
        wallet_address: wallet_address.to_string(),
        trade_count,
        settled_trade_count,
        total_volume_usd,
        realized_pnl_usd,
        roi,
        win_rate,
        max_drawdown_usd: Decimal::ZERO,
        avg_trade_usd,
        median_trade_usd,
        avg_hold_secs: None,
        active_days,
        markets_traded,
        category_concentration_score: Decimal::ZERO,
        market_concentration_score,
        low_liquidity_trade_ratio: Decimal::ZERO,
        stale_copy_window_ratio: Decimal::ZERO,
        last_trade_at,
        updated_at: OffsetDateTime::now_utc(),
    }
}

#[derive(Debug, Clone)]
struct SmartMoneyTradeSample {
    condition_id: String,
    notional_usd: Decimal,
    timestamp: OffsetDateTime,
}

fn smart_money_trade_samples(inputs: &SmartMoneyWalletInputs) -> Vec<SmartMoneyTradeSample> {
    if !inputs.trades.is_empty() {
        return inputs
            .trades
            .iter()
            .filter(|trade| smart_money_side(&trade.side).is_some())
            .map(|trade| SmartMoneyTradeSample {
                condition_id: trade.condition_id.clone(),
                notional_usd: trade.price * trade.size,
                timestamp: trade.timestamp,
            })
            .collect();
    }

    inputs
        .activities
        .iter()
        .filter(|activity| activity.kind.eq_ignore_ascii_case("TRADE"))
        .filter(|activity| smart_money_side(&activity.side).is_some())
        .map(|activity| SmartMoneyTradeSample {
            condition_id: activity.condition_id.clone(),
            notional_usd: activity.usdc_size,
            timestamp: activity.timestamp,
        })
        .collect()
}

fn market_concentration_score(
    trade_samples: &[SmartMoneyTradeSample],
    total_volume_usd: Decimal,
) -> Decimal {
    if total_volume_usd <= Decimal::ZERO {
        return Decimal::ZERO;
    }

    let mut market_volume = HashMap::<String, Decimal>::new();
    for sample in trade_samples {
        if let Some(condition_id) = non_empty_text(&sample.condition_id) {
            *market_volume.entry(condition_id).or_default() += sample.notional_usd;
        }
    }
    market_volume
        .values()
        .copied()
        .max()
        .map(|max_volume| max_volume / total_volume_usd)
        .unwrap_or(Decimal::ZERO)
}

fn build_smart_money_trades(
    wallet_address: &str,
    activities: &[PolymarketWalletActivity],
) -> Vec<SmartWalletTrade> {
    let discovered_at = OffsetDateTime::now_utc();
    activities
        .iter()
        .filter(|activity| activity.kind.eq_ignore_ascii_case("TRADE"))
        .filter_map(|activity| {
            let side = smart_money_side(&activity.side)?;
            let condition_id = non_empty_text(&activity.condition_id)?;
            Some(SmartWalletTrade {
                id: smart_money_trade_id(wallet_address, activity),
                wallet_address: wallet_address.to_string(),
                source: SMART_MONEY_TRADE_SOURCE_ACTIVITY.to_string(),
                condition_id,
                token_id: non_empty_text(&activity.asset),
                side,
                outcome: non_empty_text(&activity.outcome),
                price: activity.price,
                size: activity.size,
                notional_usd: activity.usdc_size.max(Decimal::ZERO),
                tx_hash: non_empty_text(&activity.transaction_hash),
                source_timestamp: activity.timestamp,
                discovered_at,
                raw: json!({
                    "proxy_wallet": activity.proxy_wallet,
                    "kind": activity.kind,
                    "side": activity.side,
                    "asset": activity.asset,
                    "condition_id": activity.condition_id,
                    "outcome": activity.outcome,
                    "outcome_index": activity.outcome_index,
                    "title": activity.title,
                    "slug": activity.slug,
                    "transaction_hash": activity.transaction_hash
                }),
            })
        })
        .collect()
}

fn smart_money_side(raw: &str) -> Option<SmartMoneySide> {
    if raw.eq_ignore_ascii_case("BUY") {
        Some(SmartMoneySide::Buy)
    } else if raw.eq_ignore_ascii_case("SELL") {
        Some(SmartMoneySide::Sell)
    } else {
        None
    }
}

fn smart_money_trade_id(wallet_address: &str, activity: &PolymarketWalletActivity) -> String {
    format!(
        "{}:{}:{}:{}:{}:{}:{}:{}:{}",
        SMART_MONEY_TRADE_SOURCE_ACTIVITY,
        sanitize_smart_money_id_part(wallet_address),
        sanitize_smart_money_id_part(&activity.transaction_hash),
        sanitize_smart_money_id_part(&activity.asset),
        sanitize_smart_money_id_part(&activity.side),
        activity.outcome_index,
        sanitize_smart_money_id_part(&activity.price.to_string()),
        sanitize_smart_money_id_part(&activity.size.to_string()),
        activity.timestamp.unix_timestamp()
    )
}

fn sanitize_smart_money_id_part(value: &str) -> String {
    let sanitized = value
        .trim()
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .collect::<String>();
    if sanitized.is_empty() {
        "unknown".to_string()
    } else {
        sanitized.to_lowercase()
    }
}

fn median_decimal(mut values: Vec<Decimal>) -> Decimal {
    if values.is_empty() {
        return Decimal::ZERO;
    }
    values.sort_unstable();
    let mid = values.len() / 2;
    if values.len() % 2 == 0 {
        (values[mid - 1] + values[mid]) / Decimal::from(2)
    } else {
        values[mid]
    }
}

fn unix_day(timestamp: OffsetDateTime) -> i64 {
    timestamp.unix_timestamp().div_euclid(86_400)
}

fn non_empty_text(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}
