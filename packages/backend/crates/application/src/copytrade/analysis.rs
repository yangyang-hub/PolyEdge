/// Build wallet analysis stats from raw Data API activity and position data.
/// Pure function — no I/O, no persistence.
pub fn build_wallet_analysis(
    activities: &[WalletActivityInput],
    positions: &[WalletPositionInput],
) -> WalletAnalysisStats {
    let now = OffsetDateTime::now_utc();
    let trades: Vec<_> = activities
        .iter()
        .filter(|a| a.kind.eq_ignore_ascii_case("TRADE"))
        .collect();
    let trade_count = trades.len() as i32;
    let total_volume: Decimal = trades.iter().map(|t| t.usdc_size).sum();
    let avg_trade = if trade_count > 0 {
        total_volume / Decimal::from(trade_count)
    } else {
        Decimal::ZERO
    };

    // Approximate PnL from positions data: sum realized + (position.value - position.cost)
    let realized_pnl: Decimal = positions.iter().map(|p| p.realized_pnl).sum();
    let unrealized_pnl: Decimal = positions
        .iter()
        .map(|p| {
            let cost = p.size * p.avg_price;
            let value = p.size * p.cur_price;
            value - cost
        })
        .sum();
    let total_pnl = realized_pnl + unrealized_pnl;

    // Win rate from positions with positive pnl
    let positions_with_pnl: Vec<_> = positions
        .iter()
        .filter(|p| p.realized_pnl != Decimal::ZERO || (p.size > Decimal::ZERO && p.avg_price > Decimal::ZERO))
        .collect();
    let winners = positions_with_pnl
        .iter()
        .filter(|p| {
            let pnl = p.realized_pnl + (p.size * (p.cur_price - p.avg_price));
            pnl > Decimal::ZERO
        })
        .count();
    let win_rate = if positions_with_pnl.is_empty() {
        Decimal::ZERO
    } else {
        Decimal::from(winners) / Decimal::from(positions_with_pnl.len())
    };

    let cost_basis: Decimal = positions.iter().map(|p| p.size * p.avg_price).sum();
    let roi = if cost_basis > Decimal::ZERO {
        total_pnl / cost_basis
    } else {
        Decimal::ZERO
    };

    let mut markets = std::collections::HashSet::new();
    for activity in activities {
        if !activity.condition_id.is_empty() {
            markets.insert(activity.condition_id.clone());
        }
    }
    let last_active = trades
        .iter()
        .map(|t| t.timestamp)
        .max();

    WalletAnalysisStats {
        trades_window: trade_count,
        volume_window_usd: total_volume,
        realized_pnl_window: total_pnl,
        win_rate,
        roi,
        avg_trade_usd: avg_trade,
        markets_traded: markets.len() as i32,
        last_active_at: last_active,
        last_analyzed_at: Some(now),
    }
}
