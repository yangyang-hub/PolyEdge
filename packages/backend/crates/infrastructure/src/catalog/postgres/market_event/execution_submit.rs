impl PostgresMarketEventStore {
async fn market_event_submit_execution_request(
        &self,
        command: &SubmitExecutionStoreCommand,
    ) -> Result<ExecutionSubmissionResult> {
        let mut transaction = self.pool.begin().await.map_err(|error| {
            db_error(
                "POSTGRES_TRANSACTION_BEGIN_FAILED",
                format!("failed to begin execution request transaction: {error}"),
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
        let signal = parse_signal_row(&signal_row)?;

        if let Some(expected_signal_version) = command.expected_signal_version {
            if signal.version != expected_signal_version {
                return Err(AppError::conflict(
                    "STATE_VERSION_MISMATCH",
                    "signal version does not match the expected_signal_version",
                ));
            }
        }

        validate_signal_for_execution(&signal)?;

        let existing_request = sqlx::query(
            r#"
            SELECT id
            FROM execution_requests
            WHERE signal_id = $1 AND signal_version = $2
            LIMIT 1
            "#,
        )
        .bind(&signal.id)
        .bind(signal.version)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_QUERY_FAILED",
                format!(
                    "failed to check existing execution request for {}: {error}",
                    signal.id
                ),
            )
        })?;

        if existing_request.is_some() {
            return Err(AppError::conflict(
                "STATE_EXECUTION_REQUEST_ALREADY_EXISTS",
                "an execution request already exists for the current signal version",
            ));
        }

        let now = OffsetDateTime::now_utc();
        let order_draft = OrderDraftView {
            id: format!("odr_{}", Uuid::now_v7()),
            signal_id: signal.id.clone(),
            signal_version: signal.version,
            market_id: signal.market_id.clone(),
            connector_name: command.connector_name.clone(),
            side: signal.side,
            limit_price: command.limit_price,
            quantity: command.quantity,
            notional: compute_order_notional(command.limit_price, command.quantity)?,
            status: OrderDraftStatus::Queued,
            created_by_user_id: command.requested_by_user_id.clone(),
            external_order_id: None,
            submitted_at: None,
            failure_code: None,
            failure_message: None,
            created_at: now,
            updated_at: now,
            version: 1,
        };

        sqlx::query(
            r#"
            INSERT INTO order_drafts (
              id,
              signal_id,
              signal_version,
              market_id,
              connector_name,
              side,
              limit_price,
              quantity,
              notional,
              status,
              created_by_user_id,
              external_order_id,
              submitted_at,
              failure_code,
              failure_message,
              created_at,
              updated_at,
              version,
              trace_id
            )
            VALUES (
              $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19
            )
            "#,
        )
        .bind(&order_draft.id)
        .bind(&order_draft.signal_id)
        .bind(order_draft.signal_version)
        .bind(&order_draft.market_id)
        .bind(&order_draft.connector_name)
        .bind(order_draft.side.as_str())
        .bind(order_draft.limit_price.value())
        .bind(order_draft.quantity.value())
        .bind(order_draft.notional.value())
        .bind(order_draft.status.as_str())
        .bind(&order_draft.created_by_user_id)
        .bind(&order_draft.external_order_id)
        .bind(order_draft.submitted_at)
        .bind(&order_draft.failure_code)
        .bind(&order_draft.failure_message)
        .bind(order_draft.created_at)
        .bind(order_draft.updated_at)
        .bind(order_draft.version)
        .bind(&command.trace_id)
        .execute(&mut *transaction)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_INSERT_FAILED",
                format!("failed to insert order draft {}: {error}", order_draft.id),
            )
        })?;

        let execution_request = ExecutionRequestView {
            id: format!("exr_{}", Uuid::now_v7()),
            signal_id: signal.id,
            signal_version: signal.version,
            order_draft_id: order_draft.id.clone(),
            connector_name: command.connector_name.clone(),
            mode: command.mode,
            requested_by_user_id: command.requested_by_user_id.clone(),
            status: ExecutionRequestStatus::Queued,
            reason: command.reason.clone(),
            external_order_id: None,
            submitted_at: None,
            failure_code: None,
            failure_message: None,
            created_at: now,
            updated_at: now,
            version: 1,
        };

        sqlx::query(
            r#"
            INSERT INTO execution_requests (
              id,
              signal_id,
              signal_version,
              order_draft_id,
              connector_name,
              mode,
              risk_state_version,
              requested_by_user_id,
              status,
              reason,
              external_order_id,
              submitted_at,
              failure_code,
              failure_message,
              created_at,
              updated_at,
              version,
              trace_id
            )
            VALUES (
              $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18
            )
            "#,
        )
        .bind(&execution_request.id)
        .bind(&execution_request.signal_id)
        .bind(execution_request.signal_version)
        .bind(&execution_request.order_draft_id)
        .bind(&execution_request.connector_name)
        .bind(execution_request.mode.as_str())
        .bind(command.risk_state_version)
        .bind(&execution_request.requested_by_user_id)
        .bind(execution_request.status.as_str())
        .bind(&execution_request.reason)
        .bind(&execution_request.external_order_id)
        .bind(execution_request.submitted_at)
        .bind(&execution_request.failure_code)
        .bind(&execution_request.failure_message)
        .bind(execution_request.created_at)
        .bind(execution_request.updated_at)
        .bind(execution_request.version)
        .bind(&command.trace_id)
        .execute(&mut *transaction)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_INSERT_FAILED",
                format!(
                    "failed to insert execution request {}: {error}",
                    execution_request.id
                ),
            )
        })?;

        transaction.commit().await.map_err(|error| {
            db_error(
                "POSTGRES_TRANSACTION_COMMIT_FAILED",
                format!("failed to commit execution request transaction: {error}"),
            )
        })?;

        Ok(ExecutionSubmissionResult {
            order_draft,
            execution_request,
        })
    }

async fn market_event_list_dispatch_candidates(
        &self,
        filters: &DispatchExecutionListFilters,
    ) -> Result<Vec<ExecutionDispatchCandidate>> {
        let rows = sqlx::query(
            r#"
            SELECT
              od.id AS order_draft_id,
              od.signal_id AS order_draft_signal_id,
              od.signal_version AS order_draft_signal_version,
              od.market_id AS order_draft_market_id,
              od.connector_name AS order_draft_connector_name,
              od.side AS order_draft_side,
              od.limit_price AS order_draft_limit_price,
              od.quantity AS order_draft_quantity,
              od.notional AS order_draft_notional,
              od.status AS order_draft_status,
              od.created_by_user_id AS order_draft_created_by_user_id,
              od.external_order_id AS order_draft_external_order_id,
              od.submitted_at AS order_draft_submitted_at,
              od.failure_code AS order_draft_failure_code,
              od.failure_message AS order_draft_failure_message,
              od.created_at AS order_draft_created_at,
              od.updated_at AS order_draft_updated_at,
              od.version AS order_draft_version,
              er.id AS execution_request_id,
              er.signal_id AS execution_request_signal_id,
              er.signal_version AS execution_request_signal_version,
              er.order_draft_id AS execution_request_order_draft_id,
              er.connector_name AS execution_request_connector_name,
              er.mode AS execution_request_mode,
              er.requested_by_user_id AS execution_request_requested_by_user_id,
              er.status AS execution_request_status,
              er.reason AS execution_request_reason,
              er.external_order_id AS execution_request_external_order_id,
              er.submitted_at AS execution_request_submitted_at,
              er.failure_code AS execution_request_failure_code,
              er.failure_message AS execution_request_failure_message,
              er.created_at AS execution_request_created_at,
              er.updated_at AS execution_request_updated_at,
              er.version AS execution_request_version
            FROM execution_requests er
            INNER JOIN order_drafts od ON od.id = er.order_draft_id
            WHERE er.status = 'queued'
              AND od.status = 'queued'
              AND ($1::TEXT IS NULL OR er.connector_name = $1)
            ORDER BY er.created_at ASC, er.id ASC
            LIMIT $2
            "#,
        )
        .bind(filters.connector_name.as_deref())
        .bind(i64::from(filters.limit))
        .fetch_all(&self.pool)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_QUERY_FAILED",
                format!("failed to list dispatch candidates: {error}"),
            )
        })?;

        rows.iter().map(parse_dispatch_candidate_row).collect()
    }

async fn market_event_list_reconciliation_candidates(
        &self,
        filters: &ReconcileExecutionListFilters,
    ) -> Result<Vec<ExecutionReconciliationCandidate>> {
        let rows = sqlx::query(
            r#"
            SELECT
              od.id AS order_draft_id,
              od.signal_id AS order_draft_signal_id,
              od.signal_version AS order_draft_signal_version,
              od.market_id AS order_draft_market_id,
              od.connector_name AS order_draft_connector_name,
              od.side AS order_draft_side,
              od.limit_price AS order_draft_limit_price,
              od.quantity AS order_draft_quantity,
              od.notional AS order_draft_notional,
              od.status AS order_draft_status,
              od.created_by_user_id AS order_draft_created_by_user_id,
              od.external_order_id AS order_draft_external_order_id,
              od.submitted_at AS order_draft_submitted_at,
              od.failure_code AS order_draft_failure_code,
              od.failure_message AS order_draft_failure_message,
              od.created_at AS order_draft_created_at,
              od.updated_at AS order_draft_updated_at,
              od.version AS order_draft_version,
              er.id AS execution_request_id,
              er.signal_id AS execution_request_signal_id,
              er.signal_version AS execution_request_signal_version,
              er.order_draft_id AS execution_request_order_draft_id,
              er.connector_name AS execution_request_connector_name,
              er.mode AS execution_request_mode,
              er.requested_by_user_id AS execution_request_requested_by_user_id,
              er.status AS execution_request_status,
              er.reason AS execution_request_reason,
              er.external_order_id AS execution_request_external_order_id,
              er.submitted_at AS execution_request_submitted_at,
              er.failure_code AS execution_request_failure_code,
              er.failure_message AS execution_request_failure_message,
              er.created_at AS execution_request_created_at,
              er.updated_at AS execution_request_updated_at,
              er.version AS execution_request_version,
              o.id AS order_id,
              o.signal_id AS order_signal_id,
              o.execution_request_id AS order_execution_request_id,
              o.order_draft_id AS order_order_draft_id,
              o.market_id AS order_market_id,
              o.connector_name AS order_connector_name,
              o.account_id AS order_account_id,
              o.external_order_id AS order_external_order_id,
              o.side AS order_side,
              o.limit_price AS order_limit_price,
              o.quantity AS order_quantity,
              o.filled_quantity AS order_filled_quantity,
              o.avg_fill_price AS order_avg_fill_price,
              o.status AS order_status,
              o.submitted_at AS order_submitted_at,
              o.updated_at AS order_updated_at,
              o.version AS order_version
            FROM execution_requests er
            INNER JOIN order_drafts od ON od.id = er.order_draft_id
            LEFT JOIN orders o ON o.execution_request_id = er.id
            WHERE er.status = 'submitted'
              AND od.status = 'submitted'
              AND (
                o.id IS NULL
                OR (
                  o.status IN ('submitted', 'open', 'partially_filled')
                  AND o.filled_quantity < o.quantity
                )
              )
              AND ($1::TEXT IS NULL OR er.connector_name = $1)
            ORDER BY COALESCE(o.updated_at, er.updated_at) ASC, er.id ASC
            LIMIT $2
            "#,
        )
        .bind(filters.connector_name.as_deref())
        .bind(i64::from(filters.limit))
        .fetch_all(&self.pool)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_QUERY_FAILED",
                format!("failed to list reconciliation candidates: {error}"),
            )
        })?;

        rows.iter()
            .map(parse_reconciliation_candidate_row)
            .collect()
    }
}
