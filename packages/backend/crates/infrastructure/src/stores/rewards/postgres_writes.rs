const REWARD_UPSERT_BATCH_SIZE: usize = 100;

async fn upsert_reward_markets_tx(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    markets: &[RewardMarket],
) -> Result<()> {
    // Deactivate all existing reward markets first.
    // Only markets present in the current API response will be re-activated.
    sqlx::query("UPDATE reward_markets SET active = false WHERE active = true")
        .execute(&mut **transaction)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_UPDATE_FAILED",
                format!("failed to deactivate stale reward markets: {error}"),
            )
        })?;

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
                updated_at = EXCLUDED.updated_at"#,
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
        query.execute(&mut **transaction).await.map_err(|error| {
            db_error(
                "POSTGRES_BATCH_UPSERT_REWARD_MARKETS_FAILED",
                format!(
                    "failed to batch upsert reward markets (chunk size {}): {error}",
                    chunk.len()
                ),
            )
        })?;
    }

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
