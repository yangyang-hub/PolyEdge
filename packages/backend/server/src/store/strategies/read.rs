use super::super::*;

impl PostgresStore {
    pub(super) async fn load_strategy_data(
        &self,
        strategy_ids: &[i64],
        actor: polyedge_domain::ActorScope,
    ) -> Result<Vec<MarketStrategyData>> {
        if strategy_ids.is_empty() {
            return Ok(Vec::new());
        }
        let strategy_rows = sqlx::query(
            r#"
            SELECT s.strategy_id, s.owner_user_id, u.display_name AS owner_display_name,
              s.market_id, s.name, s.status AS strategy_status, s.visibility,
              s.active_from, s.active_until, s.expired_at,
              s.created_at AS strategy_created_at, s.updated_at AS strategy_updated_at,
              m.created_by_user_id, m.condition_id, m.slug, m.question, m.polymarket_url,
              m.status AS market_status, m.created_at AS market_created_at,
              m.updated_at AS market_updated_at
            FROM market_strategies s
            JOIN users u ON u.user_id = s.owner_user_id
            JOIN managed_markets m ON m.market_id = s.market_id
            WHERE s.strategy_id = ANY($1)
            "#,
        )
        .bind(strategy_ids)
        .fetch_all(&self.pool)
        .await?;
        let version_rows = sqlx::query(
            r#"
            SELECT strategy_version_id, strategy_id, version_number, status,
              reward_minimum_size, reward_maximum_spread, reward_daily_rate,
              book_freshness_ms, downward_reprice_confirm_ms,
              upward_reprice_confirm_ms, reprice_cooldown_ms,
              max_replaces_per_cycle, published_at, created_at
            FROM strategy_versions
            WHERE strategy_id = ANY($1) AND status = 'published'
            "#,
        )
        .bind(strategy_ids)
        .fetch_all(&self.pool)
        .await?;
        let version_ids = version_rows
            .iter()
            .map(|row| row.get::<i64, _>("strategy_version_id"))
            .collect::<Vec<_>>();
        let slot_rows = if version_ids.is_empty() {
            Vec::new()
        } else {
            sqlx::query(
                r#"
                SELECT quote_slot_id, strategy_version_id, slot_key, outcome,
                  quantity, pricing_mode, fixed_price, book_rank, price_offset,
                  minimum_price, maximum_price, post_only, enabled
                FROM strategy_quote_slots
                WHERE strategy_version_id = ANY($1)
                ORDER BY strategy_version_id, quote_slot_id
                "#,
            )
            .bind(&version_ids)
            .fetch_all(&self.pool)
            .await?
        };
        let market_ids = strategy_rows
            .iter()
            .map(|row| row.get::<i64, _>("market_id"))
            .collect::<Vec<_>>();
        let outcome_rows = sqlx::query(
            "SELECT outcome_id, market_id, outcome, token_id FROM managed_market_outcomes WHERE market_id = ANY($1)",
        )
        .bind(&market_ids)
        .fetch_all(&self.pool)
        .await?;
        let mut subscriptions = self
            .load_subscription_data_for_strategies(strategy_ids, actor.user_id)
            .await?;

        let mut versions = HashMap::new();
        let mut rewards = HashMap::new();
        for row in version_rows {
            let strategy_id: i64 = row.try_get("strategy_id")?;
            rewards.insert(
                strategy_id,
                StrategyRewardTerms {
                    strategy_version_id: row.try_get("strategy_version_id")?,
                    minimum_size: row.try_get("reward_minimum_size")?,
                    maximum_spread: row.try_get("reward_maximum_spread")?,
                    daily_rate: row.try_get("reward_daily_rate")?,
                },
            );
            versions.insert(strategy_id, super::strategy_version_from_row(&row)?);
        }
        let mut slots: HashMap<i64, Vec<StrategyQuoteSlot>> = HashMap::new();
        for row in slot_rows {
            let version_id: i64 = row.try_get("strategy_version_id")?;
            slots
                .entry(version_id)
                .or_default()
                .push(super::slot_from_row(&row)?);
        }
        let mut outcomes: HashMap<i64, Vec<ManagedMarketOutcome>> = HashMap::new();
        for row in outcome_rows {
            let market_id: i64 = row.try_get("market_id")?;
            outcomes
                .entry(market_id)
                .or_default()
                .push(ManagedMarketOutcome {
                    id: row.try_get("outcome_id")?,
                    market_id,
                    outcome: enum_value(row.try_get("outcome")?, "outcome")?,
                    token_id: row.try_get("token_id")?,
                });
        }

        let mut result = Vec::with_capacity(strategy_rows.len());
        for row in strategy_rows {
            let strategy_id: i64 = row.try_get("strategy_id")?;
            let market_id: i64 = row.try_get("market_id")?;
            let version = versions.remove(&strategy_id).ok_or_else(|| {
                ServerError::Internal(format!("strategy {strategy_id} has no published version"))
            })?;
            let version_slots = slots.remove(&version.id).unwrap_or_default();
            result.push(MarketStrategyData {
                market: ManagedMarket {
                    id: market_id,
                    created_by_user_id: row.try_get("created_by_user_id")?,
                    condition_id: row.try_get("condition_id")?,
                    slug: row.try_get("slug")?,
                    question: row.try_get("question")?,
                    polymarket_url: row.try_get("polymarket_url")?,
                    status: enum_value(row.try_get("market_status")?, "market status")?,
                    created_at: row.try_get("market_created_at")?,
                    updated_at: row.try_get("market_updated_at")?,
                },
                outcomes: outcomes.remove(&market_id).unwrap_or_default(),
                strategy: MarketStrategy {
                    id: strategy_id,
                    owner_user_id: row.try_get("owner_user_id")?,
                    owner_display_name: row.try_get("owner_display_name")?,
                    market_id,
                    name: row.try_get("name")?,
                    status: enum_value(row.try_get("strategy_status")?, "strategy status")?,
                    visibility: enum_value(row.try_get("visibility")?, "strategy visibility")?,
                    active_from: row.try_get("active_from")?,
                    active_until: row.try_get("active_until")?,
                    expired_at: row.try_get("expired_at")?,
                    created_at: row.try_get("strategy_created_at")?,
                    updated_at: row.try_get("strategy_updated_at")?,
                },
                version,
                reward_terms: rewards.remove(&strategy_id).ok_or_else(|| {
                    ServerError::Internal(format!("strategy {strategy_id} has no reward snapshot"))
                })?,
                quote_slots: version_slots,
                current_user_subscription: subscriptions.remove(&strategy_id),
            });
        }
        result.sort_by_key(|item| {
            strategy_ids
                .iter()
                .position(|id| *id == item.strategy.id)
                .unwrap_or(usize::MAX)
        });
        Ok(result)
    }
}
