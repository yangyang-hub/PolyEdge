// Quote placement for eligible markets and order / sibling-leg cancellation.

#[allow(dead_code)]
impl TickContext {
    fn cancel_sibling_legs(&mut self, condition_id: &str, filled_token: &str) {
        let indices: Vec<usize> = self
            .orders
            .iter()
            .enumerate()
            .filter(|(_, order)| {
                order.condition_id == condition_id
                    && order.token_id != filled_token
                    && order.side == RewardOrderSide::Buy
                    && order.status.is_open_like()
            })
            .map(|(index, _)| index)
            .collect();
        for index in indices {
            self.cancel_order(index, "cancelled opposite leg after fill");
        }
    }

    fn cancel_order(&mut self, index: usize, reason: impl Into<String>) {
        let reason = reason.into();
        let order = self.orders[index].clone();
        if order.side == RewardOrderSide::Buy {
            self.release_buy_reserve(order_remaining_notional(&order));
        }
        {
            let stored = &mut self.orders[index];
            stored.status = ManagedRewardOrderStatus::Cancelled;
            stored.scoring = false;
            stored.reason = reason.clone();
            stored.updated_at = self.now;
        }
        self.cancelled_orders += 1;
        self.push_event(
            Some(order.condition_id.clone()),
            order.external_order_id.clone(),
            "reward_order_cancelled",
            RewardRiskSeverity::Info,
            format!("{} order cancelled: {reason}", order.outcome),
            json!({ "order_id": order.id, "token_id": order.token_id }),
        );
    }

    fn place_new_quotes(
        &mut self,
        plans: &[RewardQuotePlan],
        books: &HashMap<String, RewardOrderBook>,
    ) {
        let max_markets = usize::from(self.config.max_markets);
        let max_open_orders = usize::from(self.config.max_open_orders);
        if max_markets == 0 || max_open_orders == 0 {
            return;
        }

        // Markets we already quote (any open-like order).
        let mut active_markets: std::collections::HashSet<String> = self
            .orders
            .iter()
            .filter(|order| order.status.is_open_like())
            .map(|order| order.condition_id.clone())
            .collect();

        for plan in plans.iter().filter(|plan| plan.eligible) {
            if !self.plan_has_fresh_quote_books(plan, books) {
                continue;
            }
            if active_markets.len() >= max_markets && !active_markets.contains(&plan.condition_id) {
                continue;
            }
            for leg in &plan.legs {
                if self.count_open_orders() >= max_open_orders {
                    return;
                }
                // Skip tokens we already have a resting buy on.
                if self.has_open_buy(&plan.condition_id, &leg.token_id) {
                    continue;
                }
                if self.position_over_cap(&leg.token_id, leg.price) {
                    continue;
                }
                let notional = (leg.price * leg.size).round_dp(4);
                if notional <= Decimal::ZERO {
                    continue;
                }
                // Risk gate: stop adding exposure once held inventory plus
                // filled inventory hits the cap. Resting validation buys reuse
                // the configured fund pool across markets; fills consume cash.
                if self.config.max_global_position_usd > Decimal::ZERO
                    && self.global_exposure_notional() + notional
                        > self.config.max_global_position_usd
                {
                    continue;
                }

                active_markets.insert(plan.condition_id.clone());
                self.reserve_buy_notional(notional);

                let id = self.next_id("rew");
                let external = id.clone();
                self.push_event(
                    Some(plan.condition_id.clone()),
                    Some(external.clone()),
                    "reward_order_placed",
                    RewardRiskSeverity::Info,
                    format!("{} quote placed: {} @ {}", leg.outcome, leg.size, leg.price),
                    json!({
                        "token_id": leg.token_id,
                        "size": leg.size,
                        "price": leg.price,
                        "notional": notional,
                    }),
                );
                self.orders.push(ManagedRewardOrder {
                    id,
                    account_id: self.account.account_id.clone(),
                    condition_id: plan.condition_id.clone(),
                    token_id: leg.token_id.clone(),
                    outcome: leg.outcome.clone(),
                    side: RewardOrderSide::Buy,
                    price: leg.price,
                    size: leg.size,
                    external_order_id: Some(external),
                    status: ManagedRewardOrderStatus::Open,
                    scoring: true,
                    reason: "validation post-only rewards quote".to_string(),
                    filled_size: Decimal::ZERO,
                    reward_earned: Decimal::ZERO,
                    last_scored_at: None,
                    created_at: self.now,
                    updated_at: self.now,
                });
                self.placed_orders += 1;
            }
        }
    }

    fn plan_has_fresh_quote_books(
        &self,
        plan: &RewardQuotePlan,
        books: &HashMap<String, RewardOrderBook>,
    ) -> bool {
        plan.legs.iter().all(|leg| {
            fresh_book(books, &leg.token_id, self.config.stale_book_ms, self.now)
                .is_some_and(|book| !book.bids.is_empty() && !book.asks.is_empty())
        })
    }
}
