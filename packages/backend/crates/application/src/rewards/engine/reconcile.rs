// Per-tick reconciliation of resting orders against fresh books: drift cancels and fills.

impl TickContext {
    fn reconcile_open_orders(
        &mut self,
        plan_index: &HashMap<String, RewardQuotePlan>,
        books: &HashMap<String, RewardOrderBook>,
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

            let (best_bid, best_ask, has_book) = book_top(books, &order.token_id, &self.config, self.now);

            // Cancel resting entry buys that drifted out of the scoring band.
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

            let draw = self.draw(&order.id);
            if let Some(fill_size) = simulate_fill(&order, best_bid, best_ask, has_book, draw, &self.config) {
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
