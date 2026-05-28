impl PostgresMarketEventStore {
async fn market_event_upsert_markets(
    &self,
    markets: &[MarketView],
    trace_id: &str,
) -> Result<usize> {
    if markets.is_empty() {
        return Ok(0);
    }

    let mut transaction = self.pool.begin().await.map_err(|error| {
        db_error(
            "POSTGRES_TRANSACTION_BEGIN_FAILED",
            format!("failed to begin market upsert transaction: {error}"),
        )
    })?;

    let mut count = 0usize;

    for market in markets {
        sqlx::query(
            r#"
            INSERT INTO markets (
              id,
              question,
              category,
              status,
              best_bid,
              best_ask,
              mid_price,
              volume_24h,
              ambiguity_level,
              tradability_status,
              polymarket_condition_id,
              polymarket_yes_asset_id,
              polymarket_no_asset_id,
              updated_at,
              version,
              trace_id
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16)
            ON CONFLICT (id) DO UPDATE
            SET
              question = EXCLUDED.question,
              category = EXCLUDED.category,
              status = EXCLUDED.status,
              best_bid = EXCLUDED.best_bid,
              best_ask = EXCLUDED.best_ask,
              mid_price = EXCLUDED.mid_price,
              volume_24h = EXCLUDED.volume_24h,
              ambiguity_level = EXCLUDED.ambiguity_level,
              tradability_status = EXCLUDED.tradability_status,
              polymarket_condition_id = EXCLUDED.polymarket_condition_id,
              polymarket_yes_asset_id = EXCLUDED.polymarket_yes_asset_id,
              polymarket_no_asset_id = EXCLUDED.polymarket_no_asset_id,
              updated_at = EXCLUDED.updated_at,
              version = EXCLUDED.version,
              trace_id = EXCLUDED.trace_id
            WHERE EXCLUDED.version > markets.version
            "#,
        )
        .bind(&market.id)
        .bind(&market.question)
        .bind(&market.category)
        .bind(market.status.as_str())
        .bind(market.best_bid.value())
        .bind(market.best_ask.value())
        .bind(market.mid_price.value())
        .bind(market.volume_24h.value())
        .bind(market.ambiguity_level.as_str())
        .bind(market.tradability_status.as_str())
        .bind(&market.polymarket_condition_id)
        .bind(&market.polymarket_yes_asset_id)
        .bind(&market.polymarket_no_asset_id)
        .bind(market.updated_at)
        .bind(market.version)
        .bind(trace_id)
        .execute(&mut *transaction)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_UPSERT_MARKET_FAILED",
                format!("failed to upsert market {}: {error}", market.id),
            )
        })?;

        sqlx::query(
            r#"
            INSERT INTO market_resolution_rules (
              market_id,
              resolution_source,
              edge_case_notes,
              updated_at,
              version,
              trace_id
            )
            VALUES ($1, $2, $3, $4, $5, $6)
            ON CONFLICT (market_id) DO UPDATE
            SET
              resolution_source = EXCLUDED.resolution_source,
              edge_case_notes = EXCLUDED.edge_case_notes,
              updated_at = EXCLUDED.updated_at,
              version = EXCLUDED.version,
              trace_id = EXCLUDED.trace_id
            WHERE EXCLUDED.version > market_resolution_rules.version
            "#,
        )
        .bind(&market.id)
        .bind(&market.resolution_source)
        .bind(&market.edge_case_notes)
        .bind(market.updated_at)
        .bind(market.version)
        .bind(trace_id)
        .execute(&mut *transaction)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_UPSERT_RESOLUTION_RULE_FAILED",
                format!(
                    "failed to upsert market resolution rules for {}: {error}",
                    market.id
                ),
            )
        })?;

        count += 1;
    }

    transaction.commit().await.map_err(|error| {
        db_error(
            "POSTGRES_TRANSACTION_COMMIT_FAILED",
            format!("failed to commit market upsert transaction: {error}"),
        )
    })?;

    Ok(count)
}
}
