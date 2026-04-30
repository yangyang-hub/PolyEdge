use async_trait::async_trait;
use polyedge_domain::{AppError, Probability, Result};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::sync::Arc;
use time::OffsetDateTime;

const NEWS_SOURCE_TYPES: &[&str] = &["news", "social", "official", "calendar", "market"];
const DEFAULT_LIST_LIMIT: u16 = 100;
const MAX_LIST_LIMIT: u16 = 200;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewsIngestionItem {
    pub source: String,
    pub source_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub external_id: Option<String>,
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(with = "time::serde::rfc3339::option")]
    pub published_at: Option<OffsetDateTime>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_snippet: Option<String>,
    pub raw_payload: Value,
}

#[derive(Debug, Clone)]
pub struct NewsIngestSourceCommand {
    pub source: String,
    pub source_type: String,
    pub reliability: Probability,
    pub items: Vec<NewsIngestionItem>,
    pub trace_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewsSourceIngestionReport {
    pub source: String,
    pub fetched: usize,
    pub inserted: usize,
    pub deduped: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewsRawEventInsert {
    pub id: String,
    pub source: String,
    pub source_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub external_id: Option<String>,
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(with = "time::serde::rfc3339::option")]
    pub published_at: Option<OffsetDateTime>,
    #[serde(with = "time::serde::rfc3339")]
    pub event_time: OffsetDateTime,
    pub hash: String,
    pub raw_payload: Value,
    #[serde(with = "time::serde::rfc3339")]
    pub ingested_at: OffsetDateTime,
    pub trace_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewsRawEventView {
    pub id: String,
    pub source: String,
    pub source_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub external_id: Option<String>,
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(with = "time::serde::rfc3339::option")]
    pub published_at: Option<OffsetDateTime>,
    #[serde(with = "time::serde::rfc3339")]
    pub event_time: OffsetDateTime,
    pub hash: String,
    pub raw_payload: Value,
    #[serde(with = "time::serde::rfc3339")]
    pub ingested_at: OffsetDateTime,
    pub trace_id: String,
}

#[derive(Debug, Clone)]
pub struct NewsSourceSuccessUpdate {
    pub source: String,
    pub source_type: String,
    pub reliability: Probability,
    pub fetched: usize,
    pub inserted: usize,
    pub deduped: usize,
    pub observed_at: OffsetDateTime,
    pub trace_id: String,
}

#[derive(Debug, Clone)]
pub struct NewsSourceFailureUpdate {
    pub source: String,
    pub source_type: String,
    pub reliability: Probability,
    pub error_message: String,
    pub observed_at: OffsetDateTime,
    pub trace_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewsSourceHealthView {
    pub source: String,
    pub source_type: String,
    pub enabled: bool,
    pub reliability: Probability,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(with = "time::serde::rfc3339::option")]
    pub last_success_at: Option<OffsetDateTime>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(with = "time::serde::rfc3339::option")]
    pub last_error_at: Option<OffsetDateTime>,
    pub consecutive_failures: u64,
    pub items_fetched: u64,
    pub items_inserted: u64,
    pub items_deduped: u64,
    pub health_score: Probability,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone)]
pub struct NewsSourceHealthListFilters {
    pub source_type: Option<String>,
    pub limit: u16,
}

impl NewsSourceHealthListFilters {
    pub fn new(source_type: Option<String>, limit: Option<u16>) -> Result<Self> {
        Ok(Self {
            source_type: source_type
                .map(|value| normalize_source_type(&value))
                .transpose()?,
            limit: validate_limit(limit)?,
        })
    }
}

#[derive(Debug, Clone)]
pub struct NewsRawEventListFilters {
    pub source: Option<String>,
    pub source_type: Option<String>,
    pub limit: u16,
}

impl NewsRawEventListFilters {
    pub fn new(
        source: Option<String>,
        source_type: Option<String>,
        limit: Option<u16>,
    ) -> Result<Self> {
        let source = normalize_optional_string(source.as_deref());
        let source_type = normalize_optional_string(source_type.as_deref())
            .map(|value| normalize_source_type(&value))
            .transpose()?;

        Ok(Self {
            source,
            source_type,
            limit: validate_limit(limit)?,
        })
    }
}

#[async_trait]
pub trait NewsIngestionStore: Send + Sync {
    async fn list_news_source_health(
        &self,
        filters: &NewsSourceHealthListFilters,
    ) -> Result<Vec<NewsSourceHealthView>>;

    async fn list_raw_news_events(
        &self,
        filters: &NewsRawEventListFilters,
    ) -> Result<Vec<NewsRawEventView>>;

    async fn insert_raw_news_event(&self, event: &NewsRawEventInsert) -> Result<bool>;

    async fn record_news_source_success(&self, update: &NewsSourceSuccessUpdate) -> Result<()>;

    async fn record_news_source_failure(&self, update: &NewsSourceFailureUpdate) -> Result<()>;
}

pub struct NewsIngestionService {
    store: Arc<dyn NewsIngestionStore>,
}

impl NewsIngestionService {
    #[must_use]
    pub fn new(store: Arc<dyn NewsIngestionStore>) -> Self {
        Self { store }
    }

    pub async fn ingest_source_items(
        &self,
        command: NewsIngestSourceCommand,
    ) -> Result<NewsSourceIngestionReport> {
        let source = normalize_required_string(&command.source, "NEWS_SOURCE_REQUIRED")?;
        let source_type = normalize_source_type(&command.source_type)?;
        let trace_id = normalize_required_string(&command.trace_id, "NEWS_TRACE_ID_REQUIRED")?;
        let ingested_at = OffsetDateTime::now_utc();
        let mut inserted = 0usize;

        for item in &command.items {
            let prepared = prepare_raw_event(item, &source, &source_type, &trace_id, ingested_at)?;
            if self.store.insert_raw_news_event(&prepared).await? {
                inserted += 1;
            }
        }

        let fetched = command.items.len();
        let deduped = fetched.saturating_sub(inserted);
        self.store
            .record_news_source_success(&NewsSourceSuccessUpdate {
                source: source.clone(),
                source_type,
                reliability: command.reliability,
                fetched,
                inserted,
                deduped,
                observed_at: ingested_at,
                trace_id,
            })
            .await?;

        Ok(NewsSourceIngestionReport {
            source,
            fetched,
            inserted,
            deduped,
        })
    }

    pub async fn record_source_failure(&self, update: NewsSourceFailureUpdate) -> Result<()> {
        let source = normalize_required_string(&update.source, "NEWS_SOURCE_REQUIRED")?;
        let source_type = normalize_source_type(&update.source_type)?;
        let error_message =
            normalize_required_string(&update.error_message, "NEWS_SOURCE_ERROR_REQUIRED")?;
        let trace_id = normalize_required_string(&update.trace_id, "NEWS_TRACE_ID_REQUIRED")?;

        self.store
            .record_news_source_failure(&NewsSourceFailureUpdate {
                source,
                source_type,
                reliability: update.reliability,
                error_message,
                observed_at: update.observed_at,
                trace_id,
            })
            .await
    }

    pub async fn list_source_health(
        &self,
        filters: NewsSourceHealthListFilters,
    ) -> Result<Vec<NewsSourceHealthView>> {
        self.store.list_news_source_health(&filters).await
    }

    pub async fn list_raw_events(
        &self,
        filters: NewsRawEventListFilters,
    ) -> Result<Vec<NewsRawEventView>> {
        self.store.list_raw_news_events(&filters).await
    }
}

fn prepare_raw_event(
    item: &NewsIngestionItem,
    expected_source: &str,
    expected_source_type: &str,
    trace_id: &str,
    ingested_at: OffsetDateTime,
) -> Result<NewsRawEventInsert> {
    let source = normalize_required_string(&item.source, "NEWS_ITEM_SOURCE_REQUIRED")?;
    if source != expected_source {
        return Err(AppError::invalid_input(
            "NEWS_ITEM_SOURCE_MISMATCH",
            format!("news item source {source} does not match configured source {expected_source}"),
        ));
    }

    let source_type = normalize_source_type(&item.source_type)?;
    if source_type != expected_source_type {
        return Err(AppError::invalid_input(
            "NEWS_ITEM_SOURCE_TYPE_MISMATCH",
            format!(
                "news item source_type {source_type} does not match configured source_type {expected_source_type}"
            ),
        ));
    }

    if !item.raw_payload.is_object() {
        return Err(AppError::invalid_input(
            "NEWS_RAW_PAYLOAD_INVALID",
            "news raw_payload must be a JSON object",
        ));
    }

    let title = normalize_required_string(&item.title, "NEWS_ITEM_TITLE_REQUIRED")?;
    let external_id = normalize_optional_string(item.external_id.as_deref());
    let url = normalize_optional_string(item.url.as_deref()).map(canonicalize_url);
    let author = normalize_optional_string(item.author.as_deref());
    let content_snippet = normalize_optional_string(item.content_snippet.as_deref());
    let event_time = item.published_at.unwrap_or(ingested_at);
    let hash = raw_news_hash(
        &source,
        external_id.as_deref(),
        &title,
        url.as_deref(),
        item.published_at,
        content_snippet.as_deref(),
    );
    let id = format!("raw_news_{}_{}", id_fragment(&source), &hash[..24]);

    Ok(NewsRawEventInsert {
        id,
        source,
        source_type,
        external_id,
        title,
        url,
        author,
        published_at: item.published_at,
        event_time,
        hash,
        raw_payload: item.raw_payload.clone(),
        ingested_at,
        trace_id: trace_id.to_string(),
    })
}

fn raw_news_hash(
    source: &str,
    external_id: Option<&str>,
    title: &str,
    url: Option<&str>,
    published_at: Option<OffsetDateTime>,
    content_snippet: Option<&str>,
) -> String {
    let mut hasher = Sha256::new();
    push_hash_part(&mut hasher, source);
    push_hash_part(&mut hasher, external_id.unwrap_or(""));
    push_hash_part(&mut hasher, &normalize_for_hash(title));
    push_hash_part(&mut hasher, url.unwrap_or(""));
    push_hash_part(
        &mut hasher,
        &published_at
            .map(|timestamp| timestamp.unix_timestamp().to_string())
            .unwrap_or_default(),
    );
    push_hash_part(
        &mut hasher,
        &content_snippet.map(normalize_for_hash).unwrap_or_default(),
    );

    let digest = hasher.finalize();
    let mut hex = String::with_capacity(digest.len() * 2);
    for byte in digest {
        hex.push_str(&format!("{byte:02x}"));
    }
    hex
}

fn push_hash_part(hasher: &mut Sha256, value: &str) {
    hasher.update(value.as_bytes());
    hasher.update([0]);
}

fn normalize_required_string(value: &str, code: &'static str) -> Result<String> {
    let normalized = value.trim();
    if normalized.is_empty() {
        return Err(AppError::invalid_input(code, "value must not be empty"));
    }
    Ok(normalized.to_string())
}

fn normalize_optional_string(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(std::borrow::ToOwned::to_owned)
}

fn normalize_source_type(value: &str) -> Result<String> {
    let source_type = normalize_required_string(value, "NEWS_SOURCE_TYPE_REQUIRED")?;
    if NEWS_SOURCE_TYPES.contains(&source_type.as_str()) {
        Ok(source_type)
    } else {
        Err(AppError::invalid_input(
            "NEWS_SOURCE_TYPE_INVALID",
            format!("unknown news source_type: {source_type}"),
        ))
    }
}

fn validate_limit(limit: Option<u16>) -> Result<u16> {
    let limit = limit.unwrap_or(DEFAULT_LIST_LIMIT);
    if limit == 0 {
        return Err(AppError::invalid_input(
            "NEWS_LIST_LIMIT_INVALID",
            "news list limit must be greater than zero",
        ));
    }

    if limit > MAX_LIST_LIMIT {
        return Err(AppError::invalid_input(
            "NEWS_LIST_LIMIT_INVALID",
            format!("news list limit must be at most {MAX_LIST_LIMIT}"),
        ));
    }

    Ok(limit)
}

pub fn degraded_health_score(
    reliability: Probability,
    consecutive_failures: u64,
) -> Result<Probability> {
    let penalty = Decimal::from(consecutive_failures.min(5)) / Decimal::from(5);
    let score = reliability.value() - penalty;
    Probability::new(score.max(Decimal::ZERO))
}

fn normalize_for_hash(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

fn canonicalize_url(value: String) -> String {
    value.trim_end_matches('/').to_string()
}

fn id_fragment(source: &str) -> String {
    let fragment: String = source
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect();
    let trimmed = fragment.trim_matches('_');
    if trimmed.is_empty() {
        "source".to_string()
    } else {
        trimmed.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::{
        NewsIngestSourceCommand, NewsIngestionItem, NewsIngestionService, NewsIngestionStore,
        NewsRawEventInsert, NewsRawEventListFilters, NewsRawEventView, NewsSourceFailureUpdate,
        NewsSourceHealthListFilters, NewsSourceHealthView, NewsSourceSuccessUpdate,
        normalize_source_type,
    };
    use async_trait::async_trait;
    use polyedge_domain::{Probability, Result};
    use rust_decimal::Decimal;
    use serde_json::json;
    use std::{
        collections::HashSet,
        str::FromStr,
        sync::{Arc, Mutex},
    };

    #[derive(Default)]
    struct TestNewsStore {
        ids: Mutex<HashSet<String>>,
    }

    #[async_trait]
    impl NewsIngestionStore for TestNewsStore {
        async fn list_news_source_health(
            &self,
            _filters: &NewsSourceHealthListFilters,
        ) -> Result<Vec<NewsSourceHealthView>> {
            Ok(Vec::new())
        }

        async fn list_raw_news_events(
            &self,
            _filters: &NewsRawEventListFilters,
        ) -> Result<Vec<NewsRawEventView>> {
            Ok(Vec::new())
        }

        async fn insert_raw_news_event(&self, event: &NewsRawEventInsert) -> Result<bool> {
            let mut ids = self.ids.lock().expect("test store lock");
            Ok(ids.insert(event.id.clone()))
        }

        async fn record_news_source_success(
            &self,
            _update: &NewsSourceSuccessUpdate,
        ) -> Result<()> {
            Ok(())
        }

        async fn record_news_source_failure(
            &self,
            _update: &NewsSourceFailureUpdate,
        ) -> Result<()> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn ingest_source_items_deduplicates_identical_news() {
        let service = NewsIngestionService::new(Arc::new(TestNewsStore::default()));
        let reliability =
            Probability::new(Decimal::from_str("0.95").expect("valid decimal")).expect("prob");
        let item = NewsIngestionItem {
            source: "sec_feed".to_string(),
            source_type: "official".to_string(),
            external_id: Some("entry-1".to_string()),
            title: "SEC updates ETF calendar".to_string(),
            url: Some("https://example.com/item/".to_string()),
            author: None,
            published_at: None,
            content_snippet: Some("Calendar window narrowed".to_string()),
            raw_payload: json!({"id": "entry-1"}),
        };

        let report = service
            .ingest_source_items(NewsIngestSourceCommand {
                source: "sec_feed".to_string(),
                source_type: "official".to_string(),
                reliability,
                items: vec![item.clone(), item],
                trace_id: "trc_test".to_string(),
            })
            .await
            .expect("ingestion report");

        assert_eq!(report.fetched, 2);
        assert_eq!(report.inserted, 1);
        assert_eq!(report.deduped, 1);
    }

    #[test]
    fn normalize_source_type_rejects_unknown_values() {
        let error = normalize_source_type("blog").expect_err("invalid source type");
        assert_eq!(error.code(), "NEWS_SOURCE_TYPE_INVALID");
    }
}
