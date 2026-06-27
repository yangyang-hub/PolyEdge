use super::{InMemoryMarketEventStore, PostgresMarketEventStore};
use polyedge_application::{
    MarketEventStore, NewsIngestionStore, NewsSourceFailureUpdate, NewsSourceHealthListFilters,
    NewsSourceSuccessUpdate, PageQuery, RecomputeSignalCommand, demo_fixture_bundle,
};
use polyedge_domain::{Probability, Result};
use rust_decimal::Decimal;
use sqlx::{Executor, postgres::PgPoolOptions};
use std::error::Error;
use time::{Duration, OffsetDateTime};
use uuid::Uuid;

static TEST_MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("../../migrations");

fn quote_pg_ident(value: &str) -> String {
    format!(r#""{}""#, value.replace('"', r#""""#))
}

#[tokio::test]
async fn in_memory_news_source_health_tracks_counts_failures_and_filters() -> Result<()> {
    let store = InMemoryMarketEventStore::new();
    let reliability = Probability::new(Decimal::new(90, 2))?;
    let first_seen = OffsetDateTime::UNIX_EPOCH + Duration::seconds(1);
    let failed_at = OffsetDateTime::UNIX_EPOCH + Duration::seconds(2);
    let official_seen = OffsetDateTime::UNIX_EPOCH + Duration::seconds(3);

    store
        .record_news_source_success(&NewsSourceSuccessUpdate {
            source: "rss_feed".to_string(),
            source_type: "news".to_string(),
            reliability,
            fetched: 3,
            inserted: 2,
            deduped: 1,
            observed_at: first_seen,
            trace_id: "trc_success".to_string(),
        })
        .await?;
    store
        .record_news_source_failure(&NewsSourceFailureUpdate {
            source: "rss_feed".to_string(),
            source_type: "news".to_string(),
            reliability,
            error_message: "upstream timeout".to_string(),
            observed_at: failed_at,
            trace_id: "trc_failure".to_string(),
        })
        .await?;
    store
        .record_news_source_success(&NewsSourceSuccessUpdate {
            source: "sec_feed".to_string(),
            source_type: "official".to_string(),
            reliability,
            fetched: 1,
            inserted: 1,
            deduped: 0,
            observed_at: official_seen,
            trace_id: "trc_official".to_string(),
        })
        .await?;

    let page = PageQuery::default();
    let all_sources = store
        .list_news_source_health(&NewsSourceHealthListFilters::new(None, Some(10))?, &page)
        .await?;
    assert_eq!(all_sources.data.len(), 2);
    assert_eq!(all_sources.data[0].source, "sec_feed");

    let news_sources = store
        .list_news_source_health(
            &NewsSourceHealthListFilters::new(Some(" news ".to_string()), Some(10))?,
            &page,
        )
        .await?;
    assert_eq!(news_sources.data.len(), 1);

    let rss_feed = &news_sources.data[0];
    assert_eq!(rss_feed.source, "rss_feed");
    assert_eq!(rss_feed.items_fetched, 3);
    assert_eq!(rss_feed.items_inserted, 2);
    assert_eq!(rss_feed.items_deduped, 1);
    assert_eq!(rss_feed.consecutive_failures, 1);
    assert_eq!(rss_feed.last_success_at, Some(first_seen));
    assert_eq!(rss_feed.last_error_at, Some(failed_at));
    assert_eq!(rss_feed.last_error.as_deref(), Some("upstream timeout"));
    assert_eq!(
        rss_feed.health_score,
        Probability::new(Decimal::new(70, 2))?
    );

    Ok(())
}

#[tokio::test]
async fn in_memory_recompute_discounts_degraded_event_source_health() -> Result<()> {
    let store = InMemoryMarketEventStore::new();
    let bundle = demo_fixture_bundle();
    store
        .ingest_fixture_bundle(&bundle, "trc_seed_recompute_health")
        .await?;
    store
        .record_news_source_failure(&NewsSourceFailureUpdate {
            source: "reuters".to_string(),
            source_type: "news".to_string(),
            reliability: Probability::new(Decimal::new(90, 2))?,
            error_message: "source timeout".to_string(),
            observed_at: OffsetDateTime::UNIX_EPOCH + Duration::seconds(10),
            trace_id: "trc_degrade_reuters".to_string(),
        })
        .await?;

    let result = store
        .recompute_signal(&RecomputeSignalCommand {
            signal_id: "sig_2412".to_string(),
            reason: "test source health adjustment".to_string(),
            trace_id: "trc_recompute".to_string(),
        })
        .await?;

    assert!(
        result
            .estimate
            .reason_codes
            .contains(&"source_health_degraded".to_string())
    );

    Ok(())
}

#[tokio::test]
async fn postgres_news_source_health_round_trips_filters_and_migrates_index()
-> std::result::Result<(), Box<dyn Error>> {
    let Some(database_url) = std::env::var("POLYEDGE_TEST_DATABASE_URL")
        .ok()
        .filter(|value| !value.trim().is_empty())
    else {
        return Ok(());
    };

    let schema = format!("polyedge_test_{}", Uuid::now_v7().simple());
    let quoted_schema = quote_pg_ident(&schema);
    let admin_pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(&database_url)
        .await?;
    admin_pool
        .execute(format!("CREATE SCHEMA {quoted_schema}").as_str())
        .await?;

    let test_result: std::result::Result<(), Box<dyn Error>> = async {
        let search_path_schema = quoted_schema.clone();
        let pool = PgPoolOptions::new()
            .max_connections(2)
            .after_connect(move |connection, _meta| {
                let search_path_schema = search_path_schema.clone();
                Box::pin(async move {
                    connection
                        .execute(format!("SET search_path TO {search_path_schema}").as_str())
                        .await?;
                    Ok(())
                })
            })
            .connect(&database_url)
            .await?;
        TEST_MIGRATOR.run(&pool).await?;

        let index_exists: bool = sqlx::query_scalar("SELECT to_regclass($1) IS NOT NULL")
            .bind(format!(
                "{schema}.news_source_health_source_type_updated_at_idx"
            ))
            .fetch_one(&admin_pool)
            .await?;
        assert!(index_exists);

        let store = PostgresMarketEventStore::new(pool.clone());
        let reliability = Probability::new(Decimal::new(90, 2))?;
        let news_seen = OffsetDateTime::UNIX_EPOCH + Duration::seconds(1);
        let news_failed = OffsetDateTime::UNIX_EPOCH + Duration::seconds(2);
        let official_seen = OffsetDateTime::UNIX_EPOCH + Duration::seconds(3);

        store
            .record_news_source_success(&NewsSourceSuccessUpdate {
                source: "wire_feed".to_string(),
                source_type: "news".to_string(),
                reliability,
                fetched: 4,
                inserted: 3,
                deduped: 1,
                observed_at: news_seen,
                trace_id: "trc_pg_success".to_string(),
            })
            .await?;
        store
            .record_news_source_failure(&NewsSourceFailureUpdate {
                source: "wire_feed".to_string(),
                source_type: "news".to_string(),
                reliability,
                error_message: "upstream timeout".to_string(),
                observed_at: news_failed,
                trace_id: "trc_pg_failure".to_string(),
            })
            .await?;
        store
            .record_news_source_success(&NewsSourceSuccessUpdate {
                source: "sec_feed".to_string(),
                source_type: "official".to_string(),
                reliability,
                fetched: 1,
                inserted: 1,
                deduped: 0,
                observed_at: official_seen,
                trace_id: "trc_pg_official".to_string(),
            })
            .await?;

        let page = PageQuery::default();
        let all_sources = store
            .list_news_source_health(&NewsSourceHealthListFilters::new(None, Some(10))?, &page)
            .await?;
        assert_eq!(all_sources.data.len(), 2);
        assert_eq!(all_sources.data[0].source, "sec_feed");

        let news_sources = store
            .list_news_source_health(
                &NewsSourceHealthListFilters::new(Some("news".to_string()), Some(10))?,
                &page,
            )
            .await?;
        assert_eq!(news_sources.data.len(), 1);

        let wire_feed = &news_sources.data[0];
        assert_eq!(wire_feed.source, "wire_feed");
        assert_eq!(wire_feed.items_fetched, 4);
        assert_eq!(wire_feed.items_inserted, 3);
        assert_eq!(wire_feed.items_deduped, 1);
        assert_eq!(wire_feed.consecutive_failures, 1);
        assert_eq!(wire_feed.last_error.as_deref(), Some("upstream timeout"));
        assert_eq!(
            wire_feed.health_score,
            Probability::new(Decimal::new(70, 2))?
        );

        pool.close().await;
        Ok(())
    }
    .await;

    admin_pool
        .execute(format!("DROP SCHEMA IF EXISTS {quoted_schema} CASCADE").as_str())
        .await?;
    admin_pool.close().await;

    test_result
}
