const REWARD_UPSERT_BATCH_SIZE: usize = 100;
const REWARD_MARKET_TOUCH_AFTER_SECS: i64 = 60 * 60;
const REWARD_MARKET_LOCK_TIMEOUT_MS: i64 = 5_000;
const REWARD_MARKET_STATEMENT_TIMEOUT_MS: i64 = 60_000;

async fn upsert_reward_markets_tx(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    markets: &[RewardMarket],
) -> Result<()> {
    set_local_reward_market_upsert_timeouts(transaction).await?;

    let current_condition_ids = markets
        .iter()
        .map(|market| market.condition_id.clone())
        .collect::<Vec<_>>();

    for chunk in markets.chunks(REWARD_UPSERT_BATCH_SIZE) {
        let cols = 11usize;
        let placeholders: String = chunk
            .iter()
            .enumerate()
            .map(|(i, _)| {
                let base = i * cols;
                let params: Vec<String> = (1..=cols)
                    .map(|j| format!("${}", base + j))
                    .collect();
                format!("({})", params.join(", "))
            })
            .collect::<Vec<_>>()
            .join(", ");

        let sql = format!(
            r#"INSERT INTO reward_markets (
              condition_id,
              question,
              market_slug,
              event_slug,
              image,
              rewards_max_spread,
              rewards_min_size,
              total_daily_rate,
              tokens_json,
              active,
              updated_at
            )
            VALUES {placeholders}
            ON CONFLICT (condition_id) DO UPDATE
            SET question = EXCLUDED.question,
                market_slug = EXCLUDED.market_slug,
                event_slug = EXCLUDED.event_slug,
                image = EXCLUDED.image,
                rewards_max_spread = EXCLUDED.rewards_max_spread,
                rewards_min_size = EXCLUDED.rewards_min_size,
                total_daily_rate = EXCLUDED.total_daily_rate,
                tokens_json = EXCLUDED.tokens_json,
                active = EXCLUDED.active,
                updated_at = EXCLUDED.updated_at
            WHERE reward_markets.question IS DISTINCT FROM EXCLUDED.question
               OR reward_markets.market_slug IS DISTINCT FROM EXCLUDED.market_slug
               OR reward_markets.event_slug IS DISTINCT FROM EXCLUDED.event_slug
               OR reward_markets.image IS DISTINCT FROM EXCLUDED.image
               OR reward_markets.rewards_max_spread IS DISTINCT FROM EXCLUDED.rewards_max_spread
               OR reward_markets.rewards_min_size IS DISTINCT FROM EXCLUDED.rewards_min_size
               OR reward_markets.total_daily_rate IS DISTINCT FROM EXCLUDED.total_daily_rate
               OR reward_markets.tokens_json IS DISTINCT FROM EXCLUDED.tokens_json
               OR reward_markets.active IS DISTINCT FROM EXCLUDED.active
               OR reward_markets.updated_at < now() - (${}::BIGINT * interval '1 second')"#,
            chunk.len() * cols + 1,
        );

        let mut query = sqlx::query(&sql);
        for market in chunk {
            query = query
                .bind(&market.condition_id)
                .bind(&market.question)
                .bind(&market.market_slug)
                .bind(&market.event_slug)
                .bind(&market.image)
                .bind(market.rewards_max_spread)
                .bind(market.rewards_min_size)
                .bind(market.total_daily_rate)
                .bind(Json(market.tokens.clone()))
                .bind(market.active)
                .bind(market.updated_at);
        }
        query
            .bind(REWARD_MARKET_TOUCH_AFTER_SECS)
            .execute(&mut **transaction)
            .await
            .map_err(|error| {
                db_error(
                    "POSTGRES_BATCH_UPSERT_REWARD_MARKETS_FAILED",
                    format!(
                        "failed to batch upsert reward markets (chunk size {}): {error}",
                        chunk.len()
                    ),
                )
            })?;
    }

    sqlx::query(
        r#"
        UPDATE reward_markets
        SET active = false,
            updated_at = now()
        WHERE active = true
          AND NOT (condition_id = ANY($1))
        "#,
    )
    .bind(&current_condition_ids)
    .execute(&mut **transaction)
    .await
    .map_err(|error| {
        db_error(
            "POSTGRES_UPDATE_FAILED",
            format!("failed to deactivate stale reward markets: {error}"),
        )
    })?;

    Ok(())
}

async fn set_local_reward_market_upsert_timeouts(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
) -> Result<()> {
    sqlx::query("SELECT set_config('lock_timeout', $1, true), set_config('statement_timeout', $2, true)")
        .bind(format!("{REWARD_MARKET_LOCK_TIMEOUT_MS}ms"))
        .bind(format!("{REWARD_MARKET_STATEMENT_TIMEOUT_MS}ms"))
        .execute(&mut **transaction)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_REWARD_MARKET_UPSERT_TIMEOUT_CONFIG_FAILED",
                format!("failed to configure reward market upsert statement timeouts: {error}"),
            )
        })?;
    Ok(())
}

async fn replace_reward_quote_plans_tx(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    plans: &[RewardQuotePlan],
) -> Result<()> {
    sqlx::query("DELETE FROM reward_quote_plans")
        .execute(&mut **transaction)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_DELETE_FAILED",
                format!("failed to clear reward quote plans: {error}"),
            )
        })?;

    for chunk in plans.chunks(REWARD_UPSERT_BATCH_SIZE) {
        let cols = 6usize;
        let placeholders: String = chunk
            .iter()
            .enumerate()
            .map(|(i, _)| {
                let base = i * cols;
                let params: Vec<String> = (1..=cols)
                    .map(|j| format!("${}", base + j))
                    .collect();
                format!("({})", params.join(", "))
            })
            .collect::<Vec<_>>()
            .join(", ");

        let sql = format!(
            r#"INSERT INTO reward_quote_plans (
              condition_id,
              score,
              eligible,
              reason,
              quote_plan_json,
              updated_at
            )
            VALUES {placeholders}
            ON CONFLICT (condition_id) DO UPDATE
            SET score = EXCLUDED.score,
                eligible = EXCLUDED.eligible,
                reason = EXCLUDED.reason,
                quote_plan_json = EXCLUDED.quote_plan_json,
                updated_at = EXCLUDED.updated_at"#,
        );

        let mut query = sqlx::query(&sql);
        for plan in chunk {
            query = query
                .bind(&plan.condition_id)
                .bind(plan.score)
                .bind(plan.eligible)
                .bind(&plan.reason)
                .bind(Json(plan.clone()))
                .bind(plan.updated_at);
        }
        query.execute(&mut **transaction).await.map_err(|error| {
            db_error(
                "POSTGRES_BATCH_UPSERT_REWARD_QUOTE_PLANS_FAILED",
                format!(
                    "failed to batch upsert reward quote plans (chunk size {}): {error}",
                    chunk.len()
                ),
            )
        })?;
    }

    Ok(())
}
