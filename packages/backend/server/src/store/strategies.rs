use super::*;

impl PostgresStore {
    pub async fn list_strategies(
        &self,
        query: &ManualTradingListQuery,
    ) -> Result<Vec<MarketStrategyData>> {
        let (limit, offset) = page_values(query);
        let ids = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT strategy_id
            FROM market_strategies
            WHERE ($1::text IS NULL OR status = $1)
              AND ($2::bigint IS NULL OR market_id = $2)
              AND ($3::bigint IS NULL OR strategy_id = $3)
            ORDER BY updated_at DESC, strategy_id DESC
            LIMIT $4 OFFSET $5
            "#,
        )
        .bind(query.status.as_deref())
        .bind(query.market_id)
        .bind(query.strategy_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;
        self.load_strategy_data(&ids).await
    }

    pub async fn get_strategy(&self, strategy_id: i64) -> Result<MarketStrategyData> {
        self.load_strategy_data(&[strategy_id])
            .await?
            .into_iter()
            .next()
            .ok_or_else(|| ServerError::NotFound(format!("strategy {strategy_id}")))
    }

    pub async fn create_strategy(
        &self,
        request: &CreateMarketStrategyRequest,
        actor_id: &str,
        request_id: &str,
    ) -> Result<MarketStrategyData> {
        validate_strategy_input(request)?;
        let name = required_text(&request.name, "name", 160)?;
        let operator_note = optional_note(request.operator_note.as_deref())?;
        let mut tx = self.pool.begin().await?;
        let market_id = upsert_market(&mut tx, &request.market).await?;
        let strategy_id: i64 = sqlx::query_scalar(
            r#"
            INSERT INTO market_strategies (market_id, name, status)
            VALUES ($1, $2, 'active')
            RETURNING strategy_id
            "#,
        )
        .bind(market_id)
        .bind(name)
        .fetch_one(&mut *tx)
        .await?;
        let version_id = insert_strategy_version(&mut tx, strategy_id, 1, &request.version).await?;
        insert_quote_slots(&mut tx, version_id, &request.version.quote_slots).await?;
        replace_wallet_targets(&mut tx, strategy_id, &request.version.wallet_ids).await?;
        insert_audit(
            &mut tx,
            request_id,
            actor_id,
            "strategy.create",
            "strategy",
            &strategy_id.to_string(),
            operator_note.as_deref(),
        )
        .await?;
        tx.commit().await?;
        self.get_strategy(strategy_id).await
    }

    pub async fn update_strategy(
        &self,
        strategy_id: i64,
        request: &UpdateMarketStrategyRequest,
        actor_id: &str,
        request_id: &str,
    ) -> Result<MarketStrategyData> {
        let existing = self.get_strategy(strategy_id).await?;
        let name = request
            .name
            .as_deref()
            .map(|value| required_text(value, "name", 160))
            .transpose()?
            .unwrap_or(existing.strategy.name);
        let status = request.status.unwrap_or(existing.strategy.status);
        let operator_note = optional_note(request.operator_note.as_deref())?;
        let mut tx = self.pool.begin().await?;
        sqlx::query(
            "UPDATE market_strategies SET name = $2, status = $3, updated_at = now() WHERE strategy_id = $1",
        )
        .bind(strategy_id)
        .bind(name)
        .bind(status.as_str())
        .execute(&mut *tx)
        .await?;
        if let Some(market) = request.market.as_ref() {
            sqlx::query(
                r#"
                UPDATE managed_markets
                SET slug = COALESCE($2, slug),
                    question = COALESCE($3, question),
                    polymarket_url = COALESCE($4, polymarket_url),
                    status = COALESCE($5, status),
                    updated_at = now()
                WHERE market_id = $1
                "#,
            )
            .bind(existing.market.id)
            .bind(market.slug.as_deref())
            .bind(market.question.as_deref())
            .bind(market.polymarket_url.as_deref())
            .bind(market.status.map(|value| value.as_str()))
            .execute(&mut *tx)
            .await?;
            sqlx::query(
                r#"
                UPDATE market_reward_terms
                SET minimum_size = COALESCE($2, minimum_size),
                    maximum_spread = COALESCE($3, maximum_spread),
                    daily_rate = COALESCE($4, daily_rate),
                    updated_at = now()
                WHERE market_id = $1
                "#,
            )
            .bind(existing.market.id)
            .bind(market.reward_minimum_size)
            .bind(market.reward_maximum_spread)
            .bind(market.reward_daily_rate)
            .execute(&mut *tx)
            .await?;
        }
        if let Some(version) = request.version.as_ref() {
            validate_version_input(version)?;
            sqlx::query(
                "UPDATE strategy_versions SET status = 'retired' WHERE strategy_id = $1 AND status = 'published'",
            )
            .bind(strategy_id)
            .execute(&mut *tx)
            .await?;
            let version_number: i64 = sqlx::query_scalar(
                "SELECT COALESCE(MAX(version_number), 0) + 1 FROM strategy_versions WHERE strategy_id = $1",
            )
            .bind(strategy_id)
            .fetch_one(&mut *tx)
            .await?;
            let version_id =
                insert_strategy_version(&mut tx, strategy_id, version_number, version).await?;
            insert_quote_slots(&mut tx, version_id, &version.quote_slots).await?;
            replace_wallet_targets(&mut tx, strategy_id, &version.wallet_ids).await?;
        }
        insert_audit(
            &mut tx,
            request_id,
            actor_id,
            "strategy.update",
            "strategy",
            &strategy_id.to_string(),
            operator_note.as_deref(),
        )
        .await?;
        tx.commit().await?;
        self.get_strategy(strategy_id).await
    }

    async fn load_strategy_data(&self, strategy_ids: &[i64]) -> Result<Vec<MarketStrategyData>> {
        if strategy_ids.is_empty() {
            return Ok(Vec::new());
        }
        let strategy_rows = sqlx::query(
            r#"
            SELECT
              s.strategy_id, s.market_id, s.name, s.status AS strategy_status,
              s.created_at AS strategy_created_at, s.updated_at AS strategy_updated_at,
              m.condition_id, m.slug, m.question, m.polymarket_url,
              m.status AS market_status, m.created_at AS market_created_at,
              m.updated_at AS market_updated_at,
              r.minimum_size, r.maximum_spread, r.daily_rate,
              r.updated_at AS reward_updated_at
            FROM market_strategies s
            JOIN managed_markets m ON m.market_id = s.market_id
            JOIN market_reward_terms r ON r.market_id = m.market_id
            WHERE s.strategy_id = ANY($1)
            "#,
        )
        .bind(strategy_ids)
        .fetch_all(&self.pool)
        .await?;
        let version_rows = sqlx::query(
            r#"
            SELECT
              strategy_version_id, strategy_id, version_number, status,
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
        let target_rows = sqlx::query(
            "SELECT strategy_id, wallet_id, enabled, created_at FROM strategy_wallet_targets WHERE strategy_id = ANY($1) ORDER BY strategy_id, wallet_id",
        )
        .bind(strategy_ids)
        .fetch_all(&self.pool)
        .await?;

        let mut versions = HashMap::new();
        for row in version_rows {
            let strategy_id: i64 = row.try_get("strategy_id")?;
            versions.insert(strategy_id, strategy_version_from_row(&row)?);
        }
        let mut slots: HashMap<i64, Vec<StrategyQuoteSlot>> = HashMap::new();
        for row in slot_rows {
            let version_id: i64 = row.try_get("strategy_version_id")?;
            slots
                .entry(version_id)
                .or_default()
                .push(slot_from_row(&row)?);
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
        let mut targets: HashMap<i64, Vec<StrategyWalletTarget>> = HashMap::new();
        for row in target_rows {
            let strategy_id: i64 = row.try_get("strategy_id")?;
            targets
                .entry(strategy_id)
                .or_default()
                .push(StrategyWalletTarget {
                    strategy_id,
                    wallet_id: row.try_get("wallet_id")?,
                    enabled: row.try_get("enabled")?,
                    created_at: row.try_get("created_at")?,
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
                    condition_id: row.try_get("condition_id")?,
                    slug: row.try_get("slug")?,
                    question: row.try_get("question")?,
                    polymarket_url: row.try_get("polymarket_url")?,
                    status: enum_value(row.try_get("market_status")?, "market status")?,
                    created_at: row.try_get("market_created_at")?,
                    updated_at: row.try_get("market_updated_at")?,
                },
                outcomes: outcomes.remove(&market_id).unwrap_or_default(),
                reward_terms: MarketRewardTerms {
                    market_id,
                    minimum_size: row.try_get("minimum_size")?,
                    maximum_spread: row.try_get("maximum_spread")?,
                    daily_rate: row.try_get("daily_rate")?,
                    updated_at: row.try_get("reward_updated_at")?,
                },
                strategy: MarketStrategy {
                    id: strategy_id,
                    market_id,
                    name: row.try_get("name")?,
                    status: enum_value(row.try_get("strategy_status")?, "strategy status")?,
                    created_at: row.try_get("strategy_created_at")?,
                    updated_at: row.try_get("strategy_updated_at")?,
                },
                version,
                quote_slots: version_slots,
                wallet_targets: targets.remove(&strategy_id).unwrap_or_default(),
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

fn validate_strategy_input(request: &CreateMarketStrategyRequest) -> Result<()> {
    required_text(&request.market.condition_id, "condition_id", 256)?;
    required_text(&request.market.yes_token_id, "yes_token_id", 256)?;
    required_text(&request.market.no_token_id, "no_token_id", 256)?;
    if request.market.yes_token_id.trim() == request.market.no_token_id.trim() {
        return Err(ServerError::InvalidInput(
            "YES and NO token ids must differ".to_string(),
        ));
    }
    if request.market.reward_minimum_size <= Decimal::ZERO
        || request.market.reward_maximum_spread < Decimal::ZERO
        || request.market.reward_maximum_spread > Decimal::ONE
    {
        return Err(ServerError::InvalidInput(
            "reward terms are outside their valid range".to_string(),
        ));
    }
    validate_version_input(&request.version)
}

fn validate_version_input(input: &polyedge_contracts::StrategyVersionInput) -> Result<()> {
    if input.book_freshness_ms <= 0
        || input.downward_reprice_confirm_ms < 0
        || input.upward_reprice_confirm_ms < 0
        || input.reprice_cooldown_ms < 0
        || input.max_replaces_per_cycle < 0
        || input.quote_slots.is_empty()
    {
        return Err(ServerError::InvalidInput(
            "strategy version timing and quote slots are invalid".to_string(),
        ));
    }
    let mut keys = std::collections::HashSet::new();
    for slot in &input.quote_slots {
        if !keys.insert(slot.slot_key.trim().to_ascii_lowercase()) {
            return Err(ServerError::InvalidInput(
                "quote slot keys must be unique".to_string(),
            ));
        }
        if slot.quantity <= Decimal::ZERO
            || slot.minimum_price > slot.maximum_price
            || slot.price_offset < -Decimal::ONE
            || slot.price_offset > Decimal::ONE
        {
            return Err(ServerError::InvalidInput(
                "quote slot quantity or price range is invalid".to_string(),
            ));
        }
        validate_price(slot.minimum_price, "minimum_price")?;
        validate_price(slot.maximum_price, "maximum_price")?;
        match slot.pricing_mode {
            QuotePricingMode::Fixed => {
                let price = slot.fixed_price.ok_or_else(|| {
                    ServerError::InvalidInput("fixed quote slot requires fixed_price".to_string())
                })?;
                validate_price(price, "fixed_price")?;
                if slot.book_rank.is_some() {
                    return Err(ServerError::InvalidInput(
                        "fixed quote slot may not include book_rank".to_string(),
                    ));
                }
            }
            QuotePricingMode::BookRank => {
                if slot.book_rank.is_none_or(|rank| rank <= 0) || slot.fixed_price.is_some() {
                    return Err(ServerError::InvalidInput(
                        "book-rank quote slot requires a positive rank and no fixed_price"
                            .to_string(),
                    ));
                }
            }
        }
    }
    Ok(())
}

async fn upsert_market(
    tx: &mut Transaction<'_, Postgres>,
    market: &polyedge_contracts::ManagedMarketInput,
) -> Result<i64> {
    let market_id: i64 = sqlx::query_scalar(
        r#"
        INSERT INTO managed_markets (condition_id, slug, question, polymarket_url, status)
        VALUES ($1, $2, $3, $4, 'open')
        ON CONFLICT (condition_id) DO UPDATE SET
          slug = EXCLUDED.slug,
          question = EXCLUDED.question,
          polymarket_url = EXCLUDED.polymarket_url,
          updated_at = now()
        RETURNING market_id
        "#,
    )
    .bind(required_text(&market.condition_id, "condition_id", 256)?)
    .bind(required_text(&market.slug, "slug", 256)?)
    .bind(required_text(&market.question, "question", 2_000)?)
    .bind(market.polymarket_url.as_deref())
    .fetch_one(&mut **tx)
    .await?;
    for (outcome, token_id) in [
        (QuoteOutcome::Yes, market.yes_token_id.as_str()),
        (QuoteOutcome::No, market.no_token_id.as_str()),
    ] {
        sqlx::query(
            r#"
            INSERT INTO managed_market_outcomes (market_id, outcome, token_id)
            VALUES ($1, $2, $3)
            ON CONFLICT (market_id, outcome) DO UPDATE SET token_id = EXCLUDED.token_id
            "#,
        )
        .bind(market_id)
        .bind(outcome.as_str())
        .bind(required_text(token_id, "token_id", 256)?)
        .execute(&mut **tx)
        .await?;
    }
    sqlx::query(
        r#"
        INSERT INTO market_reward_terms (market_id, minimum_size, maximum_spread, daily_rate)
        VALUES ($1, $2, $3, $4)
        ON CONFLICT (market_id) DO UPDATE SET
          minimum_size = EXCLUDED.minimum_size,
          maximum_spread = EXCLUDED.maximum_spread,
          daily_rate = EXCLUDED.daily_rate,
          updated_at = now()
        "#,
    )
    .bind(market_id)
    .bind(market.reward_minimum_size)
    .bind(market.reward_maximum_spread)
    .bind(market.reward_daily_rate)
    .execute(&mut **tx)
    .await?;
    Ok(market_id)
}

async fn insert_strategy_version(
    tx: &mut Transaction<'_, Postgres>,
    strategy_id: i64,
    version_number: i64,
    input: &polyedge_contracts::StrategyVersionInput,
) -> Result<i64> {
    Ok(sqlx::query_scalar(
        r#"
        INSERT INTO strategy_versions (
          strategy_id, version_number, status, book_freshness_ms,
          downward_reprice_confirm_ms, upward_reprice_confirm_ms,
          reprice_cooldown_ms, max_replaces_per_cycle, published_at
        ) VALUES ($1, $2, 'published', $3, $4, $5, $6, $7, now())
        RETURNING strategy_version_id
        "#,
    )
    .bind(strategy_id)
    .bind(version_number)
    .bind(input.book_freshness_ms)
    .bind(input.downward_reprice_confirm_ms)
    .bind(input.upward_reprice_confirm_ms)
    .bind(input.reprice_cooldown_ms)
    .bind(input.max_replaces_per_cycle)
    .fetch_one(&mut **tx)
    .await?)
}

async fn insert_quote_slots(
    tx: &mut Transaction<'_, Postgres>,
    version_id: i64,
    slots: &[polyedge_contracts::QuoteSlotInput],
) -> Result<()> {
    for slot in slots {
        sqlx::query(
            r#"
            INSERT INTO strategy_quote_slots (
              strategy_version_id, slot_key, outcome, quantity, pricing_mode,
              fixed_price, book_rank, price_offset, minimum_price, maximum_price,
              post_only, enabled
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
            "#,
        )
        .bind(version_id)
        .bind(required_text(&slot.slot_key, "slot_key", 120)?)
        .bind(slot.outcome.as_str())
        .bind(slot.quantity)
        .bind(slot.pricing_mode.as_str())
        .bind(slot.fixed_price)
        .bind(slot.book_rank)
        .bind(slot.price_offset)
        .bind(slot.minimum_price)
        .bind(slot.maximum_price)
        .bind(slot.post_only)
        .bind(slot.enabled)
        .execute(&mut **tx)
        .await?;
    }
    Ok(())
}

async fn replace_wallet_targets(
    tx: &mut Transaction<'_, Postgres>,
    strategy_id: i64,
    wallet_ids: &[i64],
) -> Result<()> {
    sqlx::query("DELETE FROM strategy_wallet_targets WHERE strategy_id = $1")
        .bind(strategy_id)
        .execute(&mut **tx)
        .await?;
    for wallet_id in wallet_ids {
        sqlx::query(
            "INSERT INTO strategy_wallet_targets (strategy_id, wallet_id, enabled) VALUES ($1, $2, TRUE)",
        )
        .bind(strategy_id)
        .bind(*wallet_id)
        .execute(&mut **tx)
        .await?;
    }
    Ok(())
}

fn strategy_version_from_row(row: &sqlx::postgres::PgRow) -> Result<StrategyVersion> {
    Ok(StrategyVersion {
        id: row.try_get("strategy_version_id")?,
        strategy_id: row.try_get("strategy_id")?,
        version_number: row.try_get("version_number")?,
        status: enum_value(row.try_get("status")?, "strategy version status")?,
        book_freshness_ms: row.try_get("book_freshness_ms")?,
        downward_reprice_confirm_ms: row.try_get("downward_reprice_confirm_ms")?,
        upward_reprice_confirm_ms: row.try_get("upward_reprice_confirm_ms")?,
        reprice_cooldown_ms: row.try_get("reprice_cooldown_ms")?,
        max_replaces_per_cycle: row.try_get("max_replaces_per_cycle")?,
        published_at: row.try_get("published_at")?,
        created_at: row.try_get("created_at")?,
    })
}

pub(super) fn slot_from_row(row: &sqlx::postgres::PgRow) -> Result<StrategyQuoteSlot> {
    Ok(StrategyQuoteSlot {
        id: row.try_get("quote_slot_id")?,
        strategy_version_id: row.try_get("strategy_version_id")?,
        slot_key: row.try_get("slot_key")?,
        outcome: enum_value(row.try_get("outcome")?, "quote outcome")?,
        quantity: row.try_get("quantity")?,
        pricing_mode: enum_value(row.try_get("pricing_mode")?, "pricing mode")?,
        fixed_price: row.try_get("fixed_price")?,
        book_rank: row.try_get("book_rank")?,
        price_offset: row.try_get("price_offset")?,
        minimum_price: row.try_get("minimum_price")?,
        maximum_price: row.try_get("maximum_price")?,
        post_only: row.try_get("post_only")?,
        enabled: row.try_get("enabled")?,
    })
}
