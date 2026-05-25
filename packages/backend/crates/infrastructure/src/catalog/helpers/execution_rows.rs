#[derive(Debug, Clone, Copy)]
struct MarketEventExecutionFill<'a> {
    execution_request_id: &'a str,
    account_id: &'a str,
    external_trade_id: &'a str,
    fill_price: Probability,
    filled_quantity: Quantity,
    fee: UsdAmount,
    trace_id: &'a str,
}

fn parse_order_draft_row(row: &sqlx::postgres::PgRow) -> Result<OrderDraftView> {
    let side_raw: String = decode_column(row, "side")?;
    let limit_price: Decimal = decode_column(row, "limit_price")?;
    let quantity: Decimal = decode_column(row, "quantity")?;
    let notional: Decimal = decode_column(row, "notional")?;
    let status_raw: String = decode_column(row, "status")?;

    Ok(OrderDraftView {
        id: decode_column(row, "id")?,
        signal_id: decode_column(row, "signal_id")?,
        signal_version: decode_column(row, "signal_version")?,
        market_id: decode_column(row, "market_id")?,
        connector_name: decode_column(row, "connector_name")?,
        side: SignalSide::from_str(&side_raw).map_err(|error| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("failed to decode order draft side: {error}"),
            )
        })?,
        limit_price: Probability::new(limit_price).map_err(|error| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("failed to decode order draft limit_price: {error}"),
            )
        })?,
        quantity: Quantity::new(quantity).map_err(|error| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("failed to decode order draft quantity: {error}"),
            )
        })?,
        notional: UsdAmount::new(notional).map_err(|error| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("failed to decode order draft notional: {error}"),
            )
        })?,
        status: OrderDraftStatus::from_str(&status_raw).map_err(|error| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("failed to decode order draft status: {error}"),
            )
        })?,
        created_by_user_id: decode_column(row, "created_by_user_id")?,
        external_order_id: decode_column(row, "external_order_id")?,
        submitted_at: decode_column(row, "submitted_at")?,
        failure_code: decode_column(row, "failure_code")?,
        failure_message: decode_column(row, "failure_message")?,
        created_at: decode_column(row, "created_at")?,
        updated_at: decode_column(row, "updated_at")?,
        version: decode_column(row, "version")?,
    })
}

fn parse_execution_request_row(row: &sqlx::postgres::PgRow) -> Result<ExecutionRequestView> {
    let mode_raw: String = decode_column(row, "mode")?;
    let status_raw: String = decode_column(row, "status")?;

    Ok(ExecutionRequestView {
        id: decode_column(row, "id")?,
        signal_id: decode_column(row, "signal_id")?,
        signal_version: decode_column(row, "signal_version")?,
        order_draft_id: decode_column(row, "order_draft_id")?,
        connector_name: decode_column(row, "connector_name")?,
        mode: polyedge_domain::SystemMode::from_str(&mode_raw).map_err(|error| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("failed to decode execution request mode: {error}"),
            )
        })?,
        requested_by_user_id: decode_column(row, "requested_by_user_id")?,
        status: ExecutionRequestStatus::from_str(&status_raw).map_err(|error| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("failed to decode execution request status: {error}"),
            )
        })?,
        reason: decode_column(row, "reason")?,
        external_order_id: decode_column(row, "external_order_id")?,
        submitted_at: decode_column(row, "submitted_at")?,
        failure_code: decode_column(row, "failure_code")?,
        failure_message: decode_column(row, "failure_message")?,
        created_at: decode_column(row, "created_at")?,
        updated_at: decode_column(row, "updated_at")?,
        version: decode_column(row, "version")?,
    })
}

fn parse_dispatch_candidate_row(row: &sqlx::postgres::PgRow) -> Result<ExecutionDispatchCandidate> {
    let order_draft_side_raw: String = decode_column(row, "order_draft_side")?;
    let order_draft_limit_price: Decimal = decode_column(row, "order_draft_limit_price")?;
    let order_draft_quantity: Decimal = decode_column(row, "order_draft_quantity")?;
    let order_draft_notional: Decimal = decode_column(row, "order_draft_notional")?;
    let order_draft_status_raw: String = decode_column(row, "order_draft_status")?;
    let execution_request_mode_raw: String = decode_column(row, "execution_request_mode")?;
    let execution_request_status_raw: String = decode_column(row, "execution_request_status")?;

    Ok(ExecutionDispatchCandidate {
        order_draft: OrderDraftView {
            id: decode_column(row, "order_draft_id")?,
            signal_id: decode_column(row, "order_draft_signal_id")?,
            signal_version: decode_column(row, "order_draft_signal_version")?,
            market_id: decode_column(row, "order_draft_market_id")?,
            connector_name: decode_column(row, "order_draft_connector_name")?,
            side: SignalSide::from_str(&order_draft_side_raw).map_err(|error| {
                db_error(
                    "POSTGRES_DECODE_FAILED",
                    format!("failed to decode dispatch order draft side: {error}"),
                )
            })?,
            limit_price: Probability::new(order_draft_limit_price).map_err(|error| {
                db_error(
                    "POSTGRES_DECODE_FAILED",
                    format!("failed to decode dispatch order draft limit_price: {error}"),
                )
            })?,
            quantity: Quantity::new(order_draft_quantity).map_err(|error| {
                db_error(
                    "POSTGRES_DECODE_FAILED",
                    format!("failed to decode dispatch order draft quantity: {error}"),
                )
            })?,
            notional: UsdAmount::new(order_draft_notional).map_err(|error| {
                db_error(
                    "POSTGRES_DECODE_FAILED",
                    format!("failed to decode dispatch order draft notional: {error}"),
                )
            })?,
            status: OrderDraftStatus::from_str(&order_draft_status_raw).map_err(|error| {
                db_error(
                    "POSTGRES_DECODE_FAILED",
                    format!("failed to decode dispatch order draft status: {error}"),
                )
            })?,
            created_by_user_id: decode_column(row, "order_draft_created_by_user_id")?,
            external_order_id: decode_column(row, "order_draft_external_order_id")?,
            submitted_at: decode_column(row, "order_draft_submitted_at")?,
            failure_code: decode_column(row, "order_draft_failure_code")?,
            failure_message: decode_column(row, "order_draft_failure_message")?,
            created_at: decode_column(row, "order_draft_created_at")?,
            updated_at: decode_column(row, "order_draft_updated_at")?,
            version: decode_column(row, "order_draft_version")?,
        },
        execution_request: ExecutionRequestView {
            id: decode_column(row, "execution_request_id")?,
            signal_id: decode_column(row, "execution_request_signal_id")?,
            signal_version: decode_column(row, "execution_request_signal_version")?,
            order_draft_id: decode_column(row, "execution_request_order_draft_id")?,
            connector_name: decode_column(row, "execution_request_connector_name")?,
            mode: polyedge_domain::SystemMode::from_str(&execution_request_mode_raw).map_err(
                |error| {
                    db_error(
                        "POSTGRES_DECODE_FAILED",
                        format!("failed to decode dispatch execution request mode: {error}"),
                    )
                },
            )?,
            requested_by_user_id: decode_column(row, "execution_request_requested_by_user_id")?,
            status: ExecutionRequestStatus::from_str(&execution_request_status_raw).map_err(
                |error| {
                    db_error(
                        "POSTGRES_DECODE_FAILED",
                        format!("failed to decode dispatch execution request status: {error}"),
                    )
                },
            )?,
            reason: decode_column(row, "execution_request_reason")?,
            external_order_id: decode_column(row, "execution_request_external_order_id")?,
            submitted_at: decode_column(row, "execution_request_submitted_at")?,
            failure_code: decode_column(row, "execution_request_failure_code")?,
            failure_message: decode_column(row, "execution_request_failure_message")?,
            created_at: decode_column(row, "execution_request_created_at")?,
            updated_at: decode_column(row, "execution_request_updated_at")?,
            version: decode_column(row, "execution_request_version")?,
        },
    })
}

fn parse_reconciliation_candidate_row(
    row: &sqlx::postgres::PgRow,
) -> Result<ExecutionReconciliationCandidate> {
    let candidate = parse_dispatch_candidate_row(row)?;
    let order = if decode_column::<Option<String>>(row, "order_id")?.is_some() {
        Some(parse_order_row_with_prefix(row, "order_")?)
    } else {
        None
    };
    Ok(ExecutionReconciliationCandidate {
        order_draft: candidate.order_draft,
        execution_request: candidate.execution_request,
        order,
    })
}

fn parse_order_row(row: &sqlx::postgres::PgRow) -> Result<OrderView> {
    parse_order_row_with_prefix(row, "")
}

fn parse_order_row_with_prefix(row: &sqlx::postgres::PgRow, prefix: &str) -> Result<OrderView> {
    let side_raw: String = decode_column(row, &format!("{prefix}side"))?;
    let limit_price: Decimal = decode_column(row, &format!("{prefix}limit_price"))?;
    let quantity: Decimal = decode_column(row, &format!("{prefix}quantity"))?;
    let filled_quantity: Decimal = decode_column(row, &format!("{prefix}filled_quantity"))?;
    let avg_fill_price: Decimal = decode_column(row, &format!("{prefix}avg_fill_price"))?;
    let status_raw: String = decode_column(row, &format!("{prefix}status"))?;

    Ok(OrderView {
        id: decode_column(row, &format!("{prefix}id"))?,
        signal_id: decode_column(row, &format!("{prefix}signal_id"))?,
        execution_request_id: decode_column(row, &format!("{prefix}execution_request_id"))?,
        order_draft_id: decode_column(row, &format!("{prefix}order_draft_id"))?,
        market_id: decode_column(row, &format!("{prefix}market_id"))?,
        connector_name: decode_column(row, &format!("{prefix}connector_name"))?,
        account_id: decode_column(row, &format!("{prefix}account_id"))?,
        external_order_id: decode_column(row, &format!("{prefix}external_order_id"))?,
        side: SignalSide::from_str(&side_raw).map_err(|error| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("failed to decode order side: {error}"),
            )
        })?,
        limit_price: Probability::new(limit_price).map_err(|error| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("failed to decode order limit_price: {error}"),
            )
        })?,
        quantity: Quantity::new(quantity).map_err(|error| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("failed to decode order quantity: {error}"),
            )
        })?,
        filled_quantity: Quantity::new(filled_quantity).map_err(|error| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("failed to decode order filled_quantity: {error}"),
            )
        })?,
        avg_fill_price: Probability::new(avg_fill_price).map_err(|error| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("failed to decode order avg_fill_price: {error}"),
            )
        })?,
        status: OrderStatus::from_str(&status_raw).map_err(|error| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("failed to decode order status: {error}"),
            )
        })?,
        submitted_at: decode_column(row, &format!("{prefix}submitted_at"))?,
        updated_at: decode_column(row, &format!("{prefix}updated_at"))?,
        version: decode_column(row, &format!("{prefix}version"))?,
    })
}

fn parse_trade_row(row: &sqlx::postgres::PgRow) -> Result<TradeView> {
    let side_raw: String = decode_column(row, "side")?;
    let price: Decimal = decode_column(row, "price")?;
    let quantity: Decimal = decode_column(row, "quantity")?;
    let fee: Decimal = decode_column(row, "fee")?;

    Ok(TradeView {
        id: decode_column(row, "id")?,
        order_id: decode_column(row, "order_id")?,
        signal_id: decode_column(row, "signal_id")?,
        market_id: decode_column(row, "market_id")?,
        connector_name: decode_column(row, "connector_name")?,
        external_trade_id: decode_column(row, "external_trade_id")?,
        side: SignalSide::from_str(&side_raw).map_err(|error| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("failed to decode trade side: {error}"),
            )
        })?,
        price: Probability::new(price).map_err(|error| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("failed to decode trade price: {error}"),
            )
        })?,
        quantity: Quantity::new(quantity).map_err(|error| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("failed to decode trade quantity: {error}"),
            )
        })?,
        fee: UsdAmount::new(fee).map_err(|error| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("failed to decode trade fee: {error}"),
            )
        })?,
        executed_at: decode_column(row, "executed_at")?,
    })
}

fn parse_position_row(row: &sqlx::postgres::PgRow) -> Result<PositionView> {
    let side_raw: String = decode_column(row, "side")?;
    let net_quantity: Decimal = decode_column(row, "net_quantity")?;
    let avg_cost: Decimal = decode_column(row, "avg_cost")?;
    let mark_price: Decimal = decode_column(row, "mark_price")?;
    let unrealized_pnl: Decimal = decode_column(row, "unrealized_pnl")?;
    let realized_pnl: Decimal = decode_column(row, "realized_pnl")?;

    Ok(PositionView {
        id: decode_column(row, "id")?,
        market_id: decode_column(row, "market_id")?,
        connector_name: decode_column(row, "connector_name")?,
        account_id: decode_column(row, "account_id")?,
        side: SignalSide::from_str(&side_raw).map_err(|error| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("failed to decode position side: {error}"),
            )
        })?,
        net_quantity: Quantity::new(net_quantity).map_err(|error| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("failed to decode position net_quantity: {error}"),
            )
        })?,
        avg_cost: Probability::new(avg_cost).map_err(|error| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("failed to decode position avg_cost: {error}"),
            )
        })?,
        mark_price: Probability::new(mark_price).map_err(|error| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("failed to decode position mark_price: {error}"),
            )
        })?,
        unrealized_pnl: SignedUsdAmount::new(unrealized_pnl).map_err(|error| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("failed to decode position unrealized_pnl: {error}"),
            )
        })?,
        realized_pnl: SignedUsdAmount::new(realized_pnl).map_err(|error| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("failed to decode position realized_pnl: {error}"),
            )
        })?,
        updated_at: decode_column(row, "updated_at")?,
        version: decode_column(row, "version")?,
    })
}
