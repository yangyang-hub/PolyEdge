async fn postgres_upsert_reward_market_event_windows(
    pool: &PgPool,
    windows: &[RewardMarketEventWindow],
) -> Result<()> {
    if windows.is_empty() {
        return Ok(());
    }

    let mut transaction = pool.begin().await.map_err(|error| {
        db_error(
            "POSTGRES_TRANSACTION_BEGIN_FAILED",
            format!("failed to begin reward event-window transaction: {error}"),
        )
    })?;

    for window in windows {
        sqlx::query(
            r#"
            INSERT INTO reward_market_event_windows (
                condition_id,
                source,
                event_type,
                event_start_at,
                event_end_at,
                confidence,
                source_url,
                source_payload,
                notes,
                active,
                reviewed_by,
                reviewed_at,
                updated_at
            )
            SELECT $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13
            WHERE EXISTS (
                SELECT 1
                FROM reward_markets
                WHERE condition_id = $1
            )
            ON CONFLICT (condition_id, source) DO UPDATE
            SET event_type = EXCLUDED.event_type,
                event_start_at = EXCLUDED.event_start_at,
                event_end_at = EXCLUDED.event_end_at,
                confidence = EXCLUDED.confidence,
                source_url = EXCLUDED.source_url,
                source_payload = EXCLUDED.source_payload,
                notes = EXCLUDED.notes,
                active = EXCLUDED.active,
                reviewed_by = EXCLUDED.reviewed_by,
                reviewed_at = EXCLUDED.reviewed_at,
                updated_at = EXCLUDED.updated_at
            WHERE EXCLUDED.updated_at >= reward_market_event_windows.updated_at
            "#,
        )
        .bind(&window.condition_id)
        .bind(&window.source)
        .bind(&window.event_type)
        .bind(window.event_start_at)
        .bind(window.event_end_at)
        .bind(window.confidence.as_str())
        .bind(&window.source_url)
        .bind(Json(window.source_payload.clone()))
        .bind(&window.notes)
        .bind(window.active)
        .bind(&window.reviewed_by)
        .bind(window.reviewed_at)
        .bind(window.updated_at)
        .execute(&mut *transaction)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_UPSERT_FAILED",
                format!("failed to upsert reward market event window: {error}"),
            )
        })?;
    }

    transaction.commit().await.map_err(|error| {
        db_error(
            "POSTGRES_TRANSACTION_COMMIT_FAILED",
            format!("failed to commit reward event-window transaction: {error}"),
        )
    })?;
    Ok(())
}

async fn postgres_list_effective_reward_market_event_windows(
    pool: &PgPool,
    condition_ids: &[String],
) -> Result<Vec<RewardMarketEventWindow>> {
    if condition_ids.is_empty() {
        return Ok(Vec::new());
    }

    let rows = sqlx::query(
        r#"
        SELECT DISTINCT ON (condition_id)
               condition_id,
               source,
               event_type,
               event_start_at,
               event_end_at,
               confidence,
               source_url,
               source_payload,
               notes,
               active,
               reviewed_by,
               reviewed_at,
               updated_at
        FROM reward_market_event_windows
        WHERE active = true
          AND condition_id = ANY($1)
        ORDER BY condition_id ASC,
                 CASE confidence
                     WHEN 'high' THEN 3
                     WHEN 'medium' THEN 2
                     ELSE 1
                 END DESC,
                 CASE source
                     WHEN 'manual' THEN 6
                     WHEN 'official' THEN 5
                     WHEN 'sports_api' THEN 5
                     WHEN 'economic_calendar' THEN 5
                     WHEN 'earnings_calendar' THEN 5
                     WHEN 'governance_calendar' THEN 5
                     WHEN 'gamma_reviewed' THEN 4
                     WHEN 'gamma' THEN 3
                     WHEN 'news' THEN 2
                     WHEN 'rss' THEN 2
                     WHEN 'ai_extracted' THEN 1
                     ELSE 0
                 END DESC,
                 updated_at DESC,
                 source ASC
        "#,
    )
    .bind(condition_ids)
    .fetch_all(pool)
    .await
    .map_err(|error| {
        db_error(
            "POSTGRES_QUERY_FAILED",
            format!("failed to query reward market event windows: {error}"),
        )
    })?;

    rows.into_iter()
        .map(reward_market_event_window_from_row)
        .collect()
}

fn reward_market_event_window_from_row(
    row: sqlx::postgres::PgRow,
) -> Result<RewardMarketEventWindow> {
    let confidence: String = row.try_get("confidence").map_err(postgres_decode_error)?;
    let source_payload: Json<Value> = row.try_get("source_payload").map_err(postgres_decode_error)?;

    Ok(RewardMarketEventWindow {
        condition_id: row.try_get("condition_id").map_err(postgres_decode_error)?,
        source: row.try_get("source").map_err(postgres_decode_error)?,
        event_type: row.try_get("event_type").map_err(postgres_decode_error)?,
        event_start_at: row.try_get("event_start_at").map_err(postgres_decode_error)?,
        event_end_at: row.try_get("event_end_at").map_err(postgres_decode_error)?,
        confidence: RewardEventTimeConfidence::from_str(&confidence)?,
        source_url: row.try_get("source_url").map_err(postgres_decode_error)?,
        source_payload: source_payload.0,
        notes: row.try_get("notes").map_err(postgres_decode_error)?,
        active: row.try_get("active").map_err(postgres_decode_error)?,
        reviewed_by: row.try_get("reviewed_by").map_err(postgres_decode_error)?,
        reviewed_at: row.try_get("reviewed_at").map_err(postgres_decode_error)?,
        updated_at: row.try_get("updated_at").map_err(postgres_decode_error)?,
    })
}
