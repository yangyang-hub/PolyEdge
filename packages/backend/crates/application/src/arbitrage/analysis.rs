#[must_use]
pub fn build_arbitrage_analysis(
    opportunities: &[ArbitrageOpportunityView],
    lookback_hours: u16,
    generated_at: OffsetDateTime,
) -> ArbitrageAnalysisSummary {
    let mut market_ids = HashSet::new();
    let mut type_counts = BTreeMap::<&'static str, (ArbitrageOpportunityType, u32)>::new();
    let mut market_groups = BTreeMap::<String, Vec<&ArbitrageOpportunityView>>::new();

    for opportunity in opportunities {
        market_ids.insert(opportunity.market_id.clone());
        let entry = type_counts
            .entry(opportunity.opportunity_type.as_str())
            .or_insert((opportunity.opportunity_type, 0));
        entry.1 += 1;
        market_groups
            .entry(opportunity.market_id.clone())
            .or_default()
            .push(opportunity);
    }

    let mut top_markets: Vec<_> = market_groups
        .into_iter()
        .map(|(market_id, items)| market_summary(market_id, &items))
        .collect();
    top_markets.sort_by(|left, right| {
        right
            .opportunity_count
            .cmp(&left.opportunity_count)
            .then_with(|| right.max_gross_edge.cmp(&left.max_gross_edge))
            .then_with(|| left.market_id.cmp(&right.market_id))
    });
    top_markets.truncate(20);

    ArbitrageAnalysisSummary {
        generated_at,
        lookback_hours,
        opportunity_count: u32::try_from(opportunities.len()).unwrap_or(u32::MAX),
        market_count: u32::try_from(market_ids.len()).unwrap_or(u32::MAX),
        type_counts: type_counts
            .into_values()
            .map(|(opportunity_type, count)| ArbitrageTypeCount {
                opportunity_type,
                count,
            })
            .collect(),
        top_markets,
    }
}

#[must_use]
pub fn market_book_snapshot_id(scan_id: &str, market_id: &str) -> String {
    format!(
        "book_{}_{}",
        id_fragment(scan_id).trim_start_matches("scan_"),
        id_fragment(market_id)
    )
}

#[must_use]
pub fn opportunity_id(
    scan_id: &str,
    market_id: &str,
    opportunity_type: ArbitrageOpportunityType,
) -> String {
    format!(
        "arb_{}_{}_{}",
        id_fragment(scan_id).trim_start_matches("scan_"),
        id_fragment(market_id),
        opportunity_type.as_str()
    )
}

fn market_summary(
    market_id: String,
    opportunities: &[&ArbitrageOpportunityView],
) -> ArbitrageMarketSummary {
    let count = Decimal::from(opportunities.len() as u64);
    let first = opportunities
        .iter()
        .map(|opportunity| opportunity.observed_at)
        .min()
        .unwrap_or(OffsetDateTime::UNIX_EPOCH);
    let last = opportunities
        .iter()
        .map(|opportunity| opportunity.observed_at)
        .max()
        .unwrap_or(OffsetDateTime::UNIX_EPOCH);
    let gross_edge_sum = opportunities
        .iter()
        .map(|opportunity| opportunity.gross_edge.value())
        .sum::<Decimal>();
    let capacity_sum = opportunities
        .iter()
        .map(|opportunity| opportunity.capacity.value())
        .sum::<Decimal>();
    let max_gross_edge = opportunities
        .iter()
        .map(|opportunity| opportunity.gross_edge.value())
        .max()
        .unwrap_or(Decimal::ZERO);
    let max_capacity = opportunities
        .iter()
        .map(|opportunity| opportunity.capacity.value())
        .max()
        .unwrap_or(Decimal::ZERO);

    ArbitrageMarketSummary {
        market_id,
        opportunity_count: u32::try_from(opportunities.len()).unwrap_or(u32::MAX),
        first_observed_at: first.to_string(),
        last_observed_at: last.to_string(),
        duration_seconds: (last - first).whole_seconds(),
        max_gross_edge,
        avg_gross_edge: if count > Decimal::ZERO {
            gross_edge_sum / count
        } else {
            Decimal::ZERO
        },
        max_capacity,
        avg_capacity: if count > Decimal::ZERO {
            capacity_sum / count
        } else {
            Decimal::ZERO
        },
    }
}

fn min_quantity(left: Quantity, right: Quantity) -> Quantity {
    if left <= right { left } else { right }
}

fn set_validation_status(
    current: &mut ArbitrageValidationStatus,
    next: ArbitrageValidationStatus,
    reason_codes: &mut Vec<String>,
    reason_code: &str,
) {
    if *current == ArbitrageValidationStatus::Valid {
        *current = next;
    }
    reason_codes.push(reason_code.to_string());
}

fn nonnegative_millis(duration: time::Duration) -> u64 {
    let millis = duration.whole_milliseconds();
    if millis <= 0 {
        0
    } else {
        u64::try_from(millis).unwrap_or(u64::MAX)
    }
}

fn clamp_edge(value: Decimal) -> Decimal {
    value.max(-Decimal::ONE).min(Decimal::ONE)
}

fn arbitrage_validation_id(opportunity_id: &str, validated_at: OffsetDateTime) -> String {
    format!(
        "arb_val_{}_{}",
        id_fragment(opportunity_id).trim_start_matches("arb_"),
        validated_at.unix_timestamp_nanos()
    )
}

fn arbitrage_event_id(
    event_type: ArbitrageEventType,
    resource_id: &str,
    occurred_at: OffsetDateTime,
) -> String {
    format!(
        "arb_evt_{}_{}_{}",
        id_fragment(event_type.as_str()),
        id_fragment(resource_id),
        occurred_at.unix_timestamp_nanos()
    )
}

fn timestamp_payload(timestamp: OffsetDateTime) -> String {
    timestamp
        .format(&Rfc3339)
        .unwrap_or_else(|_| timestamp.to_string())
}

fn scan_payload(scan: &ArbitrageScanView) -> Value {
    json!({
        "scan_id": &scan.id,
        "started_at": timestamp_payload(scan.started_at),
        "finished_at": scan.finished_at.map(timestamp_payload),
        "market_count": scan.market_count,
        "snapshot_count": scan.snapshot_count,
        "opportunity_count": scan.opportunity_count,
        "scanner_version": &scan.scanner_version,
        "metadata": &scan.metadata,
        "trace_id": &scan.trace_id,
    })
}

fn opportunity_payload(opportunity: &ArbitrageOpportunityView) -> Value {
    json!({
        "opportunity_id": &opportunity.id,
        "scan_id": &opportunity.scan_id,
        "market_id": &opportunity.market_id,
        "opportunity_type": opportunity.opportunity_type,
        "status": opportunity.status,
        "gross_edge": opportunity.gross_edge,
        "price_sum": opportunity.price_sum,
        "capacity": opportunity.capacity,
        "yes_price": opportunity.yes_price,
        "no_price": opportunity.no_price,
        "yes_size": opportunity.yes_size,
        "no_size": opportunity.no_size,
        "observed_at": timestamp_payload(opportunity.observed_at),
        "reason_codes": &opportunity.reason_codes,
        "analysis_payload": &opportunity.analysis_payload,
        "validation": &opportunity.validation,
        "trace_id": &opportunity.trace_id,
    })
}

fn validation_payload(validation: &ArbitrageOpportunityValidationView) -> Value {
    json!({
        "validation_id": &validation.id,
        "opportunity_id": &validation.opportunity_id,
        "validation_status": validation.status,
        "gross_edge": validation.gross_edge,
        "net_edge": validation.net_edge,
        "fee_estimate": validation.fee_estimate,
        "slippage_buffer": validation.slippage_buffer,
        "validated_capacity": validation.validated_capacity,
        "book_age_ms": validation.book_age_ms,
        "reason_codes": &validation.reason_codes,
        "validation_payload": &validation.validation_payload,
        "validated_at": timestamp_payload(validation.validated_at),
        "trace_id": &validation.trace_id,
    })
}

fn analysis_payload(analysis: &ArbitrageAnalysisRunView) -> Value {
    json!({
        "analysis_id": &analysis.id,
        "generated_at": timestamp_payload(analysis.generated_at),
        "lookback_hours": analysis.lookback_hours,
        "opportunity_count": analysis.opportunity_count,
        "market_count": analysis.market_count,
        "summary_payload": &analysis.summary_payload,
        "trace_id": &analysis.trace_id,
    })
}

fn validate_limit(limit: Option<u16>) -> Result<u16> {
    crate::list_filters::validate_list_limit(
        limit,
        DEFAULT_LIST_LIMIT,
        MAX_LIST_LIMIT,
        "ARBITRAGE_LIST_LIMIT_INVALID",
        "arbitrage list limit must be greater than zero",
        "ARBITRAGE_LIST_LIMIT_INVALID",
        format!("arbitrage list limit must be at most {MAX_LIST_LIMIT}"),
    )
}

fn normalize_optional_id(field: &'static str, value: Option<String>) -> Result<Option<String>> {
    crate::list_filters::normalize_optional_filter_id(
        field,
        value,
        "ARBITRAGE_FILTER_INVALID",
        |field| format!("{field} filter must not be empty"),
    )
}

fn id_fragment(value: &str) -> String {
    let fragment: String = value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect();
    let trimmed = fragment.trim_matches('_');
    if trimmed.is_empty() {
        "id".to_string()
    } else {
        trimmed.to_string()
    }
}
