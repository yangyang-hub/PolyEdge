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

    for market in markets {
        sqlx::query(
            r#"
            INSERT INTO reward_markets (
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
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
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
            "#,
        )
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
        .bind(market.updated_at)
        .execute(&mut **transaction)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_UPSERT_FAILED",
                format!("failed to upsert reward market: {error}"),
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

    for plan in plans {
        sqlx::query(
            r#"
            INSERT INTO reward_quote_plans (
              condition_id,
              score,
              eligible,
              reason,
              quote_plan_json,
              updated_at
            )
            VALUES ($1, $2, $3, $4, $5, $6)
            ON CONFLICT (condition_id) DO UPDATE
            SET score = EXCLUDED.score,
                eligible = EXCLUDED.eligible,
                reason = EXCLUDED.reason,
                quote_plan_json = EXCLUDED.quote_plan_json,
                updated_at = EXCLUDED.updated_at
            "#,
        )
        .bind(&plan.condition_id)
        .bind(plan.score)
        .bind(plan.eligible)
        .bind(&plan.reason)
        .bind(Json(plan.clone()))
        .bind(plan.updated_at)
        .execute(&mut **transaction)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_UPSERT_FAILED",
                format!("failed to upsert reward quote plan: {error}"),
            )
        })?;
    }

    Ok(())
}
