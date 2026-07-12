async fn postgres_start_reward_strategy_run(
    pool: &PgPool,
    run: &RewardStrategyRunStart,
) -> Result<i64> {
    sqlx::query_scalar(
        r#"
        INSERT INTO reward_strategy_runs (
          account_id,
          trace_id,
          trigger_type,
          status,
          config_hash,
          config_json,
          input_summary_json,
          metrics_json,
          started_at
        )
        VALUES ($1, $2, $3, 'running', $4, $5, $6, '{}'::jsonb, $7)
        RETURNING run_id
        "#,
    )
    .bind(&run.account_id)
    .bind(&run.trace_id)
    .bind(run.trigger_type.as_str())
    .bind(&run.config_hash)
    .bind(Json(run.config_json.clone()))
    .bind(Json(run.input_summary.clone()))
    .bind(run.started_at)
    .fetch_one(pool)
    .await
    .map_err(|error| {
        db_error(
            "POSTGRES_INSERT_FAILED",
            format!("failed to insert reward strategy run: {error}"),
        )
    })
}

async fn postgres_complete_reward_strategy_run(
    pool: &PgPool,
    run_id: i64,
    metrics: Value,
    completed_at: OffsetDateTime,
) -> Result<()> {
    sqlx::query(
        r#"
        UPDATE reward_strategy_runs
        SET status = 'completed',
            metrics_json = $2,
            completed_at = $3,
            error_code = NULL,
            error_message = NULL
        WHERE run_id = $1
        "#,
    )
    .bind(run_id)
    .bind(Json(metrics))
    .bind(completed_at)
    .execute(pool)
    .await
    .map_err(|error| {
        db_error(
            "POSTGRES_UPDATE_FAILED",
            format!("failed to complete reward strategy run: {error}"),
        )
    })?;
    Ok(())
}

async fn postgres_fail_reward_strategy_run(
    pool: &PgPool,
    run_id: i64,
    error_code: &str,
    error_message: &str,
    metrics: Value,
    completed_at: OffsetDateTime,
) -> Result<()> {
    sqlx::query(
        r#"
        UPDATE reward_strategy_runs
        SET status = 'failed',
            metrics_json = $2,
            completed_at = $3,
            error_code = $4,
            error_message = $5
        WHERE run_id = $1
        "#,
    )
    .bind(run_id)
    .bind(Json(metrics))
    .bind(completed_at)
    .bind(error_code)
    .bind(error_message)
    .execute(pool)
    .await
    .map_err(|error| {
        db_error(
            "POSTGRES_UPDATE_FAILED",
            format!("failed to fail reward strategy run: {error}"),
        )
    })?;
    Ok(())
}

async fn postgres_record_reward_strategy_decisions(
    pool: &PgPool,
    decisions: &[RewardStrategyDecision],
) -> Result<()> {
    if decisions.is_empty() {
        return Ok(());
    }
    let mut transaction = pool.begin().await.map_err(|error| {
        db_error(
            "POSTGRES_TRANSACTION_BEGIN_FAILED",
            format!("failed to begin reward strategy decision transaction: {error}"),
        )
    })?;
    postgres_record_reward_strategy_decisions_tx(&mut transaction, decisions).await?;
    transaction.commit().await.map_err(|error| {
        db_error(
            "POSTGRES_TRANSACTION_COMMIT_FAILED",
            format!("failed to commit reward strategy decision transaction: {error}"),
        )
    })?;
    Ok(())
}

async fn postgres_record_reward_strategy_decisions_tx(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    decisions: &[RewardStrategyDecision],
) -> Result<()> {
    for chunk in decisions.chunks(REWARD_UPSERT_BATCH_SIZE) {
        let mut builder: QueryBuilder<Postgres> = QueryBuilder::new(
            r#"
            INSERT INTO reward_strategy_decisions (
              run_id,
              condition_id,
              strategy_profile,
              decision_rank,
              eligible,
              quote_readiness,
              quote_mode,
              score,
              selection_score,
              reason_code,
              reason,
              blocker_codes,
              planned_buy_notional_usd,
              fair_value_passed,
              fair_value_effective_edge_cents,
              opportunity_score,
              event_window_status,
              ai_action,
              info_risk_action,
              info_risk_level,
              decision_json,
              created_at
            )
            "#,
        );
        builder.push_values(chunk.iter(), |mut row, decision| {
            row.push_bind(decision.run_id)
                .push_bind(&decision.condition_id)
                .push_bind(decision.strategy_profile.as_str())
                .push_bind(decision.decision_rank)
                .push_bind(decision.eligible)
                .push_bind(decision.quote_readiness.as_str())
                .push_bind(decision.quote_mode.as_str())
                .push_bind(decision.score)
                .push_bind(decision.selection_score)
                .push_bind(&decision.reason_code)
                .push_bind(&decision.reason)
                .push_bind(&decision.blocker_codes)
                .push_bind(decision.planned_buy_notional_usd)
                .push_bind(decision.fair_value_passed)
                .push_bind(decision.fair_value_effective_edge_cents)
                .push_bind(decision.opportunity_score)
                .push_bind(&decision.event_window_status)
                .push_bind(&decision.ai_action)
                .push_bind(&decision.info_risk_action)
                .push_bind(&decision.info_risk_level)
                .push_bind(Json(decision.decision_json.clone()))
                .push_bind(decision.created_at);
        });
        builder.push(
            r#"
            ON CONFLICT (run_id, condition_id, strategy_profile) DO UPDATE
            SET decision_rank = EXCLUDED.decision_rank,
                eligible = EXCLUDED.eligible,
                quote_readiness = EXCLUDED.quote_readiness,
                quote_mode = EXCLUDED.quote_mode,
                score = EXCLUDED.score,
                selection_score = EXCLUDED.selection_score,
                reason_code = EXCLUDED.reason_code,
                reason = EXCLUDED.reason,
                blocker_codes = EXCLUDED.blocker_codes,
                planned_buy_notional_usd = EXCLUDED.planned_buy_notional_usd,
                fair_value_passed = EXCLUDED.fair_value_passed,
                fair_value_effective_edge_cents = EXCLUDED.fair_value_effective_edge_cents,
                opportunity_score = EXCLUDED.opportunity_score,
                event_window_status = EXCLUDED.event_window_status,
                ai_action = EXCLUDED.ai_action,
                info_risk_action = EXCLUDED.info_risk_action,
                info_risk_level = EXCLUDED.info_risk_level,
                decision_json = EXCLUDED.decision_json,
                created_at = EXCLUDED.created_at
            WHERE EXCLUDED.created_at >= reward_strategy_decisions.created_at
            "#,
        );
        builder
            .build()
            .execute(&mut **transaction)
            .await
            .map_err(|error| {
                db_error(
                    "POSTGRES_BATCH_UPSERT_REWARD_STRATEGY_DECISIONS_FAILED",
                    format!(
                        "failed to batch upsert reward strategy decisions (chunk size {}): {error}",
                        chunk.len()
                    ),
                )
            })?;
    }
    Ok(())
}

async fn postgres_record_reward_strategy_actions(
    pool: &PgPool,
    actions: &[RewardStrategyAction],
) -> Result<()> {
    if actions.is_empty() {
        return Ok(());
    }
    let mut transaction = pool.begin().await.map_err(|error| {
        db_error(
            "POSTGRES_TRANSACTION_BEGIN_FAILED",
            format!("failed to begin reward strategy action transaction: {error}"),
        )
    })?;
    postgres_record_reward_strategy_actions_tx(&mut transaction, actions).await?;
    transaction.commit().await.map_err(|error| {
        db_error(
            "POSTGRES_TRANSACTION_COMMIT_FAILED",
            format!("failed to commit reward strategy action transaction: {error}"),
        )
    })?;
    Ok(())
}

async fn postgres_record_reward_strategy_actions_tx(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    actions: &[RewardStrategyAction],
) -> Result<()> {
    for chunk in actions.chunks(REWARD_UPSERT_BATCH_SIZE) {
        let mut builder: QueryBuilder<Postgres> = QueryBuilder::new(
            r#"
            INSERT INTO reward_strategy_actions (
              run_id,
              account_id,
              condition_id,
              token_id,
              managed_order_id,
              external_order_id,
              action_type,
              status,
              reason_code,
              reason,
              idempotency_key,
              request_json,
              result_json,
              lease_owner,
              lease_expires_at,
              execution_attempts,
              created_at,
              updated_at
            )
            "#,
        );
        builder.push_values(chunk.iter(), |mut row, action| {
            row.push_bind(action.run_id)
                .push_bind(&action.account_id)
                .push_bind(&action.condition_id)
                .push_bind(&action.token_id)
                .push_bind(&action.managed_order_id)
                .push_bind(&action.external_order_id)
                .push_bind(action.action_type.as_str())
                .push_bind(action.status.as_str())
                .push_bind(&action.reason_code)
                .push_bind(&action.reason)
                .push_bind(&action.idempotency_key)
                .push_bind(Json(action.request_json.clone()))
                .push_bind(Json(action.result_json.clone()))
                .push_bind(&action.lease_owner)
                .push_bind(action.lease_expires_at)
                .push_bind(action.execution_attempts)
                .push_bind(action.created_at)
                .push_bind(action.updated_at);
        });
        builder.push(
            r#"
            ON CONFLICT (idempotency_key) DO UPDATE
            SET status = EXCLUDED.status,
                reason_code = EXCLUDED.reason_code,
                reason = EXCLUDED.reason,
                result_json = EXCLUDED.result_json,
                lease_owner = CASE
                    WHEN EXCLUDED.status IN ('succeeded', 'failed', 'skipped', 'unknown') THEN NULL
                    ELSE COALESCE(EXCLUDED.lease_owner, reward_strategy_actions.lease_owner)
                END,
                lease_expires_at = CASE
                    WHEN EXCLUDED.status IN ('succeeded', 'failed', 'skipped', 'unknown') THEN NULL
                    ELSE COALESCE(EXCLUDED.lease_expires_at, reward_strategy_actions.lease_expires_at)
                END,
                execution_attempts = GREATEST(
                    reward_strategy_actions.execution_attempts,
                    EXCLUDED.execution_attempts
                ),
                updated_at = EXCLUDED.updated_at
            WHERE EXCLUDED.updated_at >= reward_strategy_actions.updated_at
              AND (
                  reward_strategy_actions.status NOT IN ('succeeded', 'failed', 'skipped', 'unknown')
                  OR EXCLUDED.status = reward_strategy_actions.status
              )
            "#,
        );
        builder
            .build()
            .execute(&mut **transaction)
            .await
            .map_err(|error| {
                db_error(
                    "POSTGRES_BATCH_UPSERT_REWARD_STRATEGY_ACTIONS_FAILED",
                    format!(
                        "failed to batch upsert reward strategy actions (chunk size {}): {error}",
                        chunk.len()
                    ),
                )
            })?;
    }
    Ok(())
}

async fn postgres_claim_reward_strategy_actions(
    pool: &PgPool,
    account_id: &str,
    lease_owner: &str,
    now: OffsetDateTime,
    lease_expires_at: OffsetDateTime,
    limit: u16,
) -> Result<Vec<RewardStrategyAction>> {
    let rows = sqlx::query(
        r#"
        WITH claimable AS (
            SELECT action_id, status AS previous_status
            FROM reward_strategy_actions
            WHERE account_id = $1
              AND (
                    status = 'planned'
                    OR (
                        status = 'executing'
                        AND lease_expires_at IS NOT NULL
                        AND lease_expires_at <= $3
                    )
              )
            ORDER BY created_at ASC, action_id ASC
            FOR UPDATE SKIP LOCKED
            LIMIT $5
        )
        UPDATE reward_strategy_actions AS action
        SET status = 'executing',
            lease_owner = $2,
            lease_expires_at = $4,
            execution_attempts = action.execution_attempts + 1,
            updated_at = $3,
            result_json = action.result_json || jsonb_build_object(
                'status', 'executing',
                'lease_owner', $2,
                'lease_expires_at', $4,
                'execution_attempts', action.execution_attempts + 1,
                'claim_previous_status', claimable.previous_status
            )
        FROM claimable
        WHERE action.action_id = claimable.action_id
        RETURNING action.action_id,
                  action.run_id,
                  action.account_id,
                  action.condition_id,
                  action.token_id,
                  action.managed_order_id,
                  action.external_order_id,
                  action.action_type,
                  action.status,
                  action.reason_code,
                  action.reason,
                  action.idempotency_key,
                  action.request_json,
                  action.result_json,
                  action.lease_owner,
                  action.lease_expires_at,
                  action.execution_attempts,
                  action.created_at,
                  action.updated_at
        "#,
    )
    .bind(account_id)
    .bind(lease_owner)
    .bind(now)
    .bind(lease_expires_at)
    .bind(i64::from(limit))
    .fetch_all(pool)
    .await
    .map_err(|error| {
        db_error(
            "POSTGRES_CLAIM_REWARD_STRATEGY_ACTIONS_FAILED",
            format!("failed to claim reward strategy actions: {error}"),
        )
    })?;

    let mut actions = rows
        .iter()
        .map(reward_strategy_action_from_row)
        .collect::<Result<Vec<_>>>()?;
    actions.sort_by_key(|action| (action.created_at, action.action_id));
    Ok(actions)
}

async fn postgres_renew_reward_strategy_action_lease(
    pool: &PgPool,
    action_id: i64,
    lease_owner: &str,
    now: OffsetDateTime,
    lease_expires_at: OffsetDateTime,
) -> Result<bool> {
    let result = sqlx::query(
        r#"
        UPDATE reward_strategy_actions
        SET lease_expires_at = $4,
            result_json = result_json || jsonb_build_object('lease_expires_at', $4),
            updated_at = $3
        WHERE action_id = $1
          AND status = 'executing'
          AND lease_owner = $2
          AND lease_expires_at > $3
        "#,
    )
    .bind(action_id)
    .bind(lease_owner)
    .bind(now)
    .bind(lease_expires_at)
    .execute(pool)
    .await
    .map_err(|error| {
        db_error(
            "POSTGRES_RENEW_REWARD_STRATEGY_ACTION_LEASE_FAILED",
            format!("failed to renew reward strategy action lease: {error}"),
        )
    })?;
    Ok(result.rows_affected() == 1)
}

async fn postgres_finalize_reward_strategy_action_lease(
    pool: &PgPool,
    action: &RewardStrategyAction,
    lease_owner: &str,
) -> Result<bool> {
    if !action.status.is_terminal() || !action.result_json.is_object() {
        return Err(AppError::invalid_input(
            "REWARD_STRATEGY_ACTION_RESOLUTION_INVALID",
            "strategy action resolution requires a terminal status and object result",
        ));
    }
    let result = sqlx::query(
        r#"
        UPDATE reward_strategy_actions
        SET status = $3,
            reason_code = $4,
            reason = $5,
            external_order_id = $6,
            result_json = $7,
            lease_owner = NULL,
            lease_expires_at = NULL,
            updated_at = $8
        WHERE action_id = $1
          AND status = 'executing'
          AND lease_owner = $2
          AND lease_expires_at > CURRENT_TIMESTAMP
        "#,
    )
    .bind(action.action_id)
    .bind(lease_owner)
    .bind(action.status.as_str())
    .bind(&action.reason_code)
    .bind(&action.reason)
    .bind(&action.external_order_id)
    .bind(&action.result_json)
    .bind(action.updated_at)
    .execute(pool)
    .await
    .map_err(|error| {
        db_error(
            "POSTGRES_FINALIZE_REWARD_STRATEGY_ACTION_LEASE_FAILED",
            format!("failed to finalize reward strategy action lease: {error}"),
        )
    })?;
    Ok(result.rows_affected() == 1)
}

async fn postgres_get_reward_strategy_action(
    pool: &PgPool,
    action_id: i64,
) -> Result<Option<RewardStrategyAction>> {
    let row = sqlx::query(
        r#"
        SELECT action_id,
               run_id,
               account_id,
               condition_id,
               token_id,
               managed_order_id,
               external_order_id,
               action_type,
               status,
               reason_code,
               reason,
               idempotency_key,
               request_json,
               result_json,
               lease_owner,
               lease_expires_at,
               execution_attempts,
               created_at,
               updated_at
        FROM reward_strategy_actions
        WHERE action_id = $1
        "#,
    )
    .bind(action_id)
    .fetch_optional(pool)
    .await
    .map_err(|error| {
        db_error(
            "POSTGRES_QUERY_REWARD_STRATEGY_ACTION_FAILED",
            format!("failed to query reward strategy action: {error}"),
        )
    })?;
    row.as_ref().map(reward_strategy_action_from_row).transpose()
}

async fn postgres_release_reward_strategy_action_lease(
    pool: &PgPool,
    action_id: i64,
    lease_owner: &str,
    reason_code: &str,
    reason: &str,
    result: Value,
    now: OffsetDateTime,
) -> Result<bool> {
    let update = sqlx::query(
        r#"
        UPDATE reward_strategy_actions
        SET status = 'planned',
            reason_code = $3,
            reason = $4,
            result_json = result_json || $5 || jsonb_build_object('status', 'planned'),
            lease_owner = NULL,
            lease_expires_at = NULL,
            updated_at = $6
        WHERE action_id = $1
          AND status = 'executing'
          AND lease_owner = $2
          AND lease_expires_at > $6
        "#,
    )
    .bind(action_id)
    .bind(lease_owner)
    .bind(reason_code)
    .bind(reason)
    .bind(Json(result))
    .bind(now)
    .execute(pool)
    .await
    .map_err(|error| {
        db_error(
            "POSTGRES_RELEASE_REWARD_STRATEGY_ACTION_LEASE_FAILED",
            format!("failed to release reward strategy action lease: {error}"),
        )
    })?;
    Ok(update.rows_affected() == 1)
}

async fn postgres_record_reward_order_transitions(
    pool: &PgPool,
    transitions: &[RewardOrderTransition],
) -> Result<()> {
    if transitions.is_empty() {
        return Ok(());
    }
    let mut transaction = pool.begin().await.map_err(|error| {
        db_error(
            "POSTGRES_TRANSACTION_BEGIN_FAILED",
            format!("failed to begin reward order transition transaction: {error}"),
        )
    })?;
    postgres_record_reward_order_transitions_tx(&mut transaction, transitions).await?;
    transaction.commit().await.map_err(|error| {
        db_error(
            "POSTGRES_TRANSACTION_COMMIT_FAILED",
            format!("failed to commit reward order transition transaction: {error}"),
        )
    })?;
    Ok(())
}

async fn postgres_record_reward_order_transitions_tx(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    transitions: &[RewardOrderTransition],
) -> Result<()> {
    for chunk in transitions.chunks(REWARD_UPSERT_BATCH_SIZE) {
        let mut builder: QueryBuilder<Postgres> = QueryBuilder::new(
            r#"
            INSERT INTO reward_order_transitions (
              run_id,
              action_id,
              managed_order_id,
              external_order_id,
              from_status,
              to_status,
              reason_code,
              reason,
              metadata_json,
              created_at
            )
            "#,
        );
        builder.push_values(chunk.iter(), |mut row, transition| {
            row.push_bind(transition.run_id)
                .push_bind(transition.action_id)
                .push_bind(&transition.managed_order_id)
                .push_bind(&transition.external_order_id)
                .push_bind(transition.from_status.map(ManagedRewardOrderStatus::as_str))
                .push_bind(transition.to_status.as_str())
                .push_bind(&transition.reason_code)
                .push_bind(&transition.reason)
                .push_bind(Json(transition.metadata.clone()))
                .push_bind(transition.created_at);
        });
        builder
            .build()
            .execute(&mut **transaction)
            .await
            .map_err(|error| {
                db_error(
                    "POSTGRES_BATCH_INSERT_REWARD_ORDER_TRANSITIONS_FAILED",
                    format!(
                        "failed to batch insert reward order transitions (chunk size {}): {error}",
                        chunk.len()
                    ),
                )
            })?;
    }
    Ok(())
}

async fn postgres_latest_reward_strategy_run_id_for_trace_tx(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    trace_id: &str,
) -> Result<Option<i64>> {
    sqlx::query_scalar(
        r#"
        SELECT run_id
        FROM reward_strategy_runs
        WHERE trace_id = $1
        ORDER BY started_at DESC, run_id DESC
        LIMIT 1
        "#,
    )
    .bind(trace_id)
    .fetch_optional(&mut **transaction)
    .await
    .map_err(|error| {
        db_error(
            "POSTGRES_QUERY_FAILED",
            format!("failed to query reward strategy run by trace id: {error}"),
        )
    })
}

async fn postgres_reward_order_statuses_for_transition_tx(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    order_ids: &[String],
) -> Result<HashMap<String, ManagedRewardOrderStatus>> {
    if order_ids.is_empty() {
        return Ok(HashMap::new());
    }
    let rows = sqlx::query(
        r#"
        SELECT id,
               status
        FROM reward_managed_orders
        WHERE id = ANY($1)
        "#,
    )
    .bind(order_ids)
    .fetch_all(&mut **transaction)
    .await
    .map_err(|error| {
        db_error(
            "POSTGRES_QUERY_FAILED",
            format!("failed to query reward order statuses before transition insert: {error}"),
        )
    })?;

    rows.iter()
        .map(|row| {
            let id: String = row.try_get("id").map_err(postgres_decode_error)?;
            let status: String = row.try_get("status").map_err(postgres_decode_error)?;
            Ok((id, ManagedRewardOrderStatus::from_str(&status)?))
        })
        .collect()
}

async fn postgres_list_reward_strategy_runs(
    pool: &PgPool,
    query: &RewardStrategyRunListQuery,
) -> Result<RewardStrategyRunPage> {
    let status = query.status.map(RewardStrategyRunStatus::as_str);
    let total_items: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM reward_strategy_runs
        WHERE ($1::text IS NULL OR account_id = $1)
          AND ($2::text IS NULL OR status = $2)
        "#,
    )
    .bind(query.account_id.as_deref())
    .bind(status)
    .fetch_one(pool)
    .await
    .map_err(|error| {
        db_error(
            "POSTGRES_QUERY_FAILED",
            format!("failed to count reward strategy runs: {error}"),
        )
    })?;

    let page = query.page_for_total(total_items.max(0) as usize);
    let offset = (page.page - 1) * page.page_size;
    let rows = sqlx::query(REWARD_STRATEGY_RUN_SELECT_SQL)
        .bind(query.account_id.as_deref())
        .bind(status)
        .bind(page.page_size as i64)
        .bind(offset as i64)
        .fetch_all(pool)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_QUERY_FAILED",
                format!("failed to query reward strategy runs: {error}"),
            )
        })?;
    let items = rows
        .iter()
        .map(reward_strategy_run_from_row)
        .collect::<Result<Vec<_>>>()?;
    Ok(RewardStrategyRunPage { items, page })
}

async fn postgres_get_reward_strategy_run(
    pool: &PgPool,
    run_id: i64,
) -> Result<Option<RewardStrategyRun>> {
    let row = sqlx::query(
        r#"
        SELECT run_id,
               account_id,
               trace_id,
               trigger_type,
               status,
               config_hash,
               config_json,
               input_summary_json,
               metrics_json,
               started_at,
               completed_at,
               error_code,
               error_message
        FROM reward_strategy_runs
        WHERE run_id = $1
        "#,
    )
    .bind(run_id)
    .fetch_optional(pool)
    .await
    .map_err(|error| {
        db_error(
            "POSTGRES_QUERY_FAILED",
            format!("failed to query reward strategy run: {error}"),
        )
    })?;
    row.as_ref().map(reward_strategy_run_from_row).transpose()
}

async fn postgres_save_reward_strategy_replay_fixture(
    pool: &PgPool,
    fixture: &RewardStrategyReplayFixture,
) -> Result<()> {
    let json_bytes = i32::try_from(fixture.json_bytes).map_err(|_| {
        AppError::invalid_input(
            "REWARD_REPLAY_FIXTURE_TOO_LARGE",
            "rewards replay fixture size cannot be stored",
        )
    })?;
    sqlx::query(
        r#"
        INSERT INTO reward_strategy_replay_fixtures (
          run_id,
          schema_version,
          fixture_json,
          json_bytes,
          sha256,
          captured_at
        )
        VALUES ($1, $2, $3, $4, $5, $6)
        ON CONFLICT (run_id) DO UPDATE
        SET schema_version = EXCLUDED.schema_version,
            fixture_json = EXCLUDED.fixture_json,
            json_bytes = EXCLUDED.json_bytes,
            sha256 = EXCLUDED.sha256,
            captured_at = EXCLUDED.captured_at
        "#,
    )
    .bind(fixture.run_id)
    .bind(i32::from(fixture.schema_version))
    .bind(Json(&fixture.fixture))
    .bind(json_bytes)
    .bind(&fixture.sha256)
    .bind(fixture.captured_at)
    .execute(pool)
    .await
    .map_err(|error| {
        db_error(
            "POSTGRES_UPSERT_FAILED",
            format!("failed to persist reward strategy replay fixture: {error}"),
        )
    })?;
    Ok(())
}

async fn postgres_get_reward_strategy_replay_fixture(
    pool: &PgPool,
    run_id: i64,
) -> Result<Option<RewardStrategyReplayFixture>> {
    let row = sqlx::query(
        r#"
        SELECT run_id,
               schema_version,
               fixture_json,
               json_bytes,
               sha256,
               captured_at
        FROM reward_strategy_replay_fixtures
        WHERE run_id = $1
        "#,
    )
    .bind(run_id)
    .fetch_optional(pool)
    .await
    .map_err(|error| {
        db_error(
            "POSTGRES_QUERY_FAILED",
            format!("failed to query reward strategy replay fixture: {error}"),
        )
    })?;
    let Some(row) = row else {
        return Ok(None);
    };
    let schema_version: i32 = row.try_get("schema_version").map_err(postgres_decode_error)?;
    let schema_version = u16::try_from(schema_version).map_err(|_| {
        db_error(
            "POSTGRES_DECODE_FAILED",
            "reward strategy replay schema version is out of range",
        )
    })?;
    let Json(fixture) = row
        .try_get::<Json<RewardDecisionReplayFixture>, _>("fixture_json")
        .map_err(postgres_decode_error)?;
    let json_bytes: i32 = row.try_get("json_bytes").map_err(postgres_decode_error)?;
    let json_bytes = u32::try_from(json_bytes).map_err(|_| {
        db_error(
            "POSTGRES_DECODE_FAILED",
            "reward strategy replay fixture byte size is out of range",
        )
    })?;
    let fixture = RewardStrategyReplayFixture {
        run_id: row.try_get("run_id").map_err(postgres_decode_error)?,
        schema_version,
        fixture,
        json_bytes,
        sha256: row.try_get("sha256").map_err(postgres_decode_error)?,
        captured_at: row.try_get("captured_at").map_err(postgres_decode_error)?,
    };
    fixture.validate_integrity()?;
    Ok(Some(fixture))
}

async fn postgres_list_reward_strategy_decisions(
    pool: &PgPool,
    run_id: i64,
    query: &RewardStrategyDecisionListQuery,
) -> Result<RewardStrategyDecisionPage> {
    let total_items: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM reward_strategy_decisions
        WHERE run_id = $1
          AND ($2::text IS NULL
               OR condition_id ILIKE '%' || $2 || '%'
               OR reason ILIKE '%' || $2 || '%'
               OR decision_json::text ILIKE '%' || $2 || '%')
          AND ($3::boolean IS NULL OR eligible = $3)
        "#,
    )
    .bind(run_id)
    .bind(query.search.as_deref())
    .bind(query.eligible)
    .fetch_one(pool)
    .await
    .map_err(|error| {
        db_error(
            "POSTGRES_QUERY_FAILED",
            format!("failed to count reward strategy decisions: {error}"),
        )
    })?;

    let page = query.page_for_total(total_items.max(0) as usize);
    let offset = (page.page - 1) * page.page_size;
    let rows = sqlx::query(
        r#"
        SELECT run_id,
               condition_id,
               strategy_profile,
               decision_rank,
               eligible,
               quote_readiness,
               quote_mode,
               score,
               selection_score,
               reason_code,
               reason,
               blocker_codes,
               planned_buy_notional_usd,
               fair_value_passed,
               fair_value_effective_edge_cents,
               opportunity_score,
               event_window_status,
               ai_action,
               info_risk_action,
               info_risk_level,
               decision_json,
               created_at
        FROM reward_strategy_decisions
        WHERE run_id = $1
          AND ($2::text IS NULL
               OR condition_id ILIKE '%' || $2 || '%'
               OR reason ILIKE '%' || $2 || '%'
               OR decision_json::text ILIKE '%' || $2 || '%')
          AND ($3::boolean IS NULL OR eligible = $3)
        ORDER BY decision_rank ASC, selection_score DESC, condition_id ASC
        LIMIT $4
        OFFSET $5
        "#,
    )
    .bind(run_id)
    .bind(query.search.as_deref())
    .bind(query.eligible)
    .bind(page.page_size as i64)
    .bind(offset as i64)
    .fetch_all(pool)
    .await
    .map_err(|error| {
        db_error(
            "POSTGRES_QUERY_FAILED",
            format!("failed to query reward strategy decisions: {error}"),
        )
    })?;

    let items = rows
        .iter()
        .map(reward_strategy_decision_from_row)
        .collect::<Result<Vec<_>>>()?;
    Ok(RewardStrategyDecisionPage { items, page })
}

async fn postgres_list_reward_strategy_actions(
    pool: &PgPool,
    run_id: i64,
    query: &RewardStrategyActionListQuery,
) -> Result<RewardStrategyActionPage> {
    let status = query.status.map(RewardStrategyActionStatus::as_str);
    let action_type = query.action_type.map(RewardStrategyActionType::as_str);
    let total_items: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM reward_strategy_actions
        WHERE run_id = $1
          AND ($2::text IS NULL OR status = $2)
          AND ($3::text IS NULL OR action_type = $3)
        "#,
    )
    .bind(run_id)
    .bind(status)
    .bind(action_type)
    .fetch_one(pool)
    .await
    .map_err(|error| {
        db_error(
            "POSTGRES_QUERY_FAILED",
            format!("failed to count reward strategy actions: {error}"),
        )
    })?;

    let page = query.page_for_total(total_items.max(0) as usize);
    let offset = (page.page - 1) * page.page_size;
    let rows = sqlx::query(
        r#"
        SELECT action_id,
               run_id,
               account_id,
               condition_id,
               token_id,
               managed_order_id,
               external_order_id,
               action_type,
               status,
               reason_code,
               reason,
               idempotency_key,
               request_json,
               result_json,
               lease_owner,
               lease_expires_at,
               execution_attempts,
               created_at,
               updated_at
        FROM reward_strategy_actions
        WHERE run_id = $1
          AND ($2::text IS NULL OR status = $2)
          AND ($3::text IS NULL OR action_type = $3)
        ORDER BY created_at DESC, action_id DESC
        LIMIT $4
        OFFSET $5
        "#,
    )
    .bind(run_id)
    .bind(status)
    .bind(action_type)
    .bind(page.page_size as i64)
    .bind(offset as i64)
    .fetch_all(pool)
    .await
    .map_err(|error| {
        db_error(
            "POSTGRES_QUERY_FAILED",
            format!("failed to query reward strategy actions: {error}"),
        )
    })?;

    let items = rows
        .iter()
        .map(reward_strategy_action_from_row)
        .collect::<Result<Vec<_>>>()?;
    Ok(RewardStrategyActionPage { items, page })
}

async fn postgres_list_reward_order_transitions(
    pool: &PgPool,
    managed_order_id: &str,
    query: &RewardOrderTransitionListQuery,
) -> Result<RewardOrderTransitionPage> {
    let total_items: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM reward_order_transitions
        WHERE managed_order_id = $1
        "#,
    )
    .bind(managed_order_id)
    .fetch_one(pool)
    .await
    .map_err(|error| {
        db_error(
            "POSTGRES_QUERY_FAILED",
            format!("failed to count reward order transitions: {error}"),
        )
    })?;

    let page = query.page_for_total(total_items.max(0) as usize);
    let offset = (page.page - 1) * page.page_size;
    let rows = sqlx::query(
        r#"
        SELECT transition_id,
               run_id,
               action_id,
               managed_order_id,
               external_order_id,
               from_status,
               to_status,
               reason_code,
               reason,
               metadata_json,
               created_at
        FROM reward_order_transitions
        WHERE managed_order_id = $1
        ORDER BY created_at DESC, transition_id DESC
        LIMIT $2
        OFFSET $3
        "#,
    )
    .bind(managed_order_id)
    .bind(page.page_size as i64)
    .bind(offset as i64)
    .fetch_all(pool)
    .await
    .map_err(|error| {
        db_error(
            "POSTGRES_QUERY_FAILED",
            format!("failed to query reward order transitions: {error}"),
        )
    })?;

    let items = rows
        .iter()
        .map(reward_order_transition_from_row)
        .collect::<Result<Vec<_>>>()?;
    Ok(RewardOrderTransitionPage { items, page })
}

const REWARD_STRATEGY_RUN_SELECT_SQL: &str = r#"
    SELECT run_id,
           account_id,
           trace_id,
           trigger_type,
           status,
           config_hash,
           config_json,
           input_summary_json,
           metrics_json,
           started_at,
           completed_at,
           error_code,
           error_message
    FROM reward_strategy_runs
    WHERE ($1::text IS NULL OR account_id = $1)
      AND ($2::text IS NULL OR status = $2)
    ORDER BY started_at DESC, run_id DESC
    LIMIT $3
    OFFSET $4
"#;

fn reward_strategy_run_from_row(row: &sqlx::postgres::PgRow) -> Result<RewardStrategyRun> {
    let trigger_type: String = row.try_get("trigger_type").map_err(postgres_decode_error)?;
    let status: String = row.try_get("status").map_err(postgres_decode_error)?;
    let config_json: Json<Value> = row.try_get("config_json").map_err(postgres_decode_error)?;
    let input_summary: Json<Value> = row
        .try_get("input_summary_json")
        .map_err(postgres_decode_error)?;
    let metrics: Json<Value> = row.try_get("metrics_json").map_err(postgres_decode_error)?;
    Ok(RewardStrategyRun {
        run_id: row.try_get("run_id").map_err(postgres_decode_error)?,
        account_id: row.try_get("account_id").map_err(postgres_decode_error)?,
        trace_id: row.try_get("trace_id").map_err(postgres_decode_error)?,
        trigger_type: RewardStrategyRunTrigger::from_str(&trigger_type)?,
        status: RewardStrategyRunStatus::from_str(&status)?,
        config_hash: row.try_get("config_hash").map_err(postgres_decode_error)?,
        config_json: config_json.0,
        input_summary: input_summary.0,
        metrics: metrics.0,
        started_at: row.try_get("started_at").map_err(postgres_decode_error)?,
        completed_at: row.try_get("completed_at").map_err(postgres_decode_error)?,
        error_code: row.try_get("error_code").map_err(postgres_decode_error)?,
        error_message: row.try_get("error_message").map_err(postgres_decode_error)?,
    })
}

fn reward_strategy_decision_from_row(
    row: &sqlx::postgres::PgRow,
) -> Result<RewardStrategyDecision> {
    let strategy_profile: String = row
        .try_get("strategy_profile")
        .map_err(postgres_decode_error)?;
    let quote_readiness: String = row
        .try_get("quote_readiness")
        .map_err(postgres_decode_error)?;
    let quote_mode: String = row.try_get("quote_mode").map_err(postgres_decode_error)?;
    let decision_json: Json<Value> = row.try_get("decision_json").map_err(postgres_decode_error)?;
    Ok(RewardStrategyDecision {
        run_id: row.try_get("run_id").map_err(postgres_decode_error)?,
        condition_id: row.try_get("condition_id").map_err(postgres_decode_error)?,
        strategy_profile: RewardStrategyProfile::from_str(&strategy_profile)?,
        decision_rank: row.try_get("decision_rank").map_err(postgres_decode_error)?,
        eligible: row.try_get("eligible").map_err(postgres_decode_error)?,
        quote_readiness: RewardQuoteReadiness::from_str(&quote_readiness)?,
        quote_mode: RewardPlanQuoteMode::from_str(&quote_mode)?,
        score: row.try_get("score").map_err(postgres_decode_error)?,
        selection_score: row.try_get("selection_score").map_err(postgres_decode_error)?,
        reason_code: row.try_get("reason_code").map_err(postgres_decode_error)?,
        reason: row.try_get("reason").map_err(postgres_decode_error)?,
        blocker_codes: row.try_get("blocker_codes").map_err(postgres_decode_error)?,
        planned_buy_notional_usd: row
            .try_get("planned_buy_notional_usd")
            .map_err(postgres_decode_error)?,
        fair_value_passed: row
            .try_get("fair_value_passed")
            .map_err(postgres_decode_error)?,
        fair_value_effective_edge_cents: row
            .try_get("fair_value_effective_edge_cents")
            .map_err(postgres_decode_error)?,
        opportunity_score: row
            .try_get("opportunity_score")
            .map_err(postgres_decode_error)?,
        event_window_status: row
            .try_get("event_window_status")
            .map_err(postgres_decode_error)?,
        ai_action: row.try_get("ai_action").map_err(postgres_decode_error)?,
        info_risk_action: row
            .try_get("info_risk_action")
            .map_err(postgres_decode_error)?,
        info_risk_level: row.try_get("info_risk_level").map_err(postgres_decode_error)?,
        decision_json: decision_json.0,
        created_at: row.try_get("created_at").map_err(postgres_decode_error)?,
    })
}

fn reward_strategy_action_from_row(row: &sqlx::postgres::PgRow) -> Result<RewardStrategyAction> {
    let action_type: String = row.try_get("action_type").map_err(postgres_decode_error)?;
    let status: String = row.try_get("status").map_err(postgres_decode_error)?;
    let request_json: Json<Value> = row.try_get("request_json").map_err(postgres_decode_error)?;
    let result_json: Json<Value> = row.try_get("result_json").map_err(postgres_decode_error)?;
    Ok(RewardStrategyAction {
        action_id: row.try_get("action_id").map_err(postgres_decode_error)?,
        run_id: row.try_get("run_id").map_err(postgres_decode_error)?,
        account_id: row.try_get("account_id").map_err(postgres_decode_error)?,
        condition_id: row.try_get("condition_id").map_err(postgres_decode_error)?,
        token_id: row.try_get("token_id").map_err(postgres_decode_error)?,
        managed_order_id: row.try_get("managed_order_id").map_err(postgres_decode_error)?,
        external_order_id: row.try_get("external_order_id").map_err(postgres_decode_error)?,
        action_type: RewardStrategyActionType::from_str(&action_type)?,
        status: RewardStrategyActionStatus::from_str(&status)?,
        reason_code: row.try_get("reason_code").map_err(postgres_decode_error)?,
        reason: row.try_get("reason").map_err(postgres_decode_error)?,
        idempotency_key: row.try_get("idempotency_key").map_err(postgres_decode_error)?,
        request_json: request_json.0,
        result_json: result_json.0,
        lease_owner: row.try_get("lease_owner").map_err(postgres_decode_error)?,
        lease_expires_at: row
            .try_get("lease_expires_at")
            .map_err(postgres_decode_error)?,
        execution_attempts: row
            .try_get("execution_attempts")
            .map_err(postgres_decode_error)?,
        created_at: row.try_get("created_at").map_err(postgres_decode_error)?,
        updated_at: row.try_get("updated_at").map_err(postgres_decode_error)?,
    })
}

fn reward_order_transition_from_row(
    row: &sqlx::postgres::PgRow,
) -> Result<RewardOrderTransition> {
    let from_status: Option<String> = row.try_get("from_status").map_err(postgres_decode_error)?;
    let to_status: String = row.try_get("to_status").map_err(postgres_decode_error)?;
    let metadata: Json<Value> = row.try_get("metadata_json").map_err(postgres_decode_error)?;
    Ok(RewardOrderTransition {
        transition_id: row.try_get("transition_id").map_err(postgres_decode_error)?,
        run_id: row.try_get("run_id").map_err(postgres_decode_error)?,
        action_id: row.try_get("action_id").map_err(postgres_decode_error)?,
        managed_order_id: row
            .try_get("managed_order_id")
            .map_err(postgres_decode_error)?,
        external_order_id: row
            .try_get("external_order_id")
            .map_err(postgres_decode_error)?,
        from_status: from_status
            .as_deref()
            .map(ManagedRewardOrderStatus::from_str)
            .transpose()?,
        to_status: ManagedRewardOrderStatus::from_str(&to_status)?,
        reason_code: row.try_get("reason_code").map_err(postgres_decode_error)?,
        reason: row.try_get("reason").map_err(postgres_decode_error)?,
        metadata: metadata.0,
        created_at: row.try_get("created_at").map_err(postgres_decode_error)?,
    })
}
