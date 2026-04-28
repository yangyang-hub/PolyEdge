use async_trait::async_trait;
use polyedge_domain::{AppError, Result};
use reqwest::Client;
use serde_json::{Value, json};
use std::time::Duration;
use time::{
    OffsetDateTime,
    format_description::well_known::{Rfc2822, Rfc3339},
};

const DEFAULT_USER_AGENT: &str = "polyedge-news-ingestor/0.1";

#[derive(Debug, Clone)]
pub struct RssNewsSourceConfig {
    pub id: String,
    pub source_type: String,
    pub url: String,
}

#[derive(Debug, Clone)]
pub struct ConnectorNewsItem {
    pub source: String,
    pub source_type: String,
    pub external_id: Option<String>,
    pub title: String,
    pub url: Option<String>,
    pub author: Option<String>,
    pub published_at: Option<OffsetDateTime>,
    pub content_snippet: Option<String>,
    pub raw_payload: Value,
}

#[async_trait]
pub trait NewsSource: Send + Sync {
    async fn fetch(&self) -> Result<Vec<ConnectorNewsItem>>;
}

pub struct RssNewsConnector {
    config: RssNewsSourceConfig,
    client: Client,
}

impl RssNewsConnector {
    pub fn new(config: RssNewsSourceConfig, timeout: Duration) -> Result<Self> {
        let source = config.id.trim();
        if source.is_empty() {
            return Err(AppError::invalid_input(
                "NEWS_CONNECTOR_SOURCE_REQUIRED",
                "news source id must not be empty",
            ));
        }

        if config.url.trim().is_empty() {
            return Err(AppError::invalid_input(
                "NEWS_CONNECTOR_URL_REQUIRED",
                "news source url must not be empty",
            ));
        }

        let client = Client::builder()
            .timeout(timeout)
            .user_agent(DEFAULT_USER_AGENT)
            .build()
            .map_err(|error| {
                AppError::internal(
                    "NEWS_HTTP_CLIENT_BUILD_FAILED",
                    format!("failed to build news HTTP client: {error}"),
                )
            })?;

        Ok(Self { config, client })
    }

    fn parse_feed(&self, bytes: &[u8]) -> Result<Vec<ConnectorNewsItem>> {
        parse_feed_bytes(&self.config, bytes)
    }
}

#[async_trait]
impl NewsSource for RssNewsConnector {
    async fn fetch(&self) -> Result<Vec<ConnectorNewsItem>> {
        let response = self
            .client
            .get(self.config.url.trim())
            .send()
            .await
            .map_err(|error| {
                AppError::dependency_unavailable(
                    "NEWS_SOURCE_FETCH_FAILED",
                    format!("failed to fetch news source {}: {error}", self.config.id),
                )
            })?;

        let status = response.status();
        if !status.is_success() {
            return Err(AppError::dependency_unavailable(
                "NEWS_SOURCE_HTTP_STATUS_FAILED",
                format!(
                    "news source {} returned HTTP status {status}",
                    self.config.id
                ),
            ));
        }

        let bytes = response.bytes().await.map_err(|error| {
            AppError::dependency_unavailable(
                "NEWS_SOURCE_READ_FAILED",
                format!(
                    "failed to read news source {} body: {error}",
                    self.config.id
                ),
            )
        })?;

        self.parse_feed(&bytes)
    }
}

fn parse_feed_bytes(config: &RssNewsSourceConfig, bytes: &[u8]) -> Result<Vec<ConnectorNewsItem>> {
    let feed = std::str::from_utf8(bytes).map_err(|error| {
        AppError::invalid_input(
            "NEWS_FEED_PARSE_FAILED",
            format!("news feed {} is not valid UTF-8: {error}", config.id),
        )
    })?;

    let blocks = item_blocks(feed, "item");
    let entry_blocks = if blocks.is_empty() {
        item_blocks(feed, "entry")
    } else {
        blocks
    };

    Ok(entry_blocks
        .into_iter()
        .filter_map(|entry| parse_item_block(config, entry))
        .collect())
}

fn parse_item_block(config: &RssNewsSourceConfig, entry: &str) -> Option<ConnectorNewsItem> {
    let title = tag_text(entry, "title")?;
    let external_id = tag_text(entry, "guid")
        .or_else(|| tag_text(entry, "id"))
        .or_else(|| atom_link_href(entry));
    let url = tag_text(entry, "link").or_else(|| atom_link_href(entry));
    let author = tag_text(entry, "author")
        .or_else(|| tag_text(entry, "dc:creator"))
        .or_else(|| tag_text(entry, "name"));
    let published_raw = tag_text(entry, "pubDate")
        .or_else(|| tag_text(entry, "published"))
        .or_else(|| tag_text(entry, "updated"));
    let published_at = published_raw.as_deref().and_then(parse_feed_timestamp);
    let content_snippet = tag_text(entry, "description")
        .or_else(|| tag_text(entry, "summary"))
        .or_else(|| tag_text(entry, "content"))
        .map(|value| strip_xml_tags(&value))
        .and_then(|value| normalize_optional(&value));

    let raw_payload = json!({
        "entry_id": external_id.clone(),
        "title": title.clone(),
        "url": url.clone(),
        "author": author.clone(),
        "published_at": published_at.and_then(|timestamp| timestamp.format(&Rfc3339).ok()),
        "summary": content_snippet.clone(),
        "feed_url": config.url.clone(),
    });

    Some(ConnectorNewsItem {
        source: config.id.trim().to_string(),
        source_type: config.source_type.trim().to_string(),
        external_id,
        title,
        url,
        author,
        published_at,
        content_snippet,
        raw_payload,
    })
}

fn item_blocks<'a>(input: &'a str, tag: &str) -> Vec<&'a str> {
    let mut blocks = Vec::new();
    let mut cursor = 0usize;
    let open_pattern = format!("<{tag}");
    let close_pattern = format!("</{tag}>");

    while let Some(open_relative) = input[cursor..].find(&open_pattern) {
        let open = cursor + open_relative;
        let Some(open_end_relative) = input[open..].find('>') else {
            break;
        };
        let content_start = open + open_end_relative + 1;
        let Some(close_relative) = input[content_start..].find(&close_pattern) else {
            break;
        };
        let close = content_start + close_relative;
        blocks.push(&input[content_start..close]);
        cursor = close + close_pattern.len();
    }

    blocks
}

fn tag_text(input: &str, tag: &str) -> Option<String> {
    let open_pattern = format!("<{tag}");
    let close_pattern = format!("</{tag}>");
    let open = input.find(&open_pattern)?;
    let open_end = input[open..].find('>')? + open + 1;
    let close = input[open_end..].find(&close_pattern)? + open_end;
    normalize_optional(&decode_xml_text(&input[open_end..close]))
}

fn atom_link_href(input: &str) -> Option<String> {
    let mut cursor = 0usize;
    while let Some(open_relative) = input[cursor..].find("<link") {
        let open = cursor + open_relative;
        let Some(end_relative) = input[open..].find('>') else {
            break;
        };
        let end = open + end_relative;
        let tag = &input[open..=end];
        let rel = attribute_value(tag, "rel");
        if rel.as_deref().is_none_or(|value| value == "alternate")
            && let Some(href) = attribute_value(tag, "href")
        {
            return Some(href);
        }
        cursor = end + 1;
    }

    None
}

fn attribute_value(input: &str, name: &str) -> Option<String> {
    let pattern = format!("{name}=");
    let start = input.find(&pattern)? + pattern.len();
    let quote = input[start..].chars().next()?;
    if quote != '"' && quote != '\'' {
        return None;
    }
    let value_start = start + quote.len_utf8();
    let value_end = input[value_start..].find(quote)? + value_start;
    normalize_optional(&decode_xml_text(&input[value_start..value_end]))
}

fn parse_feed_timestamp(value: &str) -> Option<OffsetDateTime> {
    OffsetDateTime::parse(value, &Rfc3339)
        .or_else(|_| OffsetDateTime::parse(value, &Rfc2822))
        .ok()
}

fn normalize_optional(value: &str) -> Option<String> {
    let normalized = value.trim();
    if normalized.is_empty() {
        None
    } else {
        Some(normalized.to_string())
    }
}

fn decode_xml_text(value: &str) -> String {
    let without_cdata = value
        .trim()
        .strip_prefix("<![CDATA[")
        .and_then(|inner| inner.strip_suffix("]]>"))
        .unwrap_or_else(|| value.trim());

    without_cdata
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
}

fn strip_xml_tags(value: &str) -> String {
    let mut stripped = String::with_capacity(value.len());
    let mut in_tag = false;
    for character in value.chars() {
        match character {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => stripped.push(character),
            _ => {}
        }
    }
    stripped
}

#[cfg(test)]
mod tests {
    use super::{RssNewsSourceConfig, parse_feed_bytes};

    #[test]
    fn parses_rss_feed_items() {
        let bytes = br#"
            <rss version="2.0">
              <channel>
                <title>SEC Updates</title>
                <item>
                  <guid>sec-1</guid>
                  <title>SEC updates ETF calendar</title>
                  <link>https://example.com/sec-1</link>
                  <pubDate>Mon, 20 Apr 2026 10:00:00 GMT</pubDate>
                  <description>Review window narrowed.</description>
                </item>
              </channel>
            </rss>
        "#;

        let items = parse_feed_bytes(
            &RssNewsSourceConfig {
                id: "sec_feed".to_string(),
                source_type: "official".to_string(),
                url: "https://example.com/rss".to_string(),
            },
            bytes,
        )
        .expect("parsed feed");

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].source, "sec_feed");
        assert_eq!(items[0].source_type, "official");
        assert_eq!(items[0].external_id.as_deref(), Some("sec-1"));
        assert_eq!(items[0].title, "SEC updates ETF calendar");
    }

    #[test]
    fn parses_atom_feed_entries() {
        let bytes = br#"
            <feed xmlns="http://www.w3.org/2005/Atom">
              <title>Macro Calendar</title>
              <entry>
                <id>tag:example.com,2026:macro-1</id>
                <title>FOMC calendar updated</title>
                <link href="https://example.com/macro-1" rel="alternate" />
                <updated>2026-04-20T10:00:00Z</updated>
                <summary>Review window changed.</summary>
              </entry>
            </feed>
        "#;

        let items = parse_feed_bytes(
            &RssNewsSourceConfig {
                id: "fomc_calendar".to_string(),
                source_type: "calendar".to_string(),
                url: "https://example.com/atom".to_string(),
            },
            bytes,
        )
        .expect("parsed feed");

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].source, "fomc_calendar");
        assert_eq!(items[0].source_type, "calendar");
        assert_eq!(
            items[0].external_id.as_deref(),
            Some("tag:example.com,2026:macro-1")
        );
        assert_eq!(items[0].url.as_deref(), Some("https://example.com/macro-1"));
    }
}
