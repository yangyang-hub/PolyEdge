fn parse_event_row(row: &sqlx::postgres::PgRow) -> Result<EventView> {
    let status_raw: String = decode_column(row, "status")?;
    let relevance_score: Decimal = decode_column(row, "relevance_score")?;
    let confidence: Decimal = decode_column(row, "confidence")?;

    Ok(EventView {
        id: decode_column(row, "id")?,
        source: decode_column(row, "source")?,
        summary: decode_column(row, "summary")?,
        relevance_score: Probability::new(relevance_score).map_err(|error| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("failed to decode event relevance_score: {error}"),
            )
        })?,
        confidence: Probability::new(confidence).map_err(|error| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("failed to decode event confidence: {error}"),
            )
        })?,
        status: EventStatus::from_str(&status_raw).map_err(|error| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("failed to decode event status: {error}"),
            )
        })?,
        related_market_ids: decode_column(row, "related_market_ids")?,
        reason_trace: decode_column(row, "reason_trace")?,
        created_at: decode_column(row, "created_at")?,
        updated_at: decode_column(row, "updated_at")?,
        version: decode_column(row, "version")?,
    })
}

fn parse_evidence_row(row: &sqlx::postgres::PgRow) -> Result<EvidenceView> {
    let direction_raw: String = decode_column(row, "direction")?;
    let status_raw: String = decode_column(row, "status")?;
    let strength: Decimal = decode_column(row, "strength")?;
    let source_reliability: Decimal = decode_column(row, "source_reliability")?;
    let novelty: Decimal = decode_column(row, "novelty")?;
    let resolution_relevance: Decimal = decode_column(row, "resolution_relevance")?;

    Ok(EvidenceView {
        id: decode_column(row, "id")?,
        market_id: decode_column(row, "market_id")?,
        event_id: decode_column(row, "event_id")?,
        direction: EvidenceDirection::from_str(&direction_raw).map_err(|error| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("failed to decode evidence direction: {error}"),
            )
        })?,
        strength: Probability::new(strength).map_err(|error| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("failed to decode evidence strength: {error}"),
            )
        })?,
        source_reliability: Probability::new(source_reliability).map_err(|error| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("failed to decode evidence source_reliability: {error}"),
            )
        })?,
        novelty: Probability::new(novelty).map_err(|error| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("failed to decode evidence novelty: {error}"),
            )
        })?,
        resolution_relevance: Probability::new(resolution_relevance).map_err(|error| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("failed to decode evidence resolution_relevance: {error}"),
            )
        })?,
        status: EvidenceStatus::from_str(&status_raw).map_err(|error| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("failed to decode evidence status: {error}"),
            )
        })?,
        expires_at: decode_column(row, "expires_at")?,
        created_at: decode_column(row, "created_at")?,
        updated_at: decode_column(row, "updated_at")?,
        version: decode_column(row, "version")?,
    })
}

fn parse_signal_row(row: &sqlx::postgres::PgRow) -> Result<SignalView> {
    let action_raw: String = decode_column(row, "action")?;
    let side_raw: String = decode_column(row, "side")?;
    let lifecycle_state_raw: String = decode_column(row, "lifecycle_state")?;
    let market_price: Decimal = decode_column(row, "market_price")?;
    let fair_price: Decimal = decode_column(row, "fair_price")?;
    let edge: Decimal = decode_column(row, "edge")?;
    let confidence: Decimal = decode_column(row, "confidence")?;

    Ok(SignalView {
        id: decode_column(row, "id")?,
        market_id: decode_column(row, "market_id")?,
        event_id: decode_column(row, "event_id")?,
        action: SignalAction::from_str(&action_raw).map_err(|error| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("failed to decode signal action: {error}"),
            )
        })?,
        side: SignalSide::from_str(&side_raw).map_err(|error| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("failed to decode signal side: {error}"),
            )
        })?,
        market_price: Probability::new(market_price).map_err(|error| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("failed to decode signal market_price: {error}"),
            )
        })?,
        fair_price: Probability::new(fair_price).map_err(|error| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("failed to decode signal fair_price: {error}"),
            )
        })?,
        edge: Edge::new(edge).map_err(|error| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("failed to decode signal edge: {error}"),
            )
        })?,
        confidence: Probability::new(confidence).map_err(|error| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("failed to decode signal confidence: {error}"),
            )
        })?,
        lifecycle_state: SignalLifecycleState::from_str(&lifecycle_state_raw).map_err(|error| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("failed to decode signal lifecycle_state: {error}"),
            )
        })?,
        reason: decode_column(row, "reason")?,
        risk_decision: decode_column(row, "risk_decision")?,
        evidence_ids: decode_column(row, "evidence_ids")?,
        approved_by_user_id: decode_column(row, "approved_by_user_id")?,
        approved_at: decode_column(row, "approved_at")?,
        rejected_by_user_id: decode_column(row, "rejected_by_user_id")?,
        rejected_at: decode_column(row, "rejected_at")?,
        updated_at: decode_column(row, "updated_at")?,
        version: decode_column(row, "version")?,
    })
}

fn parse_probability_estimate_row(row: &sqlx::postgres::PgRow) -> Result<ProbabilityEstimateView> {
    let prior_price: Decimal = decode_column(row, "prior_price")?;
    let posterior_price: Decimal = decode_column(row, "posterior_price")?;
    let fair_price: Decimal = decode_column(row, "fair_price")?;
    let market_price: Decimal = decode_column(row, "market_price")?;
    let edge: Decimal = decode_column(row, "edge")?;
    let confidence: Decimal = decode_column(row, "confidence")?;
    let time_horizon_raw: String = decode_column(row, "time_horizon")?;
    let reason_codes_json: Json<Vec<String>> = decode_column(row, "reason_codes_json")?;
    let evidence_count: i32 = decode_column(row, "evidence_count")?;

    Ok(ProbabilityEstimateView {
        id: decode_column(row, "id")?,
        market_id: decode_column(row, "market_id")?,
        event_id: decode_column(row, "event_id")?,
        signal_id: decode_column(row, "signal_id")?,
        prior_price: Probability::new(prior_price).map_err(|error| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("failed to decode prior_price: {error}"),
            )
        })?,
        posterior_price: Probability::new(posterior_price).map_err(|error| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("failed to decode posterior_price: {error}"),
            )
        })?,
        fair_price: Probability::new(fair_price).map_err(|error| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("failed to decode fair_price: {error}"),
            )
        })?,
        market_price: Probability::new(market_price).map_err(|error| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("failed to decode market_price: {error}"),
            )
        })?,
        edge: Edge::new(edge).map_err(|error| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("failed to decode estimate edge: {error}"),
            )
        })?,
        confidence: Probability::new(confidence).map_err(|error| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("failed to decode estimate confidence: {error}"),
            )
        })?,
        time_horizon: TimeHorizon::from_str(&time_horizon_raw).map_err(|error| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("failed to decode time_horizon: {error}"),
            )
        })?,
        model_version: decode_column(row, "model_version")?,
        reason_codes: reason_codes_json.0,
        evidence_count: evidence_count.max(0) as u32,
        created_at: decode_column(row, "created_at")?,
    })
}

fn parse_signal_transition_row(row: &sqlx::postgres::PgRow) -> Result<SignalTransitionView> {
    let from_state_raw: String = decode_column(row, "from_state")?;
    let to_state_raw: String = decode_column(row, "to_state")?;
    let trigger_payload_json: Json<Value> = decode_column(row, "trigger_payload_json")?;

    Ok(SignalTransitionView {
        id: decode_column(row, "id")?,
        signal_id: decode_column(row, "signal_id")?,
        from_state: SignalLifecycleState::from_str(&from_state_raw).map_err(|error| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("failed to decode from_state: {error}"),
            )
        })?,
        to_state: SignalLifecycleState::from_str(&to_state_raw).map_err(|error| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("failed to decode to_state: {error}"),
            )
        })?,
        trigger_type: decode_column(row, "trigger_type")?,
        trigger_payload: trigger_payload_json.0,
        created_at: decode_column(row, "created_at")?,
    })
}
