fn build_promoted_event_record(
    raw_event: &NewsRawEventView,
    related_market_ids: Vec<String>,
    health: Option<&NewsSourceHealthView>,
) -> Result<FixtureEventRecord> {
    let confidence = health
        .map(|health| health.health_score)
        .unwrap_or_else(|| default_news_confidence(&raw_event.source_type));
    let relevance_score =
        promotion_relevance_score(&raw_event.source_type, related_market_ids.len())?;

    Ok(FixtureEventRecord {
        id: promoted_event_id(raw_event),
        raw_event_id: Some(raw_event.id.clone()),
        source: raw_event.source.clone(),
        summary: raw_event.title.clone(),
        relevance_score,
        confidence,
        status: EventStatus::Active,
        related_market_ids,
        reason_trace: format!(
            "Promoted from raw news {} by source/title lexical market matching.",
            raw_event.id
        ),
        created_at: raw_event.event_time,
        updated_at: OffsetDateTime::now_utc(),
        version: 1,
    })
}

fn build_promoted_evidence_record(
    raw_event: &NewsRawEventView,
    market_id: &str,
    event_id: &str,
    health: Option<&NewsSourceHealthView>,
) -> Result<FixtureEvidenceRecord> {
    let direction = promoted_evidence_direction(raw_event);
    let source_reliability = health
        .map(|health| health.reliability)
        .unwrap_or_else(|| default_news_confidence(&raw_event.source_type));

    Ok(FixtureEvidenceRecord {
        id: promoted_evidence_id(raw_event, market_id),
        market_id: market_id.to_string(),
        event_id: event_id.to_string(),
        direction,
        strength: promotion_evidence_strength(&raw_event.source_type, direction),
        source_reliability,
        novelty: promotion_evidence_novelty(&raw_event.source_type),
        resolution_relevance: promotion_evidence_resolution_relevance(
            &raw_event.source_type,
            direction,
        ),
        status: EvidenceStatus::Active,
        expires_at: raw_event.event_time + promotion_evidence_ttl(&raw_event.source_type),
        created_at: raw_event.event_time,
        updated_at: OffsetDateTime::now_utc(),
        version: 1,
    })
}

fn match_raw_news_markets(raw_event: &NewsRawEventView, markets: &[MarketView]) -> Vec<String> {
    let raw_text = format!("{} {}", raw_event.title, raw_event.source);
    let raw_tokens = tokenize_match_text(&raw_text);
    let raw_lower = raw_text.to_ascii_lowercase();
    let mut matches = Vec::new();

    for market in markets {
        let market_text = format!(
            "{} {} {} {}",
            market.question,
            market.category,
            market.resolution_source,
            market.edge_case_notes.join(" ")
        );
        let market_tokens = tokenize_match_text(&market_text);
        let overlap = raw_tokens
            .iter()
            .filter(|token| market_tokens.contains(*token))
            .count();
        let category_match = raw_lower.contains(&market.category.to_ascii_lowercase());

        if overlap >= 2 || category_match || (raw_event.source_type == "official" && overlap >= 1) {
            matches.push(market.id.clone());
        }
    }

    matches
}

fn tokenize_match_text(value: &str) -> HashSet<String> {
    value
        .split(|character: char| !character.is_ascii_alphanumeric())
        .map(|token| token.trim().to_ascii_lowercase())
        .filter(|token| token.len() >= 3 && !is_news_match_stop_word(token))
        .collect()
}

fn is_news_match_stop_word(token: &str) -> bool {
    matches!(
        token,
        "the"
            | "and"
            | "for"
            | "with"
            | "will"
            | "was"
            | "were"
            | "from"
            | "into"
            | "after"
            | "before"
            | "above"
            | "below"
            | "market"
            | "markets"
            | "news"
            | "feed"
            | "watch"
            | "update"
            | "updated"
            | "reports"
            | "publishes"
    )
}

fn promoted_event_id(raw_event: &NewsRawEventView) -> String {
    let suffix = raw_event.hash.chars().take(24).collect::<String>();
    format!("evt_news_{suffix}")
}

fn promoted_evidence_id(raw_event: &NewsRawEventView, market_id: &str) -> String {
    let suffix = raw_event.hash.chars().take(24).collect::<String>();
    format!("evd_news_{market_id}_{suffix}")
}

fn promoted_evidence_direction(raw_event: &NewsRawEventView) -> EvidenceDirection {
    let lower_title = raw_event.title.to_ascii_lowercase();
    let tokens = tokenize_match_text(&lower_title);

    if tokens.iter().any(|token| {
        matches!(
            token.as_str(),
            "reject"
                | "rejects"
                | "rejected"
                | "denies"
                | "denied"
                | "denial"
                | "delay"
                | "delays"
                | "delayed"
                | "postpone"
                | "postpones"
                | "postponed"
                | "retract"
                | "retracted"
                | "withdraw"
                | "withdraws"
                | "withdrawn"
                | "concern"
                | "concerns"
                | "investigation"
                | "lawsuit"
                | "halts"
                | "blocks"
        )
    }) {
        return EvidenceDirection::SupportsNo;
    }

    if lower_title.contains("approval granted")
        || lower_title.contains("approved")
        || lower_title.contains("greenlight")
        || lower_title.contains("green-light")
        || tokens.iter().any(|token| {
            matches!(
                token.as_str(),
                "approve"
                    | "approves"
                    | "grants"
                    | "granted"
                    | "clears"
                    | "accepts"
                    | "authorizes"
                    | "authorized"
            )
        })
    {
        return EvidenceDirection::SupportsYes;
    }

    EvidenceDirection::Background
}

fn default_news_confidence(source_type: &str) -> Probability {
    match source_type {
        "official" => static_probability(78, 2),
        "calendar" => static_probability(66, 2),
        "market" => static_probability(62, 2),
        "social" => static_probability(48, 2),
        _ => static_probability(60, 2),
    }
}

fn promotion_relevance_score(
    source_type: &str,
    matched_market_count: usize,
) -> Result<Probability> {
    let base = match source_type {
        "official" => Decimal::new(72, 2),
        "calendar" => Decimal::new(62, 2),
        "market" => Decimal::new(58, 2),
        "social" => Decimal::new(45, 2),
        _ => Decimal::new(60, 2),
    };
    let boost = Decimal::new(
        (matched_market_count.saturating_sub(1).min(3) as i64) * 5,
        2,
    );

    Probability::new((base + boost).min(Decimal::new(90, 2)))
}

fn promotion_evidence_strength(source_type: &str, direction: EvidenceDirection) -> Probability {
    let is_directional = direction != EvidenceDirection::Background;
    match (source_type, is_directional) {
        ("official", true) => static_probability(34, 2),
        ("official", false) => static_probability(18, 2),
        ("calendar", true) => static_probability(26, 2),
        ("calendar", false) => static_probability(16, 2),
        ("market", true) => static_probability(22, 2),
        ("market", false) => static_probability(14, 2),
        ("social", true) => static_probability(12, 2),
        ("social", false) => static_probability(8, 2),
        (_, true) => static_probability(20, 2),
        (_, false) => static_probability(12, 2),
    }
}

fn promotion_evidence_novelty(source_type: &str) -> Probability {
    match source_type {
        "official" => static_probability(72, 2),
        "calendar" => static_probability(62, 2),
        "market" => static_probability(55, 2),
        "social" => static_probability(40, 2),
        _ => static_probability(50, 2),
    }
}

fn promotion_evidence_resolution_relevance(
    source_type: &str,
    direction: EvidenceDirection,
) -> Probability {
    let directional_boost = if direction == EvidenceDirection::Background {
        Decimal::ZERO
    } else {
        Decimal::new(8, 2)
    };
    let base = match source_type {
        "official" => Decimal::new(76, 2),
        "calendar" => Decimal::new(68, 2),
        "market" => Decimal::new(60, 2),
        "social" => Decimal::new(42, 2),
        _ => Decimal::new(55, 2),
    };

    static_probability_from_decimal((base + directional_boost).min(Decimal::new(90, 2)))
}

fn promotion_evidence_ttl(source_type: &str) -> TimeDuration {
    match source_type {
        "official" => TimeDuration::days(7),
        "calendar" => TimeDuration::days(3),
        "market" => TimeDuration::days(1),
        "social" => TimeDuration::hours(6),
        _ => TimeDuration::days(2),
    }
}

fn static_probability(value: i64, scale: u32) -> Probability {
    Probability::new(Decimal::new(value, scale))
        .expect("static worker probability default must be valid")
}

fn static_probability_from_decimal(value: Decimal) -> Probability {
    Probability::new(value).expect("static worker probability default must be valid")
}
