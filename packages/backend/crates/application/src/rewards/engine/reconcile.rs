// Per-tick reconciliation of resting orders against fresh books: risk checks, drift cancels, and validation fills.

impl TickContext {
    fn reconcile_open_orders(
        &mut self,
        plan_index: &HashMap<String, RewardQuotePlan>,
        books: &HashMap<String, RewardOrderBook>,
        book_history: &HashMap<String, VecDeque<BookSnapshot>>,
    ) {
        // Work on a snapshot of the currently open orders so the post-fill
        // handlers can freely push new orders / cancel siblings.
        let open_ids: Vec<String> = self
            .orders
            .iter()
            .filter(|order| order.status.is_open_like())
            .map(|order| order.id.clone())
            .collect();

        for order_id in open_ids {
            let Some(index) = self.orders.iter().position(|order| order.id == order_id) else {
                continue;
            };
            let order = self.orders[index].clone();
            if !order.status.is_open_like() {
                continue;
            }

            let fresh_order_book =
                fresh_book(books, &order.token_id, self.config.stale_book_ms, self.now);
            let (best_bid, _, _) = book_top(books, &order.token_id, &self.config, self.now);

            // --- Existing drift cancel for Buy orders ---
            if order.side == RewardOrderSide::Buy {
                if let Some(plan) = plan_index.get(&order.condition_id) {
                    if let Some(reason) = self.should_cancel_for_drift(&order, plan) {
                        self.cancel_order(index, reason);
                        continue;
                    }
                } else {
                    self.cancel_order(index, "market no longer offers rewards");
                    continue;
                }
            }

            // --- Risk-control checks (Buy orders only) ---
            if order.side == RewardOrderSide::Buy {
                let history = book_history.get(&order.token_id);

                // Feature 1: Minimum depth threshold
                if let Some(reason) = check_min_depth(&order, books, &self.config) {
                    self.cancel_order(index, reason);
                    self.risk_cancelled_orders += 1;
                    continue;
                }
                // Feature 2: Bid-rank promotion cancel
                if let Some(reason) = check_bid_rank(&order, books, &self.config) {
                    self.cancel_order(index, reason);
                    self.risk_cancelled_orders += 1;
                    continue;
                }
                // Feature 3: Depth-drop detection
                if let Some(hist) = history {
                    if let Some(reason) = check_depth_drop(&order, books, hist, &self.config, self.now) {
                        self.cancel_order(index, reason);
                        self.risk_cancelled_orders += 1;
                        continue;
                    }
                }
                // Feature 4: Fill-velocity detection
                if let Some(hist) = history {
                    if let Some(reason) = check_fill_velocity(&order, books, hist, &self.config, self.now) {
                        self.cancel_order(index, reason);
                        self.risk_cancelled_orders += 1;
                        continue;
                    }
                }
                // Feature 5: Mass-cancel following
                if let Some(hist) = history {
                    if let Some(reason) = check_mass_cancel(&order, books, hist, &self.config, self.now) {
                        self.cancel_order(index, reason);
                        self.risk_cancelled_orders += 1;
                        continue;
                    }
                }
                // Feature 6: Periodic requote
                if let Some(reason) = check_requote_age(&order, &self.config, self.now) {
                    self.cancel_order(index, reason);
                    self.risk_cancelled_orders += 1;
                    continue;
                }
            }

            // --- Validation fill trigger ---
            let draw = self.draw(&order.id);
            if let Some(fill_size) = simulate_fill(&order, fresh_order_book, draw, &self.config) {
                match order.side {
                    RewardOrderSide::Buy => self.handle_buy_fill(index, fill_size, books),
                    RewardOrderSide::Sell => self.handle_exit_fill(index, fill_size, best_bid),
                }
            }
        }
    }

    fn should_cancel_for_drift(
        &self,
        order: &ManagedRewardOrder,
        plan: &RewardQuotePlan,
    ) -> Option<String> {
        if !plan.eligible {
            return Some("market dropped below eligibility threshold".to_string());
        }
        let leg = plan.legs.iter().find(|leg| leg.token_id == order.token_id)?;
        if self.config.requote_drift_cents > Decimal::ZERO {
            let drift_cents = ((order.price - leg.price).abs()) * decimal("100");
            if drift_cents > self.config.requote_drift_cents {
                return Some(format!(
                    "midpoint drifted {drift_cents} cents beyond requote threshold"
                ));
            }
        }
        None
    }
}
