pub fn build_recompute_signal_draft(
    signal: &SignalView,
    market: &MarketView,
    evidences: &[EvidenceView],
    recompute_reason: &str,
    estimate_id: impl Into<String>,
) -> Result<RecomputeSignalDraft> {
    build_recompute_signal_draft_with_source_health(
        signal,
        market,
        evidences,
        recompute_reason,
        None,
        estimate_id,
    )
}

pub fn build_recompute_signal_draft_with_source_health(
    signal: &SignalView,
    market: &MarketView,
    evidences: &[EvidenceView],
    recompute_reason: &str,
    source_health: Option<&SourceHealthAdjustment>,
    estimate_id: impl Into<String>,
) -> Result<RecomputeSignalDraft> {
    let active_evidences: Vec<_> = evidences
        .iter()
        .filter(|evidence| evidence.status == EvidenceStatus::Active)
        .cloned()
        .collect();

    let reference_time = active_evidences
        .iter()
        .map(|evidence| evidence.updated_at)
        .fold(max_time(signal.updated_at, market.updated_at), max_time);

    let evidence_count = active_evidences.len();
    let prior_price = market.mid_price;
    let market_price = market.mid_price;

    let source_health_score = source_health
        .map(|adjustment| adjustment.health_score.value())
        .unwrap_or(Decimal::ONE);
    let (weighted_delta, avg_signal_quality) =
        compute_evidence_signal(&active_evidences, reference_time, source_health_score);
    let ambiguity_factor = match market.ambiguity_level {
        AmbiguityLevel::Low => dec("1.00"),
        AmbiguityLevel::Medium => dec("0.85"),
        AmbiguityLevel::High => dec("0.70"),
    };
    let posterior_raw =
        clamp_zero_one(prior_price.value() + weighted_delta * ambiguity_factor * dec("0.25"));
    let posterior_price = probability_from_decimal(posterior_raw)?;
    let fair_price = posterior_price;
    let edge = Edge::new(fair_price.value() - market_price.value())?;

    let ambiguity_penalty = match market.ambiguity_level {
        AmbiguityLevel::Low => Decimal::ZERO,
        AmbiguityLevel::Medium => dec("0.05"),
        AmbiguityLevel::High => dec("0.10"),
    };
    let tradability_penalty = match market.tradability_status {
        TradabilityStatus::Tradable => Decimal::ZERO,
        TradabilityStatus::ManualReview => dec("0.04"),
        TradabilityStatus::ObserveOnly => dec("0.08"),
        TradabilityStatus::Blocked => dec("0.12"),
    };
    let confidence_raw = clamp_zero_one(
        dec("0.35") + avg_signal_quality * dec("0.40") - ambiguity_penalty - tradability_penalty,
    );
    let confidence = probability_from_decimal(confidence_raw)?;

    let time_horizon = derive_time_horizon(&active_evidences, reference_time);
    let reason_codes = derive_reason_codes(
        market,
        &active_evidences,
        edge,
        confidence,
        evidence_count,
        source_health,
    );
    let next_side = if edge.value() >= Decimal::ZERO {
        SignalSide::Yes
    } else {
        SignalSide::No
    };
    let directional_edge = if next_side == SignalSide::Yes {
        edge.value()
    } else {
        -edge.value()
    };
    let next_state = derive_signal_lifecycle_state(
        directional_edge,
        confidence,
        evidence_count,
        market.tradability_status,
    );
    let risk_decision = derive_risk_decision(market.tradability_status, next_state);
    let reason = format!(
        "posterior recomputed from {} active evidence(s): {}",
        evidence_count,
        reason_codes.join(", ")
    );

    let estimate = ProbabilityEstimateView {
        id: estimate_id.into(),
        market_id: signal.market_id.clone(),
        event_id: signal.event_id.clone(),
        signal_id: Some(signal.id.clone()),
        prior_price,
        posterior_price,
        fair_price,
        market_price,
        edge,
        confidence,
        time_horizon,
        model_version: "v1_evidence_weighted".to_string(),
        reason_codes: reason_codes.clone(),
        evidence_count: u32::try_from(evidence_count).unwrap_or(u32::MAX),
        created_at: reference_time,
    };

    let next_signal = SignalView {
        id: signal.id.clone(),
        market_id: signal.market_id.clone(),
        event_id: signal.event_id.clone(),
        action: SignalAction::Buy,
        side: next_side,
        market_price,
        fair_price,
        edge,
        confidence,
        lifecycle_state: next_state,
        reason,
        risk_decision,
        evidence_ids: active_evidences
            .into_iter()
            .map(|evidence| evidence.id)
            .collect(),
        approved_by_user_id: None,
        approved_at: None,
        rejected_by_user_id: None,
        rejected_at: None,
        updated_at: reference_time,
        version: signal.version + 1,
    };

    let transition = if signal.lifecycle_state != next_state {
        Some(SignalTransitionDraft {
            from_state: signal.lifecycle_state,
            to_state: next_state,
            trigger_type: "recompute".to_string(),
            trigger_payload: json!({
                "reason": recompute_reason,
                "estimate_id": estimate.id,
                "reason_codes": reason_codes,
                "prior_price": estimate.prior_price,
                "posterior_price": estimate.posterior_price,
                "market_tradability_status": market.tradability_status,
                "source_health": source_health.map(|adjustment| json!({
                    "source": adjustment.source,
                    "health_score": adjustment.health_score,
                })),
            }),
            created_at: reference_time,
        })
    } else {
        None
    };

    Ok(RecomputeSignalDraft {
        next_signal,
        estimate,
        transition,
    })
}
