impl PostgresMarketEventStore {
async fn market_event_mark_execution_failed(
        &self,
        execution_request_id: &str,
        failure_code: &str,
        failure_message: &str,
        trace_id: &str,
    ) -> Result<ExecutionDispatchResult> {
        let mut transaction = self.pool.begin().await.map_err(|error| {
            db_error(
                "POSTGRES_TRANSACTION_BEGIN_FAILED",
                format!("failed to begin execution failure transaction: {error}"),
            )
        })?;

        let request_row = sqlx::query(
            r#"
            SELECT
              id,
              signal_id,
              signal_version,
              order_draft_id,
              connector_name,
              mode,
              requested_by_user_id,
              status,
              reason,
              external_order_id,
              submitted_at,
              failure_code,
              failure_message,
              created_at,
              updated_at,
              version
            FROM execution_requests
            WHERE id = $1
            FOR UPDATE
            "#,
        )
        .bind(execution_request_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_QUERY_FAILED",
                format!("failed to lock execution request {execution_request_id}: {error}"),
            )
        })?
        .ok_or_else(|| {
            AppError::not_found(
                "EXECUTION_REQUEST_NOT_FOUND",
                format!("execution request was not found: {execution_request_id}"),
            )
        })?;
        let request = parse_execution_request_row(&request_row)?;

        if request.status != ExecutionRequestStatus::Queued {
            return Err(AppError::conflict(
                "STATE_EXECUTION_REQUEST_NOT_DISPATCHABLE",
                "execution request is no longer queued",
            ));
        }

        let order_draft_row = sqlx::query(
            r#"
            SELECT
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
              version
            FROM order_drafts
            WHERE id = $1
            FOR UPDATE
            "#,
        )
        .bind(&request.order_draft_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_QUERY_FAILED",
                format!(
                    "failed to lock order draft {}: {error}",
                    request.order_draft_id
                ),
            )
        })?
        .ok_or_else(|| {
            AppError::not_found(
                "ORDER_DRAFT_NOT_FOUND",
                format!("order draft was not found: {}", request.order_draft_id),
            )
        })?;
        let order_draft = parse_order_draft_row(&order_draft_row)?;

        if order_draft.status != OrderDraftStatus::Queued {
            return Err(AppError::conflict(
                "STATE_ORDER_DRAFT_NOT_DISPATCHABLE",
                "order draft is no longer queued",
            ));
        }

        let failed_at = OffsetDateTime::now_utc();
        let next_order_draft = OrderDraftView {
            status: OrderDraftStatus::Rejected,
            external_order_id: None,
            submitted_at: None,
            failure_code: Some(failure_code.to_string()),
            failure_message: Some(failure_message.to_string()),
            updated_at: failed_at,
            version: order_draft.version + 1,
            ..order_draft
        };
        let next_request = ExecutionRequestView {
            status: ExecutionRequestStatus::Failed,
            external_order_id: None,
            submitted_at: None,
            failure_code: Some(failure_code.to_string()),
            failure_message: Some(failure_message.to_string()),
            updated_at: failed_at,
            version: request.version + 1,
            ..request
        };

        sqlx::query(
            r#"
            UPDATE order_drafts
            SET
              status = $1,
              external_order_id = NULL,
              submitted_at = NULL,
              failure_code = $2,
              failure_message = $3,
              updated_at = $4,
              version = $5,
              trace_id = $6
            WHERE id = $7
            "#,
        )
        .bind(next_order_draft.status.as_str())
        .bind(next_order_draft.failure_code.as_deref())
        .bind(next_order_draft.failure_message.as_deref())
        .bind(next_order_draft.updated_at)
        .bind(next_order_draft.version)
        .bind(trace_id)
        .bind(&next_order_draft.id)
        .execute(&mut *transaction)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_UPDATE_FAILED",
                format!(
                    "failed to update order draft {}: {error}",
                    next_order_draft.id
                ),
            )
        })?;

        sqlx::query(
            r#"
            UPDATE execution_requests
            SET
              status = $1,
              external_order_id = NULL,
              submitted_at = NULL,
              failure_code = $2,
              failure_message = $3,
              updated_at = $4,
              version = $5,
              trace_id = $6
            WHERE id = $7
            "#,
        )
        .bind(next_request.status.as_str())
        .bind(next_request.failure_code.as_deref())
        .bind(next_request.failure_message.as_deref())
        .bind(next_request.updated_at)
        .bind(next_request.version)
        .bind(trace_id)
        .bind(&next_request.id)
        .execute(&mut *transaction)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_UPDATE_FAILED",
                format!(
                    "failed to update execution request {}: {error}",
                    next_request.id
                ),
            )
        })?;

        transaction.commit().await.map_err(|error| {
            db_error(
                "POSTGRES_TRANSACTION_COMMIT_FAILED",
                format!("failed to commit execution failure transaction: {error}"),
            )
        })?;

        Ok(ExecutionDispatchResult {
            order_draft: next_order_draft,
            execution_request: next_request,
        })
    }
}
