fn parse_arbitrage_scan_row(row: &sqlx::postgres::PgRow) -> Result<ArbitrageScanView> {
    let metadata_json: Json<Value> = decode_column(row, "metadata_json")?;
    let market_count: i32 = decode_column(row, "market_count")?;
    let snapshot_count: i32 = decode_column(row, "snapshot_count")?;
    let opportunity_count: i32 = decode_column(row, "opportunity_count")?;

    Ok(ArbitrageScanView {
        id: decode_column(row, "id")?,
        started_at: decode_column(row, "started_at")?,
        finished_at: decode_column(row, "finished_at")?,
        market_count: nonnegative_i32_to_u32("market_count", market_count)?,
        snapshot_count: nonnegative_i32_to_u32("snapshot_count", snapshot_count)?,
        opportunity_count: nonnegative_i32_to_u32("opportunity_count", opportunity_count)?,
        scanner_version: decode_column(row, "scanner_version")?,
        metadata: metadata_json.0,
        trace_id: decode_column(row, "trace_id")?,
    })
}

fn parse_arbitrage_opportunity_row(
    row: &sqlx::postgres::PgRow,
) -> Result<ArbitrageOpportunityView> {
    let opportunity_type_raw: String = decode_column(row, "opportunity_type")?;
    let status_raw: String = decode_column(row, "status")?;
    let gross_edge: Decimal = decode_column(row, "gross_edge")?;
    let price_sum: Decimal = decode_column(row, "price_sum")?;
    let capacity: Decimal = decode_column(row, "capacity")?;
    let yes_price: Decimal = decode_column(row, "yes_price")?;
    let no_price: Decimal = decode_column(row, "no_price")?;
    let yes_size: Decimal = decode_column(row, "yes_size")?;
    let no_size: Decimal = decode_column(row, "no_size")?;
    let reason_codes_json: Json<Vec<String>> = decode_column(row, "reason_codes_json")?;
    let analysis_payload_json: Json<Value> = decode_column(row, "analysis_payload_json")?;

    Ok(ArbitrageOpportunityView {
        id: decode_column(row, "id")?,
        scan_id: decode_column(row, "scan_id")?,
        market_id: decode_column(row, "market_id")?,
        opportunity_type: ArbitrageOpportunityType::from_str(&opportunity_type_raw).map_err(
            |error| {
                db_error(
                    "POSTGRES_DECODE_FAILED",
                    format!("failed to decode arbitrage opportunity_type: {error}"),
                )
            },
        )?,
        status: ArbitrageOpportunityStatus::from_str(&status_raw).map_err(|error| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("failed to decode arbitrage opportunity status: {error}"),
            )
        })?,
        gross_edge: Edge::new(gross_edge).map_err(|error| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("failed to decode arbitrage gross_edge: {error}"),
            )
        })?,
        price_sum,
        capacity: Quantity::new(capacity).map_err(|error| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("failed to decode arbitrage capacity: {error}"),
            )
        })?,
        yes_price: Probability::new(yes_price).map_err(|error| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("failed to decode arbitrage yes_price: {error}"),
            )
        })?,
        no_price: Probability::new(no_price).map_err(|error| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("failed to decode arbitrage no_price: {error}"),
            )
        })?,
        yes_size: Quantity::new(yes_size).map_err(|error| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("failed to decode arbitrage yes_size: {error}"),
            )
        })?,
        no_size: Quantity::new(no_size).map_err(|error| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("failed to decode arbitrage no_size: {error}"),
            )
        })?,
        observed_at: decode_column(row, "observed_at")?,
        reason_codes: reason_codes_json.0,
        analysis_payload: analysis_payload_json.0,
        trace_id: decode_column(row, "trace_id")?,
        validation: parse_arbitrage_validation_from_row(row)?,
    })
}

fn parse_arbitrage_validation_from_row(
    row: &sqlx::postgres::PgRow,
) -> Result<Option<ArbitrageOpportunityValidationView>> {
    let Some(id) = decode_column::<Option<String>>(row, "validation_id")? else {
        return Ok(None);
    };
    let status_raw = required_optional_column::<String>(row, "validation_status", &id)?;
    let gross_edge = required_optional_column::<Decimal>(row, "validation_gross_edge", &id)?;
    let net_edge = required_optional_column::<Decimal>(row, "validation_net_edge", &id)?;
    let fee_estimate = required_optional_column::<Decimal>(row, "validation_fee_estimate", &id)?;
    let slippage_buffer =
        required_optional_column::<Decimal>(row, "validation_slippage_buffer", &id)?;
    let validated_capacity =
        required_optional_column::<Decimal>(row, "validation_validated_capacity", &id)?;
    let book_age_ms = required_optional_column::<i64>(row, "validation_book_age_ms", &id)?;
    let reason_codes_json =
        required_optional_column::<Json<Vec<String>>>(row, "validation_reason_codes_json", &id)?;
    let validation_payload_json =
        required_optional_column::<Json<Value>>(row, "validation_payload_json", &id)?;

    Ok(Some(ArbitrageOpportunityValidationView {
        id,
        opportunity_id: decode_column(row, "id")?,
        status: ArbitrageValidationStatus::from_str(&status_raw).map_err(|error| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("failed to decode arbitrage validation status: {error}"),
            )
        })?,
        gross_edge: Edge::new(gross_edge).map_err(|error| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("failed to decode arbitrage validation gross_edge: {error}"),
            )
        })?,
        net_edge: Edge::new(net_edge).map_err(|error| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("failed to decode arbitrage validation net_edge: {error}"),
            )
        })?,
        fee_estimate: Edge::new(fee_estimate).map_err(|error| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("failed to decode arbitrage validation fee_estimate: {error}"),
            )
        })?,
        slippage_buffer: Edge::new(slippage_buffer).map_err(|error| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("failed to decode arbitrage validation slippage_buffer: {error}"),
            )
        })?,
        validated_capacity: Quantity::new(validated_capacity).map_err(|error| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("failed to decode arbitrage validation capacity: {error}"),
            )
        })?,
        book_age_ms: i64_to_u64("validation_book_age_ms", book_age_ms)?,
        reason_codes: reason_codes_json.0,
        validation_payload: validation_payload_json.0,
        validated_at: required_optional_column(row, "validation_validated_at", "validation")?,
        trace_id: required_optional_column(row, "validation_trace_id", "validation")?,
    }))
}

fn parse_arbitrage_analysis_run_row(
    row: &sqlx::postgres::PgRow,
) -> Result<ArbitrageAnalysisRunView> {
    let lookback_hours: i32 = decode_column(row, "lookback_hours")?;
    let opportunity_count: i32 = decode_column(row, "opportunity_count")?;
    let market_count: i32 = decode_column(row, "market_count")?;
    let summary_payload_json: Json<Value> = decode_column(row, "summary_payload_json")?;

    Ok(ArbitrageAnalysisRunView {
        id: decode_column(row, "id")?,
        generated_at: decode_column(row, "generated_at")?,
        lookback_hours: nonnegative_i32_to_u32("lookback_hours", lookback_hours)?
            .min(u32::from(u16::MAX)) as u16,
        opportunity_count: nonnegative_i32_to_u32("opportunity_count", opportunity_count)?,
        market_count: nonnegative_i32_to_u32("market_count", market_count)?,
        summary_payload: summary_payload_json.0,
        trace_id: decode_column(row, "trace_id")?,
    })
}

fn parse_arbitrage_event_row(row: &sqlx::postgres::PgRow) -> Result<ArbitrageEventView> {
    let sequence: i64 = decode_column(row, "sequence")?;
    let event_type_raw: String = decode_column(row, "event_type")?;
    let payload_json: Json<Value> = decode_column(row, "payload_json")?;

    Ok(ArbitrageEventView {
        sequence: i64_to_u64("sequence", sequence)?,
        id: decode_column(row, "id")?,
        event_type: ArbitrageEventType::from_str(&event_type_raw).map_err(|error| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("failed to decode arbitrage event type: {error}"),
            )
        })?,
        resource_type: decode_column(row, "resource_type")?,
        resource_id: decode_column(row, "resource_id")?,
        payload: payload_json.0,
        occurred_at: decode_column(row, "occurred_at")?,
        trace_id: decode_column(row, "trace_id")?,
    })
}
