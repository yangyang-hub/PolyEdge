impl PostgresMarketEventStore {
async fn market_event_recompute_signal(
        &self,
        command: &RecomputeSignalCommand,
    ) -> Result<RecomputeSignalResult> {
        let mut transaction = self.pool.begin().await.map_err(|error| {
            db_error(
                "POSTGRES_TRANSACTION_BEGIN_FAILED",
                format!("failed to begin signal recompute transaction: {error}"),
            )
        })?;

        let signal_row = sqlx::query(
            r#"
            SELECT
              s.id,
              s.market_id,
              s.event_id,
              s.action,
              s.side,
              s.market_price,
              s.fair_price,
              s.edge,
              s.confidence,
              s.lifecycle_state,
              s.reason,
              s.risk_decision,
              s.approved_by_user_id,
              s.approved_at,
              s.rejected_by_user_id,
              s.rejected_at,
              s.updated_at,
              s.version,
              COALESCE(
                array_agg(sel.evidence_id ORDER BY sel.evidence_id)
                FILTER (WHERE sel.evidence_id IS NOT NULL),
                '{}'::TEXT[]
              ) AS evidence_ids
            FROM signals s
            LEFT JOIN signal_evidence_links sel ON sel.signal_id = s.id
            WHERE s.id = $1
            GROUP BY
              s.id,
              s.market_id,
              s.event_id,
              s.action,
              s.side,
              s.market_price,
              s.fair_price,
              s.edge,
              s.confidence,
              s.lifecycle_state,
              s.reason,
              s.risk_decision,
              s.approved_by_user_id,
              s.approved_at,
              s.rejected_by_user_id,
              s.rejected_at,
              s.updated_at,
              s.version
            FOR UPDATE
            "#,
        )
        .bind(&command.signal_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_QUERY_FAILED",
                format!("failed to lock signal {}: {error}", command.signal_id),
            )
        })?
        .ok_or_else(|| {
            AppError::not_found(
                "SIGNAL_NOT_FOUND",
                format!("signal was not found: {}", command.signal_id),
            )
        })?;
        let current_signal = parse_signal_row(&signal_row)?;

        let market = fetch_market_by_id(&mut transaction, &current_signal.market_id)
            .await?
            .ok_or_else(|| {
                AppError::not_found(
                    "MARKET_NOT_FOUND",
                    format!("market was not found: {}", current_signal.market_id),
                )
            })?;

        let evidences = fetch_evidences_for_signal(
            &mut transaction,
            &current_signal.market_id,
            &current_signal.event_id,
        )
        .await?;
        let source_health =
            fetch_source_health_adjustment_for_event(&mut transaction, &current_signal.event_id)
                .await?;
        let estimate_id = format!("est_{}", Uuid::now_v7());
        let draft = build_recompute_signal_draft_with_source_health(
            &current_signal,
            &market,
            &evidences,
            &command.reason,
            source_health.as_ref(),
            &estimate_id,
        )?;

        sqlx::query(
            r#"
            INSERT INTO probability_estimates (
              id,
              market_id,
              event_id,
              signal_id,
              prior_price,
              posterior_price,
              fair_price,
              market_price,
              edge,
              confidence,
              time_horizon,
              model_version,
              reason_codes_json,
              evidence_count,
              trace_id,
              created_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16)
            "#,
        )
        .bind(&draft.estimate.id)
        .bind(&draft.estimate.market_id)
        .bind(&draft.estimate.event_id)
        .bind(draft.estimate.signal_id.as_deref())
        .bind(draft.estimate.prior_price.value())
        .bind(draft.estimate.posterior_price.value())
        .bind(draft.estimate.fair_price.value())
        .bind(draft.estimate.market_price.value())
        .bind(draft.estimate.edge.value())
        .bind(draft.estimate.confidence.value())
        .bind(draft.estimate.time_horizon.as_str())
        .bind(&draft.estimate.model_version)
        .bind(Json(draft.estimate.reason_codes.clone()))
        .bind(i32::try_from(draft.estimate.evidence_count).unwrap_or(i32::MAX))
        .bind(&command.trace_id)
        .bind(draft.estimate.created_at)
        .execute(&mut *transaction)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_INSERT_FAILED",
                format!("failed to insert probability estimate: {error}"),
            )
        })?;

        sqlx::query(
            r#"
            UPDATE signals
            SET
              action = $1,
              side = $2,
              market_price = $3,
              fair_price = $4,
              edge = $5,
              confidence = $6,
              lifecycle_state = $7,
              reason = $8,
              risk_decision = $9,
              approved_by_user_id = NULL,
              approved_at = NULL,
              rejected_by_user_id = NULL,
              rejected_at = NULL,
              estimate_id = $10,
              updated_at = $11,
              version = $12,
              trace_id = $13
            WHERE id = $14
            "#,
        )
        .bind(draft.next_signal.action.as_str())
        .bind(draft.next_signal.side.as_str())
        .bind(draft.next_signal.market_price.value())
        .bind(draft.next_signal.fair_price.value())
        .bind(draft.next_signal.edge.value())
        .bind(draft.next_signal.confidence.value())
        .bind(draft.next_signal.lifecycle_state.as_str())
        .bind(&draft.next_signal.reason)
        .bind(&draft.next_signal.risk_decision)
        .bind(&draft.estimate.id)
        .bind(draft.next_signal.updated_at)
        .bind(draft.next_signal.version)
        .bind(&command.trace_id)
        .bind(&draft.next_signal.id)
        .execute(&mut *transaction)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_UPDATE_FAILED",
                format!("failed to update signal {}: {error}", draft.next_signal.id),
            )
        })?;

        sqlx::query(
            r#"
            DELETE FROM signal_evidence_links
            WHERE signal_id = $1
            "#,
        )
        .bind(&draft.next_signal.id)
        .execute(&mut *transaction)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_DELETE_FAILED",
                format!(
                    "failed to reset signal evidence links for {}: {error}",
                    draft.next_signal.id
                ),
            )
        })?;

        for evidence_id in &draft.next_signal.evidence_ids {
            sqlx::query(
                r#"
                INSERT INTO signal_evidence_links (signal_id, evidence_id, created_at)
                VALUES ($1, $2, $3)
                ON CONFLICT (signal_id, evidence_id) DO NOTHING
                "#,
            )
            .bind(&draft.next_signal.id)
            .bind(evidence_id)
            .bind(draft.next_signal.updated_at)
            .execute(&mut *transaction)
            .await
            .map_err(|error| {
                db_error(
                    "POSTGRES_INSERT_FAILED",
                    format!(
                        "failed to insert signal-evidence link {} -> {}: {error}",
                        draft.next_signal.id, evidence_id
                    ),
                )
            })?;
        }

        let transition = if let Some(transition) = draft.transition {
            let view = SignalTransitionView {
                id: format!("sgt_{}", Uuid::now_v7()),
                signal_id: draft.next_signal.id.clone(),
                from_state: transition.from_state,
                to_state: transition.to_state,
                trigger_type: transition.trigger_type,
                trigger_payload: transition.trigger_payload,
                created_at: transition.created_at,
            };

            sqlx::query(
                r#"
                INSERT INTO signal_transitions (
                  id,
                  signal_id,
                  from_state,
                  to_state,
                  trigger_type,
                  trigger_payload_json,
                  trace_id,
                  created_at
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
                "#,
            )
            .bind(&view.id)
            .bind(&view.signal_id)
            .bind(view.from_state.as_str())
            .bind(view.to_state.as_str())
            .bind(&view.trigger_type)
            .bind(Json(view.trigger_payload.clone()))
            .bind(&command.trace_id)
            .bind(view.created_at)
            .execute(&mut *transaction)
            .await
            .map_err(|error| {
                db_error(
                    "POSTGRES_INSERT_FAILED",
                    format!("failed to insert signal transition: {error}"),
                )
            })?;

            Some(view)
        } else {
            None
        };

        transaction.commit().await.map_err(|error| {
            db_error(
                "POSTGRES_TRANSACTION_COMMIT_FAILED",
                format!("failed to commit signal recompute transaction: {error}"),
            )
        })?;

        Ok(RecomputeSignalResult {
            signal: draft.next_signal,
            estimate: draft.estimate,
            transition,
        })
    }

async fn market_event_approve_signal(
        &self,
        signal_id: &str,
        approved_by_user_id: &str,
        approval_reason: &str,
        trace_id: &str,
        expected_version: Option<i64>,
    ) -> Result<SignalView> {
        let mut transaction = self.pool.begin().await.map_err(|error| {
            db_error(
                "POSTGRES_TRANSACTION_BEGIN_FAILED",
                format!("failed to begin signal approval transaction: {error}"),
            )
        })?;

        let signal_row = sqlx::query(
            r#"
            SELECT
              s.id,
              s.market_id,
              s.event_id,
              s.action,
              s.side,
              s.market_price,
              s.fair_price,
              s.edge,
              s.confidence,
              s.lifecycle_state,
              s.reason,
              s.risk_decision,
              s.approved_by_user_id,
              s.approved_at,
              s.rejected_by_user_id,
              s.rejected_at,
              s.updated_at,
              s.version,
              COALESCE(
                array_agg(sel.evidence_id ORDER BY sel.evidence_id)
                FILTER (WHERE sel.evidence_id IS NOT NULL),
                '{}'::TEXT[]
              ) AS evidence_ids
            FROM signals s
            LEFT JOIN signal_evidence_links sel ON sel.signal_id = s.id
            WHERE s.id = $1
            GROUP BY
              s.id,
              s.market_id,
              s.event_id,
              s.action,
              s.side,
              s.market_price,
              s.fair_price,
              s.edge,
              s.confidence,
              s.lifecycle_state,
              s.reason,
              s.risk_decision,
              s.approved_by_user_id,
              s.approved_at,
              s.rejected_by_user_id,
              s.rejected_at,
              s.updated_at,
              s.version
            FOR UPDATE
            "#,
        )
        .bind(signal_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_QUERY_FAILED",
                format!("failed to lock signal {signal_id}: {error}"),
            )
        })?
        .ok_or_else(|| {
            AppError::not_found(
                "SIGNAL_NOT_FOUND",
                format!("signal was not found: {signal_id}"),
            )
        })?;
        let current_signal = parse_signal_row(&signal_row)?;

        if let Some(expected_version) = expected_version {
            if current_signal.version != expected_version {
                return Err(AppError::conflict(
                    "STATE_VERSION_MISMATCH",
                    "signal version does not match the expected_version",
                ));
            }
        }

        if current_signal.approved_by_user_id.is_some() {
            return Err(AppError::conflict(
                "STATE_SIGNAL_ALREADY_APPROVED",
                "signal has already been approved",
            ));
        }

        if current_signal.rejected_by_user_id.is_some() {
            return Err(AppError::conflict(
                "STATE_SIGNAL_ALREADY_REJECTED",
                "signal has already been rejected for the current version",
            ));
        }

        let approved_at = OffsetDateTime::now_utc();
        let next_signal = SignalView {
            id: current_signal.id.clone(),
            market_id: current_signal.market_id.clone(),
            event_id: current_signal.event_id.clone(),
            action: current_signal.action,
            side: current_signal.side,
            market_price: current_signal.market_price,
            fair_price: current_signal.fair_price,
            edge: current_signal.edge,
            confidence: current_signal.confidence,
            lifecycle_state: current_signal.lifecycle_state,
            reason: current_signal.reason.clone(),
            risk_decision: approval_reason.to_string(),
            evidence_ids: current_signal.evidence_ids.clone(),
            approved_by_user_id: Some(approved_by_user_id.to_string()),
            approved_at: Some(approved_at),
            rejected_by_user_id: None,
            rejected_at: None,
            updated_at: approved_at,
            version: current_signal.version + 1,
        };

        sqlx::query(
            r#"
            UPDATE signals
            SET
              risk_decision = $1,
              approved_by_user_id = $2,
              approved_at = $3,
              rejected_by_user_id = NULL,
              rejected_at = NULL,
              updated_at = $4,
              version = $5,
              trace_id = $6
            WHERE id = $7
            "#,
        )
        .bind(&next_signal.risk_decision)
        .bind(next_signal.approved_by_user_id.as_deref())
        .bind(next_signal.approved_at)
        .bind(next_signal.updated_at)
        .bind(next_signal.version)
        .bind(trace_id)
        .bind(signal_id)
        .execute(&mut *transaction)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_UPDATE_FAILED",
                format!("failed to approve signal {signal_id}: {error}"),
            )
        })?;

        transaction.commit().await.map_err(|error| {
            db_error(
                "POSTGRES_TRANSACTION_COMMIT_FAILED",
                format!("failed to commit signal approval transaction: {error}"),
            )
        })?;

        Ok(next_signal)
    }

async fn market_event_reject_signal(
        &self,
        signal_id: &str,
        rejected_by_user_id: &str,
        rejection_reason: &str,
        trace_id: &str,
        expected_version: Option<i64>,
    ) -> Result<SignalView> {
        let mut transaction = self.pool.begin().await.map_err(|error| {
            db_error(
                "POSTGRES_TRANSACTION_BEGIN_FAILED",
                format!("failed to begin signal rejection transaction: {error}"),
            )
        })?;

        let signal_row = sqlx::query(
            r#"
            SELECT
              s.id,
              s.market_id,
              s.event_id,
              s.action,
              s.side,
              s.market_price,
              s.fair_price,
              s.edge,
              s.confidence,
              s.lifecycle_state,
              s.reason,
              s.risk_decision,
              s.approved_by_user_id,
              s.approved_at,
              s.rejected_by_user_id,
              s.rejected_at,
              s.updated_at,
              s.version,
              COALESCE(
                array_agg(sel.evidence_id ORDER BY sel.evidence_id)
                FILTER (WHERE sel.evidence_id IS NOT NULL),
                '{}'::TEXT[]
              ) AS evidence_ids
            FROM signals s
            LEFT JOIN signal_evidence_links sel ON sel.signal_id = s.id
            WHERE s.id = $1
            GROUP BY
              s.id,
              s.market_id,
              s.event_id,
              s.action,
              s.side,
              s.market_price,
              s.fair_price,
              s.edge,
              s.confidence,
              s.lifecycle_state,
              s.reason,
              s.risk_decision,
              s.approved_by_user_id,
              s.approved_at,
              s.rejected_by_user_id,
              s.rejected_at,
              s.updated_at,
              s.version
            FOR UPDATE
            "#,
        )
        .bind(signal_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_QUERY_FAILED",
                format!("failed to lock signal {signal_id}: {error}"),
            )
        })?
        .ok_or_else(|| {
            AppError::not_found(
                "SIGNAL_NOT_FOUND",
                format!("signal was not found: {signal_id}"),
            )
        })?;
        let current_signal = parse_signal_row(&signal_row)?;

        if let Some(expected_version) = expected_version {
            if current_signal.version != expected_version {
                return Err(AppError::conflict(
                    "STATE_VERSION_MISMATCH",
                    "signal version does not match the expected_version",
                ));
            }
        }

        if current_signal.approved_by_user_id.is_some() {
            return Err(AppError::conflict(
                "STATE_SIGNAL_ALREADY_APPROVED",
                "approved signals cannot be rejected for the current version",
            ));
        }

        if current_signal.rejected_by_user_id.is_some() {
            return Err(AppError::conflict(
                "STATE_SIGNAL_ALREADY_REJECTED",
                "signal has already been rejected for the current version",
            ));
        }

        let rejected_at = OffsetDateTime::now_utc();
        let next_signal = SignalView {
            id: current_signal.id.clone(),
            market_id: current_signal.market_id.clone(),
            event_id: current_signal.event_id.clone(),
            action: current_signal.action,
            side: current_signal.side,
            market_price: current_signal.market_price,
            fair_price: current_signal.fair_price,
            edge: current_signal.edge,
            confidence: current_signal.confidence,
            lifecycle_state: current_signal.lifecycle_state,
            reason: current_signal.reason.clone(),
            risk_decision: rejection_reason.to_string(),
            evidence_ids: current_signal.evidence_ids.clone(),
            approved_by_user_id: None,
            approved_at: None,
            rejected_by_user_id: Some(rejected_by_user_id.to_string()),
            rejected_at: Some(rejected_at),
            updated_at: rejected_at,
            version: current_signal.version + 1,
        };

        sqlx::query(
            r#"
            UPDATE signals
            SET
              risk_decision = $1,
              approved_by_user_id = NULL,
              approved_at = NULL,
              rejected_by_user_id = $2,
              rejected_at = $3,
              updated_at = $4,
              version = $5,
              trace_id = $6
            WHERE id = $7
            "#,
        )
        .bind(&next_signal.risk_decision)
        .bind(next_signal.rejected_by_user_id.as_deref())
        .bind(next_signal.rejected_at)
        .bind(next_signal.updated_at)
        .bind(next_signal.version)
        .bind(trace_id)
        .bind(signal_id)
        .execute(&mut *transaction)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_UPDATE_FAILED",
                format!("failed to reject signal {signal_id}: {error}"),
            )
        })?;

        transaction.commit().await.map_err(|error| {
            db_error(
                "POSTGRES_TRANSACTION_COMMIT_FAILED",
                format!("failed to commit signal rejection transaction: {error}"),
            )
        })?;

        Ok(next_signal)
    }
}
