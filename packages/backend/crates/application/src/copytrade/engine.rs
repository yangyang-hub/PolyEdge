/// Run a single copy-trading simulation tick.
///
/// For each newly-planned copy order: if the book crosses the order price,
/// the order fills deterministically (up to max_fill_ratio). For resting open
/// orders from previous ticks: probabilistic fill against the top of book.
///
/// The ledger tracks available/reserved/realized on fills; fills update
/// positions via average-cost accounting.
#[must_use]
pub fn run_copy_simulation_tick(
    config: &CopyTradeConfig,
    account: CopyAccountState,
    open_orders: Vec<CopyOrder>,
    positions: Vec<CopyPosition>,
    new_orders: Vec<CopyOrder>,
    books: &HashMap<String, CopyOrderBook>,
    processed_source_trade_ids: &[String],
    elapsed_seconds: i64,
    trace_id: &str,
) -> CopySimulationOutcome {
    let now = OffsetDateTime::now_utc();
    let positions_map: HashMap<String, CopyPosition> = positions
        .into_iter()
        .map(|p| (p.token_id.clone(), p))
        .collect();

    // Reset daily PnL when the UTC date rolls over.
    let mut account = account;
    if now.date() != account.updated_at.date() {
        account.daily_realized_pnl = Decimal::ZERO;
    }

    let mut ctx = CopyTickContext {
        now,
        config: config.clone(),
        account,
        orders: open_orders,
        positions: positions_map,
        fills: Vec::new(),
        events: Vec::new(),
        processed_source_trade_ids: processed_source_trade_ids.to_vec(),
        trace_id: trace_id.to_string(),
        seq: 0,
        filled_orders: 0,
        placed_orders: 0,
    };

    // Phase 1: Try to fill existing resting orders.
    let _ = elapsed_seconds;
    ctx.reconcile_open_orders(books);

    // Phase 2: Place and attempt immediate fill on new planned orders.
    ctx.place_new_orders(new_orders, books);

    // Finalize the ledger tick.
    ctx.account.tick_index += 1;
    ctx.account.updated_at = now;

    CopySimulationOutcome {
        account: ctx.account,
        orders: ctx.orders,
        positions: ctx.positions.into_values().collect(),
        fills: ctx.fills,
        events: ctx.events,
        processed_source_trade_ids: ctx.processed_source_trade_ids,
        report: CopyTradeRunReport {
            wallets_scanned: 0, // filled by caller
            trades_detected: 0,
            orders_placed: ctx.placed_orders,
            orders_filled: ctx.filled_orders,
            orders_skipped: 0,
        },
    }
}

impl CopyTickContext {
    fn reconcile_open_orders(&mut self, books: &HashMap<String, CopyOrderBook>) {
        let order_indices: Vec<usize> = self
            .orders
            .iter()
            .enumerate()
            .filter(|(_, order)| order.status.is_open_like())
            .map(|(index, _)| index)
            .collect();

        for index in order_indices {
            let order = &mut self.orders[index];
            let Some(book) = books.get(&order.token_id) else {
                continue;
            };
            let remaining = order.remaining_size();
            if remaining <= Decimal::ZERO {
                continue;
            }

            let is_crossed = match order.side {
                CopyOrderSide::Buy => {
                    book.asks.first().is_some_and(|ask| ask.price <= order.price)
                }
                CopyOrderSide::Sell => {
                    book.bids.first().is_some_and(|bid| bid.price >= order.price)
                }
            };

            if is_crossed {
                self.fill_order(index, remaining, true, "crossed_by_book".into());
            } else {
                // Probabilistic fill for resting orders.
                let seed = (self.account.tick_index as u64)
                    .wrapping_add(order.id.len() as u64)
                    .wrapping_add(self.seq as u64);
                let roll = deterministic_probability(seed);
                if roll < self.config.fill_rate_per_tick {
                    // Pass raw remaining — fill_order applies max_fill_ratio once.
                    self.fill_order(index, remaining, false, "probabilistic_fill".into());
                }
            }
            self.seq += 1;
        }
    }

    fn place_new_orders(
        &mut self,
        new_orders: Vec<CopyOrder>,
        books: &HashMap<String, CopyOrderBook>,
    ) {
        for mut order in new_orders {
            self.placed_orders += 1;
            // Reserve capital for buy orders.
            let notional = order.size * order.price;
            if order.side == CopyOrderSide::Buy {
                if self.account.available_usd < notional {
                    order.status = CopyOrderStatus::Skipped;
                    order.reason = "insufficient_capital".into();
                    order.updated_at = self.now;
                    self.orders.push(order);
                    continue;
                }
                self.account.available_usd -= notional;
                self.account.reserved_usd += notional;
            }

            // Attempt immediate fill against the book.
            let Some(book) = books.get(&order.token_id) else {
                order.status = CopyOrderStatus::Open;
                order.updated_at = self.now;
                self.orders.push(order);
                continue;
            };
            let remaining = order.size;
            let is_crossed = match order.side {
                CopyOrderSide::Buy => {
                    book.asks.first().is_some_and(|ask| ask.price <= order.price)
                }
                CopyOrderSide::Sell => {
                    book.bids.first().is_some_and(|bid| bid.price >= order.price)
                }
            };

            if is_crossed {
                let index = self.orders.len();
                self.orders.push(order);
                self.fill_order(index, remaining, true, "immediate_fill".into());
            } else {
                order.status = CopyOrderStatus::Open;
                order.updated_at = self.now;
                self.orders.push(order);
            }
        }
    }

    /// Fill (part of) an order against the book.
    ///
    /// `full_fill = true` marks a marketable (crossed) execution that takes the
    /// whole remaining size — this is what releases the order's reserved capital
    /// in full and drives it to `Filled`. `full_fill = false` is the
    /// probabilistic resting-fill path, which fills at most `max_fill_ratio` of
    /// the remaining size and legitimately leaves the order open.
    fn fill_order(&mut self, index: usize, available: Decimal, full_fill: bool, reason: String) {
        // Dust below this many shares is absorbed into a fill so partial-fill
        // orders converge to `Filled` instead of resting forever with a tiny
        // unfilled remainder (which would strand its reserved capital).
        let dust = Decimal::new(1, 2); // 0.01 shares

        let order = &mut self.orders[index];
        let remaining = order.remaining_size();
        let mut fill_size = if full_fill {
            available.min(remaining)
        } else {
            (available * self.config.max_fill_ratio).min(remaining)
        }
        .round_dp_with_strategy(8, RoundingStrategy::MidpointNearestEven);

        // Absorb a sub-dust tail so the order can close and release its reserve.
        if remaining - fill_size <= dust {
            fill_size = remaining;
        }

        if fill_size <= Decimal::ZERO {
            return;
        }

        let notional = fill_size * order.price;
        order.filled_size += fill_size;
        order.updated_at = self.now;

        let closed = order.remaining_size() <= Decimal::ZERO;
        if closed {
            order.status = CopyOrderStatus::Filled;
        }

        match order.side {
            CopyOrderSide::Buy => {
                // Release the filled notional from reserve. Because crossed
                // orders take the whole remaining (full_fill) and sub-dust tails
                // are absorbed, every order converges to `Filled`, at which point
                // the cumulative released notional equals the reserved amount —
                // so reserved_usd does not leak even when max_fill_ratio < 1.
                self.account.reserved_usd =
                    (self.account.reserved_usd - notional).max(Decimal::ZERO);
                // Update position.
                let position = self
                    .positions
                    .entry(order.token_id.clone())
                    .or_insert_with(|| CopyPosition {
                        account_id: self.account.account_id.clone(),
                        wallet_address: order.wallet_address.clone(),
                        condition_id: order.condition_id.clone(),
                        token_id: order.token_id.clone(),
                        outcome: order.outcome.clone(),
                        size: Decimal::ZERO,
                        avg_price: Decimal::ZERO,
                        realized_pnl: Decimal::ZERO,
                        updated_at: self.now,
                    });
                let total_cost = position.size * position.avg_price + fill_size * order.price;
                position.size += fill_size;
                position.avg_price = if position.size > Decimal::ZERO {
                    total_cost / position.size
                } else {
                    Decimal::ZERO
                };
                position.updated_at = self.now;
            }
            CopyOrderSide::Sell => {
                // Realized P&L. A sell only ever reduces a held position; orders
                // for unheld inventory are rejected upstream in compute_copy_size,
                // so the no-position branch is a defensive no-op that must NOT
                // credit phantom proceeds.
                if let Some(position) = self.positions.get_mut(&order.token_id) {
                    let pnl = (order.price - position.avg_price) * fill_size;
                    order.realized_pnl += pnl;
                    self.account.realized_pnl += pnl;
                    self.account.daily_realized_pnl += pnl;
                    self.account.available_usd += notional;
                    position.size = (position.size - fill_size).max(Decimal::ZERO);
                    position.realized_pnl += pnl;
                    position.updated_at = self.now;
                }
            }
        }

        self.fills.push(CopyFill {
            id: new_copy_fill_id(),
            order_id: order.id.clone(),
            account_id: self.account.account_id.clone(),
            wallet_address: order.wallet_address.clone(),
            condition_id: order.condition_id.clone(),
            token_id: order.token_id.clone(),
            outcome: order.outcome.clone(),
            side: order.side,
            price: order.price,
            size: fill_size,
            notional_usd: notional,
            realized_pnl: order.realized_pnl,
            reason,
            trace_id: self.trace_id.clone(),
            created_at: self.now,
        });
        self.filled_orders += 1;
    }
}

/// Deterministic pseudo-random probability in [0, 1) seeded from a u64.
fn deterministic_probability(seed: u64) -> Decimal {
    // SplitMix64-style mixing.
    let mut z = seed.wrapping_add(0x9e3779b97f4a7c15);
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
    z ^= z >> 31;
    let unit = (z & 0xFFFF_FFFF) as u32;
    Decimal::from(unit) / Decimal::from(u32::MAX)
}
