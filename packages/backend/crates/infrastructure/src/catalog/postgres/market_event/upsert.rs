const UPSERT_BATCH_SIZE: usize = 100;
const MARKET_UPSERT_LOCK_TIMEOUT_MS: i64 = 5_000;
const MARKET_UPSERT_STATEMENT_TIMEOUT_MS: i64 = 30_000;

impl PostgresMarketEventStore {
    async fn market_event_upsert_markets(
        &self,
        markets: &[MarketView],
        trace_id: &str,
    ) -> Result<usize> {
        self.market_event_upsert_markets_with_options(
            markets,
            trace_id,
            MarketUpsertOptions::refresh_always(),
        )
        .await
    }

    async fn market_event_upsert_markets_with_options(
        &self,
        markets: &[MarketView],
        trace_id: &str,
        options: MarketUpsertOptions,
    ) -> Result<usize> {
        if markets.is_empty() {
            return Ok(0);
        }

        let refresh_after_secs = options
            .refresh_synced_at_after_secs
            .map(|seconds| i64::try_from(seconds).unwrap_or(i64::MAX))
            .unwrap_or(0);
        let mut count = 0usize;

        for chunk in markets.chunks(UPSERT_BATCH_SIZE) {
            let mut transaction = self.pool.begin().await.map_err(|error| {
                db_error(
                    "POSTGRES_TRANSACTION_BEGIN_FAILED",
                    format!("failed to begin market upsert transaction: {error}"),
                )
            })?;
            set_local_market_upsert_timeouts(&mut transaction).await?;

            // -- markets batch insert/update/refresh --
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
            let refresh_param = chunk.len() * market_cols + 1;

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
                WHERE EXCLUDED.version >= markets.version
                  AND (
                    EXCLUDED.version > markets.version
                    OR (
                      markets.slug,
                      markets.question,
                      markets.category,
                      markets.status,
                      markets.best_bid,
                      markets.best_ask,
                      markets.mid_price,
                      markets.volume_24h,
                      markets.liquidity_usd,
                      markets.end_at,
                      markets.ambiguity_level,
                      markets.tradability_status,
                      markets.polymarket_condition_id,
                      markets.polymarket_yes_asset_id,
                      markets.polymarket_no_asset_id,
                      markets.updated_at,
                      markets.version
                    ) IS DISTINCT FROM (
                      EXCLUDED.slug,
                      EXCLUDED.question,
                      EXCLUDED.category,
                      EXCLUDED.status,
                      EXCLUDED.best_bid,
                      EXCLUDED.best_ask,
                      EXCLUDED.mid_price,
                      EXCLUDED.volume_24h,
                      EXCLUDED.liquidity_usd,
                      EXCLUDED.end_at,
                      EXCLUDED.ambiguity_level,
                      EXCLUDED.tradability_status,
                      EXCLUDED.polymarket_condition_id,
                      EXCLUDED.polymarket_yes_asset_id,
                      EXCLUDED.polymarket_no_asset_id,
                      EXCLUDED.updated_at,
                      EXCLUDED.version
                    )
                    OR (
                      ${refresh_param}::BIGINT <= 0
                      OR markets.synced_at < now() - (${refresh_param}::BIGINT * interval '1 second')
                    )
                  )"#,
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
            let written_count = query
                .bind(refresh_after_secs)
                .execute(&mut *transaction)
                .await
                .map_err(|error| {
                    db_error(
                        "POSTGRES_BATCH_UPSERT_MARKETS_FAILED",
                        format!(
                            "failed to batch upsert markets (chunk size {}): {error}",
                            chunk.len()
                        ),
                    )
                })?
                .rows_affected();

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
                WHERE EXCLUDED.version > market_resolution_rules.version
                   OR (
                     market_resolution_rules.resolution_source,
                     market_resolution_rules.edge_case_notes,
                     market_resolution_rules.updated_at,
                     market_resolution_rules.version
                   ) IS DISTINCT FROM (
                     EXCLUDED.resolution_source,
                     EXCLUDED.edge_case_notes,
                     EXCLUDED.updated_at,
                     EXCLUDED.version
                   )"#,
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
            rule_query
                .execute(&mut *transaction)
                .await
                .map_err(|error| {
                    db_error(
                        "POSTGRES_BATCH_UPSERT_RESOLUTION_RULES_FAILED",
                        format!(
                            "failed to batch upsert market resolution rules (chunk size {}): {error}",
                            chunk.len()
                        ),
                    )
                })?;

            transaction.commit().await.map_err(|error| {
                db_error(
                    "POSTGRES_TRANSACTION_COMMIT_FAILED",
                    format!("failed to commit market upsert transaction: {error}"),
                )
            })?;

            count = count.saturating_add(usize::try_from(written_count).unwrap_or(usize::MAX));
        }

        Ok(count)
    }
}

async fn set_local_market_upsert_timeouts(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
) -> Result<()> {
    sqlx::query("SELECT set_config('lock_timeout', $1, true), set_config('statement_timeout', $2, true)")
        .bind(format!("{MARKET_UPSERT_LOCK_TIMEOUT_MS}ms"))
        .bind(format!("{MARKET_UPSERT_STATEMENT_TIMEOUT_MS}ms"))
        .execute(&mut **transaction)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_MARKET_UPSERT_TIMEOUT_CONFIG_FAILED",
                format!("failed to configure market upsert statement timeouts: {error}"),
            )
        })?;
    Ok(())
}
