fn parse_news_source_health_row(row: &sqlx::postgres::PgRow) -> Result<NewsSourceHealthView> {
    let reliability: Decimal = decode_column(row, "reliability")?;
    let health_score: Decimal = decode_column(row, "health_score")?;
    let consecutive_failures: i64 = decode_column(row, "consecutive_failures")?;
    let items_fetched: i64 = decode_column(row, "items_fetched")?;
    let items_inserted: i64 = decode_column(row, "items_inserted")?;
    let items_deduped: i64 = decode_column(row, "items_deduped")?;

    Ok(NewsSourceHealthView {
        source: decode_column(row, "source")?,
        source_type: decode_column(row, "source_type")?,
        enabled: decode_column(row, "enabled")?,
        reliability: Probability::new(reliability).map_err(|error| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("failed to decode news source reliability: {error}"),
            )
        })?,
        last_success_at: decode_column(row, "last_success_at")?,
        last_error_at: decode_column(row, "last_error_at")?,
        consecutive_failures: i64_to_u64("consecutive_failures", consecutive_failures)?,
        items_fetched: i64_to_u64("items_fetched", items_fetched)?,
        items_inserted: i64_to_u64("items_inserted", items_inserted)?,
        items_deduped: i64_to_u64("items_deduped", items_deduped)?,
        health_score: Probability::new(health_score).map_err(|error| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("failed to decode news source health_score: {error}"),
            )
        })?,
        last_error: decode_column(row, "last_error")?,
        updated_at: decode_column(row, "updated_at")?,
    })
}

fn parse_news_raw_event_row(row: &sqlx::postgres::PgRow) -> Result<NewsRawEventView> {
    let raw_payload: Json<Value> = decode_column(row, "raw_payload")?;

    Ok(NewsRawEventView {
        id: decode_column(row, "id")?,
        source: decode_column(row, "source")?,
        source_type: decode_column(row, "source_type")?,
        external_id: decode_column(row, "external_id")?,
        title: decode_column(row, "title")?,
        url: decode_column(row, "url")?,
        author: decode_column(row, "author")?,
        published_at: decode_column(row, "published_at")?,
        event_time: decode_column(row, "event_time")?,
        hash: decode_column(row, "hash")?,
        raw_payload: raw_payload.0,
        ingested_at: decode_column(row, "ingested_at")?,
        trace_id: decode_column(row, "trace_id")?,
    })
}
