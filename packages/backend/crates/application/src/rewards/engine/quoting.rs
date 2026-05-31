// Quote placement for eligible markets and order / sibling-leg cancellation.

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

    fn place_new_quotes(&mut self, plans: &[RewardQuotePlan]) {
        let max_markets = if self.config.max_markets == 0 {
            usize::MAX
        } else {
            usize::from(self.config.max_markets)
        };
        let max_open_orders = if self.config.max_open_orders == 0 {
            usize::MAX
        } else {
            usize::from(self.config.max_open_orders)
        };

        // Markets we already quote (any open-like order).
        let mut active_markets: std::collections::HashSet<String> = self
            .orders
            .iter()
            .filter(|order| order.status.is_open_like())
            .map(|order| order.condition_id.clone())
            .collect();

        for plan in plans.iter().filter(|plan| plan.eligible) {
            if active_markets.len() >= max_markets
                && !active_markets.contains(&plan.condition_id)
            {
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
                // Risk gate: stop adding exposure once held inventory hits the cap.
                if self.config.max_global_position_usd > Decimal::ZERO
                    && self.global_inventory_notional() >= self.config.max_global_position_usd
                {
                    continue;
                }

                active_markets.insert(plan.condition_id.clone());

                let id = self.next_id("rew");
                let external = format!("sim_{id}");
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
                    reason: "simulated post-only rewards quote".to_string(),
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
}
