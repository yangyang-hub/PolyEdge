impl PostgresMarketEventStore {
async fn market_event_mark_order_open(&self, order_id: &str, trace_id: &str) -> Result<OrderView> {
        let mut transaction = self.pool.begin().await.map_err(|error| {
            db_error(
                "POSTGRES_TRANSACTION_BEGIN_FAILED",
                format!("failed to begin order status polling transaction: {error}"),
            )
        })?;

        let order_row = sqlx::query(
            r#"
            SELECT
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
              version
            FROM orders
            WHERE id = $1
            FOR UPDATE
            "#,
        )
        .bind(order_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_QUERY_FAILED",
                format!("failed to lock order {order_id}: {error}"),
            )
        })?
        .ok_or_else(|| {
            AppError::not_found(
                "ORDER_NOT_FOUND",
                format!("order was not found: {order_id}"),
            )
        })?;
        let order = parse_order_row(&order_row)?;
        let current_status = order.status;

        let next_order = match current_status {
            OrderStatus::Submitted => OrderView {
                status: OrderStatus::Open,
                updated_at: OffsetDateTime::now_utc(),
                version: order.version + 1,
                ..order
            },
            OrderStatus::Open => order,
            _ => {
                return Err(AppError::conflict(
                    "STATE_ORDER_NOT_POLLABLE",
                    "only submitted/open orders can be polled as open",
                ));
            }
        };

        if next_order.status != current_status {
            sqlx::query(
                r#"
                UPDATE orders
                SET
                  status = $1,
                  updated_at = $2,
                  trace_id = $3,
                  version = $4
                WHERE id = $5
                "#,
            )
            .bind(next_order.status.as_str())
            .bind(next_order.updated_at)
            .bind(trace_id)
            .bind(next_order.version)
            .bind(&next_order.id)
            .execute(&mut *transaction)
            .await
            .map_err(|error| {
                db_error(
                    "POSTGRES_UPDATE_FAILED",
                    format!("failed to update order {} as open: {error}", next_order.id),
                )
            })?;
        }

        transaction.commit().await.map_err(|error| {
            db_error(
                "POSTGRES_TRANSACTION_COMMIT_FAILED",
                format!("failed to commit order status polling transaction: {error}"),
            )
        })?;

        Ok(next_order)
    }
}
