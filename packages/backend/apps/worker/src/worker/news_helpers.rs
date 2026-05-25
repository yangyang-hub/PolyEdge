async fn record_news_failure(
    state: &AppState,
    source: &NewsSourceSettings,
    error: &AppError,
    trace_id: &str,
) -> Result<()> {
    state
        .news_ingestion_service
        .record_source_failure(NewsSourceFailureUpdate {
            source: source.id.clone(),
            source_type: source.source_type.clone(),
            reliability: source.reliability,
            error_message: format!("{}: {}", error.code(), error.message()),
            observed_at: OffsetDateTime::now_utc(),
            trace_id: trace_id.to_string(),
        })
        .await
}

fn news_item_to_ingestion_item(item: ConnectorNewsItem) -> NewsIngestionItem {
    NewsIngestionItem {
        source: item.source,
        source_type: item.source_type,
        external_id: item.external_id,
        title: item.title,
        url: item.url,
        author: item.author,
        published_at: item.published_at,
        content_snippet: item.content_snippet,
        raw_payload: item.raw_payload,
    }
}
