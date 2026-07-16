use super::*;

mod read;

impl PostgresStore {
    pub async fn list_strategies(
        &self,
        query: &ManualTradingListQuery,
        actor: polyedge_domain::ActorScope,
    ) -> Result<Vec<MarketStrategyData>> {
        let (limit, offset) = page_values(query);
        let ids = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT strategy_id
            FROM market_strategies
            WHERE ($1::text IS NULL OR status = $1)
              AND ($2::bigint IS NULL OR market_id = $2)
              AND ($3::bigint IS NULL OR strategy_id = $3)
              AND ($4 OR owner_user_id = $5 OR visibility = 'followable')
            ORDER BY updated_at DESC, strategy_id DESC
            LIMIT $6 OFFSET $7
            "#,
        )
        .bind(query.status.as_deref())
        .bind(query.market_id)
        .bind(query.strategy_id)
        .bind(actor.is_admin())
        .bind(actor.user_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;
        self.load_strategy_data(&ids, actor).await
    }

    pub async fn discover_strategies(
        &self,
        query: &ManualTradingListQuery,
        actor: polyedge_domain::ActorScope,
    ) -> Result<Vec<MarketStrategyData>> {
        let (limit, offset) = page_values(query);
        let ids = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT strategy_id
            FROM market_strategies
            WHERE visibility = 'followable'
              AND owner_user_id <> $1
              AND status NOT IN ('expired', 'archived')
              AND active_until > now()
              AND ($2::text IS NULL OR status = $2)
              AND ($3::bigint IS NULL OR market_id = $3)
            ORDER BY updated_at DESC, strategy_id DESC
            LIMIT $4 OFFSET $5
            "#,
        )
        .bind(actor.user_id)
        .bind(query.status.as_deref())
        .bind(query.market_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;
        self.load_strategy_data(&ids, actor).await
    }

    pub async fn get_strategy(
        &self,
        strategy_id: i64,
        actor: polyedge_domain::ActorScope,
    ) -> Result<MarketStrategyData> {
        let accessible = sqlx::query_scalar::<_, bool>(
            r#"
            SELECT EXISTS (
              SELECT 1 FROM market_strategies
              WHERE strategy_id = $1
                AND ($2 OR owner_user_id = $3 OR visibility = 'followable')
            )
            "#,
        )
        .bind(strategy_id)
        .bind(actor.is_admin())
        .bind(actor.user_id)
        .fetch_one(&self.pool)
        .await?;
        if !accessible {
            return Err(ServerError::NotFound(format!("strategy {strategy_id}")));
        }
        self.load_strategy_data(&[strategy_id], actor)
            .await?
            .into_iter()
            .next()
            .ok_or_else(|| ServerError::NotFound(format!("strategy {strategy_id}")))
    }

    pub async fn create_strategy(
        &self,
        request: &CreateMarketStrategyRequest,
        actor: polyedge_domain::ActorScope,
        request_id: &str,
    ) -> Result<MarketStrategyData> {
        require_market_writer(actor)?;
        validate_strategy_input(request)?;
        let name = required_text(&request.name, "name", 160)?;
        let operator_note = optional_note(request.operator_note.as_deref())?;
        let mut tx = self.pool.begin().await?;
        let market_id = upsert_market(&mut tx, &request.market, actor.user_id).await?;
        let strategy_id: i64 = sqlx::query_scalar(
            r#"
            INSERT INTO market_strategies (
              owner_user_id, market_id, name, status, visibility, active_from, active_until
            ) VALUES ($1, $2, $3, 'active', $4, $5, $6)
            RETURNING strategy_id
            "#,
        )
        .bind(actor.user_id)
        .bind(market_id)
        .bind(name)
        .bind(request.visibility.as_str())
        .bind(request.active_from)
        .bind(request.active_until)
        .fetch_one(&mut *tx)
        .await?;
        let version_id = insert_strategy_version(&mut tx, strategy_id, 1, &request.version).await?;
        insert_quote_slots(&mut tx, version_id, &request.version.quote_slots).await?;
        let subscription_id: i64 = sqlx::query_scalar(
            r#"
            INSERT INTO strategy_subscriptions (
              follower_user_id, source_strategy_id, subscription_kind, status
            ) VALUES ($1, $2, 'owner', 'active')
            RETURNING subscription_id
            "#,
        )
        .bind(actor.user_id)
        .bind(strategy_id)
        .fetch_one(&mut *tx)
        .await?;
        replace_subscription_wallets(&mut tx, subscription_id, actor.user_id, &request.wallet_ids)
            .await?;
        insert_strategy_command(
            &mut tx,
            strategy_id,
            actor.user_id,
            Some(version_id),
            polyedge_domain::StrategyCommandType::Publish,
        )
        .await?;
        insert_strategy_command(
            &mut tx,
            strategy_id,
            actor.user_id,
            Some(version_id),
            polyedge_domain::StrategyCommandType::Activate,
        )
        .await?;
        insert_strategy_audit(
            &mut tx,
            request_id,
            actor,
            actor.user_id,
            "strategy.create",
            "strategy",
            strategy_id,
            operator_note.as_deref(),
        )
        .await?;
        tx.commit().await?;
        self.get_strategy(strategy_id, actor).await
    }

    pub async fn update_strategy(
        &self,
        strategy_id: i64,
        request: &UpdateMarketStrategyRequest,
        actor: polyedge_domain::ActorScope,
        request_id: &str,
    ) -> Result<MarketStrategyData> {
        require_market_writer(actor)?;
        let existing = self.get_strategy(strategy_id, actor).await?;
        if !actor.is_admin() && existing.strategy.owner_user_id != actor.user_id {
            return Err(ServerError::Forbidden);
        }
        if matches!(
            existing.strategy.status,
            StrategyStatus::Expired | StrategyStatus::Archived
        ) {
            return Err(ServerError::Conflict(
                "expired or archived strategies are immutable; create a new strategy window"
                    .to_string(),
            ));
        }
        let name = request
            .name
            .as_deref()
            .map(|value| required_text(value, "name", 160))
            .transpose()?
            .unwrap_or(existing.strategy.name.clone());
        let active_from = request.active_from.unwrap_or(existing.strategy.active_from);
        let active_until = request
            .active_until
            .unwrap_or(existing.strategy.active_until);
        validate_active_window(active_from, active_until, false)?;
        let mut status = request.status.unwrap_or(existing.strategy.status);
        if active_until <= OffsetDateTime::now_utc() {
            status = StrategyStatus::Expired;
        }
        if status == StrategyStatus::Draft {
            return Err(ServerError::InvalidInput(
                "published strategies cannot return to draft".to_string(),
            ));
        }
        let visibility = request.visibility.unwrap_or(existing.strategy.visibility);
        let operator_note = optional_note(request.operator_note.as_deref())?;
        let mut tx = self.pool.begin().await?;
        sqlx::query("SELECT strategy_id FROM market_strategies WHERE strategy_id = $1 FOR UPDATE")
            .bind(strategy_id)
            .fetch_one(&mut *tx)
            .await?;
        sqlx::query(
            r#"
            UPDATE market_strategies
            SET name = $2, status = $3, visibility = $4,
                active_from = $5, active_until = $6,
                expired_at = CASE WHEN $3 = 'expired' THEN COALESCE(expired_at, now()) ELSE NULL END,
                updated_at = now()
            WHERE strategy_id = $1
            "#,
        )
        .bind(strategy_id)
        .bind(name)
        .bind(status.as_str())
        .bind(visibility.as_str())
        .bind(active_from)
        .bind(active_until)
        .execute(&mut *tx)
        .await?;

        if let Some(market) = request.market.as_ref() {
            if !actor.is_admin() && existing.market.created_by_user_id != actor.user_id {
                return Err(ServerError::Forbidden);
            }
            sqlx::query(
                r#"
                UPDATE managed_markets
                SET slug = COALESCE($2, slug), question = COALESCE($3, question),
                    polymarket_url = COALESCE($4, polymarket_url),
                    status = COALESCE($5, status), updated_at = now()
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
        }

        let mut published_version_id = None;
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
            insert_strategy_command(
                &mut tx,
                strategy_id,
                existing.strategy.owner_user_id,
                Some(version_id),
                polyedge_domain::StrategyCommandType::Publish,
            )
            .await?;
            published_version_id = Some(version_id);
        }
        if let Some(wallet_ids) = request.wallet_ids.as_ref() {
            let owner_subscription_id: i64 = sqlx::query_scalar(
                r#"
                SELECT subscription_id FROM strategy_subscriptions
                WHERE source_strategy_id = $1 AND subscription_kind = 'owner'
                "#,
            )
            .bind(strategy_id)
            .fetch_one(&mut *tx)
            .await?;
            replace_subscription_wallets(
                &mut tx,
                owner_subscription_id,
                existing.strategy.owner_user_id,
                wallet_ids,
            )
            .await?;
        }

        if visibility == polyedge_domain::StrategyVisibility::Private
            && existing.strategy.visibility == polyedge_domain::StrategyVisibility::Followable
        {
            stop_follower_subscriptions(&mut tx, strategy_id).await?;
            insert_strategy_command(
                &mut tx,
                strategy_id,
                existing.strategy.owner_user_id,
                published_version_id.or(Some(existing.version.id)),
                polyedge_domain::StrategyCommandType::ForceCancel,
            )
            .await?;
        }
        if status != existing.strategy.status {
            let command_type = match status {
                StrategyStatus::Active if existing.strategy.status == StrategyStatus::Paused => {
                    polyedge_domain::StrategyCommandType::Resume
                }
                StrategyStatus::Active => polyedge_domain::StrategyCommandType::Activate,
                StrategyStatus::Paused => polyedge_domain::StrategyCommandType::Pause,
                StrategyStatus::Expired => polyedge_domain::StrategyCommandType::Expire,
                StrategyStatus::Archived => polyedge_domain::StrategyCommandType::Archive,
                StrategyStatus::Draft => unreachable!("draft was rejected above"),
            };
            if status == StrategyStatus::Expired {
                expire_follower_subscriptions(&mut tx, strategy_id).await?;
            } else if status == StrategyStatus::Archived {
                stop_follower_subscriptions(&mut tx, strategy_id).await?;
            }
            insert_strategy_command(
                &mut tx,
                strategy_id,
                existing.strategy.owner_user_id,
                published_version_id.or(Some(existing.version.id)),
                command_type,
            )
            .await?;
        }
        insert_strategy_audit(
            &mut tx,
            request_id,
            actor,
            existing.strategy.owner_user_id,
            "strategy.update",
            "strategy",
            strategy_id,
            operator_note.as_deref(),
        )
        .await?;
        tx.commit().await?;
        self.get_strategy(strategy_id, actor).await
    }

    pub async fn expire_due_strategies(&self, limit: i64) -> Result<u64> {
        let mut tx = self.pool.begin().await?;
        sqlx::query(
            r#"
            UPDATE strategy_subscriptions
            SET status = 'expired', stopped_at = COALESCE(stopped_at, now()),
                updated_at = now()
            WHERE status IN ('active', 'paused')
              AND active_until IS NOT NULL AND active_until <= now()
            "#,
        )
        .execute(&mut *tx)
        .await?;
        let rows = sqlx::query(
            r#"
            SELECT strategy_id, owner_user_id
            FROM market_strategies
            WHERE status IN ('active', 'paused') AND active_until <= now()
            ORDER BY active_until, strategy_id
            LIMIT $1 FOR UPDATE SKIP LOCKED
            "#,
        )
        .bind(limit.clamp(1, 500))
        .fetch_all(&mut *tx)
        .await?;
        for row in &rows {
            let strategy_id: i64 = row.try_get("strategy_id")?;
            let owner_user_id: i64 = row.try_get("owner_user_id")?;
            sqlx::query(
                "UPDATE market_strategies SET status = 'expired', expired_at = now(), updated_at = now() WHERE strategy_id = $1",
            )
            .bind(strategy_id)
            .execute(&mut *tx)
            .await?;
            sqlx::query(
                r#"
                UPDATE strategy_subscriptions
                SET status = 'expired', stopped_at = COALESCE(stopped_at, now()), updated_at = now()
                WHERE source_strategy_id = $1 AND status IN ('active', 'paused')
                "#,
            )
            .bind(strategy_id)
            .execute(&mut *tx)
            .await?;
            let version_id = sqlx::query_scalar::<_, i64>(
                "SELECT strategy_version_id FROM strategy_versions WHERE strategy_id = $1 AND status = 'published'",
            )
            .bind(strategy_id)
            .fetch_optional(&mut *tx)
            .await?;
            insert_strategy_command(
                &mut tx,
                strategy_id,
                owner_user_id,
                version_id,
                polyedge_domain::StrategyCommandType::Expire,
            )
            .await?;
        }
        let count = rows.len() as u64;
        tx.commit().await?;
        Ok(count)
    }
}

fn require_market_writer(actor: polyedge_domain::ActorScope) -> Result<()> {
    if actor.can_write_markets() {
        Ok(())
    } else {
        Err(ServerError::Forbidden)
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
    validate_active_window(request.active_from, request.active_until, true)?;
    validate_version_input(&request.version)
}

fn validate_active_window(
    active_from: OffsetDateTime,
    active_until: OffsetDateTime,
    require_future: bool,
) -> Result<()> {
    if active_until <= active_from || (require_future && active_until <= OffsetDateTime::now_utc())
    {
        return Err(ServerError::InvalidInput(
            "active_until must be later than active_from and in the future".to_string(),
        ));
    }
    Ok(())
}

fn validate_version_input(input: &polyedge_contracts::StrategyVersionInput) -> Result<()> {
    if input.reward_minimum_size <= Decimal::ZERO
        || input.reward_maximum_spread < Decimal::ZERO
        || input.reward_maximum_spread > Decimal::ONE
        || input
            .reward_daily_rate
            .is_some_and(|rate| rate < Decimal::ZERO)
        || input.book_freshness_ms <= 0
        || input.downward_reprice_confirm_ms < 0
        || input.upward_reprice_confirm_ms < 0
        || input.reprice_cooldown_ms < 0
        || input.max_replaces_per_cycle < 0
        || input.quote_slots.is_empty()
    {
        return Err(ServerError::InvalidInput(
            "strategy version rewards, timing, or quote slots are invalid".to_string(),
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
    actor_user_id: i64,
) -> Result<i64> {
    let market_id: i64 = sqlx::query_scalar(
        r#"
        INSERT INTO managed_markets (
          created_by_user_id, condition_id, slug, question, polymarket_url, status
        ) VALUES ($1, $2, $3, $4, $5, 'open')
        ON CONFLICT (condition_id) DO UPDATE SET condition_id = EXCLUDED.condition_id
        RETURNING market_id
        "#,
    )
    .bind(actor_user_id)
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
        let token_id = required_text(token_id, "token_id", 256)?;
        sqlx::query(
            r#"
            INSERT INTO managed_market_outcomes (market_id, outcome, token_id)
            VALUES ($1, $2, $3)
            ON CONFLICT (market_id, outcome) DO NOTHING
            "#,
        )
        .bind(market_id)
        .bind(outcome.as_str())
        .bind(&token_id)
        .execute(&mut **tx)
        .await?;
        let stored: String = sqlx::query_scalar(
            "SELECT token_id FROM managed_market_outcomes WHERE market_id = $1 AND outcome = $2",
        )
        .bind(market_id)
        .bind(outcome.as_str())
        .fetch_one(&mut **tx)
        .await?;
        if stored != token_id {
            return Err(ServerError::Conflict(format!(
                "condition already exists with a different {} token",
                outcome.as_str()
            )));
        }
    }
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
          strategy_id, version_number, status,
          reward_minimum_size, reward_maximum_spread, reward_daily_rate,
          book_freshness_ms, downward_reprice_confirm_ms, upward_reprice_confirm_ms,
          reprice_cooldown_ms, max_replaces_per_cycle, published_at
        ) VALUES ($1, $2, 'published', $3, $4, $5, $6, $7, $8, $9, $10, now())
        RETURNING strategy_version_id
        "#,
    )
    .bind(strategy_id)
    .bind(version_number)
    .bind(input.reward_minimum_size)
    .bind(input.reward_maximum_spread)
    .bind(input.reward_daily_rate)
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

pub(super) async fn insert_strategy_command(
    tx: &mut Transaction<'_, Postgres>,
    strategy_id: i64,
    source_user_id: i64,
    version_id: Option<i64>,
    command_type: polyedge_domain::StrategyCommandType,
) -> Result<i64> {
    let sequence: i64 = sqlx::query_scalar(
        "SELECT COALESCE(MAX(command_sequence), 0) + 1 FROM strategy_commands WHERE source_strategy_id = $1",
    )
    .bind(strategy_id)
    .fetch_one(&mut **tx)
    .await?;
    Ok(sqlx::query_scalar(
        r#"
        INSERT INTO strategy_commands (
          source_strategy_id, source_user_id, strategy_version_id,
          command_sequence, command_type, status
        ) VALUES ($1, $2, $3, $4, $5, 'pending')
        RETURNING command_id
        "#,
    )
    .bind(strategy_id)
    .bind(source_user_id)
    .bind(version_id)
    .bind(sequence)
    .bind(command_type.as_str())
    .fetch_one(&mut **tx)
    .await?)
}

pub(super) async fn replace_subscription_wallets(
    tx: &mut Transaction<'_, Postgres>,
    subscription_id: i64,
    follower_user_id: i64,
    wallet_ids: &[i64],
) -> Result<()> {
    let mut unique = wallet_ids.to_vec();
    unique.sort_unstable();
    unique.dedup();
    if !unique.is_empty() {
        let owned: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM wallet_accounts WHERE owner_user_id = $1 AND wallet_id = ANY($2)",
        )
        .bind(follower_user_id)
        .bind(&unique)
        .fetch_one(&mut **tx)
        .await?;
        if owned != unique.len() as i64 {
            return Err(ServerError::Forbidden);
        }
    }
    sqlx::query("DELETE FROM strategy_subscription_wallets WHERE subscription_id = $1")
        .bind(subscription_id)
        .execute(&mut **tx)
        .await?;
    for wallet_id in unique {
        sqlx::query(
            r#"
            INSERT INTO strategy_subscription_wallets (
              subscription_id, follower_user_id, wallet_id, enabled
            ) VALUES ($1, $2, $3, TRUE)
            "#,
        )
        .bind(subscription_id)
        .bind(follower_user_id)
        .bind(wallet_id)
        .execute(&mut **tx)
        .await?;
    }
    Ok(())
}

async fn stop_follower_subscriptions(
    tx: &mut Transaction<'_, Postgres>,
    strategy_id: i64,
) -> Result<()> {
    sqlx::query(
        r#"
        UPDATE strategy_subscriptions
        SET status = 'stopped', stopped_at = COALESCE(stopped_at, now()), updated_at = now()
        WHERE source_strategy_id = $1 AND subscription_kind = 'follower'
          AND status IN ('active', 'paused')
        "#,
    )
    .bind(strategy_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

async fn expire_follower_subscriptions(
    tx: &mut Transaction<'_, Postgres>,
    strategy_id: i64,
) -> Result<()> {
    sqlx::query(
        r#"
        UPDATE strategy_subscriptions
        SET status = 'expired', stopped_at = COALESCE(stopped_at, now()), updated_at = now()
        WHERE source_strategy_id = $1 AND subscription_kind = 'follower'
          AND status IN ('active', 'paused')
        "#,
    )
    .bind(strategy_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(super) async fn insert_strategy_audit(
    tx: &mut Transaction<'_, Postgres>,
    request_id: &str,
    actor: polyedge_domain::ActorScope,
    resource_owner_user_id: i64,
    action: &str,
    resource_type: &str,
    resource_id: i64,
    operator_note: Option<&str>,
) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO audit_logs (
          request_id, actor_type, actor_user_id, action, resource_owner_user_id,
          resource_type, resource_id, result, operator_note
        ) VALUES ($1, 'user', $2, $3, $4, $5, $6, 'succeeded', $7)
        "#,
    )
    .bind(request_id)
    .bind(actor.user_id)
    .bind(action)
    .bind(resource_owner_user_id)
    .bind(resource_type)
    .bind(resource_id.to_string())
    .bind(operator_note)
    .execute(&mut **tx)
    .await?;
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
