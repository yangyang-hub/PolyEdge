pub fn detect_arbitrage_opportunities(
    snapshot: &MarketBookSnapshotView,
) -> Result<Vec<ArbitrageOpportunityDraft>> {
    let mut opportunities = Vec::new();

    if let (Some(yes_ask), Some(no_ask)) = (snapshot.yes_ask, snapshot.no_ask) {
        let price_sum = yes_ask.value() + no_ask.value();
        let gross_edge = Decimal::ONE - price_sum;
        if gross_edge > Decimal::ZERO {
            opportunities.push(ArbitrageOpportunityDraft {
                opportunity_type: ArbitrageOpportunityType::BinaryBuyBoth,
                gross_edge: Edge::new(gross_edge)?,
                price_sum,
                capacity: min_quantity(snapshot.yes_ask_size, snapshot.no_ask_size),
                yes_price: yes_ask,
                no_price: no_ask,
                yes_size: snapshot.yes_ask_size,
                no_size: snapshot.no_ask_size,
                reason_codes: vec!["yes_ask_plus_no_ask_below_one".to_string()],
                analysis_payload: json!({
                    "formula": "1 - yes_ask - no_ask",
                    "yes_ask": yes_ask,
                    "no_ask": no_ask,
                    "price_sum": price_sum,
                    "gross_edge": gross_edge,
                }),
            });
        }
    }

    if let (Some(yes_bid), Some(no_bid)) = (snapshot.yes_bid, snapshot.no_bid) {
        let price_sum = yes_bid.value() + no_bid.value();
        let gross_edge = price_sum - Decimal::ONE;
        if gross_edge > Decimal::ZERO {
            opportunities.push(ArbitrageOpportunityDraft {
                opportunity_type: ArbitrageOpportunityType::BinarySellBoth,
                gross_edge: Edge::new(gross_edge)?,
                price_sum,
                capacity: min_quantity(snapshot.yes_bid_size, snapshot.no_bid_size),
                yes_price: yes_bid,
                no_price: no_bid,
                yes_size: snapshot.yes_bid_size,
                no_size: snapshot.no_bid_size,
                reason_codes: vec!["yes_bid_plus_no_bid_above_one".to_string()],
                analysis_payload: json!({
                    "formula": "yes_bid + no_bid - 1",
                    "yes_bid": yes_bid,
                    "no_bid": no_bid,
                    "price_sum": price_sum,
                    "gross_edge": gross_edge,
                }),
            });
        }
    }

    Ok(opportunities)
}

pub fn validate_arbitrage_opportunity(
    opportunity: &ArbitrageOpportunityView,
    snapshot: &MarketBookSnapshotView,
    config: &ArbitrageValidationConfig,
    validated_at: OffsetDateTime,
    trace_id: &str,
) -> Result<ArbitrageOpportunityValidationView> {
    let mut status = ArbitrageValidationStatus::Valid;
    let mut reason_codes = Vec::new();
    let book_age_ms = nonnegative_millis(validated_at - snapshot.observed_at);
    let current_draft = detect_arbitrage_opportunities(snapshot)?
        .into_iter()
        .find(|draft| draft.opportunity_type == opportunity.opportunity_type);
    let gross_edge = current_draft
        .as_ref()
        .map_or(Decimal::ZERO, |draft| draft.gross_edge.value());
    let current_capacity = current_draft
        .as_ref()
        .map_or(Quantity::new(Decimal::ZERO)?, |draft| draft.capacity);
    let fee_estimate = config.fee_buffer.value();
    let slippage_buffer = config.slippage_buffer.value();
    let net_edge = clamp_edge(gross_edge - fee_estimate - slippage_buffer);

    if snapshot.market_id != opportunity.market_id || snapshot.scan_id != opportunity.scan_id {
        set_validation_status(
            &mut status,
            ArbitrageValidationStatus::InvalidMarket,
            &mut reason_codes,
            "snapshot_opportunity_mismatch",
        );
    }

    if book_age_ms > config.max_book_age_ms {
        set_validation_status(
            &mut status,
            ArbitrageValidationStatus::StaleBook,
            &mut reason_codes,
            "book_age_exceeds_threshold",
        );
    }

    if current_draft.is_none() {
        set_validation_status(
            &mut status,
            ArbitrageValidationStatus::PriceMoved,
            &mut reason_codes,
            "opportunity_no_longer_present_in_latest_book",
        );
    }

    if gross_edge < config.min_gross_edge.value() {
        set_validation_status(
            &mut status,
            ArbitrageValidationStatus::BelowThreshold,
            &mut reason_codes,
            "gross_edge_below_threshold",
        );
    }

    if current_capacity.value() < config.min_capacity.value() {
        set_validation_status(
            &mut status,
            ArbitrageValidationStatus::InsufficientDepth,
            &mut reason_codes,
            "capacity_below_threshold",
        );
    }

    if net_edge <= Decimal::ZERO {
        set_validation_status(
            &mut status,
            ArbitrageValidationStatus::FeesExceedEdge,
            &mut reason_codes,
            "net_edge_not_positive_after_buffers",
        );
    }

    if status == ArbitrageValidationStatus::Valid {
        reason_codes.push("net_edge_positive_after_buffers".to_string());
    }

    let validated_capacity = if status == ArbitrageValidationStatus::Valid {
        current_capacity
    } else {
        Quantity::new(Decimal::ZERO)?
    };

    Ok(ArbitrageOpportunityValidationView {
        id: arbitrage_validation_id(&opportunity.id, validated_at),
        opportunity_id: opportunity.id.clone(),
        status,
        gross_edge: Edge::new(gross_edge)?,
        net_edge: Edge::new(net_edge)?,
        fee_estimate: config.fee_buffer,
        slippage_buffer: config.slippage_buffer,
        validated_capacity,
        book_age_ms,
        reason_codes,
        validation_payload: json!({
            "max_book_age_ms": config.max_book_age_ms,
            "min_gross_edge": config.min_gross_edge,
            "min_capacity": config.min_capacity,
            "fee_buffer": config.fee_buffer,
            "slippage_buffer": config.slippage_buffer,
            "snapshot_id": snapshot.id,
            "snapshot_observed_at": snapshot.observed_at,
            "discovery_gross_edge": opportunity.gross_edge,
            "discovery_capacity": opportunity.capacity,
            "current_capacity": current_capacity,
            "validated_at": validated_at,
        }),
        validated_at,
        trace_id: trace_id.to_string(),
    })
}
