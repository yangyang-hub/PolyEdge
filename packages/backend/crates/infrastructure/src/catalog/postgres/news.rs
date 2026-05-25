use super::*;

#[async_trait]
impl NewsIngestionStore for PostgresMarketEventStore {
    async fn list_news_source_health(
        &self,
        filters: &NewsSourceHealthListFilters,
    ) -> Result<Vec<NewsSourceHealthView>> {
        let rows = sqlx::query(
            r#"
            SELECT
              source,
              source_type,
              enabled,
              reliability,
              last_success_at,
              last_error_at,
              consecutive_failures,
              items_fetched,
              items_inserted,
              items_deduped,
              health_score,
              last_error,
              updated_at
            FROM news_source_health
            WHERE ($1::TEXT IS NULL OR source_type = $1)
            ORDER BY updated_at DESC, source ASC
            LIMIT $2
            "#,
        )
        .bind(filters.source_type.as_deref())
        .bind(i64::from(filters.limit))
        .fetch_all(&self.pool)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_QUERY_FAILED",
                format!("failed to list news source health: {error}"),
            )
        })?;

        rows.iter().map(parse_news_source_health_row).collect()
    }

    async fn list_raw_news_events(
        &self,
        filters: &NewsRawEventListFilters,
    ) -> Result<Vec<NewsRawEventView>> {
        let rows = sqlx::query(
            r#"
            SELECT
              id,
              source,
              source_type,
              external_id,
              title,
              url,
              author,
              published_at,
              event_time,
              hash,
              raw_payload,
              ingested_at,
              trace_id
            FROM raw_events
            WHERE source_type IS NOT NULL
              AND title IS NOT NULL
              AND event_time IS NOT NULL
              AND ($1::TEXT IS NULL OR source = $1)
              AND ($2::TEXT IS NULL OR source_type = $2)
            ORDER BY event_time DESC, ingested_at DESC, id ASC
            LIMIT $3
            "#,
        )
        .bind(filters.source.as_deref())
        .bind(filters.source_type.as_deref())
        .bind(i64::from(filters.limit))
        .fetch_all(&self.pool)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_QUERY_FAILED",
                format!("failed to list raw news events: {error}"),
            )
        })?;

        rows.iter().map(parse_news_raw_event_row).collect()
    }

    async fn insert_raw_news_event(&self, event: &NewsRawEventInsert) -> Result<bool> {
        let result = sqlx::query(
            r#"
            INSERT INTO raw_events (
              id,
              source,
              source_type,
              external_id,
              title,
              url,
              author,
              published_at,
              event_time,
              hash,
              raw_payload,
              ingested_at,
              trace_id
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
            ON CONFLICT DO NOTHING
            "#,
        )
        .bind(&event.id)
        .bind(&event.source)
        .bind(&event.source_type)
        .bind(&event.external_id)
        .bind(&event.title)
        .bind(&event.url)
        .bind(&event.author)
        .bind(event.published_at)
        .bind(event.event_time)
        .bind(&event.hash)
        .bind(Json(event.raw_payload.clone()))
        .bind(event.ingested_at)
        .bind(&event.trace_id)
        .execute(&self.pool)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_INSERT_FAILED",
                format!("failed to insert raw news event {}: {error}", event.id),
            )
        })?;

        Ok(result.rows_affected() > 0)
    }

    async fn record_news_source_success(&self, update: &NewsSourceSuccessUpdate) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO news_source_health (
              source,
              source_type,
              enabled,
              reliability,
              last_success_at,
              last_error_at,
              consecutive_failures,
              items_fetched,
              items_inserted,
              items_deduped,
              health_score,
              last_error,
              updated_at,
              trace_id
            )
            VALUES ($1, $2, TRUE, $3, $4, NULL, 0, $5, $6, $7, $3, NULL, $4, $8)
            ON CONFLICT (source) DO UPDATE
            SET
              source_type = EXCLUDED.source_type,
              enabled = TRUE,
              reliability = EXCLUDED.reliability,
              last_success_at = EXCLUDED.last_success_at,
              consecutive_failures = 0,
              items_fetched = news_source_health.items_fetched + EXCLUDED.items_fetched,
              items_inserted = news_source_health.items_inserted + EXCLUDED.items_inserted,
              items_deduped = news_source_health.items_deduped + EXCLUDED.items_deduped,
              health_score = EXCLUDED.health_score,
              last_error = NULL,
              updated_at = EXCLUDED.updated_at,
              trace_id = EXCLUDED.trace_id
            "#,
        )
        .bind(&update.source)
        .bind(&update.source_type)
        .bind(update.reliability.value())
        .bind(update.observed_at)
        .bind(usize_to_i64(update.fetched)?)
        .bind(usize_to_i64(update.inserted)?)
        .bind(usize_to_i64(update.deduped)?)
        .bind(&update.trace_id)
        .execute(&self.pool)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_UPSERT_FAILED",
                format!(
                    "failed to record news source success {}: {error}",
                    update.source
                ),
            )
        })?;

        Ok(())
    }

    async fn record_news_source_failure(&self, update: &NewsSourceFailureUpdate) -> Result<()> {
        let last_error = clamped_error_message(&update.error_message);
        sqlx::query(
            r#"
            INSERT INTO news_source_health (
              source,
              source_type,
              enabled,
              reliability,
              last_success_at,
              last_error_at,
              consecutive_failures,
              items_fetched,
              items_inserted,
              items_deduped,
              health_score,
              last_error,
              updated_at,
              trace_id
            )
            VALUES (
              $1,
              $2,
              TRUE,
              $3,
              NULL,
              $4,
              1,
              0,
              0,
              0,
              GREATEST(0::numeric, $3 - (1::numeric / 5::numeric)),
              $5,
              $4,
              $6
            )
            ON CONFLICT (source) DO UPDATE
            SET
              source_type = EXCLUDED.source_type,
              enabled = TRUE,
              reliability = EXCLUDED.reliability,
              last_error_at = EXCLUDED.last_error_at,
              consecutive_failures = news_source_health.consecutive_failures + 1,
              health_score = GREATEST(
                0::numeric,
                EXCLUDED.reliability
                  - (LEAST(news_source_health.consecutive_failures + 1, 5)::numeric / 5::numeric)
              ),
              last_error = EXCLUDED.last_error,
              updated_at = EXCLUDED.updated_at,
              trace_id = EXCLUDED.trace_id
            "#,
        )
        .bind(&update.source)
        .bind(&update.source_type)
        .bind(update.reliability.value())
        .bind(update.observed_at)
        .bind(last_error)
        .bind(&update.trace_id)
        .execute(&self.pool)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_UPSERT_FAILED",
                format!(
                    "failed to record news source failure {}: {error}",
                    update.source
                ),
            )
        })?;

        Ok(())
    }
}
