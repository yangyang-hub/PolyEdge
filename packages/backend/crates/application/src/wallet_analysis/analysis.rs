use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
use std::collections::{HashMap, HashSet};
use time::OffsetDateTime;

use super::helpers::*;
use super::models::*;

/// Build a comprehensive wallet analysis report from raw Polymarket data.
/// Pure function — no I/O, no persistence.
#[allow(clippy::too_many_arguments)]
pub fn build_wallet_analysis_report(
    address: &str,
    profile_name: &str,
    profile_pseudonym: &str,
    profile_bio: &str,
    profile_x_username: &str,
    profile_image: &str,
    profile_created_at: &str,
    profile_verified: bool,
    _activities: &[ActivityInput],
    closed_positions: &[ClosedPositionInput],
    open_positions: &[OpenPositionInput],
    trades: &[TradeInput],
    leaderboard_rank: i64,
    leaderboard_volume: Decimal,
    leaderboard_pnl: Decimal,
    portfolio_value: Decimal,
    total_markets_traded: i64,
) -> WalletAnalysisReport {
    let profile = build_profile(
        address,
        profile_name,
        profile_pseudonym,
        profile_bio,
        profile_x_username,
        profile_image,
        profile_created_at,
        profile_verified,
        leaderboard_rank,
        leaderboard_volume,
        leaderboard_pnl,
        portfolio_value,
        total_markets_traded,
    );

    let pnl = build_pnl_stats(closed_positions, open_positions);
    let activity = build_activity_stats(trades);
    let categories = build_category_breakdown(trades, closed_positions);
    let market_aggregates = group_trades_by_market(trades);
    let style = build_style_stats(trades, &activity, &market_aggregates);
    let risk = build_risk_profile(open_positions, closed_positions, portfolio_value);
    let top_markets = build_top_markets(&market_aggregates, closed_positions);
    let recent_trades = build_recent_trades(trades, 20);
    let (winners, losers) = build_winners_losers(closed_positions, 5);
    let recent_closed = build_recent_closed(closed_positions, 10);

    WalletAnalysisReport {
        profile,
        pnl,
        activity,
        categories,
        style,
        risk,
        top_markets,
        recent_trades,
        winners,
        losers,
        recent_closed,
    }
}

// ── Section builders ─────────────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
fn build_profile(
    address: &str,
    name: &str,
    pseudonym: &str,
    bio: &str,
    x_username: &str,
    image: &str,
    created_at: &str,
    verified: bool,
    rank: i64,
    volume: Decimal,
    pnl: Decimal,
    portfolio_value: Decimal,
    total_markets_traded: i64,
) -> WalletProfile {
    WalletProfile {
        address: address.to_string(),
        name: name.to_string(),
        pseudonym: pseudonym.to_string(),
        bio: bio.to_string(),
        x_username: x_username.to_string(),
        profile_image: image.to_string(),
        created_at: created_at.to_string(),
        verified_badge: verified,
        leaderboard_rank: rank,
        leaderboard_volume: volume,
        leaderboard_pnl: pnl,
        portfolio_value,
        total_markets_traded,
    }
}

fn build_pnl_stats(
    closed: &[ClosedPositionInput],
    open: &[OpenPositionInput],
) -> WalletPnlStats {
    let total_realized: Decimal = closed.iter().map(|c| c.realized_pnl).sum();

    let mut total_unrealized = Decimal::ZERO;
    for p in open {
        let cost = p.size * p.avg_price;
        let value = p.size * p.cur_price;
        total_unrealized += value - cost;
    }

    let total_pnl = total_realized + total_unrealized;

    // Cost basis: sum of (total_bought * avg_price) for closed, sum of (size * avg_price) for open.
    let cost_closed: Decimal = closed.iter().map(|c| c.total_bought * c.avg_price).sum();
    let cost_open: Decimal = open.iter().map(|p| p.size * p.avg_price).sum();
    let cost_basis = cost_closed + cost_open;
    let overall_roi = if cost_basis > Decimal::ZERO {
        total_pnl / cost_basis
    } else {
        Decimal::ZERO
    };

    // Win rate from closed positions.
    let winners_closed = closed.iter().filter(|c| c.realized_pnl > Decimal::ZERO).count();
    let win_rate_closed = if closed.is_empty() {
        Decimal::ZERO
    } else {
        Decimal::from(winners_closed) / Decimal::from(closed.len())
    };

    // Win rate from open positions.
    let winners_open = open
        .iter()
        .filter(|p| {
            let pnl = p.realized_pnl + (p.size * (p.cur_price - p.avg_price));
            pnl > Decimal::ZERO
        })
        .count();
    let win_rate_open = if open.is_empty() {
        Decimal::ZERO
    } else {
        Decimal::from(winners_open) / Decimal::from(open.len())
    };

    let largest_win = closed
        .iter()
        .map(|c| c.realized_pnl)
        .max()
        .unwrap_or(Decimal::ZERO)
        .max(Decimal::ZERO);
    let largest_loss = closed
        .iter()
        .map(|c| c.realized_pnl)
        .min()
        .unwrap_or(Decimal::ZERO)
        .min(Decimal::ZERO);

    WalletPnlStats {
        total_realized_pnl: total_realized,
        total_unrealized_pnl: total_unrealized,
        total_pnl,
        overall_roi,
        win_rate_closed,
        win_rate_open,
        largest_win,
        largest_loss,
        closed_positions_count: closed.len() as i32,
        open_positions_count: open.len() as i32,
    }
}

fn build_activity_stats(trades: &[TradeInput]) -> WalletActivityStats {
    let total_trades = trades.len() as i32;
    let total_volume: Decimal = trades.iter().map(|t| t.price * t.size).sum();
    let avg_trade = if total_trades > 0 {
        total_volume / Decimal::from(total_trades)
    } else {
        Decimal::ZERO
    };

    // Median trade size.
    let mut sizes: Vec<Decimal> = trades.iter().map(|t| t.price * t.size).collect();
    sizes.sort();
    let median_trade = if sizes.is_empty() {
        Decimal::ZERO
    } else if sizes.len() % 2 == 0 {
        let mid = sizes.len() / 2;
        (sizes[mid - 1] + sizes[mid]) / Decimal::from(2)
    } else {
        sizes[sizes.len() / 2]
    };

    // First and last trade timestamps.
    let first_trade_at = trades.iter().map(|t| t.timestamp).min();
    let last_trade_at = trades.iter().map(|t| t.timestamp).max();

    // Unique trading days.
    let mut days: HashSet<String> = HashSet::new();
    for t in trades {
        let date_str = format!(
            "{:04}-{:02}-{:02}",
            t.timestamp.year(),
            t.timestamp.month() as u8,
            t.timestamp.day()
        );
        days.insert(date_str);
    }
    let trading_days = days.len() as i32;

    let avg_trades_per_day = if trading_days > 0 {
        Decimal::from(total_trades) / Decimal::from(trading_days)
    } else {
        Decimal::ZERO
    };

    // Buy/sell ratio.
    let buy_count = trades
        .iter()
        .filter(|t| t.side.eq_ignore_ascii_case("BUY"))
        .count();
    let buy_ratio = if total_trades > 0 {
        Decimal::from(buy_count) / Decimal::from(total_trades)
    } else {
        Decimal::ZERO
    };

    let total_buy_volume: Decimal = trades
        .iter()
        .filter(|t| t.side.eq_ignore_ascii_case("BUY"))
        .map(|t| t.price * t.size)
        .sum();
    let total_sell_volume = total_volume - total_buy_volume;

    WalletActivityStats {
        total_volume_usd: total_volume,
        total_trades,
        avg_trade_usd: avg_trade,
        median_trade_usd: median_trade,
        first_trade_at,
        last_trade_at,
        trading_days,
        avg_trades_per_day,
        buy_ratio,
        total_buy_volume,
        total_sell_volume,
    }
}

fn build_category_breakdown(
    trades: &[TradeInput],
    closed: &[ClosedPositionInput],
) -> Vec<WalletCategoryItem> {
    let mut cats: HashMap<String, WalletCategoryItem> = HashMap::new();

    // Aggregate from trades.
    for t in trades {
        let cat = infer_category(&t.title).to_string();
        let entry = cats.entry(cat.clone()).or_insert_with(|| WalletCategoryItem {
            category: cat,
            trade_count: 0,
            volume_usd: Decimal::ZERO,
            pnl: Decimal::ZERO,
            win_count: 0,
            loss_count: 0,
        });
        entry.trade_count += 1;
        entry.volume_usd += t.price * t.size;
    }

    // Aggregate P&L from closed positions.
    for c in closed {
        let cat = infer_category(&c.title).to_string();
        let entry = cats.entry(cat.clone()).or_insert_with(|| WalletCategoryItem {
            category: cat,
            trade_count: 0,
            volume_usd: Decimal::ZERO,
            pnl: Decimal::ZERO,
            win_count: 0,
            loss_count: 0,
        });
        entry.pnl += c.realized_pnl;
        if c.realized_pnl > Decimal::ZERO {
            entry.win_count += 1;
        } else if c.realized_pnl < Decimal::ZERO {
            entry.loss_count += 1;
        }
    }

    let mut result: Vec<WalletCategoryItem> = cats.into_values().collect();
    result.sort_by(|a, b| b.volume_usd.cmp(&a.volume_usd));
    result
}

fn build_style_stats(
    trades: &[TradeInput],
    activity: &WalletActivityStats,
    _market_aggregates: &HashMap<String, MarketAggregate>,
) -> WalletStyleStats {
    // Estimate average hold duration from BUY → next SELL on the same market.
    // Simple approach: for each market, pair first BUY with first SELL.
    let mut market_buys: HashMap<String, Vec<OffsetDateTime>> = HashMap::new();
    let mut market_sells: HashMap<String, Vec<OffsetDateTime>> = HashMap::new();
    for t in trades {
        if t.side.eq_ignore_ascii_case("BUY") {
            market_buys
                .entry(t.condition_id.clone())
                .or_default()
                .push(t.timestamp);
        } else {
            market_sells
                .entry(t.condition_id.clone())
                .or_default()
                .push(t.timestamp);
        }
    }

    let mut hold_durations_hours: Vec<Decimal> = Vec::new();
    for (cid, buys) in &market_buys {
        if let Some(sells) = market_sells.get(cid) {
            // Pair earliest buy with earliest sell.
            if let (Some(&first_buy), Some(&first_sell)) = (buys.first(), sells.first()) {
                if first_sell > first_buy {
                    let duration_secs = (first_sell - first_buy).whole_seconds();
                    if duration_secs > 0 {
                        hold_durations_hours.push(Decimal::from(duration_secs) / Decimal::from(3600));
                    }
                }
            }
        }
    }

    let avg_hold_hours = if hold_durations_hours.is_empty() {
        Decimal::ZERO
    } else {
        hold_durations_hours.iter().sum::<Decimal>() / Decimal::from(hold_durations_hours.len())
    };

    // Trade size standard deviation.
    let trade_sizes: Vec<Decimal> = trades.iter().map(|t| t.price * t.size).collect();
    let trade_size_stddev = decimal_stddev(&trade_sizes);

    // Directional bias: net buy volume ratio.
    let total_volume = activity.total_buy_volume + activity.total_sell_volume;
    let directional_bias = if total_volume > Decimal::ZERO {
        (activity.total_buy_volume - activity.total_sell_volume) / total_volume
    } else {
        Decimal::ZERO
    };

    // Preferred price range: 25th and 75th percentile of entry prices.
    let mut prices: Vec<Decimal> = trades
        .iter()
        .filter(|t| t.side.eq_ignore_ascii_case("BUY"))
        .map(|t| t.price)
        .collect();
    prices.sort();
    let preferred_low = percentile(&prices, 25);
    let preferred_high = percentile(&prices, 75);

    let price_concentration = if preferred_high - preferred_low < decimal("0.15") {
        "narrow"
    } else if preferred_high - preferred_low < decimal("0.40") {
        "moderate"
    } else {
        "wide"
    }
    .to_string();

    let trades_per_day = activity.avg_trades_per_day;
    let style_label = classify_style(avg_hold_hours, trades_per_day).to_string();

    WalletStyleStats {
        style_label,
        avg_hold_hours,
        trade_size_stddev,
        directional_bias,
        preferred_price_range_low: preferred_low,
        preferred_price_range_high: preferred_high,
        price_concentration,
    }
}

fn build_risk_profile(
    open: &[OpenPositionInput],
    closed: &[ClosedPositionInput],
    portfolio_value: Decimal,
) -> WalletRiskProfile {
    // Max single market exposure.
    let max_position_value: Decimal = open
        .iter()
        .map(|p| p.size * p.cur_price)
        .max()
        .unwrap_or(Decimal::ZERO);
    let max_single_market_exposure_pct = if portfolio_value > Decimal::ZERO {
        max_position_value / portfolio_value
    } else {
        Decimal::ZERO
    };

    // Average position size as % of portfolio.
    let total_position_value: Decimal = open.iter().map(|p| p.size * p.cur_price).sum();
    let avg_position_size_pct = if portfolio_value > Decimal::ZERO && !open.is_empty() {
        (total_position_value / Decimal::from(open.len())) / portfolio_value
    } else {
        Decimal::ZERO
    };

    // Max drawdown estimate from closed positions: largest negative realized P&L.
    let max_drawdown = closed
        .iter()
        .map(|c| c.realized_pnl)
        .min()
        .unwrap_or(Decimal::ZERO)
        .min(Decimal::ZERO);

    // Diversification: Shannon entropy of position sizes.
    let position_weights: Vec<Decimal> = open.iter().map(|p| p.size * p.cur_price).collect();
    let diversification_score = shannon_entropy(&position_weights);

    let concentration_label = if open.len() <= 2 {
        "concentrated"
    } else if diversification_score.to_f64().unwrap_or(0.0) > 2.0 {
        "well_diversified"
    } else {
        "moderate"
    }
    .to_string();

    WalletRiskProfile {
        max_single_market_exposure_pct,
        max_drawdown_estimate: max_drawdown,
        avg_position_size_pct,
        diversification_score,
        concentration_label,
    }
}

fn build_top_markets(
    market_aggregates: &HashMap<String, MarketAggregate>,
    closed: &[ClosedPositionInput],
) -> Vec<WalletTopMarket> {
    // Build a P&L map from closed positions.
    let mut pnl_map: HashMap<String, Decimal> = HashMap::new();
    for c in closed {
        *pnl_map.entry(c.condition_id.clone()).or_insert(Decimal::ZERO) += c.realized_pnl;
    }

    let mut markets: Vec<WalletTopMarket> = market_aggregates
        .iter()
        .map(|(cid, agg)| WalletTopMarket {
            condition_id: cid.clone(),
            title: agg.title.clone(),
            slug: agg.slug.clone(),
            trade_count: agg.trade_count,
            volume_usd: agg.volume_usd,
            pnl: pnl_map.get(cid).copied().unwrap_or(Decimal::ZERO),
            buy_count: agg.buy_count,
            sell_count: agg.sell_count,
        })
        .collect();
    markets.sort_by(|a, b| b.volume_usd.cmp(&a.volume_usd));
    markets.truncate(10);
    markets
}

fn build_recent_trades(trades: &[TradeInput], limit: usize) -> Vec<WalletRecentTrade> {
    let mut sorted = trades.to_vec();
    sorted.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
    sorted
        .into_iter()
        .take(limit)
        .map(|t| WalletRecentTrade {
            side: t.side,
            title: t.title,
            slug: t.slug,
            outcome: t.outcome,
            price: t.price,
            size: t.size,
            notional_usd: t.price * t.size,
            timestamp: t.timestamp,
        })
        .collect()
}

fn build_winners_losers(
    closed: &[ClosedPositionInput],
    limit: usize,
) -> (Vec<WalletClosedPositionItem>, Vec<WalletClosedPositionItem>) {
    let mut sorted = closed.to_vec();
    sorted.sort_by(|a, b| b.realized_pnl.cmp(&a.realized_pnl));

    let winners: Vec<WalletClosedPositionItem> = sorted
        .iter()
        .filter(|c| c.realized_pnl > Decimal::ZERO)
        .take(limit)
        .map(closed_position_item)
        .collect();

    let losers: Vec<WalletClosedPositionItem> = sorted
        .iter()
        .filter(|c| c.realized_pnl < Decimal::ZERO)
        .rev()
        .take(limit)
        .map(closed_position_item)
        .collect();

    (winners, losers)
}

fn build_recent_closed(
    closed: &[ClosedPositionInput],
    limit: usize,
) -> Vec<WalletClosedPositionItem> {
    let mut sorted = closed.to_vec();
    sorted.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
    sorted
        .iter()
        .take(limit)
        .map(closed_position_item)
        .collect()
}

// ── Helpers ──────────────────────────────────────────────────────────────────

fn closed_position_item(c: &ClosedPositionInput) -> WalletClosedPositionItem {
    WalletClosedPositionItem {
        title: c.title.clone(),
        slug: c.slug.clone(),
        outcome: c.outcome.clone(),
        avg_price: c.avg_price,
        realized_pnl: c.realized_pnl,
        total_bought: c.total_bought,
        end_date: c.end_date.clone(),
        timestamp: c.timestamp,
    }
}

fn percentile(sorted: &[Decimal], p: u8) -> Decimal {
    if sorted.is_empty() {
        return Decimal::ZERO;
    }
    let idx = (f64::from(p) / 100.0 * sorted.len() as f64).floor() as usize;
    let idx = idx.min(sorted.len() - 1);
    sorted[idx]
}

fn decimal(s: &str) -> Decimal {
    s.parse().unwrap_or(Decimal::ZERO)
}
