impl PostgresMarketEventStore {
async fn market_event_reconcile_execution_fill(
        &self,
        input: MarketEventExecutionFill<'_>,
    ) -> Result<ExecutionFillResult> {
        let MarketEventExecutionFill {
            execution_request_id,
            account_id,
            external_trade_id,
            fill_price,
            filled_quantity,
            fee,
            trace_id,
        } = input;

        let mut transaction = self.pool.begin().await.map_err(|error| {
            db_error(
                "POSTGRES_TRANSACTION_BEGIN_FAILED",
                format!("failed to begin execution reconciliation transaction: {error}"),
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
                format!(
                    "failed to lock reconciliation execution request {execution_request_id}: {error}"
                ),
            )
        })?
        .ok_or_else(|| {
            AppError::not_found(
                "EXECUTION_REQUEST_NOT_FOUND",
                format!("execution request was not found: {execution_request_id}"),
            )
        })?;
        let request = parse_execution_request_row(&request_row)?;

        if request.status != ExecutionRequestStatus::Submitted {
            return Err(AppError::conflict(
                "STATE_EXECUTION_REQUEST_NOT_RECONCILABLE",
                "execution request is not in submitted state",
            ));
        }

        let existing_order_row = sqlx::query(
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
            WHERE execution_request_id = $1
            FOR UPDATE
            "#,
        )
        .bind(&request.id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_QUERY_FAILED",
                format!(
                    "failed to check existing reconciled order for execution request {}: {error}",
                    request.id
                ),
            )
        })?;
        let existing_order = existing_order_row
            .as_ref()
            .map(parse_order_row)
            .transpose()?;

        let existing_trade_row = sqlx::query(
            r#"
            SELECT id
            FROM trades
            WHERE connector_name = $1
              AND external_trade_id = $2
            LIMIT 1
            "#,
        )
        .bind(&request.connector_name)
        .bind(external_trade_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_QUERY_FAILED",
                format!(
                    "failed to check existing trade {} for connector {}: {error}",
                    external_trade_id, request.connector_name
                ),
            )
        })?;
        if existing_trade_row.is_some() {
            return Err(AppError::conflict(
                "STATE_TRADE_ALREADY_RECORDED",
                "external trade id has already been recorded",
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
                    "failed to lock order draft {} for reconciliation: {error}",
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

        if order_draft.status != OrderDraftStatus::Submitted {
            return Err(AppError::conflict(
                "STATE_ORDER_DRAFT_NOT_RECONCILABLE",
                "order draft is not in submitted state",
            ));
        }

        let external_order_id = request
            .external_order_id
            .clone()
            .or_else(|| order_draft.external_order_id.clone())
            .ok_or_else(|| {
                AppError::conflict(
                    "STATE_EXTERNAL_ORDER_ID_MISSING",
                    "submitted execution request is missing external_order_id",
                )
            })?;

        let signal_row = sqlx::query(
            r#"
            SELECT id, lifecycle_state, updated_at, version
            FROM signals
            WHERE id = $1
            FOR UPDATE
            "#,
        )
        .bind(&request.signal_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_QUERY_FAILED",
                format!("failed to lock signal {}: {error}", request.signal_id),
            )
        })?
        .ok_or_else(|| {
            AppError::not_found(
                "SIGNAL_NOT_FOUND",
                format!("signal was not found: {}", request.signal_id),
            )
        })?;
        let current_signal_state = SignalLifecycleState::from_str(&decode_column::<String>(
            &signal_row,
            "lifecycle_state",
        )?)?;
        let current_signal_version: i64 = decode_column(&signal_row, "version")?;

        let position_row = sqlx::query(
            r#"
            SELECT
              id,
              market_id,
              connector_name,
              account_id,
              side,
              net_quantity,
              avg_cost,
              mark_price,
              unrealized_pnl,
              realized_pnl,
              updated_at,
              version
            FROM positions
            WHERE connector_name = $1
              AND account_id = $2
              AND market_id = $3
              AND side = $4
            FOR UPDATE
            "#,
        )
        .bind(&request.connector_name)
        .bind(account_id)
        .bind(&order_draft.market_id)
        .bind(order_draft.side.as_str())
        .fetch_optional(&mut *transaction)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_QUERY_FAILED",
                format!(
                    "failed to lock position for connector={} account={} market={} side={}: {error}",
                    request.connector_name,
                    account_id,
                    order_draft.market_id,
                    order_draft.side.as_str(),
                ),
            )
        })?;

        let now = OffsetDateTime::now_utc();
        let submitted_at = request
            .submitted_at
            .or(order_draft.submitted_at)
            .unwrap_or(now);
        let order = if let Some(current) = existing_order {
            if !matches!(
                current.status,
                OrderStatus::Submitted | OrderStatus::Open | OrderStatus::PartiallyFilled
            ) {
                return Err(AppError::conflict(
                    "STATE_ORDER_NOT_RECONCILABLE",
                    "existing order is not in a reconcilable state",
                ));
            }

            let next_filled_quantity_value =
                current.filled_quantity.value() + filled_quantity.value();
            if next_filled_quantity_value > current.quantity.value() {
                return Err(AppError::conflict(
                    "STATE_FILL_QUANTITY_EXCEEDS_ORDER",
                    "filled quantity exceeds order quantity",
                ));
            }

            let next_filled_quantity = Quantity::new(next_filled_quantity_value)?;
            let next_avg_fill_price = weighted_fill_price(
                current.avg_fill_price,
                current.filled_quantity,
                fill_price,
                filled_quantity,
            )?;
            let next_status = if next_filled_quantity.value() == current.quantity.value() {
                OrderStatus::Filled
            } else {
                OrderStatus::PartiallyFilled
            };

            OrderView {
                filled_quantity: next_filled_quantity,
                avg_fill_price: next_avg_fill_price,
                status: next_status,
                updated_at: now,
                version: current.version + 1,
                ..current
            }
        } else {
            if filled_quantity.value() > order_draft.quantity.value() {
                return Err(AppError::conflict(
                    "STATE_FILL_QUANTITY_EXCEEDS_ORDER",
                    "filled quantity exceeds queued order quantity",
                ));
            }

            let next_status = if filled_quantity.value() == order_draft.quantity.value() {
                OrderStatus::Filled
            } else {
                OrderStatus::PartiallyFilled
            };

            OrderView {
                id: format!("ord_{}", Uuid::now_v7()),
                signal_id: request.signal_id.clone(),
                execution_request_id: request.id.clone(),
                order_draft_id: order_draft.id.clone(),
                market_id: order_draft.market_id.clone(),
                connector_name: request.connector_name.clone(),
                account_id: account_id.to_string(),
                external_order_id,
                side: order_draft.side,
                limit_price: order_draft.limit_price,
                quantity: order_draft.quantity,
                filled_quantity,
                avg_fill_price: fill_price,
                status: next_status,
                submitted_at,
                updated_at: now,
                version: 1,
            }
        };
        let trade = TradeView {
            id: format!("trd_{}", Uuid::now_v7()),
            order_id: order.id.clone(),
            signal_id: order.signal_id.clone(),
            market_id: order.market_id.clone(),
            connector_name: order.connector_name.clone(),
            external_trade_id: external_trade_id.to_string(),
            side: order.side,
            price: fill_price,
            quantity: filled_quantity,
            fee,
            executed_at: now,
        };
        let position_key = in_memory_position_key(
            &order.connector_name,
            account_id,
            &order.market_id,
            order.side,
        );
        let position = if let Some(row) = position_row.as_ref() {
            build_next_position(
                parse_position_row(row)?,
                filled_quantity,
                fill_price,
                trace_id,
            )?
        } else {
            PositionView {
                id: position_key,
                market_id: order.market_id.clone(),
                connector_name: order.connector_name.clone(),
                account_id: account_id.to_string(),
                side: order.side,
                net_quantity: filled_quantity,
                avg_cost: fill_price,
                mark_price: fill_price,
                unrealized_pnl: SignedUsdAmount::new(Decimal::ZERO)?,
                realized_pnl: SignedUsdAmount::new(Decimal::ZERO)?,
                updated_at: now,
                version: 1,
            }
        };

        if existing_order_row.is_some() {
            sqlx::query(
                r#"
                UPDATE orders
                SET
                  filled_quantity = $1,
                  avg_fill_price = $2,
                  status = $3,
                  updated_at = $4,
                  trace_id = $5,
                  version = $6
                WHERE id = $7
                "#,
            )
            .bind(order.filled_quantity.value())
            .bind(order.avg_fill_price.value())
            .bind(order.status.as_str())
            .bind(order.updated_at)
            .bind(trace_id)
            .bind(order.version)
            .bind(&order.id)
            .execute(&mut *transaction)
            .await
            .map_err(|error| {
                db_error(
                    "POSTGRES_UPDATE_FAILED",
                    format!("failed to update reconciled order {}: {error}", order.id),
                )
            })?;
        } else {
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
            .bind(&order.id)
            .bind(&order.signal_id)
            .bind(&order.execution_request_id)
            .bind(&order.order_draft_id)
            .bind(&order.market_id)
            .bind(&order.connector_name)
            .bind(&order.account_id)
            .bind(&order.external_order_id)
            .bind(order.side.as_str())
            .bind(order.limit_price.value())
            .bind(order.quantity.value())
            .bind(order.filled_quantity.value())
            .bind(order.avg_fill_price.value())
            .bind(order.status.as_str())
            .bind(order.submitted_at)
            .bind(order.updated_at)
            .bind(trace_id)
            .bind(order.version)
            .execute(&mut *transaction)
            .await
            .map_err(|error| {
                db_error(
                    "POSTGRES_INSERT_FAILED",
                    format!("failed to insert reconciled order {}: {error}", order.id),
                )
            })?;
        }

        sqlx::query(
            r#"
            INSERT INTO trades (
              id,
              order_id,
              signal_id,
              market_id,
              connector_name,
              external_trade_id,
              side,
              price,
              quantity,
              fee,
              executed_at,
              trace_id,
              created_at
            )
            VALUES (
              $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13
            )
            "#,
        )
        .bind(&trade.id)
        .bind(&trade.order_id)
        .bind(&trade.signal_id)
        .bind(&trade.market_id)
        .bind(&trade.connector_name)
        .bind(&trade.external_trade_id)
        .bind(trade.side.as_str())
        .bind(trade.price.value())
        .bind(trade.quantity.value())
        .bind(trade.fee.value())
        .bind(trade.executed_at)
        .bind(trace_id)
        .bind(now)
        .execute(&mut *transaction)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_INSERT_FAILED",
                format!("failed to insert reconciled trade {}: {error}", trade.id),
            )
        })?;

        if position_row.is_some() {
            sqlx::query(
                r#"
                UPDATE positions
                SET
                  net_quantity = $1,
                  avg_cost = $2,
                  mark_price = $3,
                  unrealized_pnl = $4,
                  realized_pnl = $5,
                  updated_at = $6,
                  trace_id = $7,
                  version = $8
                WHERE id = $9
                "#,
            )
            .bind(position.net_quantity.value())
            .bind(position.avg_cost.value())
            .bind(position.mark_price.value())
            .bind(position.unrealized_pnl.value())
            .bind(position.realized_pnl.value())
            .bind(position.updated_at)
            .bind(trace_id)
            .bind(position.version)
            .bind(&position.id)
            .execute(&mut *transaction)
            .await
            .map_err(|error| {
                db_error(
                    "POSTGRES_UPDATE_FAILED",
                    format!("failed to update position {}: {error}", position.id),
                )
            })?;
        } else {
            sqlx::query(
                r#"
                INSERT INTO positions (
                  id,
                  market_id,
                  connector_name,
                  account_id,
                  side,
                  net_quantity,
                  avg_cost,
                  mark_price,
                  unrealized_pnl,
                  realized_pnl,
                  updated_at,
                  trace_id,
                  version
                )
                VALUES (
                  $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13
                )
                "#,
            )
            .bind(&position.id)
            .bind(&position.market_id)
            .bind(&position.connector_name)
            .bind(&position.account_id)
            .bind(position.side.as_str())
            .bind(position.net_quantity.value())
            .bind(position.avg_cost.value())
            .bind(position.mark_price.value())
            .bind(position.unrealized_pnl.value())
            .bind(position.realized_pnl.value())
            .bind(position.updated_at)
            .bind(trace_id)
            .bind(position.version)
            .execute(&mut *transaction)
            .await
            .map_err(|error| {
                db_error(
                    "POSTGRES_INSERT_FAILED",
                    format!("failed to insert position {}: {error}", position.id),
                )
            })?;
        }

        if current_signal_state != SignalLifecycleState::Executed {
            sqlx::query(
                r#"
                UPDATE signals
                SET
                  lifecycle_state = $1,
                  updated_at = $2,
                  version = $3
                WHERE id = $4
                "#,
            )
            .bind(SignalLifecycleState::Executed.as_str())
            .bind(now)
            .bind(current_signal_version + 1)
            .bind(&request.signal_id)
            .execute(&mut *transaction)
            .await
            .map_err(|error| {
                db_error(
                    "POSTGRES_UPDATE_FAILED",
                    format!(
                        "failed to update signal {} after reconciliation: {error}",
                        request.signal_id
                    ),
                )
            })?;

            sqlx::query(
                r#"
                INSERT INTO signal_transitions (
                  id,
                  signal_id,
                  from_state,
                  to_state,
                  trigger_type,
                  trigger_payload,
                  created_at
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7)
                "#,
            )
            .bind(format!("sgt_{}", Uuid::now_v7()))
            .bind(&request.signal_id)
            .bind(current_signal_state.as_str())
            .bind(SignalLifecycleState::Executed.as_str())
            .bind("execution_fill_reconciled")
            .bind(Json(json!({
                "execution_request_id": request.id,
                "external_trade_id": external_trade_id,
                "trace_id": trace_id,
            })))
            .bind(now)
            .execute(&mut *transaction)
            .await
            .map_err(|error| {
                db_error(
                    "POSTGRES_INSERT_FAILED",
                    format!(
                        "failed to insert signal transition for signal {}: {error}",
                        request.signal_id
                    ),
                )
            })?;
        }

        transaction.commit().await.map_err(|error| {
            db_error(
                "POSTGRES_TRANSACTION_COMMIT_FAILED",
                format!("failed to commit execution reconciliation transaction: {error}"),
            )
        })?;

        Ok(ExecutionFillResult {
            order,
            trade,
            position,
        })
    }
}
