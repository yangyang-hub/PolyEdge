#[cfg(any(test, feature = "test-fixtures"))]
#[must_use]
pub fn demo_fixture_bundle() -> FixtureBundle {
    FixtureBundle {
        markets: vec![
            fixture_market(
                "mkt_120",
                "Will BTC close above 95k on Apr 30?",
                "Crypto",
                MarketStatus::Open,
                "0.51",
                "0.53",
                "0.52",
                "125000.00",
                AmbiguityLevel::Low,
                TradabilityStatus::Tradable,
                "Polymarket BTC settlement reference close.",
                &[
                    "Low ambiguity market.",
                    "Resolution uses published close methodology.",
                ],
                Some("0x0000000000000000000000000000000000000000000000000000000000000120"),
                Some("120001"),
                Some("120002"),
                "2026-04-16T14:30:00Z",
                12,
            ),
            fixture_market(
                "mkt_121",
                "Will SEC approve ETH staking ETF by Q2?",
                "Regulation",
                MarketStatus::Open,
                "0.40",
                "0.42",
                "0.41",
                "98400.00",
                AmbiguityLevel::Medium,
                TradabilityStatus::ManualReview,
                "Official SEC filing or public approval announcement.",
                &[
                    "Delayed filing language may not equal approval.",
                    "Conditional launch wording requires operator review.",
                    "Partial scope approval should be escalated.",
                ],
                Some("0x0000000000000000000000000000000000000000000000000000000000000121"),
                Some("121001"),
                Some("121002"),
                "2026-04-16T14:30:00Z",
                9,
            ),
            fixture_market(
                "mkt_122",
                "Will the Fed cut rates in June?",
                "Macro",
                MarketStatus::Open,
                "0.62",
                "0.64",
                "0.63",
                "141200.00",
                AmbiguityLevel::High,
                TradabilityStatus::ObserveOnly,
                "FOMC target rate decision.",
                &[
                    "Interpretation risk around corridor adjustments.",
                    "Observe only until macro source stabilizes.",
                ],
                Some("0x0000000000000000000000000000000000000000000000000000000000000122"),
                Some("122001"),
                Some("122002"),
                "2026-04-16T14:30:00Z",
                7,
            ),
            fixture_market(
                "mkt_123",
                "Will the White House publish a formal AI executive order by May 31?",
                "Policy",
                MarketStatus::Open,
                "0.36",
                "0.39",
                "0.38",
                "54200.00",
                AmbiguityLevel::High,
                TradabilityStatus::Blocked,
                "Official White House release log and executive order registry.",
                &[
                    "Draft memos do not satisfy settlement unless a formal executive order is published.",
                    "Block automation until the official publication source is stable again.",
                ],
                Some("0x0000000000000000000000000000000000000000000000000000000000000123"),
                Some("123001"),
                Some("123002"),
                "2026-04-16T14:30:00Z",
                5,
            ),
        ],
        events: vec![
            fixture_event(
                "evt_9001",
                "reuters",
                "Senior SEC staff signals concerns over ETH staking disclosures.",
                "0.81",
                "0.78",
                EventStatus::Active,
                &["mkt_121"],
                "Official language changes settlement path relevance and supports a lower approval probability.",
                "2026-04-16T13:42:00Z",
                "2026-04-16T14:30:00Z",
                3,
            ),
            fixture_event(
                "evt_9002",
                "fomc_calendar",
                "Fed speakers reinforce patience narrative ahead of June meeting.",
                "0.74",
                "0.69",
                EventStatus::Superseded,
                &["mkt_122"],
                "Original macro take was superseded by newer desk notes after rate path wording changed.",
                "2026-04-16T13:27:00Z",
                "2026-04-16T14:30:00Z",
                4,
            ),
            fixture_event(
                "evt_9003",
                "x_whitelist",
                "Market influencers push BTC breakout narrative after ETF inflows.",
                "0.46",
                "0.44",
                EventStatus::Expired,
                &["mkt_120"],
                "Social chatter is directionally aligned, but evidence quality is too weak for autonomous weighting.",
                "2026-04-16T13:08:00Z",
                "2026-04-16T14:30:00Z",
                2,
            ),
            fixture_event(
                "evt_9004",
                "official_gov_feed",
                "Draft policy memo was retracted after publication metadata proved incorrect.",
                "0.67",
                "0.72",
                EventStatus::Invalidated,
                &["mkt_123"],
                "Upstream source inconsistency invalidates the settlement path assumption and blocks automation.",
                "2026-04-16T12:51:00Z",
                "2026-04-16T14:30:00Z",
                2,
            ),
        ],
        evidences: vec![
            fixture_evidence(
                "evd_5001",
                "mkt_121",
                "evt_9001",
                EvidenceDirection::SupportsNo,
                "0.34",
                "0.90",
                "0.80",
                "0.91",
                EvidenceStatus::Active,
                "2026-04-16T18:30:00Z",
                "2026-04-16T13:43:00Z",
                "2026-04-16T14:30:00Z",
                2,
            ),
            fixture_evidence(
                "evd_5002",
                "mkt_121",
                "evt_9001",
                EvidenceDirection::Background,
                "0.18",
                "0.42",
                "0.55",
                "0.30",
                EvidenceStatus::Active,
                "2026-04-16T16:00:00Z",
                "2026-04-16T13:44:00Z",
                "2026-04-16T14:30:00Z",
                1,
            ),
            fixture_evidence(
                "evd_5003",
                "mkt_122",
                "evt_9002",
                EvidenceDirection::SupportsNo,
                "0.27",
                "0.86",
                "0.52",
                "0.88",
                EvidenceStatus::Active,
                "2026-04-16T20:00:00Z",
                "2026-04-16T13:30:00Z",
                "2026-04-16T14:30:00Z",
                2,
            ),
            fixture_evidence(
                "evd_5004",
                "mkt_120",
                "evt_9003",
                EvidenceDirection::SupportsYes,
                "0.12",
                "0.35",
                "0.41",
                "0.25",
                EvidenceStatus::Active,
                "2026-04-16T15:00:00Z",
                "2026-04-16T13:10:00Z",
                "2026-04-16T14:30:00Z",
                1,
            ),
        ],
        signals: vec![
            fixture_signal(
                "sig_2411",
                "mkt_120",
                "evt_9003",
                SignalAction::Buy,
                SignalSide::Yes,
                "0.52",
                "0.58",
                "0.06",
                "0.88",
                SignalLifecycleState::Active,
                "ETF inflow narrative still supports underpriced upside participation.",
                "Eligible for automated execution under current bucket limits.",
                &["evd_5004"],
                "2026-04-16T14:30:00Z",
                9,
            ),
            fixture_signal(
                "sig_2412",
                "mkt_121",
                "evt_9001",
                SignalAction::Buy,
                SignalSide::No,
                "0.41",
                "0.35",
                "-0.06",
                "0.62",
                SignalLifecycleState::New,
                "Official update increases review-delay odds more than current price reflects.",
                "Signal is queued for manual review because settlement ambiguity is medium and theme exposure is elevated.",
                &["evd_5001", "evd_5002"],
                "2026-04-16T14:30:00Z",
                9,
            ),
            fixture_signal(
                "sig_2413",
                "mkt_120",
                "evt_9003",
                SignalAction::Buy,
                SignalSide::Yes,
                "0.28",
                "0.30",
                "0.02",
                "0.44",
                SignalLifecycleState::Weakened,
                "Momentum evidence remains directionally positive but confidence decayed after contradictory flow.",
                "Watch only until confidence recovers above activation threshold.",
                &["evd_5004"],
                "2026-04-16T14:30:00Z",
                4,
            ),
            fixture_signal(
                "sig_2414",
                "mkt_122",
                "evt_9002",
                SignalAction::Buy,
                SignalSide::No,
                "0.63",
                "0.57",
                "-0.06",
                "0.53",
                SignalLifecycleState::Reversed,
                "Macro drift remains negative for cuts, but live macro feed instability invalidates autonomous posture.",
                "Reversed to manual monitoring because upstream data quality is degraded.",
                &["evd_5003"],
                "2026-04-16T14:30:00Z",
                6,
            ),
        ],
    }
}

fn validate_limit(limit: Option<u16>) -> Result<u16> {
    crate::list_filters::validate_list_limit(
        limit,
        DEFAULT_LIST_LIMIT,
        MAX_LIST_LIMIT,
        "LIST_LIMIT_INVALID",
        "list limit must be greater than zero",
        "LIST_LIMIT_TOO_LARGE",
        format!("list limit must be at most {MAX_LIST_LIMIT}"),
    )
}

fn validate_optional_id(field: &'static str, value: Option<String>) -> Result<Option<String>> {
    crate::list_filters::normalize_optional_filter_id(
        field,
        value,
        "LIST_FILTER_INVALID",
        |field| format!("{field} must not be empty when provided"),
    )
}

fn compute_evidence_signal(
    evidences: &[EvidenceView],
    reference_time: OffsetDateTime,
    source_health_score: Decimal,
) -> (Decimal, Decimal) {
    if evidences.is_empty() {
        return (Decimal::ZERO, Decimal::ZERO);
    }

    let mut weighted_delta = Decimal::ZERO;
    let mut quality_sum = Decimal::ZERO;

    for evidence in evidences {
        let total_window_secs = (evidence.expires_at - evidence.created_at)
            .whole_seconds()
            .max(1);
        let remaining_secs = (evidence.expires_at - reference_time).whole_seconds();
        let clamped_remaining = remaining_secs.clamp(0, total_window_secs);
        let freshness_decay = Decimal::from(clamped_remaining) / Decimal::from(total_window_secs);
        let effective_source_reliability =
            evidence.source_reliability.value() * source_health_score;
        let weight = evidence.strength.value()
            * effective_source_reliability
            * evidence.novelty.value()
            * evidence.resolution_relevance.value()
            * freshness_decay;

        let direction_multiplier = match evidence.direction {
            EvidenceDirection::SupportsYes => Decimal::ONE,
            EvidenceDirection::SupportsNo => -Decimal::ONE,
            EvidenceDirection::Background => Decimal::ZERO,
        };

        weighted_delta += weight * direction_multiplier;
        quality_sum += ((effective_source_reliability
            + evidence.novelty.value()
            + evidence.resolution_relevance.value())
            / dec("3"))
            * freshness_decay;
    }

    let avg_quality = quality_sum / Decimal::from(evidences.len() as i64);
    (weighted_delta, avg_quality)
}

fn derive_time_horizon(evidences: &[EvidenceView], reference_time: OffsetDateTime) -> TimeHorizon {
    let Some(min_remaining_secs) = evidences
        .iter()
        .map(|evidence| {
            (evidence.expires_at - reference_time)
                .whole_seconds()
                .max(0)
        })
        .min()
    else {
        return TimeHorizon::Short;
    };

    if min_remaining_secs <= 6 * 60 * 60 {
        TimeHorizon::Short
    } else if min_remaining_secs <= 24 * 60 * 60 {
        TimeHorizon::Medium
    } else {
        TimeHorizon::Long
    }
}

fn derive_reason_codes(
    market: &MarketView,
    evidences: &[EvidenceView],
    edge: Edge,
    confidence: Probability,
    evidence_count: usize,
    source_health: Option<&SourceHealthAdjustment>,
) -> Vec<String> {
    let mut reason_codes = Vec::new();
    let source_health_score = source_health
        .map(|adjustment| adjustment.health_score.value())
        .unwrap_or(Decimal::ONE);

    if evidences
        .iter()
        .any(|evidence| evidence.source_reliability.value() * source_health_score >= dec("0.85"))
    {
        reason_codes.push("official_source".to_string());
    }

    if source_health.is_some_and(|adjustment| adjustment.health_score.value() < dec("0.75")) {
        reason_codes.push("source_health_degraded".to_string());
    }

    if evidence_count >= 2 {
        reason_codes.push("corroborated".to_string());
    }

    if edge.value().abs() >= dec("0.05") {
        reason_codes.push("material_update".to_string());
    }

    if confidence.value() < dec("0.45") {
        reason_codes.push("low_confidence".to_string());
    }

    if market.ambiguity_level == AmbiguityLevel::High {
        reason_codes.push("high_ambiguity".to_string());
    }

    if market.tradability_status == TradabilityStatus::Blocked {
        reason_codes.push("blocked_market".to_string());
    }

    if evidences.is_empty() {
        reason_codes.push("no_active_evidence".to_string());
    }

    if reason_codes.is_empty() {
        reason_codes.push("steady_state".to_string());
    }

    reason_codes
}

fn derive_signal_lifecycle_state(
    directional_edge: Decimal,
    confidence: Probability,
    evidence_count: usize,
    tradability_status: TradabilityStatus,
) -> SignalLifecycleState {
    if evidence_count == 0 {
        return SignalLifecycleState::Expired;
    }

    if tradability_status == TradabilityStatus::Blocked {
        return SignalLifecycleState::Invalidated;
    }

    let edge_abs = directional_edge.abs();
    if confidence.value() < dec("0.35") {
        SignalLifecycleState::Invalidated
    } else if edge_abs >= dec("0.05") && confidence.value() >= dec("0.55") {
        SignalLifecycleState::Active
    } else if edge_abs >= dec("0.02") && confidence.value() >= dec("0.45") {
        SignalLifecycleState::New
    } else {
        SignalLifecycleState::Weakened
    }
}

fn derive_risk_decision(
    tradability_status: TradabilityStatus,
    lifecycle_state: SignalLifecycleState,
) -> String {
    match tradability_status {
        TradabilityStatus::Blocked => {
            "Blocked by tradability status; do not release to execution.".to_string()
        }
        TradabilityStatus::ObserveOnly => {
            "Observe only until posterior stabilizes and tradability restrictions are lifted."
                .to_string()
        }
        TradabilityStatus::ManualReview => {
            "Manual review required before downstream risk evaluation.".to_string()
        }
        TradabilityStatus::Tradable => match lifecycle_state {
            SignalLifecycleState::Active => {
                "Eligible for downstream risk evaluation under current tradability settings."
                    .to_string()
            }
            SignalLifecycleState::New => {
                "Queue for risk evaluation after next posterior refresh.".to_string()
            }
            _ => "Watch only until posterior strengthens.".to_string(),
        },
    }
}

fn probability_from_decimal(value: Decimal) -> Result<Probability> {
    Probability::new(clamp_zero_one(value))
}

fn clamp_zero_one(value: Decimal) -> Decimal {
    value.clamp(Decimal::ZERO, Decimal::ONE)
}

fn max_time(left: OffsetDateTime, right: OffsetDateTime) -> OffsetDateTime {
    if left >= right { left } else { right }
}

fn dec(raw: &str) -> Decimal {
    Decimal::from_str(raw).expect("static decimal must be valid")
}

#[cfg(any(test, feature = "test-fixtures"))]
fn fixture_market(
    id: &str,
    question: &str,
    category: &str,
    status: MarketStatus,
    best_bid: &str,
    best_ask: &str,
    mid_price: &str,
    volume_24h: &str,
    ambiguity_level: AmbiguityLevel,
    tradability_status: TradabilityStatus,
    resolution_source: &str,
    edge_case_notes: &[&str],
    polymarket_condition_id: Option<&str>,
    polymarket_yes_asset_id: Option<&str>,
    polymarket_no_asset_id: Option<&str>,
    updated_at: &str,
    version: i64,
) -> FixtureMarketRecord {
    FixtureMarketRecord {
        id: id.to_string(),
        slug: None,
        question: question.to_string(),
        category: category.to_string(),
        status,
        best_bid: probability(best_bid),
        best_ask: probability(best_ask),
        mid_price: probability(mid_price),
        volume_24h: usd_amount(volume_24h),
        ambiguity_level,
        tradability_status,
        resolution_source: resolution_source.to_string(),
        edge_case_notes: edge_case_notes.iter().map(ToString::to_string).collect(),
        polymarket_condition_id: polymarket_condition_id.map(ToString::to_string),
        polymarket_yes_asset_id: polymarket_yes_asset_id.map(ToString::to_string),
        polymarket_no_asset_id: polymarket_no_asset_id.map(ToString::to_string),
        updated_at: timestamp(updated_at),
        version,
    }
}

#[cfg(any(test, feature = "test-fixtures"))]
fn fixture_event(
    id: &str,
    source: &str,
    summary: &str,
    relevance_score: &str,
    confidence: &str,
    status: EventStatus,
    related_market_ids: &[&str],
    reason_trace: &str,
    created_at: &str,
    updated_at: &str,
    version: i64,
) -> FixtureEventRecord {
    FixtureEventRecord {
        id: id.to_string(),
        raw_event_id: None,
        source: source.to_string(),
        summary: summary.to_string(),
        relevance_score: probability(relevance_score),
        confidence: probability(confidence),
        status,
        related_market_ids: related_market_ids.iter().map(ToString::to_string).collect(),
        reason_trace: reason_trace.to_string(),
        created_at: timestamp(created_at),
        updated_at: timestamp(updated_at),
        version,
    }
}

#[cfg(any(test, feature = "test-fixtures"))]
fn fixture_evidence(
    id: &str,
    market_id: &str,
    event_id: &str,
    direction: EvidenceDirection,
    strength: &str,
    source_reliability: &str,
    novelty: &str,
    resolution_relevance: &str,
    status: EvidenceStatus,
    expires_at: &str,
    created_at: &str,
    updated_at: &str,
    version: i64,
) -> FixtureEvidenceRecord {
    FixtureEvidenceRecord {
        id: id.to_string(),
        market_id: market_id.to_string(),
        event_id: event_id.to_string(),
        direction,
        strength: probability(strength),
        source_reliability: probability(source_reliability),
        novelty: probability(novelty),
        resolution_relevance: probability(resolution_relevance),
        status,
        expires_at: timestamp(expires_at),
        created_at: timestamp(created_at),
        updated_at: timestamp(updated_at),
        version,
    }
}

#[cfg(any(test, feature = "test-fixtures"))]
fn fixture_signal(
    id: &str,
    market_id: &str,
    event_id: &str,
    action: SignalAction,
    side: SignalSide,
    market_price: &str,
    fair_price: &str,
    edge_value: &str,
    confidence: &str,
    lifecycle_state: SignalLifecycleState,
    reason: &str,
    risk_decision: &str,
    evidence_ids: &[&str],
    updated_at: &str,
    version: i64,
) -> FixtureSignalRecord {
    FixtureSignalRecord {
        id: id.to_string(),
        market_id: market_id.to_string(),
        event_id: event_id.to_string(),
        action,
        side,
        market_price: probability(market_price),
        fair_price: probability(fair_price),
        edge: edge(edge_value),
        confidence: probability(confidence),
        lifecycle_state,
        reason: reason.to_string(),
        risk_decision: risk_decision.to_string(),
        evidence_ids: evidence_ids.iter().map(ToString::to_string).collect(),
        approved_by_user_id: None,
        approved_at: None,
        rejected_by_user_id: None,
        rejected_at: None,
        updated_at: timestamp(updated_at),
        version,
    }
}

#[cfg(any(test, feature = "test-fixtures"))]
fn probability(raw: &str) -> Probability {
    Probability::new(Decimal::from_str(raw).expect("fixture decimal must be valid"))
        .expect("fixture probability must be valid")
}

#[cfg(any(test, feature = "test-fixtures"))]
fn edge(raw: &str) -> Edge {
    Edge::new(Decimal::from_str(raw).expect("fixture decimal must be valid"))
        .expect("fixture edge must be valid")
}

#[cfg(any(test, feature = "test-fixtures"))]
fn usd_amount(raw: &str) -> UsdAmount {
    UsdAmount::new(Decimal::from_str(raw).expect("fixture decimal must be valid"))
        .expect("fixture usd amount must be valid")
}

#[cfg(any(test, feature = "test-fixtures"))]
fn timestamp(raw: &str) -> OffsetDateTime {
    OffsetDateTime::parse(raw, &time::format_description::well_known::Rfc3339)
        .expect("fixture timestamp must be valid")
}
