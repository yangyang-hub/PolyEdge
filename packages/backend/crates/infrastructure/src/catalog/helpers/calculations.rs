fn in_memory_position_key(
    connector_name: &str,
    account_id: &str,
    market_id: &str,
    side: SignalSide,
) -> String {
    format!(
        "{connector_name}:{account_id}:{market_id}:{}",
        side.as_str()
    )
}

fn build_next_position(
    current: PositionView,
    filled_quantity: Quantity,
    fill_price: Probability,
    _trace_id: &str,
) -> Result<PositionView> {
    let next_quantity_value = current.net_quantity.value() + filled_quantity.value();
    let next_quantity = Quantity::new(next_quantity_value)?;
    let total_cost = (current.avg_cost.value() * current.net_quantity.value())
        + (fill_price.value() * filled_quantity.value());
    let avg_cost = if next_quantity.value().is_zero() {
        Probability::new(Decimal::ZERO)?
    } else {
        Probability::new(
            (total_cost / next_quantity.value())
                .round_dp_with_strategy(Probability::SCALE, RoundingStrategy::MidpointNearestEven),
        )?
    };
    let mark_price = fill_price;
    let unrealized_pnl = compute_unrealized_pnl(next_quantity, avg_cost, mark_price)?;

    Ok(PositionView {
        avg_cost,
        mark_price,
        net_quantity: next_quantity,
        unrealized_pnl,
        updated_at: OffsetDateTime::now_utc(),
        version: current.version + 1,
        ..current
    })
}

fn weighted_fill_price(
    current_avg_fill_price: Probability,
    current_filled_quantity: Quantity,
    fill_price: Probability,
    fill_quantity: Quantity,
) -> Result<Probability> {
    let next_filled_quantity_value = current_filled_quantity.value() + fill_quantity.value();
    if next_filled_quantity_value <= Decimal::ZERO {
        return Probability::new(Decimal::ZERO);
    }

    let weighted_cost = (current_avg_fill_price.value() * current_filled_quantity.value())
        + (fill_price.value() * fill_quantity.value());
    Probability::new(
        (weighted_cost / next_filled_quantity_value)
            .round_dp_with_strategy(Probability::SCALE, RoundingStrategy::MidpointNearestEven),
    )
}

fn compute_unrealized_pnl(
    quantity: Quantity,
    avg_cost: Probability,
    mark_price: Probability,
) -> Result<SignedUsdAmount> {
    let raw = (mark_price.value() - avg_cost.value()) * quantity.value();
    SignedUsdAmount::new(raw.round_dp_with_strategy(
        SignedUsdAmount::SCALE,
        RoundingStrategy::MidpointNearestEven,
    ))
}

fn validate_signal_for_execution(signal: &SignalView) -> Result<()> {
    if signal.rejected_by_user_id.is_some() {
        return Err(AppError::conflict(
            "STATE_SIGNAL_REJECTED_FOR_EXECUTION",
            "rejected signals cannot be submitted for execution",
        ));
    }

    if !matches!(
        signal.lifecycle_state,
        SignalLifecycleState::New | SignalLifecycleState::Active
    ) {
        return Err(AppError::conflict(
            "STATE_SIGNAL_NOT_EXECUTABLE",
            "only new or active signals can be submitted for execution",
        ));
    }

    Ok(())
}

fn compute_order_notional(limit_price: Probability, quantity: Quantity) -> Result<UsdAmount> {
    let notional = (limit_price.value() * quantity.value())
        .round_dp_with_strategy(UsdAmount::SCALE, RoundingStrategy::MidpointNearestEven);
    UsdAmount::new(notional).map_err(|error| {
        AppError::invalid_input(
            "ORDER_NOTIONAL_INVALID",
            format!("failed to compute order notional: {error}"),
        )
    })
}

fn raw_news_dedup_keys(event: &NewsRawEventInsert) -> Vec<String> {
    let mut keys = vec![
        format!("id:{}", event.id),
        format!("source_hash:{}:{}", event.source, event.hash),
    ];

    if let Some(external_id) = event.external_id.as_deref() {
        keys.push(format!("source_external_id:{}:{external_id}", event.source));
    }

    if let Some(url) = event.url.as_deref() {
        keys.push(format!("source_url:{}:{url}", event.source));
    }

    keys
}

fn raw_news_event_view_from_insert(event: &NewsRawEventInsert) -> NewsRawEventView {
    NewsRawEventView {
        id: event.id.clone(),
        source: event.source.clone(),
        source_type: event.source_type.clone(),
        external_id: event.external_id.clone(),
        title: event.title.clone(),
        url: event.url.clone(),
        author: event.author.clone(),
        published_at: event.published_at,
        event_time: event.event_time,
        hash: event.hash.clone(),
        raw_payload: event.raw_payload.clone(),
        ingested_at: event.ingested_at,
        trace_id: event.trace_id.clone(),
    }
}

fn usize_to_i64(value: usize) -> Result<i64> {
    i64::try_from(value).map_err(|error| {
        AppError::invalid_input(
            "NEWS_COUNT_OUT_OF_RANGE",
            format!("news ingestion count does not fit i64: {error}"),
        )
    })
}

fn usize_to_u64(value: usize) -> Result<u64> {
    u64::try_from(value).map_err(|error| {
        AppError::invalid_input(
            "NEWS_COUNT_OUT_OF_RANGE",
            format!("news ingestion count does not fit u64: {error}"),
        )
    })
}

fn add_news_count(left: u64, right: u64) -> Result<u64> {
    left.checked_add(right).ok_or_else(|| {
        AppError::invalid_input(
            "NEWS_COUNT_OUT_OF_RANGE",
            "news ingestion count exceeds u64 range",
        )
    })
}

fn i64_to_u64(column: &str, value: i64) -> Result<u64> {
    u64::try_from(value).map_err(|error| {
        db_error(
            "POSTGRES_DECODE_FAILED",
            format!("failed to decode column {column} as nonnegative count: {error}"),
        )
    })
}

fn nonnegative_i32_to_u32(column: &str, value: i32) -> Result<u32> {
    u32::try_from(value).map_err(|error| {
        db_error(
            "POSTGRES_DECODE_FAILED",
            format!("failed to decode column {column} as nonnegative count: {error}"),
        )
    })
}

fn latest_validation_for_opportunity(
    validations: &HashMap<String, ArbitrageOpportunityValidationView>,
    opportunity_id: &str,
) -> Option<ArbitrageOpportunityValidationView> {
    validations
        .values()
        .filter(|validation| validation.opportunity_id == opportunity_id)
        .max_by(|left, right| {
            left.validated_at
                .cmp(&right.validated_at)
                .then_with(|| left.id.cmp(&right.id))
        })
        .cloned()
}

fn clamped_error_message(value: &str) -> String {
    value.chars().take(1_000).collect()
}

fn required_optional_column<T>(
    row: &sqlx::postgres::PgRow,
    column: &str,
    context: &str,
) -> Result<T>
where
    T: for<'r> sqlx::Decode<'r, sqlx::Postgres> + sqlx::Type<sqlx::Postgres>,
{
    let value: Option<T> = decode_column(row, column)?;
    value.ok_or_else(|| {
        db_error(
            "POSTGRES_DECODE_FAILED",
            format!("missing column {column} while decoding {context}"),
        )
    })
}

fn decode_column<T>(row: &sqlx::postgres::PgRow, column: &str) -> Result<T>
where
    T: for<'r> sqlx::Decode<'r, sqlx::Postgres> + sqlx::Type<sqlx::Postgres>,
{
    row.try_get(column).map_err(|error| {
        db_error(
            "POSTGRES_DECODE_FAILED",
            format!("failed to decode column {column}: {error}"),
        )
    })
}
