impl PostgresMarketEventStore {
async fn market_event_mark_execution_submitted(
        &self,
        execution_request_id: &str,
        account_id: &str,
        external_order_id: &str,
        trace_id: &str,
    ) -> Result<ExecutionDispatchResult> {
        let mut transaction = self.pool.begin().await.map_err(|error| {
            db_error(
                "POSTGRES_TRANSACTION_BEGIN_FAILED",
                format!("failed to begin execution dispatch transaction: {error}"),
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

        let submitted_at = OffsetDateTime::now_utc();
        let next_order_draft = OrderDraftView {
            status: OrderDraftStatus::Submitted,
            external_order_id: Some(external_order_id.to_string()),
            submitted_at: Some(submitted_at),
            failure_code: None,
            failure_message: None,
            updated_at: submitted_at,
            version: order_draft.version + 1,
            ..order_draft
        };
        let next_request = ExecutionRequestView {
            status: ExecutionRequestStatus::Submitted,
            external_order_id: Some(external_order_id.to_string()),
            submitted_at: Some(submitted_at),
            failure_code: None,
            failure_message: None,
            updated_at: submitted_at,
            version: request.version + 1,
            ..request
        };
        let submitted_order = OrderView {
            id: format!("ord_{}", Uuid::now_v7()),
            signal_id: next_request.signal_id.clone(),
            execution_request_id: next_request.id.clone(),
            order_draft_id: next_order_draft.id.clone(),
            market_id: next_order_draft.market_id.clone(),
            connector_name: next_request.connector_name.clone(),
            account_id: account_id.to_string(),
            external_order_id: external_order_id.to_string(),
            side: next_order_draft.side,
            limit_price: next_order_draft.limit_price,
            quantity: next_order_draft.quantity,
            filled_quantity: Quantity::new(Decimal::ZERO)?,
            avg_fill_price: Probability::new(Decimal::ZERO)?,
            status: OrderStatus::Submitted,
            submitted_at,
            updated_at: submitted_at,
            version: 1,
        };

        sqlx::query(
            r#"
            UPDATE order_drafts
            SET
              status = $1,
              external_order_id = $2,
              submitted_at = $3,
              failure_code = NULL,
              failure_message = NULL,
              updated_at = $4,
              version = $5,
              trace_id = $6
            WHERE id = $7
            "#,
        )
        .bind(next_order_draft.status.as_str())
        .bind(next_order_draft.external_order_id.as_deref())
        .bind(next_order_draft.submitted_at)
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
              external_order_id = $2,
              submitted_at = $3,
              failure_code = NULL,
              failure_message = NULL,
              updated_at = $4,
              version = $5,
              trace_id = $6
            WHERE id = $7
            "#,
        )
        .bind(next_request.status.as_str())
        .bind(next_request.external_order_id.as_deref())
        .bind(next_request.submitted_at)
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

        sqlx::query(
            r#"
            INSERT INTO orders (
              id,
              signal_id,
              execution_request_id,
              order_draft_id,
              market_id,
              connector_name,
              account_id,
              external_order_id,
              side,
              limit_price,
              quantity,
              filled_quantity,
              avg_fill_price,
              status,
              submitted_at,
              updated_at,
              trace_id,
              version
            )
            VALUES (
              $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18
            )
            "#,
        )
        .bind(&submitted_order.id)
        .bind(&submitted_order.signal_id)
        .bind(&submitted_order.execution_request_id)
        .bind(&submitted_order.order_draft_id)
        .bind(&submitted_order.market_id)
        .bind(&submitted_order.connector_name)
        .bind(&submitted_order.account_id)
        .bind(&submitted_order.external_order_id)
        .bind(submitted_order.side.as_str())
        .bind(submitted_order.limit_price.value())
        .bind(submitted_order.quantity.value())
        .bind(submitted_order.filled_quantity.value())
        .bind(submitted_order.avg_fill_price.value())
        .bind(submitted_order.status.as_str())
        .bind(submitted_order.submitted_at)
        .bind(submitted_order.updated_at)
        .bind(trace_id)
        .bind(submitted_order.version)
        .execute(&mut *transaction)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_INSERT_FAILED",
                format!(
                    "failed to insert submitted order {}: {error}",
                    submitted_order.id
                ),
            )
        })?;

        transaction.commit().await.map_err(|error| {
            db_error(
                "POSTGRES_TRANSACTION_COMMIT_FAILED",
                format!("failed to commit execution dispatch transaction: {error}"),
            )
        })?;

        Ok(ExecutionDispatchResult {
            order_draft: next_order_draft,
            execution_request: next_request,
        })
    }
}
