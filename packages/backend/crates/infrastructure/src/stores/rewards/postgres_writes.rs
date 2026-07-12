const REWARD_UPSERT_BATCH_SIZE: usize = 100;
const REWARD_MARKET_TOUCH_AFTER_SECS: i64 = 60 * 60;
const REWARD_MARKET_LOCK_TIMEOUT_MS: i64 = 5_000;
const REWARD_MARKET_STATEMENT_TIMEOUT_MS: i64 = 60_000;

async fn upsert_reward_markets_tx(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    markets: &[RewardMarket],
) -> Result<()> {
    set_local_reward_market_upsert_timeouts(transaction).await?;

    let current_condition_ids = markets
        .iter()
        .map(|market| market.condition_id.clone())
        .collect::<Vec<_>>();

    for chunk in markets.chunks(REWARD_UPSERT_BATCH_SIZE) {
        let cols = 11usize;
        let placeholders: String = chunk
            .iter()
            .enumerate()
            .map(|(i, _)| {
                let base = i * cols;
                let params: Vec<String> = (1..=cols)
                    .map(|j| format!("${}", base + j))
                    .collect();
                format!("({})", params.join(", "))
            })
            .collect::<Vec<_>>()
            .join(", ");

        let sql = format!(
            r#"INSERT INTO reward_markets (
              condition_id,
              question,
              market_slug,
              event_slug,
              image,
              rewards_max_spread,
              rewards_min_size,
              total_daily_rate,
              tokens_json,
              active,
              updated_at
            )
            VALUES {placeholders}
            ON CONFLICT (condition_id) DO UPDATE
            SET question = EXCLUDED.question,
                market_slug = EXCLUDED.market_slug,
                event_slug = EXCLUDED.event_slug,
                image = EXCLUDED.image,
                rewards_max_spread = EXCLUDED.rewards_max_spread,
                rewards_min_size = EXCLUDED.rewards_min_size,
                total_daily_rate = EXCLUDED.total_daily_rate,
                tokens_json = EXCLUDED.tokens_json,
                active = EXCLUDED.active,
                updated_at = EXCLUDED.updated_at
            WHERE EXCLUDED.updated_at >= reward_markets.updated_at
              AND (reward_markets.question IS DISTINCT FROM EXCLUDED.question
               OR reward_markets.market_slug IS DISTINCT FROM EXCLUDED.market_slug
               OR reward_markets.event_slug IS DISTINCT FROM EXCLUDED.event_slug
               OR reward_markets.image IS DISTINCT FROM EXCLUDED.image
               OR reward_markets.rewards_max_spread IS DISTINCT FROM EXCLUDED.rewards_max_spread
               OR reward_markets.rewards_min_size IS DISTINCT FROM EXCLUDED.rewards_min_size
               OR reward_markets.total_daily_rate IS DISTINCT FROM EXCLUDED.total_daily_rate
               OR reward_markets.tokens_json IS DISTINCT FROM EXCLUDED.tokens_json
               OR reward_markets.active IS DISTINCT FROM EXCLUDED.active
               OR reward_markets.updated_at < now() - (${}::BIGINT * interval '1 second'))"#,
            chunk.len() * cols + 1,
        );

        let mut query = sqlx::query(&sql);
        for market in chunk {
            query = query
                .bind(&market.condition_id)
                .bind(&market.question)
                .bind(&market.market_slug)
                .bind(&market.event_slug)
                .bind(&market.image)
                .bind(market.rewards_max_spread)
                .bind(market.rewards_min_size)
                .bind(market.total_daily_rate)
                .bind(Json(market.tokens.clone()))
                .bind(market.active)
                .bind(market.updated_at);
        }
        query
            .bind(REWARD_MARKET_TOUCH_AFTER_SECS)
            .execute(&mut **transaction)
            .await
            .map_err(|error| {
                db_error(
                    "POSTGRES_BATCH_UPSERT_REWARD_MARKETS_FAILED",
                    format!(
                        "failed to batch upsert reward markets (chunk size {}): {error}",
                        chunk.len()
                    ),
                )
            })?;
    }

    sqlx::query(
        r#"
        UPDATE reward_markets
        SET active = false,
            updated_at = now()
        WHERE active = true
          AND NOT (condition_id = ANY($1))
        "#,
    )
    .bind(&current_condition_ids)
    .execute(&mut **transaction)
    .await
    .map_err(|error| {
        db_error(
            "POSTGRES_UPDATE_FAILED",
            format!("failed to deactivate stale reward markets: {error}"),
        )
    })?;

    Ok(())
}

async fn set_local_reward_market_upsert_timeouts(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
) -> Result<()> {
    sqlx::query("SELECT set_config('lock_timeout', $1, true), set_config('statement_timeout', $2, true)")
        .bind(format!("{REWARD_MARKET_LOCK_TIMEOUT_MS}ms"))
        .bind(format!("{REWARD_MARKET_STATEMENT_TIMEOUT_MS}ms"))
        .execute(&mut **transaction)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_REWARD_MARKET_UPSERT_TIMEOUT_CONFIG_FAILED",
                format!("failed to configure reward market upsert statement timeouts: {error}"),
            )
        })?;
    Ok(())
}

async fn replace_reward_quote_plans_tx(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    plans: &[RewardQuotePlan],
) -> Result<()> {
    sqlx::query("DELETE FROM reward_quote_plans")
        .execute(&mut **transaction)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_DELETE_FAILED",
                format!("failed to clear reward quote plans: {error}"),
            )
        })?;

    for chunk in plans.chunks(REWARD_UPSERT_BATCH_SIZE) {
        let cols = 18usize;
        let placeholders: String = chunk
            .iter()
            .enumerate()
            .map(|(i, _)| {
                let base = i * cols;
                let params: Vec<String> = (1..=cols)
                    .map(|j| format!("${}", base + j))
                    .collect();
                format!("({})", params.join(", "))
            })
            .collect::<Vec<_>>()
            .join(", ");

        let sql = format!(
            r#"INSERT INTO reward_quote_plans (
              condition_id,
              score,
              selection_score,
              eligible,
              reason,
              strategy_profile,
              latest_run_id,
              quote_readiness,
              quote_mode,
              reason_code,
              blocker_codes,
              fair_value_passed,
              event_window_status,
              ai_action,
              info_risk_action,
              info_risk_level,
              quote_plan_json,
              updated_at
            )
            VALUES {placeholders}
            ON CONFLICT (condition_id, strategy_profile) DO UPDATE
            SET score = EXCLUDED.score,
                selection_score = EXCLUDED.selection_score,
                eligible = EXCLUDED.eligible,
                reason = EXCLUDED.reason,
                latest_run_id = EXCLUDED.latest_run_id,
                quote_readiness = EXCLUDED.quote_readiness,
                quote_mode = EXCLUDED.quote_mode,
                reason_code = EXCLUDED.reason_code,
                blocker_codes = EXCLUDED.blocker_codes,
                fair_value_passed = EXCLUDED.fair_value_passed,
                event_window_status = EXCLUDED.event_window_status,
                ai_action = EXCLUDED.ai_action,
                info_risk_action = EXCLUDED.info_risk_action,
                info_risk_level = EXCLUDED.info_risk_level,
                quote_plan_json = EXCLUDED.quote_plan_json,
                updated_at = EXCLUDED.updated_at"#,
        );

        let mut query = sqlx::query(&sql);
        for plan in chunk {
            let mut plan = plan.clone();
            refresh_reward_quote_plan_readiness(&mut plan);
            let condition_id = plan.condition_id.clone();
            let reason = plan.reason.clone();
            let reason_code = reward_quote_plan_reason_code(&plan);
            let blocker_codes = reward_quote_plan_blocker_codes(&plan, &reason_code);
            let event_window_status = plan
                .event_window
                .as_ref()
                .map(|assessment| assessment.status.as_str().to_string());
            let ai_action = plan
                .ai_advisory
                .as_ref()
                .map(|advisory| advisory.action.as_str().to_string());
            let info_risk_action = plan
                .info_risk
                .as_ref()
                .map(|risk| risk.action.as_str().to_string());
            let info_risk_level = plan
                .info_risk
                .as_ref()
                .map(|risk| risk.risk_level.as_str().to_string());
            let fair_value_passed = plan.fair_value.as_ref().map(|decision| decision.passed);
            let updated_at = plan.updated_at;
            query = query
                .bind(condition_id)
                .bind(plan.score)
                .bind(plan.selection_score)
                .bind(plan.eligible)
                .bind(reason)
                .bind(plan.strategy_profile.as_str())
                .bind(plan.latest_run_id)
                .bind(plan.quote_readiness.as_str())
                .bind(plan.quote_mode.as_str())
                .bind(reason_code)
                .bind(blocker_codes)
                .bind(fair_value_passed)
                .bind(event_window_status)
                .bind(ai_action)
                .bind(info_risk_action)
                .bind(info_risk_level)
                .bind(Json(plan))
                .bind(updated_at);
        }
        query.execute(&mut **transaction).await.map_err(|error| {
            db_error(
                "POSTGRES_BATCH_UPSERT_REWARD_QUOTE_PLANS_FAILED",
                format!(
                    "failed to batch upsert reward quote plans (chunk size {}): {error}",
                    chunk.len()
                ),
            )
        })?;
    }

    Ok(())
}

async fn postgres_record_reward_fair_value_estimates(
    pool: &PgPool,
    estimates: &[RewardFairValueEstimate],
) -> Result<()> {
    if estimates.is_empty() {
        return Ok(());
    }
    let normalized = normalize_reward_fair_value_estimates(estimates)?;

    let mut transaction = pool.begin().await.map_err(|error| {
        db_error(
            "POSTGRES_TRANSACTION_BEGIN_FAILED",
            format!("failed to begin reward fair-value transaction: {error}"),
        )
    })?;

    for chunk in normalized.latest.chunks(REWARD_UPSERT_BATCH_SIZE) {
        let cols = 13usize;
        let placeholders: String = chunk
            .iter()
            .enumerate()
            .map(|(i, _)| {
                let base = i * cols;
                let params: Vec<String> = (1..=cols).map(|j| format!("${}", base + j)).collect();
                format!("({})", params.join(", "))
            })
            .collect::<Vec<_>>()
            .join(", ");

        let latest_sql = format!(
            r#"
            INSERT INTO reward_fair_values (
              condition_id,
              source,
              fair_yes,
              fair_no,
              market_midpoint_yes,
              confidence,
              uncertainty_cents,
              midpoint_deviation_cents,
              sample_count,
              components_json,
              do_not_quote_reason,
              observed_at,
              expires_at
            )
            VALUES {placeholders}
            ON CONFLICT (condition_id) DO UPDATE
            SET source = EXCLUDED.source,
                fair_yes = EXCLUDED.fair_yes,
                fair_no = EXCLUDED.fair_no,
                market_midpoint_yes = EXCLUDED.market_midpoint_yes,
                confidence = EXCLUDED.confidence,
                uncertainty_cents = EXCLUDED.uncertainty_cents,
                midpoint_deviation_cents = EXCLUDED.midpoint_deviation_cents,
                sample_count = EXCLUDED.sample_count,
                components_json = EXCLUDED.components_json,
                do_not_quote_reason = EXCLUDED.do_not_quote_reason,
                observed_at = EXCLUDED.observed_at,
                expires_at = EXCLUDED.expires_at,
                updated_at = now()
            WHERE EXCLUDED.observed_at >= reward_fair_values.observed_at
            "#
        );

        let mut latest_query = sqlx::query(&latest_sql);
        for estimate in chunk {
            latest_query = bind_reward_fair_value_estimate(latest_query, estimate);
        }
        latest_query
            .execute(&mut *transaction)
            .await
            .map_err(|error| {
                db_error(
                    "POSTGRES_BATCH_UPSERT_REWARD_FAIR_VALUES_FAILED",
                    format!(
                        "failed to batch upsert reward fair values (chunk size {}): {error}",
                        chunk.len()
                    ),
                )
            })?;

    }

    for chunk in normalized.history.chunks(REWARD_UPSERT_BATCH_SIZE) {
        let history_cols = 14usize;
        let history_placeholders: String = chunk
            .iter()
            .enumerate()
            .map(|(i, _)| {
                let base = i * history_cols;
                let params: Vec<String> =
                    (1..=history_cols).map(|j| format!("${}", base + j)).collect();
                format!("({})", params.join(", "))
            })
            .collect::<Vec<_>>()
            .join(", ");
        let history_sql = format!(
            r#"
            INSERT INTO reward_fair_value_history (
              id,
              condition_id,
              source,
              fair_yes,
              fair_no,
              market_midpoint_yes,
              confidence,
              uncertainty_cents,
              midpoint_deviation_cents,
              sample_count,
              components_json,
              do_not_quote_reason,
              observed_at,
              expires_at
            )
            VALUES {history_placeholders}
            ON CONFLICT DO NOTHING
            "#
        );
        let mut history_query = sqlx::query(&history_sql);
        for estimate in chunk {
            history_query = history_query.bind(Uuid::new_v4());
            history_query = bind_reward_fair_value_estimate(history_query, estimate);
        }
        history_query
            .execute(&mut *transaction)
            .await
            .map_err(|error| {
                db_error(
                    "POSTGRES_BATCH_INSERT_REWARD_FAIR_VALUE_HISTORY_FAILED",
                    format!(
                        "failed to batch insert reward fair-value history (chunk size {}): {error}",
                        chunk.len()
                    ),
                )
            })?;
    }

    transaction.commit().await.map_err(|error| {
        db_error(
            "POSTGRES_TRANSACTION_COMMIT_FAILED",
            format!("failed to commit reward fair-value transaction: {error}"),
        )
    })?;
    Ok(())
}

fn bind_reward_fair_value_estimate<'q>(
    query: sqlx::query::Query<'q, sqlx::Postgres, sqlx::postgres::PgArguments>,
    estimate: &'q RewardFairValueEstimate,
) -> sqlx::query::Query<'q, sqlx::Postgres, sqlx::postgres::PgArguments> {
    query
        .bind(&estimate.condition_id)
        .bind(&estimate.source)
        .bind(estimate.fair_yes)
        .bind(estimate.fair_no)
        .bind(estimate.market_midpoint_yes)
        .bind(estimate.confidence)
        .bind(estimate.uncertainty_cents)
        .bind(estimate.midpoint_deviation_cents)
        .bind(i64::try_from(estimate.sample_count).unwrap_or(i64::MAX))
        .bind(Json(estimate.components.clone()))
        .bind(&estimate.do_not_quote_reason)
        .bind(estimate.observed_at)
        .bind(estimate.expires_at)
}
