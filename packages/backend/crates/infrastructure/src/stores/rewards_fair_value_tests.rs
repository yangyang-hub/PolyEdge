#[cfg(test)]
mod rewards_fair_value_tests {
    use super::*;
    use sqlx::{Executor, postgres::PgPoolOptions};

    static REWARD_FAIR_VALUE_TEST_MIGRATOR: sqlx::migrate::Migrator =
        sqlx::migrate!("../../migrations");

    fn estimate(
        condition_id: &str,
        source: &str,
        fair_yes: Decimal,
        observed_at: OffsetDateTime,
    ) -> RewardFairValueEstimate {
        RewardFairValueEstimate {
            condition_id: condition_id.to_string(),
            source: source.to_string(),
            fair_yes,
            fair_no: Decimal::ONE - fair_yes,
            market_midpoint_yes: Some(fair_yes),
            confidence: Decimal::new(8, 1),
            uncertainty_cents: Decimal::ONE,
            midpoint_deviation_cents: Some(Decimal::ZERO),
            sample_count: 4,
            components: Vec::new(),
            do_not_quote_reason: None,
            observed_at,
            expires_at: observed_at + Duration::minutes(1),
        }
    }

    #[test]
    fn fair_value_normalization_collapses_identical_duplicates() {
        let now = OffsetDateTime::now_utc();
        let value = estimate("condition-a", "market_implied", Decimal::new(55, 2), now);

        let normalized = normalize_reward_fair_value_estimates(&[value.clone(), value.clone()])
            .expect("normalize identical estimates");

        assert_eq!(normalized.latest, vec![value.clone()]);
        assert_eq!(normalized.history, vec![value]);
    }

    #[test]
    fn fair_value_normalization_keeps_history_and_selects_latest_condition_value() {
        let now = OffsetDateTime::now_utc();
        let older = estimate(
            "condition-a",
            "market_implied",
            Decimal::new(54, 2),
            now - Duration::seconds(1),
        );
        let newer = estimate("condition-a", "market_implied", Decimal::new(55, 2), now);

        let normalized = normalize_reward_fair_value_estimates(&[newer.clone(), older])
            .expect("normalize versioned estimates");

        assert_eq!(normalized.latest, vec![newer]);
        assert_eq!(normalized.history.len(), 2);
    }

    #[test]
    fn fair_value_normalization_rejects_conflicting_identity() {
        let now = OffsetDateTime::now_utc();
        let first = estimate("condition-a", "market_implied", Decimal::new(54, 2), now);
        let second = estimate("condition-a", "market_implied", Decimal::new(55, 2), now);

        let error = normalize_reward_fair_value_estimates(&[first, second])
            .expect_err("conflicting identity must fail");

        assert!(
            error
                .to_string()
                .contains("conflicting reward fair-value estimates")
        );
    }

    #[tokio::test]
    async fn postgres_fair_value_batch_is_idempotent_when_test_database_is_available() {
        let Some(database_url) = std::env::var("POLYEDGE_TEST_DATABASE_URL")
            .ok()
            .filter(|value| !value.trim().is_empty())
        else {
            return;
        };
        let schema = format!("polyedge_fair_value_test_{}", Uuid::now_v7().simple());
        let quoted_schema = format!(r#"\"{}\""#, schema.replace('"', r#"\"\""#));
        let admin = PgPoolOptions::new()
            .max_connections(1)
            .connect(&database_url)
            .await
            .expect("connect test database");
        admin
            .execute(format!("CREATE SCHEMA {quoted_schema}").as_str())
            .await
            .expect("create test schema");

        let search_path = quoted_schema.clone();
        let pool = PgPoolOptions::new()
            .max_connections(2)
            .after_connect(move |connection, _| {
                let search_path = search_path.clone();
                Box::pin(async move {
                    connection
                        .execute(format!("SET search_path TO {search_path}").as_str())
                        .await?;
                    Ok(())
                })
            })
            .connect(&database_url)
            .await
            .expect("connect schema-scoped test database");
        REWARD_FAIR_VALUE_TEST_MIGRATOR
            .run(&pool)
            .await
            .expect("run migrations");

        let store = PostgresRewardBotStore::new(pool.clone());
        let now = OffsetDateTime::now_utc();
        let value = estimate("condition-a", "market_implied", Decimal::new(55, 2), now);
        store
            .record_fair_value_estimates(&[value.clone(), value.clone()])
            .await
            .expect("record duplicate batch");
        store
            .record_fair_value_estimates(&[value])
            .await
            .expect("replay identical estimate");

        let latest_count: i64 = sqlx::query_scalar("SELECT count(*) FROM reward_fair_values")
            .fetch_one(&pool)
            .await
            .expect("count latest fair values");
        let history_count: i64 =
            sqlx::query_scalar("SELECT count(*) FROM reward_fair_value_history")
                .fetch_one(&pool)
                .await
                .expect("count fair-value history");
        assert_eq!(latest_count, 1);
        assert_eq!(history_count, 1);

        pool.close().await;
        admin
            .execute(format!("DROP SCHEMA IF EXISTS {quoted_schema} CASCADE").as_str())
            .await
            .expect("drop test schema");
    }
}
