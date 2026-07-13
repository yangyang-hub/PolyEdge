#[cfg(test)]
mod reward_event_window_store_tests {
    use super::*;
    use sqlx::{Executor, postgres::PgPoolOptions};
    use std::error::Error;

    static EVENT_WINDOW_TEST_MIGRATOR: sqlx::migrate::Migrator =
        sqlx::migrate!("../../migrations");
    static EVENT_WINDOW_POSTGRES_TEST_LOCK: tokio::sync::Mutex<()> =
        tokio::sync::Mutex::const_new(());

    fn quote_pg_ident(value: &str) -> String {
        format!(r#""{}""#, value.replace('"', r#""""#))
    }

    fn reward_market(condition_id: &str, now: OffsetDateTime) -> RewardMarket {
        RewardMarket {
            condition_id: condition_id.to_string(),
            question: format!("Question for {condition_id}"),
            market_slug: format!("market-{condition_id}"),
            event_slug: format!("event-{condition_id}"),
            category: "sports".to_string(),
            image: String::new(),
            rewards_max_spread: Decimal::from(4),
            rewards_min_size: Decimal::from(5),
            total_daily_rate: Decimal::from(20),
            liquidity_usd: Decimal::from(10_000),
            volume_24h_usd: Decimal::from(20_000),
            market_spread_cents: Decimal::ONE,
            end_at: Some(now + Duration::days(30)),
            ambiguity_level: "low".to_string(),
            market_synced_at: Some(now),
            tokens: vec![
                RewardToken {
                    token_id: format!("yes-{condition_id}"),
                    outcome: "YES".to_string(),
                    price: Some(Decimal::new(49, 2)),
                },
                RewardToken {
                    token_id: format!("no-{condition_id}"),
                    outcome: "NO".to_string(),
                    price: Some(Decimal::new(51, 2)),
                },
            ],
            active: true,
            updated_at: now,
        }
    }

    fn event_window(
        condition_id: &str,
        event_key: &str,
        observed_at: OffsetDateTime,
        producer_version: u32,
    ) -> RewardMarketEventWindow {
        RewardMarketEventWindow {
            condition_id: condition_id.to_string(),
            source: "gamma_event".to_string(),
            event_key: event_key.to_string(),
            event_type: "sports_start".to_string(),
            event_time_role: RewardEventTimeRole::EventOccurrence,
            schedule_status: RewardEventScheduleStatus::Scheduled,
            time_precision: RewardEventTimePrecision::Exact,
            start_source_field: Some("events[0].startDate".to_string()),
            end_policy: RewardEventEndPolicy::Explicit,
            event_start_at: Some(observed_at + Duration::days(1)),
            event_end_at: Some(observed_at + Duration::days(1) + Duration::hours(2)),
            confidence: RewardEventTimeConfidence::Medium,
            source_url: Some("https://gamma.example/market".to_string()),
            source_payload: json!({"event_key": event_key}),
            notes: String::new(),
            active: true,
            hard_gate_eligible: true,
            producer_version,
            source_updated_at: Some(observed_at),
            observed_at: Some(observed_at),
            expires_at: Some(observed_at + Duration::hours(12)),
            reviewed_by: None,
            reviewed_at: None,
            updated_at: observed_at,
        }
    }

    fn snapshot(
        producer_version: u32,
        observed_at: OffsetDateTime,
        covered_condition_ids: Vec<String>,
        windows: Vec<RewardMarketEventWindow>,
    ) -> RewardEventWindowSourceSnapshot {
        RewardEventWindowSourceSnapshot {
            source: "gamma_event".to_string(),
            producer_version,
            observed_at,
            coverage: covered_condition_ids
                .into_iter()
                .map(|condition_id| RewardEventWindowSourceCoverage {
                    condition_id,
                    source_updated_at: Some(observed_at),
                })
                .collect(),
            windows,
        }
    }

    async fn assert_replace_semantics<S>(store: &S) -> Result<()>
    where
        S: RewardBotStore + ?Sized,
    {
        let now = OffsetDateTime::from_unix_timestamp(1_750_000_000)
            .map_err(|error| AppError::internal("TEST_TIME_INVALID", error.to_string()))?;
        store
            .upsert_markets(&[
                reward_market("condition-a", now),
                reward_market("condition-b", now),
            ])
            .await?;

        let first = snapshot(
            2,
            now,
            vec!["condition-a".to_string()],
            vec![
                event_window("condition-a", "event-a", now, 2),
                event_window("condition-a", "event-b", now, 2),
            ],
        );
        let first_report = store.replace_market_event_windows(&first).await?;
        assert_eq!(first_report.upserted_window_count, 2);
        assert_eq!(first_report.deactivated_window_count, 0);

        let listed = store
            .list_market_event_windows(&["condition-a".to_string()], now)
            .await?;
        assert_eq!(listed.len(), 2);

        let replacement_at = now + Duration::minutes(5);
        let second = snapshot(
            2,
            replacement_at,
            vec!["condition-a".to_string()],
            vec![event_window(
                "condition-a",
                "event-b",
                replacement_at,
                2,
            )],
        );
        let second_report = store.replace_market_event_windows(&second).await?;
        assert_eq!(second_report.upserted_window_count, 1);
        assert_eq!(second_report.deactivated_window_count, 1);
        let listed = store
            .list_market_event_windows(&["condition-a".to_string()], replacement_at)
            .await?;
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].event_key, "event-b");

        let stale = snapshot(
            1,
            now + Duration::minutes(1),
            vec!["condition-a".to_string()],
            vec![event_window(
                "condition-a",
                "event-b",
                now + Duration::minutes(1),
                1,
            )],
        );
        let stale_report = store.replace_market_event_windows(&stale).await?;
        assert_eq!(stale_report.upserted_window_count, 0);
        assert_eq!(stale_report.ignored_stale_count, 1);
        assert_eq!(stale_report.ignored_stale_condition_count, 1);
        assert_eq!(stale_report.deactivated_window_count, 0);

        let generation_three_at = replacement_at + Duration::minutes(5);
        let generation_three = snapshot(
            3,
            generation_three_at,
            vec!["condition-a".to_string()],
            vec![event_window(
                "condition-a",
                "event-c",
                generation_three_at,
                3,
            )],
        );
        let generation_three_report = store
            .replace_market_event_windows(&generation_three)
            .await?;
        assert_eq!(generation_three_report.upserted_window_count, 1);
        assert_eq!(generation_three_report.deactivated_window_count, 1);

        let idempotent_report = store
            .replace_market_event_windows(&generation_three)
            .await?;
        assert_eq!(idempotent_report.idempotent_window_count, 1);
        assert_eq!(idempotent_report.upserted_window_count, 0);

        let mut conflicting_generation = generation_three.clone();
        conflicting_generation.windows[0].notes = "conflicting payload".to_string();
        let conflict = store
            .replace_market_event_windows(&conflicting_generation)
            .await
            .expect_err("same source fence with a different payload must fail closed");
        assert_eq!(conflict.code(), "REWARD_EVENT_WINDOW_SNAPSHOT_CONFLICT");

        let mut stale_source_empty = snapshot(
            3,
            generation_three_at + Duration::minutes(4),
            vec!["condition-a".to_string()],
            Vec::new(),
        );
        stale_source_empty.coverage[0].source_updated_at =
            Some(generation_three_at - Duration::minutes(1));
        let stale_source_report = store
            .replace_market_event_windows(&stale_source_empty)
            .await?;
        assert_eq!(stale_source_report.ignored_stale_condition_count, 1);
        assert_eq!(stale_source_report.deactivated_window_count, 0);

        let older_empty = snapshot(
            2,
            generation_three_at + Duration::minutes(5),
            vec!["condition-a".to_string()],
            Vec::new(),
        );
        let older_empty_report = store.replace_market_event_windows(&older_empty).await?;
        assert_eq!(older_empty_report.ignored_stale_condition_count, 1);
        assert_eq!(older_empty_report.deactivated_window_count, 0);
        let listed = store
            .list_market_event_windows(
                &["condition-a".to_string()],
                generation_three_at + Duration::minutes(5),
            )
            .await?;
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].event_key, "event-c");

        let missing_parent = snapshot(
            2,
            replacement_at,
            vec!["condition-missing".to_string()],
            vec![event_window(
                "condition-missing",
                "event-missing",
                replacement_at,
                2,
            )],
        );
        let missing_report = store
            .replace_market_event_windows(&missing_parent)
            .await?;
        assert_eq!(missing_report.skipped_missing_parent_count, 1);
        assert_eq!(missing_report.upserted_window_count, 0);

        let expired = store
            .list_market_event_windows(
                &["condition-a".to_string()],
                replacement_at + Duration::hours(13),
            )
            .await?;
        assert!(expired.is_empty());
        Ok(())
    }

    #[tokio::test]
    async fn in_memory_event_window_snapshots_replace_missing_keys_and_fence_stale_writes() {
        let store = InMemoryRewardBotStore::new();
        assert_replace_semantics(&store)
            .await
            .expect("in-memory event-window replacement");
    }

    #[tokio::test]
    async fn event_window_snapshot_rejects_invalid_hard_gate_shape() {
        let store = InMemoryRewardBotStore::new();
        let now = OffsetDateTime::from_unix_timestamp(1_750_000_000)
            .expect("fixed event-window test time");
        store
            .upsert_markets(&[reward_market("condition-a", now)])
            .await
            .expect("seed reward market");
        let mut invalid = event_window("condition-a", "event-a", now, 2);
        invalid.time_precision = RewardEventTimePrecision::Inferred;
        let error = store
            .replace_market_event_windows(&snapshot(
                2,
                now,
                vec!["condition-a".to_string()],
                vec![invalid],
            ))
            .await
            .expect_err("inferred event time cannot hard gate");
        assert_eq!(error.code(), "REWARD_EVENT_WINDOW_HARD_GATE_SHAPE_INVALID");
    }

    #[tokio::test]
    async fn postgres_event_window_snapshots_match_in_memory_semantics()
    -> std::result::Result<(), Box<dyn Error>> {
        let Some(database_url) = std::env::var("POLYEDGE_TEST_DATABASE_URL")
            .ok()
            .filter(|value| !value.trim().is_empty())
        else {
            return Ok(());
        };
        let _test_guard = EVENT_WINDOW_POSTGRES_TEST_LOCK.lock().await;

        let schema = format!("polyedge_event_window_test_{}", Uuid::now_v7().simple());
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
            EVENT_WINDOW_TEST_MIGRATOR.run(&pool).await?;

            let store = PostgresRewardBotStore::new(pool);
            assert_replace_semantics(&store).await?;
            Ok(())
        }
        .await;

        admin_pool
            .execute(format!("DROP SCHEMA {quoted_schema} CASCADE").as_str())
            .await?;
        test_result
    }

    #[tokio::test]
    async fn event_window_forward_migration_quarantines_legacy_gamma_rows()
    -> std::result::Result<(), Box<dyn Error>> {
        let Some(database_url) = std::env::var("POLYEDGE_TEST_DATABASE_URL")
            .ok()
            .filter(|value| !value.trim().is_empty())
        else {
            return Ok(());
        };
        let _test_guard = EVENT_WINDOW_POSTGRES_TEST_LOCK.lock().await;

        let schema = format!("polyedge_event_migration_test_{}", Uuid::now_v7().simple());
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
                .max_connections(1)
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

            sqlx::raw_sql(include_str!(
                "../../../../migrations/0001_initial_schema.sql"
            ))
            .execute(&pool)
            .await?;
            sqlx::query(
                r#"
                INSERT INTO reward_markets (
                    condition_id,
                    question,
                    market_slug,
                    rewards_max_spread,
                    rewards_min_size,
                    total_daily_rate,
                    tokens_json,
                    active,
                    updated_at
                )
                VALUES (
                    'legacy-condition',
                    'Legacy question',
                    'legacy-market',
                    4,
                    5,
                    20,
                    '[]'::JSONB,
                    TRUE,
                    now()
                )
                "#,
            )
            .execute(&pool)
            .await?;
            sqlx::query(
                r#"
                INSERT INTO reward_market_event_windows (
                    condition_id,
                    source,
                    event_type,
                    event_start_at,
                    event_end_at,
                    confidence,
                    source_payload,
                    active,
                    reviewed_by,
                    reviewed_at,
                    updated_at
                )
                VALUES (
                    'legacy-condition',
                    'manual',
                    'sports_start',
                    now() + interval '1 day',
                    now() + interval '1 day 2 hours',
                    'high',
                    '{}'::JSONB,
                    TRUE,
                    'operator',
                    now(),
                    now()
                )
                "#,
            )
            .execute(&pool)
            .await?;
            sqlx::query(
                r#"
                INSERT INTO reward_market_event_windows (
                    condition_id,
                    source,
                    event_type,
                    event_start_at,
                    event_end_at,
                    confidence,
                    source_payload,
                    active,
                    updated_at
                )
                VALUES (
                    'legacy-condition',
                    'gamma_reviewed',
                    'gamma_date',
                    now() - interval '30 days',
                    now() + interval '60 days',
                    'medium',
                    '{"has_reviewed_dates":true}'::JSONB,
                    TRUE,
                    now() - interval '1 minute'
                )
                "#,
            )
            .execute(&pool)
            .await?;
            sqlx::raw_sql(include_str!(
                "../../../../migrations/0002_reward_fair_value_history_identity.sql"
            ))
            .execute(&pool)
            .await?;
            sqlx::raw_sql(include_str!(
                "../../../../migrations/0003_reward_event_window_semantics.sql"
            ))
            .execute(&pool)
            .await?;

            let row = sqlx::query(
                r#"
                SELECT event_key,
                       active,
                       hard_gate_eligible,
                       event_time_role,
                       schedule_status,
                       source_payload ->> 'migration_quarantined' AS quarantined
                FROM reward_market_event_windows
                WHERE condition_id = 'legacy-condition'
                  AND source = 'gamma_reviewed'
                "#,
            )
            .fetch_one(&pool)
            .await?;
            assert_eq!(row.try_get::<String, _>("event_key")?, "legacy");
            assert!(!row.try_get::<bool, _>("active")?);
            assert!(!row.try_get::<bool, _>("hard_gate_eligible")?);
            assert_eq!(row.try_get::<String, _>("event_time_role")?, "unknown");
            assert_eq!(row.try_get::<String, _>("schedule_status")?, "withdrawn");
            assert_eq!(row.try_get::<String, _>("quarantined")?, "true");

            let manual = sqlx::query(
                r#"
                SELECT active,
                       hard_gate_eligible,
                       event_time_role,
                       schedule_status,
                       time_precision,
                       start_source_field
                FROM reward_market_event_windows
                WHERE condition_id = 'legacy-condition'
                  AND source = 'manual'
                "#,
            )
            .fetch_one(&pool)
            .await?;
            assert!(manual.try_get::<bool, _>("active")?);
            assert!(!manual.try_get::<bool, _>("hard_gate_eligible")?);
            assert_eq!(
                manual.try_get::<String, _>("event_time_role")?,
                "unknown"
            );
            assert_eq!(
                manual.try_get::<String, _>("schedule_status")?,
                "unknown"
            );
            assert_eq!(manual.try_get::<String, _>("time_precision")?, "unknown");
            assert!(manual.try_get::<Option<String>, _>("start_source_field")?.is_none());
            Ok(())
        }
        .await;

        admin_pool
            .execute(format!("DROP SCHEMA {quoted_schema} CASCADE").as_str())
            .await?;
        test_result
    }
}
