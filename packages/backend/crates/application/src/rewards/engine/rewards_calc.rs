// Polymarket-faithful reward accrual and the book-top / Q_min math it relies on.

impl TickContext {
    /// Polymarket-faithful reward accrual for the resting two-sided entry quotes.
    fn accrue_rewards(
        &mut self,
        plan_index: &HashMap<String, RewardQuotePlan>,
        books: &HashMap<String, RewardOrderBook>,
        elapsed: i64,
    ) {
        let time_fraction = Decimal::from(elapsed) / decimal("86400");
        let c = decimal_to_f64(self.config.single_sided_divisor_c).max(1.0);
        let now = self.now;
        let stale_book_ms = self.config.stale_book_ms;

        // condition_id -> (yes contribution, no contribution) as (order index, S*size).
        for (condition_id, plan) in plan_index {
            let Some(midpoint) = plan.midpoint else {
                continue;
            };
            if !plan.eligible {
                continue;
            }
            let m = decimal_to_f64(midpoint);
            let v = decimal_to_f64(Decimal::min(
                normalize_reward_spread_cents(plan.rewards_max_spread),
                self.config.max_spread_cents,
            ));
            if v <= 0.0 {
                continue;
            }
            let yes_token = plan.legs.first().map(|leg| leg.token_id.clone());
            let no_token = plan.legs.get(1).map(|leg| leg.token_id.clone());

            let mut q_bid = 0.0_f64;
            let mut q_ask = 0.0_f64;
            // (order_index, weight)
            let mut weights: Vec<(usize, f64)> = Vec::new();

            for (index, order) in self.orders.iter().enumerate() {
                if order.condition_id != *condition_id
                    || order.side != RewardOrderSide::Buy
                    || order.status != ManagedRewardOrderStatus::Open
                {
                    continue;
                }
                let resting = decimal_to_f64((order.size - order.filled_size).max(Decimal::ZERO));
                if resting <= 0.0 {
                    continue;
                }
                let price = decimal_to_f64(order.price);
                let is_yes = yes_token.as_deref() == Some(order.token_id.as_str());
                let is_no = no_token.as_deref() == Some(order.token_id.as_str());

                // YES buy is a bid at `price`; NO buy is a synthetic YES ask at `1 - price`.
                let spread_cents = if is_yes {
                    (m - price) * 100.0
                } else if is_no {
                    ((1.0 - price) - m) * 100.0
                } else {
                    (m - price).abs() * 100.0
                };
                if spread_cents < 0.0 || spread_cents > v {
                    continue;
                }
                let s = ((v - spread_cents) / v).powi(2);
                let weight = s * resting;
                if is_no {
                    q_ask += weight;
                } else {
                    q_bid += weight;
                }
                weights.push((index, weight));
            }

            let single_sided_ok = (0.10..=0.90).contains(&m);
            let q_min = combine_qmin(q_bid, q_ask, c, single_sided_ok);
            if q_min <= 0.0 {
                continue;
            }

            // Estimate competitor liquidity from observed resting depth inside
            // the reward band. Without a fresh cached book, do not accrue
            // rewards; Polymarket scoring depends on live resting liquidity.
            let min_size = decimal_to_f64(plan.rewards_min_size).max(0.0);
            let observed = book_competition_qmin(
                books,
                yes_token.as_deref(),
                no_token.as_deref(),
                m,
                v,
                c,
                min_size,
                single_sided_ok,
                stale_book_ms,
                now,
            );
            let Some(competitor) = observed else {
                continue;
            };
            let competition_source = "observed_book";
            let share = q_min / (q_min + competitor);
            let daily = decimal_to_f64(plan.total_daily_rate);
            let reward_f = share * daily * decimal_to_f64(time_fraction);
            if reward_f <= 0.0 {
                continue;
            }
            let reward = decimal_from_f64(reward_f).round_dp(4);
            if reward <= Decimal::ZERO {
                continue;
            }

            self.account.reward_earned_usd += reward;
            self.account.available_usd += reward;
            self.reward_accrued += reward;

            // Distribute and mark scoring orders.
            let total_weight: f64 = weights.iter().map(|(_, weight)| weight).sum();
            for (index, weight) in &weights {
                let share = if total_weight > 0.0 {
                    *weight / total_weight
                } else {
                    0.0
                };
                let order = &mut self.orders[*index];
                order.reward_earned += (reward * decimal_from_f64(share)).round_dp(4);
                order.scoring = true;
                order.last_scored_at = Some(self.now);
            }

            self.push_event(
                Some(condition_id.clone()),
                None,
                "reward_accrued",
                RewardRiskSeverity::Info,
                format!("accrued {reward} rewards (share {:.2}%)", share * 100.0),
                json!({
                    "q_bid": q_bid,
                    "q_ask": q_ask,
                    "q_min": q_min,
                    "competitor_qmin": competitor,
                    "competition_source": competition_source,
                    "share": share,
                    "reward": reward,
                }),
            );
        }
    }
}

fn book_top(
    books: &HashMap<String, RewardOrderBook>,
    token_id: &str,
    config: &RewardBotConfig,
    now: OffsetDateTime,
) -> (Option<Decimal>, Option<Decimal>, bool) {
    let Some(book) = books.get(token_id) else {
        return (None, None, false);
    };
    let fresh = config.stale_book_ms == 0
        || (now - book.observed_at)
            .whole_milliseconds()
            .try_into()
            .ok()
            .is_some_and(|age_ms: u64| age_ms <= config.stale_book_ms);
    if !fresh {
        return (None, None, false);
    }
    (
        book.bids.first().map(|level| level.price),
        book.asks.first().map(|level| level.price),
        true,
    )
}

/// `Q_min` combining bid/ask side scores per Polymarket's formula:
/// two-sided uses `max(min(bid, ask), max(bid/c, ask/c))`; single-sided is only
/// allowed (at `/c`) when the midpoint sits inside `[0.10, 0.90]`.
fn combine_qmin(q_bid: f64, q_ask: f64, c: f64, single_sided_ok: bool) -> f64 {
    if q_bid > 0.0 && q_ask > 0.0 {
        f64::max(f64::min(q_bid, q_ask), f64::max(q_bid / c, q_ask / c))
    } else if single_sided_ok {
        f64::max(q_bid, q_ask) / c
    } else {
        0.0
    }
}

fn fresh_book<'a>(
    books: &'a HashMap<String, RewardOrderBook>,
    token_id: &str,
    stale_book_ms: u64,
    now: OffsetDateTime,
) -> Option<&'a RewardOrderBook> {
    let book = books.get(token_id)?;
    let fresh = stale_book_ms == 0
        || (now - book.observed_at)
            .whole_milliseconds()
            .try_into()
            .ok()
            .is_some_and(|age_ms: u64| age_ms <= stale_book_ms);
    fresh.then_some(book)
}

/// Estimate competing makers' `Q_min` from the *observed* resting depth inside
/// the reward band on the live YES/NO books, in the common YES frame. Levels
/// smaller than `min_size` are ignored (mirrors Polymarket's dust filtering).
///
/// Returns `None` when no fresh book is available for either token (so the
/// caller can fall back to the configured competition factor), and `Some(q)`
/// — possibly `0.0` — when a fresh book was seen but carried little/no
/// competing depth inside the band.
#[allow(clippy::too_many_arguments)]
fn book_competition_qmin(
    books: &HashMap<String, RewardOrderBook>,
    yes_token: Option<&str>,
    no_token: Option<&str>,
    midpoint: f64,
    v: f64,
    c: f64,
    min_size: f64,
    single_sided_ok: bool,
    stale_book_ms: u64,
    now: OffsetDateTime,
) -> Option<f64> {
    if v <= 0.0 {
        return None;
    }
    let score = |spread_cents: f64| -> f64 {
        if (0.0..=v).contains(&spread_cents) {
            ((v - spread_cents) / v).powi(2)
        } else {
            0.0
        }
    };
    let mut q_bid = 0.0_f64;
    let mut q_ask = 0.0_f64;
    let mut saw_book = false;

    if let Some(token) = yes_token
        && let Some(book) = fresh_book(books, token, stale_book_ms, now)
    {
        saw_book = true;
        for level in &book.bids {
            let size = decimal_to_f64(level.size);
            if size < min_size {
                continue;
            }
            q_bid += score((midpoint - decimal_to_f64(level.price)) * 100.0) * size;
        }
        for level in &book.asks {
            let size = decimal_to_f64(level.size);
            if size < min_size {
                continue;
            }
            q_ask += score((decimal_to_f64(level.price) - midpoint) * 100.0) * size;
        }
    }

    // NO book mapped into the YES frame: a NO bid at `p` is a YES ask at `1 - p`,
    // and a NO ask at `p` is a YES bid at `1 - p`.
    if let Some(token) = no_token
        && let Some(book) = fresh_book(books, token, stale_book_ms, now)
    {
        saw_book = true;
        for level in &book.bids {
            let size = decimal_to_f64(level.size);
            if size < min_size {
                continue;
            }
            let yes_ask = 1.0 - decimal_to_f64(level.price);
            q_ask += score((yes_ask - midpoint) * 100.0) * size;
        }
        for level in &book.asks {
            let size = decimal_to_f64(level.size);
            if size < min_size {
                continue;
            }
            let yes_bid = 1.0 - decimal_to_f64(level.price);
            q_bid += score((midpoint - yes_bid) * 100.0) * size;
        }
    }

    if !saw_book {
        return None;
    }
    Some(combine_qmin(q_bid, q_ask, c, single_sided_ok))
}
