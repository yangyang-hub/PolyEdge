const UPSERT_BATCH_SIZE: usize = 100;

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
                r#"WITH incoming (
                  id, slug, question, category, status,
                  best_bid, best_ask, mid_price, volume_24h, liquidity_usd, end_at,
                  ambiguity_level, tradability_status,
                  polymarket_condition_id, polymarket_yes_asset_id, polymarket_no_asset_id,
                  updated_at, version, trace_id
                ) AS (
                  VALUES {values_placeholders}
                ),
                inserted AS (
                  INSERT INTO markets (
                    id, slug, question, category, status,
                    best_bid, best_ask, mid_price, volume_24h, liquidity_usd, end_at,
                    ambiguity_level, tradability_status,
                    polymarket_condition_id, polymarket_yes_asset_id, polymarket_no_asset_id,
                    updated_at, version, trace_id
                  )
                  SELECT
                    id, slug, question, category, status,
                    best_bid, best_ask, mid_price, volume_24h, liquidity_usd, end_at,
                    ambiguity_level, tradability_status,
                    polymarket_condition_id, polymarket_yes_asset_id, polymarket_no_asset_id,
                    updated_at, version, trace_id
                  FROM incoming
                  ON CONFLICT (id) DO NOTHING
                  RETURNING id
                ),
                changed AS (
                  UPDATE markets m
                  SET
                    slug = i.slug,
                    question = i.question,
                    category = i.category,
                    status = i.status,
                    best_bid = i.best_bid,
                    best_ask = i.best_ask,
                    mid_price = i.mid_price,
                    volume_24h = i.volume_24h,
                    liquidity_usd = i.liquidity_usd,
                    end_at = i.end_at,
                    synced_at = now(),
                    ambiguity_level = i.ambiguity_level,
                    tradability_status = i.tradability_status,
                    polymarket_condition_id = i.polymarket_condition_id,
                    polymarket_yes_asset_id = i.polymarket_yes_asset_id,
                    polymarket_no_asset_id = i.polymarket_no_asset_id,
                    updated_at = i.updated_at,
                    version = i.version,
                    trace_id = i.trace_id
                  FROM incoming i
                  WHERE m.id = i.id
                    AND i.version >= m.version
                    AND NOT EXISTS (SELECT 1 FROM inserted x WHERE x.id = i.id)
                    AND (
                      i.version > m.version
                      OR (
                        m.slug,
                        m.question,
                        m.category,
                        m.status,
                        m.best_bid,
                        m.best_ask,
                        m.mid_price,
                        m.volume_24h,
                        m.liquidity_usd,
                        m.end_at,
                        m.ambiguity_level,
                        m.tradability_status,
                        m.polymarket_condition_id,
                        m.polymarket_yes_asset_id,
                        m.polymarket_no_asset_id,
                        m.updated_at,
                        m.version
                      ) IS DISTINCT FROM (
                        i.slug,
                        i.question,
                        i.category,
                        i.status,
                        i.best_bid,
                        i.best_ask,
                        i.mid_price,
                        i.volume_24h,
                        i.liquidity_usd,
                        i.end_at,
                        i.ambiguity_level,
                        i.tradability_status,
                        i.polymarket_condition_id,
                        i.polymarket_yes_asset_id,
                        i.polymarket_no_asset_id,
                        i.updated_at,
                        i.version
                      )
                    )
                  RETURNING m.id
                ),
                refreshed AS (
                  UPDATE markets m
                  SET
                    synced_at = now(),
                    trace_id = i.trace_id
                  FROM incoming i
                  WHERE m.id = i.id
                    AND i.version >= m.version
                    AND NOT EXISTS (SELECT 1 FROM inserted x WHERE x.id = i.id)
                    AND NOT EXISTS (SELECT 1 FROM changed x WHERE x.id = i.id)
                    AND (
                      ${refresh_param}::BIGINT <= 0
                      OR m.synced_at < now() - (${refresh_param}::BIGINT * interval '1 second')
                    )
                  RETURNING m.id
                )
                SELECT (
                  (SELECT COUNT(*) FROM inserted)
                  + (SELECT COUNT(*) FROM changed)
                  + (SELECT COUNT(*) FROM refreshed)
                )::BIGINT AS written_count"#,
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
            let row = query
                .bind(refresh_after_secs)
                .fetch_one(&mut *transaction)
                .await
                .map_err(|error| {
                    db_error(
                        "POSTGRES_BATCH_UPSERT_MARKETS_FAILED",
                        format!(
                            "failed to batch upsert markets (chunk size {}): {error}",
                            chunk.len()
                        ),
                    )
                })?;
            let written_count: i64 = row.try_get("written_count").map_err(|error| {
                db_error(
                    "POSTGRES_DECODE_FAILED",
                    format!("failed to decode market upsert count: {error}"),
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

            count = count.saturating_add(written_count.max(0) as usize);
        }

        Ok(count)
    }
}
