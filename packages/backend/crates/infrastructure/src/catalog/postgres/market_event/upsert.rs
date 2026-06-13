const UPSERT_BATCH_SIZE: usize = 100;

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

    for chunk in markets.chunks(UPSERT_BATCH_SIZE) {
        // -- markets batch upsert --
        let market_cols = 19usize;
        let values_placeholders: String = chunk
            .iter()
            .enumerate()
            .map(|(i, _)| {
                let base = i * market_cols;
                let params: Vec<String> = (1..=market_cols)
                    .map(|j| format!("${}", base + j))
                    .collect();
                format!("({})", params.join(", "))
            })
            .collect::<Vec<_>>()
            .join(", ");

        let market_sql = format!(
            r#"INSERT INTO markets (
              id, slug, question, category, status,
              best_bid, best_ask, mid_price, volume_24h, liquidity_usd, end_at,
              ambiguity_level, tradability_status,
              polymarket_condition_id, polymarket_yes_asset_id, polymarket_no_asset_id,
              updated_at, version, trace_id
            )
            VALUES {values_placeholders}
            ON CONFLICT (id) DO UPDATE
            SET
              slug = EXCLUDED.slug,
              question = EXCLUDED.question,
              category = EXCLUDED.category,
              status = EXCLUDED.status,
              best_bid = EXCLUDED.best_bid,
              best_ask = EXCLUDED.best_ask,
              mid_price = EXCLUDED.mid_price,
              volume_24h = EXCLUDED.volume_24h,
              liquidity_usd = EXCLUDED.liquidity_usd,
              end_at = EXCLUDED.end_at,
              synced_at = now(),
              ambiguity_level = EXCLUDED.ambiguity_level,
              tradability_status = EXCLUDED.tradability_status,
              polymarket_condition_id = EXCLUDED.polymarket_condition_id,
              polymarket_yes_asset_id = EXCLUDED.polymarket_yes_asset_id,
              polymarket_no_asset_id = EXCLUDED.polymarket_no_asset_id,
              updated_at = EXCLUDED.updated_at,
              version = EXCLUDED.version,
              trace_id = EXCLUDED.trace_id
            WHERE EXCLUDED.version >= markets.version"#,
        );

        let mut query = sqlx::query(&market_sql);
        for market in chunk {
            query = query
                .bind(&market.id)
                .bind(&market.slug)
                .bind(&market.question)
                .bind(&market.category)
                .bind(market.status.as_str())
                .bind(market.best_bid.value())
                .bind(market.best_ask.value())
                .bind(market.mid_price.value())
                .bind(market.volume_24h.value())
                .bind(market.liquidity_usd.value())
                .bind(market.end_at)
                .bind(market.ambiguity_level.as_str())
                .bind(market.tradability_status.as_str())
                .bind(&market.polymarket_condition_id)
                .bind(&market.polymarket_yes_asset_id)
                .bind(&market.polymarket_no_asset_id)
                .bind(market.updated_at)
                .bind(market.version)
                .bind(trace_id);
        }
        query.execute(&mut *transaction).await.map_err(|error| {
            db_error(
                "POSTGRES_BATCH_UPSERT_MARKETS_FAILED",
                format!("failed to batch upsert markets (chunk size {}): {error}", chunk.len()),
            )
        })?;

        // -- market_resolution_rules batch upsert --
        let rule_cols = 6usize;
        let rule_placeholders: String = chunk
            .iter()
            .enumerate()
            .map(|(i, _)| {
                let base = i * rule_cols;
                let params: Vec<String> = (1..=rule_cols)
                    .map(|j| format!("${}", base + j))
                    .collect();
                format!("({})", params.join(", "))
            })
            .collect::<Vec<_>>()
            .join(", ");

        let rule_sql = format!(
            r#"INSERT INTO market_resolution_rules (
              market_id, resolution_source, edge_case_notes,
              updated_at, version, trace_id
            )
            VALUES {rule_placeholders}
            ON CONFLICT (market_id) DO UPDATE
            SET
              resolution_source = EXCLUDED.resolution_source,
              edge_case_notes = EXCLUDED.edge_case_notes,
              updated_at = EXCLUDED.updated_at,
              version = EXCLUDED.version,
              trace_id = EXCLUDED.trace_id
            WHERE EXCLUDED.version > market_resolution_rules.version"#,
        );

        let mut rule_query = sqlx::query(&rule_sql);
        for market in chunk {
            rule_query = rule_query
                .bind(&market.id)
                .bind(&market.resolution_source)
                .bind(&market.edge_case_notes)
                .bind(market.updated_at)
                .bind(market.version)
                .bind(trace_id);
        }
        rule_query.execute(&mut *transaction).await.map_err(|error| {
            db_error(
                "POSTGRES_BATCH_UPSERT_RESOLUTION_RULES_FAILED",
                format!(
                    "failed to batch upsert market resolution rules (chunk size {}): {error}",
                    chunk.len()
                ),
            )
        })?;

        count += chunk.len();
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
