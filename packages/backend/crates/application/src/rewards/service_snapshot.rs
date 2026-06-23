impl RewardBotService {
    pub async fn snapshot(&self) -> Result<RewardBotSnapshot> {
        self.snapshot_with_order_query(
            &RewardOrderListQuery::default(),
            &RewardQuotePlanListQuery::default(),
        )
        .await
    }

    pub async fn snapshot_with_order_query(
        &self,
        order_query: &RewardOrderListQuery,
        plans_query: &RewardQuotePlanListQuery,
    ) -> Result<RewardBotSnapshot> {
        let config = self.read_config().await?;
        let account = self.load_account_state_cached(&config).await?;
        let now = OffsetDateTime::now_utc();
        let low_competition_since =
            now - TimeDuration::hours(LOW_COMPETITION_SHADOW_REPORT_WINDOW_HOURS as i64);

        // Inject the real account_id from config so SQL uses the account/status indexes.
        let order_query = RewardOrderListQuery {
            account_id: account.account_id.clone(),
            ..order_query.clone()
        };

        let (
            (markets_tracked, last_scan_at),
            plan_counts,
            plans_page_data,
            orders,
            stored_positions,
            open_order_count,
            active_orders,
            fills,
            events,
            last_run_at,
            worker_heartbeat,
            low_competition_observations,
        ) = tokio::try_join!(
            self.store.active_market_summary(),
            self.store.count_quote_plans(),
            self.store.list_quote_plans_page(plans_query),
            self.store.list_orders_page(&order_query),
            self.list_positions_cached(&account.account_id, 200),
            self.count_external_open_orders_cached(&account.account_id),
            self.store.list_open_orders(&account.account_id),
            self.list_fills_cached(&account.account_id, 200),
            self.list_events_cached(&account.account_id, 100),
            self.store.latest_quote_plan_updated_at(),
            self.latest_worker_heartbeat_cached(&account.account_id),
            self.store.list_low_competition_observations(
                &account.account_id,
                low_competition_since,
                LOW_COMPETITION_OBSERVATION_READ_LIMIT,
            ),
        )?;
        let error = active_orders
            .iter()
            .find(|order| reward_order_has_active_reconciliation_error(order))
            .map(|order| order.reason.clone());
        let low_competition_report = build_low_competition_shadow_report(
            &low_competition_observations,
            LOW_COMPETITION_SHADOW_REPORT_WINDOW_HOURS,
            &config,
            now,
        );

        Ok(RewardBotSnapshot {
            status: RewardBotStatus {
                enabled: config.enabled,
                running: reward_worker_is_running(&config, worker_heartbeat, now),
                account_id: config.account_id.clone(),
                markets_tracked,
                eligible_markets: plan_counts.eligible,
                ready_quote_markets: plan_counts.ready_to_quote,
                waiting_orderbook_markets: plan_counts.waiting_orderbook,
                provider_pending_markets: plan_counts.provider_pending,
                blocker_counts: plan_counts.blockers,
                open_orders: open_order_count,
                positions: stored_positions.len(),
                last_scan_at,
                last_run_at,
                error,
                plans_total: plan_counts.total,
            },
            config,
            account,
            low_competition_report,
            markets: Vec::new(),
            quote_plans: plans_page_data.items,
            plans_page: plans_page_data.page,
            orders: orders.items,
            orders_page: orders.page,
            positions: stored_positions,
            fills,
            events,
        })
    }
}
