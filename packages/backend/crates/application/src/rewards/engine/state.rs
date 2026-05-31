// Small state accessors, id/event helpers, deterministic RNG, and fill simulation.

impl TickContext {
    // ---- small helpers -------------------------------------------------

    fn count_open_orders(&self) -> usize {
        self.orders
            .iter()
            .filter(|order| order.status.is_open_like())
            .count()
    }

    fn has_open_buy(&self, condition_id: &str, token_id: &str) -> bool {
        self.orders.iter().any(|order| {
            order.condition_id == condition_id
                && order.token_id == token_id
                && order.side == RewardOrderSide::Buy
                && order.status.is_open_like()
        })
    }

    fn position_over_cap(&self, token_id: &str, price: Decimal) -> bool {
        if self.config.max_position_usd == Decimal::ZERO {
            return false;
        }
        self.positions
            .get(token_id)
            .is_some_and(|position| (position.size * price) >= self.config.max_position_usd)
    }

    /// Total directional inventory notional held across every market (at cost).
    fn global_inventory_notional(&self) -> Decimal {
        self.positions
            .values()
            .filter(|position| position.size > Decimal::ZERO)
            .map(|position| position.size * position.avg_price)
            .sum()
    }

    fn global_exposure_notional(&self) -> Decimal {
        self.global_inventory_notional() + self.account.reserved_usd
    }

    fn reserve_buy_notional(&mut self, notional: Decimal) {
        let notional = notional.max(Decimal::ZERO);
        self.account.available_usd = (self.account.available_usd - notional).max(Decimal::ZERO);
        self.account.reserved_usd += notional;
    }

    fn release_buy_reserve(&mut self, notional: Decimal) {
        let releasable = Decimal::min(self.account.reserved_usd, notional.max(Decimal::ZERO));
        if releasable <= Decimal::ZERO {
            return;
        }
        self.account.reserved_usd -= releasable;
        self.account.available_usd += releasable;
    }

    fn consume_buy_cost(&mut self, cost: Decimal) {
        let cost = cost.max(Decimal::ZERO);
        let from_reserved = Decimal::min(self.account.reserved_usd, cost);
        self.account.reserved_usd -= from_reserved;
        let shortfall = cost - from_reserved;
        if shortfall > Decimal::ZERO {
            self.account.available_usd =
                (self.account.available_usd - shortfall).max(Decimal::ZERO);
        }
    }

    fn position_avg(&self, token_id: &str) -> Option<Decimal> {
        self.positions
            .get(token_id)
            .filter(|position| position.size > Decimal::ZERO)
            .map(|position| position.avg_price)
    }

    fn position_entry(&mut self, order: &ManagedRewardOrder) -> &mut RewardPosition {
        self.positions
            .entry(order.token_id.clone())
            .or_insert_with(|| RewardPosition {
                account_id: self.account.account_id.clone(),
                condition_id: order.condition_id.clone(),
                token_id: order.token_id.clone(),
                outcome: order.outcome.clone(),
                size: Decimal::ZERO,
                avg_price: Decimal::ZERO,
                realized_pnl: Decimal::ZERO,
                updated_at: self.now,
            })
    }

    fn record_fill(
        &mut self,
        order: &ManagedRewardOrder,
        side: RewardOrderSide,
        price: Decimal,
        size: Decimal,
        role: RewardFillRole,
        realized_pnl: Decimal,
        reason: &str,
    ) {
        let id = self.next_id("rewfill");
        self.fills.push(RewardFill {
            id,
            order_id: order.id.clone(),
            account_id: self.account.account_id.clone(),
            condition_id: order.condition_id.clone(),
            token_id: order.token_id.clone(),
            outcome: order.outcome.clone(),
            side,
            price,
            size,
            notional_usd: (price * size).round_dp(4),
            role,
            realized_pnl,
            reason: reason.to_string(),
            trace_id: self.trace_id.clone(),
            created_at: self.now,
        });
    }

    fn push_event(
        &mut self,
        condition_id: Option<String>,
        external_order_id: Option<String>,
        event_type: &str,
        severity: RewardRiskSeverity,
        message: impl Into<String>,
        metadata: Value,
    ) {
        let id = self.next_id("rewevt");
        let mut event = new_risk_event(
            Some(self.account.account_id.clone()),
            condition_id,
            external_order_id,
            event_type,
            severity,
            message,
            metadata,
        );
        event.id = id;
        self.events.push(event);
    }

    fn next_id(&mut self, prefix: &str) -> String {
        self.seq += 1;
        format!(
            "{prefix}_{}_{}_{}",
            self.account.tick_index,
            self.seq,
            self.trace_id.trim_start_matches("trc_")
        )
    }

    /// Deterministic pseudo-random draw in `[0, 1)` seeded by the order id and
    /// the account tick counter, so simulations are reproducible in tests.
    fn draw(&self, order_id: &str) -> f64 {
        let seed = fnv1a(order_id)
            ^ splitmix64(self.account.tick_index as u64)
            ^ (self.seq as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
        let bits = splitmix64(seed) >> 11; // 53 high bits
        (bits as f64) / ((1u64 << 53) as f64)
    }
}

fn simulate_fill(
    order: &ManagedRewardOrder,
    best_bid: Option<Decimal>,
    best_ask: Option<Decimal>,
    has_book: bool,
    draw: f64,
    config: &RewardBotConfig,
) -> Option<Decimal> {
    let remaining = (order.size - order.filled_size).max(Decimal::ZERO);
    if remaining <= Decimal::ZERO {
        return None;
    }

    let crossed = match order.side {
        RewardOrderSide::Buy => best_ask.is_some_and(|ask| ask <= order.price),
        RewardOrderSide::Sell => best_bid.is_some_and(|bid| bid >= order.price),
    };
    let touching = match order.side {
        RewardOrderSide::Buy => {
            best_ask.is_some_and(|ask| ask > order.price && (ask - order.price) <= DEFAULT_TICK)
        }
        RewardOrderSide::Sell => {
            best_bid.is_some_and(|bid| bid < order.price && (order.price - bid) <= DEFAULT_TICK)
        }
    };

    let rate = decimal_to_f64(config.fill_rate_per_tick);
    let do_fill = if crossed {
        true
    } else if touching {
        draw < rate
    } else if !has_book {
        false
    } else {
        false
    };
    if !do_fill {
        return None;
    }

    let mut fill =
        (remaining * config.max_fill_ratio).round_dp_with_strategy(2, RoundingStrategy::ToZero);
    if fill <= Decimal::ZERO {
        fill = remaining;
    }
    Some(Decimal::min(fill, remaining))
}

fn order_remaining_notional(order: &ManagedRewardOrder) -> Decimal {
    ((order.size - order.filled_size).max(Decimal::ZERO) * order.price).round_dp(4)
}

fn fnv1a(input: &str) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in input.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

fn splitmix64(seed: u64) -> u64 {
    let mut z = seed.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}
