use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
use std::collections::HashMap;

/// Infer a market category from its title using keyword matching.
pub fn infer_category(title: &str) -> &'static str {
    let lower = title.to_lowercase();

    // Ordered by specificity — more specific matches first.
    if lower.contains("bitcoin")
        || lower.contains("ethereum")
        || lower.contains("crypto")
        || lower.contains("solana")
        || lower.contains("defi")
        || lower.contains("nft")
        || lower.contains("token")
        || lower.contains("blockchain")
        || lower.contains("binance")
        || lower.contains("memecoin")
        || lower.contains("airdrop")
    {
        return "crypto";
    }
    if lower.contains("trump")
        || lower.contains("biden")
        || lower.contains("election")
        || lower.contains("president")
        || lower.contains("congress")
        || lower.contains("senate")
        || lower.contains("democrat")
        || lower.contains("republican")
        || lower.contains("vote")
        || lower.contains("poll")
        || lower.contains("governor")
        || lower.contains("political")
        || lower.contains("parliament")
    {
        return "politics";
    }
    if lower.contains("nba")
        || lower.contains("nfl")
        || lower.contains("mlb")
        || lower.contains("nhl")
        || lower.contains("soccer")
        || lower.contains("football")
        || lower.contains("basketball")
        || lower.contains("tennis")
        || lower.contains("baseball")
        || lower.contains("hockey")
        || lower.contains("boxing")
        || lower.contains("mma")
        || lower.contains("ufc")
        || lower.contains("f1")
        || lower.contains("grand prix")
        || lower.contains("world cup")
        || lower.contains("championship")
        || lower.contains("playoff")
        || lower.contains("match")
        || lower.contains("game 7")
    {
        return "sports";
    }
    if lower.contains("fed")
        || lower.contains("interest rate")
        || lower.contains("inflation")
        || lower.contains("gdp")
        || lower.contains("recession")
        || lower.contains("stock")
        || lower.contains("s&p")
        || lower.contains("nasdaq")
        || lower.contains("dow")
        || lower.contains("bond")
        || lower.contains("treasury")
        || lower.contains("earnings")
        || lower.contains("ipo")
        || lower.contains("etf")
    {
        return "finance";
    }
    if lower.contains("ai")
        || lower.contains("openai")
        || lower.contains("chatgpt")
        || lower.contains("gpt")
        || lower.contains("artificial intelligence")
        || lower.contains("machine learning")
        || lower.contains("tech")
        || lower.contains("apple")
        || lower.contains("google")
        || lower.contains("microsoft")
        || lower.contains("spacex")
        || lower.contains("tesla")
        || lower.contains("nvidia")
    {
        return "tech";
    }
    if lower.contains("war")
        || lower.contains("ukraine")
        || lower.contains("russia")
        || lower.contains("china")
        || lower.contains("nato")
        || lower.contains("military")
        || lower.contains("sanction")
        || lower.contains("ceasefire")
        || lower.contains("conflict")
        || lower.contains("geopolit")
    {
        return "geopolitics";
    }
    if lower.contains("oscar")
        || lower.contains("grammy")
        || lower.contains("emmy")
        || lower.contains("movie")
        || lower.contains("film")
        || lower.contains("celebrity")
        || lower.contains("taylor swift")
        || lower.contains("kanye")
        || lower.contains("meme")
        || lower.contains("viral")
    {
        return "culture";
    }
    if lower.contains("weather")
        || lower.contains("hurricane")
        || lower.contains("earthquake")
        || lower.contains("flood")
        || lower.contains("tornado")
        || lower.contains("temperature")
        || lower.contains("climate")
    {
        return "weather";
    }

    "other"
}

/// Compute the standard deviation of a list of `Decimal` values.
pub fn decimal_stddev(values: &[Decimal]) -> Decimal {
    if values.len() < 2 {
        return Decimal::ZERO;
    }
    let count = Decimal::from(values.len());
    let mean: Decimal = values.iter().sum::<Decimal>() / count;
    let variance: Decimal = values
        .iter()
        .map(|v| {
            let diff = *v - mean;
            diff * diff
        })
        .sum::<Decimal>()
        / count;
    // Approximate square root for Decimal.
    decimal_sqrt(variance)
}

/// Approximate square root for `Decimal` using Newton's method.
fn decimal_sqrt(value: Decimal) -> Decimal {
    if value <= Decimal::ZERO {
        return Decimal::ZERO;
    }
    let f = value.to_f64().unwrap_or(0.0);
    if f <= 0.0 {
        return Decimal::ZERO;
    }
    Decimal::from_f64_retain(f.sqrt()).unwrap_or(Decimal::ZERO)
}

/// Compute Shannon entropy (base 2) of a distribution. Higher = more diversified.
pub fn shannon_entropy(weights: &[Decimal]) -> Decimal {
    let total: Decimal = weights.iter().sum();
    if total <= Decimal::ZERO || weights.is_empty() {
        return Decimal::ZERO;
    }
    let mut entropy = Decimal::ZERO;
    for w in weights {
        if *w > Decimal::ZERO {
            let p = *w / total;
            let p_f64 = p.to_f64().unwrap_or(0.0);
            if p_f64 > 0.0 {
                entropy += Decimal::from_f64_retain(-p_f64 * p_f64.log2()).unwrap_or(Decimal::ZERO);
            }
        }
    }
    entropy
}

/// Classify trading style based on hold duration and frequency.
pub fn classify_style(avg_hold_hours: Decimal, trades_per_day: Decimal) -> &'static str {
    let hold = avg_hold_hours.to_f64().unwrap_or(0.0);
    let freq = trades_per_day.to_f64().unwrap_or(0.0);

    if freq > 20.0 && hold < 2.0 {
        "scalper"
    } else if freq > 5.0 && hold < 24.0 {
        "day_trader"
    } else if (24.0..168.0).contains(&hold) {
        "swing_trader"
    } else if hold >= 168.0 {
        "position_trader"
    } else {
        "mixed"
    }
}

/// Group trades by market (condition_id) and compute per-market aggregates.
pub fn group_trades_by_market(trades: &[super::TradeInput]) -> HashMap<String, MarketAggregate> {
    let mut map: HashMap<String, MarketAggregate> = HashMap::new();
    for trade in trades {
        let entry = map
            .entry(trade.condition_id.clone())
            .or_insert_with(|| MarketAggregate {
                title: trade.title.clone(),
                slug: trade.slug.clone(),
                trade_count: 0,
                volume_usd: Decimal::ZERO,
                buy_count: 0,
                sell_count: 0,
                buy_volume: Decimal::ZERO,
                sell_volume: Decimal::ZERO,
            });
        entry.trade_count += 1;
        let notional = trade.price * trade.size;
        entry.volume_usd += notional;
        if trade.side.eq_ignore_ascii_case("BUY") {
            entry.buy_count += 1;
            entry.buy_volume += notional;
        } else {
            entry.sell_count += 1;
            entry.sell_volume += notional;
        }
    }
    map
}

/// Per-market aggregate used during analysis computation.
#[derive(Debug, Clone)]
pub struct MarketAggregate {
    pub title: String,
    pub slug: String,
    pub trade_count: i32,
    pub volume_usd: Decimal,
    pub buy_count: i32,
    pub sell_count: i32,
    pub buy_volume: Decimal,
    pub sell_volume: Decimal,
}
