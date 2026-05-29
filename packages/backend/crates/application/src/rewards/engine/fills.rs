// Fill handling: buy and exit fills, post-fill strategy, flatten/exit orders, fill recording.

impl TickContext {
    fn handle_buy_fill(
        &mut self,
        index: usize,
        fill_size: Decimal,
        books: &HashMap<String, RewardOrderBook>,
    ) {
        let order = self.orders[index].clone();
        let cost = (order.price * fill_size).round_dp(4);
        let now = self.now;

        // Move reserved cash into inventory; release any over-reserved remainder.
        self.account.reserved_usd = (self.account.reserved_usd - cost).max(Decimal::ZERO);

        // Update inventory (average cost basis).
        let position = self.position_entry(&order);
        let new_size = position.size + fill_size;
        if new_size > Decimal::ZERO {
            position.avg_price =
                ((position.size * position.avg_price) + (fill_size * order.price)) / new_size;
        }
        position.size = new_size;
        position.updated_at = now;

        // Update order accounting.
        {
            let stored = &mut self.orders[index];
            stored.filled_size += fill_size;
            stored.updated_at = self.now;
            if stored.filled_size >= stored.size {
                stored.status = ManagedRewardOrderStatus::Filled;
                stored.scoring = false;
                stored.reason = "resting quote fully taken by counterparty".to_string();
            } else {
                stored.reason = "resting quote partially taken".to_string();
            }
        }
        self.filled_orders += 1;

        self.record_fill(&order, RewardOrderSide::Buy, order.price, fill_size, RewardFillRole::Maker, Decimal::ZERO, "maker buy filled");
        self.push_event(
            Some(order.condition_id.clone()),
            order.external_order_id.clone(),
            "reward_order_filled",
            RewardRiskSeverity::Info,
            format!(
                "{} buy taken: {} @ {}",
                order.outcome, fill_size, order.price
            ),
            json!({
                "order_id": order.id,
                "token_id": order.token_id,
                "fill_size": fill_size,
                "price": order.price,
            }),
        );

        self.apply_post_fill_strategy(&order, fill_size, books);
    }

    fn apply_post_fill_strategy(
        &mut self,
        entry: &ManagedRewardOrder,
        fill_size: Decimal,
        books: &HashMap<String, RewardOrderBook>,
    ) {
        if self.config.cancel_on_fill {
            self.cancel_sibling_legs(&entry.condition_id, &entry.token_id);
        }

        match self.config.post_fill_strategy {
            PostFillStrategy::HoldAndRequote => {
                // Inventory is kept; the placement step keeps quoting subject to
                // the per-market position cap.
            }
            PostFillStrategy::ExitAtMarkup => {
                let avg = self.position_avg(&entry.token_id).unwrap_or(entry.price);
                let exit_price = floor_to_tick(
                    Decimal::min(
                        decimal("0.99"),
                        avg + self.config.exit_markup_cents / decimal("100"),
                    ),
                    DEFAULT_TICK,
                );
                self.place_exit_order(entry, fill_size, exit_price);
            }
            PostFillStrategy::FlattenImmediately => {
                let (best_bid, _, _) = book_top(books, &entry.token_id, &self.config, self.now);
                let exit_price = best_bid
                    .filter(|bid| *bid > Decimal::ZERO)
                    .unwrap_or_else(|| {
                        floor_to_tick(
                            Decimal::max(decimal("0.01"), entry.price - decimal("0.01")),
                            DEFAULT_TICK,
                        )
                    });
                self.flatten_now(entry, fill_size, exit_price);
            }
        }
    }

    fn handle_exit_fill(&mut self, index: usize, fill_size: Decimal, best_bid: Option<Decimal>) {
        let order = self.orders[index].clone();
        let avg = self.position_avg(&order.token_id).unwrap_or(order.price);
        let proceeds = (order.price * fill_size).round_dp(4);
        let realized = ((order.price - avg) * fill_size).round_dp(4);
        let now = self.now;

        self.account.available_usd += proceeds;
        self.account.realized_pnl += realized;

        let position = self.position_entry(&order);
        position.size = (position.size - fill_size).max(Decimal::ZERO);
        position.realized_pnl += realized;
        position.updated_at = now;

        {
            let stored = &mut self.orders[index];
            stored.filled_size += fill_size;
            stored.updated_at = self.now;
            if stored.filled_size >= stored.size {
                stored.status = ManagedRewardOrderStatus::Filled;
                stored.reason = "exit order fully taken".to_string();
            }
        }
        self.filled_orders += 1;

        self.record_fill(
            &order,
            RewardOrderSide::Sell,
            order.price,
            fill_size,
            RewardFillRole::Maker,
            realized,
            "exit order taken",
        );
        self.push_event(
            Some(order.condition_id.clone()),
            order.external_order_id.clone(),
            "reward_exit_filled",
            RewardRiskSeverity::Info,
            format!(
                "{} exit sold: {} @ {} (pnl {})",
                order.outcome, fill_size, order.price, realized
            ),
            json!({
                "order_id": order.id,
                "token_id": order.token_id,
                "fill_size": fill_size,
                "price": order.price,
                "best_bid": best_bid,
                "realized_pnl": realized,
            }),
        );
    }

    fn flatten_now(&mut self, entry: &ManagedRewardOrder, size: Decimal, price: Decimal) {
        let avg = self.position_avg(&entry.token_id).unwrap_or(entry.price);
        let proceeds = (price * size).round_dp(4);
        let realized = ((price - avg) * size).round_dp(4);
        let now = self.now;

        self.account.available_usd += proceeds;
        self.account.realized_pnl += realized;

        let position = self.position_entry(entry);
        position.size = (position.size - size).max(Decimal::ZERO);
        position.realized_pnl += realized;
        position.updated_at = now;

        self.record_fill(
            entry,
            RewardOrderSide::Sell,
            price,
            size,
            RewardFillRole::Taker,
            realized,
            "flattened inventory at market",
        );
        self.push_event(
            Some(entry.condition_id.clone()),
            None,
            "reward_position_flattened",
            RewardRiskSeverity::Info,
            format!(
                "{} flattened {} @ {} (pnl {})",
                entry.outcome, size, price, realized
            ),
            json!({
                "token_id": entry.token_id,
                "size": size,
                "price": price,
                "realized_pnl": realized,
            }),
        );
    }

    fn place_exit_order(&mut self, entry: &ManagedRewardOrder, size: Decimal, price: Decimal) {
        let id = self.next_id("rewx");
        let external = format!("sim_{id}");
        let order = ManagedRewardOrder {
            id,
            account_id: self.account.account_id.clone(),
            condition_id: entry.condition_id.clone(),
            token_id: entry.token_id.clone(),
            outcome: entry.outcome.clone(),
            side: RewardOrderSide::Sell,
            price,
            size,
            external_order_id: Some(external.clone()),
            status: ManagedRewardOrderStatus::ExitPending,
            scoring: false,
            reason: "post-fill exit at markup".to_string(),
            filled_size: Decimal::ZERO,
            reward_earned: Decimal::ZERO,
            last_scored_at: None,
            created_at: self.now,
            updated_at: self.now,
        };
        self.push_event(
            Some(entry.condition_id.clone()),
            Some(external),
            "reward_exit_placed",
            RewardRiskSeverity::Info,
            format!("{} exit quote: {} @ {}", entry.outcome, size, price),
            json!({ "token_id": entry.token_id, "size": size, "price": price }),
        );
        self.orders.push(order);
    }
}
