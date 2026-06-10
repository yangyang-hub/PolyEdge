impl RewardBotService {
    /// Persist an event to the store and push it into the in-memory event cache.
    async fn log_event_to_store_and_memory(&self, event: RewardRiskEvent) -> Result<()> {
        self.store.log_event(event.clone()).await?;
        let mut memory = self.memory.write().await;
        memory.events.push(event);
        memory.events.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        memory.events.truncate(MEMORY_EVENT_LIMIT);
        Ok(())
    }

    async fn load_account_state_cached(
        &self,
        config: &RewardBotConfig,
    ) -> Result<RewardAccountState> {
        if let Some(account) = self
            .memory
            .read()
            .await
            .account
            .clone()
            .filter(|account| account.account_id == config.account_id)
        {
            return Ok(account);
        }
        let account = self.store.load_account_state(config).await?;
        self.memory.write().await.account = Some(account.clone());
        Ok(account)
    }

    async fn list_positions_cached(
        &self,
        account_id: &str,
        limit: u16,
    ) -> Result<Vec<RewardPosition>> {
        if let Some(positions) = self.memory.read().await.positions.clone() {
            let mut positions = positions
                .into_iter()
                .filter(|position| position.account_id == account_id && position.size != Decimal::ZERO)
                .collect::<Vec<_>>();
            positions.sort_by(|left, right| right.updated_at.cmp(&left.updated_at));
            positions.truncate(usize::from(limit));
            return Ok(positions);
        }
        let positions = self.store.list_positions(account_id, limit).await?;
        self.memory.write().await.positions = Some(positions.clone());
        Ok(positions)
    }

    async fn list_account_positions_cached(&self, account_id: &str) -> Result<Vec<RewardPosition>> {
        if let Some(positions) = self.memory.read().await.positions.clone() {
            return Ok(positions
                .into_iter()
                .filter(|position| position.account_id == account_id && position.size != Decimal::ZERO)
                .collect());
        }
        let positions = self.store.list_account_positions(account_id).await?;
        self.memory.write().await.positions = Some(positions.clone());
        Ok(positions)
    }

    async fn latest_worker_heartbeat_cached(
        &self,
        account_id: &str,
    ) -> Result<Option<OffsetDateTime>> {
        if let Some(heartbeat) = self
            .memory
            .read()
            .await
            .worker_heartbeats
            .get(account_id)
            .copied()
        {
            return Ok(Some(heartbeat));
        }
        let heartbeat = self.store.latest_worker_heartbeat(account_id).await?;
        if let Some(heartbeat) = heartbeat {
            self.memory
                .write()
                .await
                .worker_heartbeats
                .insert(account_id.to_string(), heartbeat);
        }
        Ok(heartbeat)
    }

    /// Return events from the in-memory cache, falling back to the database when
    /// the cache is empty (cold start). Results are filtered by `account_id` and
    /// bounded to `limit`.
    async fn list_events_cached(
        &self,
        account_id: &str,
        limit: u16,
    ) -> Result<Vec<RewardRiskEvent>> {
        let memory = self.memory.read().await;
        if !memory.events.is_empty() {
            let events: Vec<_> = memory
                .events
                .iter()
                .filter(|event| event.account_id.as_deref() == Some(account_id))
                .take(usize::from(limit))
                .cloned()
                .collect();
            return Ok(events);
        }
        drop(memory);
        let events = self.store.list_events(account_id, limit).await?;
        let mut memory = self.memory.write().await;
        memory.events = events.clone();
        memory.events.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        memory.events.truncate(MEMORY_EVENT_LIMIT);
        Ok(events)
    }

    /// Return fills from the in-memory cache, falling back to the database when
    /// the cache is empty (cold start). Results are filtered by `account_id` and
    /// bounded to `limit`.
    async fn list_fills_cached(
        &self,
        account_id: &str,
        limit: u16,
    ) -> Result<Vec<RewardFill>> {
        let memory = self.memory.read().await;
        if !memory.fills.is_empty() {
            let fills: Vec<_> = memory
                .fills
                .iter()
                .filter(|fill| fill.account_id == account_id)
                .take(usize::from(limit))
                .cloned()
                .collect();
            return Ok(fills);
        }
        drop(memory);
        let fills = self.store.list_fills(account_id, limit).await?;
        let mut memory = self.memory.write().await;
        memory.fills = fills.clone();
        memory.fills.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        memory.fills.truncate(MEMORY_FILL_LIMIT);
        Ok(fills)
    }

    /// Return the cached open-order count, falling back to the database when the
    /// count has not been loaded yet or was invalidated by a tick outcome.
    async fn count_open_orders_cached(&self, account_id: &str) -> Result<usize> {
        if let Some(count) = self.memory.read().await.open_order_count {
            return Ok(count);
        }
        let count = self.store.count_open_orders(account_id).await?;
        self.memory.write().await.open_order_count = Some(count);
        Ok(count)
    }
}
