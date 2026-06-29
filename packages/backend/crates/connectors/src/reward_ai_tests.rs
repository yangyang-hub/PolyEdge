#[test]
fn reward_ai_confidence_is_clamped_to_unit_interval() {
    let high = parse_reward_ai_decision(
        r#"{
                "allow_quote": true,
                "confidence": 1.5,
                "reasons": [],
                "metrics": {}
            }"#,
    )
    .expect("parse high confidence");
    assert_eq!(high.confidence, Decimal::ONE);

    let low = parse_reward_ai_decision(
        r#"{
                "allow_quote": false,
                "confidence": "-0.2",
                "reasons": [],
                "metrics": {}
            }"#,
    )
    .expect("parse low confidence");
    assert_eq!(low.confidence, Decimal::ZERO);
    assert_eq!(low.suitability, RewardAiSuitability::Avoid);
    assert_eq!(low.quote_mode, RewardPlanQuoteMode::None);
}

#[test]
fn reward_ai_parse_binary_allow_quote_decision() {
    let parsed = parse_reward_ai_decision(
        r#"{"allow_quote":true,"confidence":0.82,"strategy_hint":{"quote_mode":"single_yes","bid_rank":2,"max_condition_notional_usd":12.5},"reasons":["pricing acceptable for ttl"],"metrics":{"edge":"ok"}}"#,
    )
    .expect("parse binary response");

    assert_eq!(parsed.suitability, RewardAiSuitability::Allow);
    assert_eq!(parsed.quote_mode, RewardPlanQuoteMode::Double);
    assert_eq!(parsed.confidence, Decimal::from_str("0.82").unwrap());
    assert_eq!(parsed.reasons, vec!["pricing acceptable for ttl"]);
    assert_eq!(
        parsed.metrics.pointer("/strategy_hint/quote_mode"),
        Some(&serde_json::json!("single_yes"))
    );
    assert_eq!(
        parsed.metrics.pointer("/strategy_hint/bid_rank"),
        Some(&serde_json::json!(2))
    );
}

#[test]
fn reward_ai_parse_skips_embedded_example_object() {
    let parsed = parse_reward_ai_decision(
        r#"Example shape: {"example": true}
Final:
{"suitability":"allow","quote_mode":"double","exit_policy":"exit_at_markup","confidence":0.8,"reasons":[],"metrics":{}}
"#,
    )
    .expect("parse embedded response");

    assert_eq!(parsed.suitability, RewardAiSuitability::Allow);
    assert_eq!(parsed.quote_mode, RewardPlanQuoteMode::Double);
}

#[test]
fn reward_ai_parse_accepts_json_string_payload() {
    // A legacy `suitability` payload wrapped in a JSON string is still
    // extracted, but a non-allow verdict is fail-closed: `watch` collapses to
    // `avoid` so the advisory gate blocks the market instead of letting an
    // unendorsed market through.
    let parsed = parse_reward_ai_decision(
        r#""{\"suitability\":\"watch\",\"quote_mode\":\"none\",\"exit_policy\":\"flatten_immediately\",\"confidence\":0.4,\"reasons\":[],\"metrics\":{}}""#,
    )
    .expect("parse json string payload");

    assert_eq!(parsed.suitability, RewardAiSuitability::Avoid);
    assert_eq!(parsed.quote_mode, RewardPlanQuoteMode::None);
    assert_eq!(parsed.confidence, Decimal::from_str("0.4").unwrap());
}

#[test]
fn reward_ai_parse_legacy_suitability_is_fail_closed() {
    // Legacy 3-way `suitability` responses honour binary fail-closed semantics:
    // only an explicit `allow` is treated as allowed; `watch`, `avoid` and any
    // other non-allow verdict collapse to `avoid` with the canonical block shape.
    let watch = parse_reward_ai_decision(
        r#"{"suitability":"watch","quote_mode":"double","exit_policy":"hold_and_requote","confidence":0.68,"reasons":["uncertain"],"metrics":{}}"#,
    )
    .expect("parse legacy watch");
    assert_eq!(watch.suitability, RewardAiSuitability::Avoid);
    assert_eq!(watch.quote_mode, RewardPlanQuoteMode::None);

    let avoid = parse_reward_ai_decision(
        r#"{"suitability":"avoid","quote_mode":"none","exit_policy":"flatten_immediately","confidence":0.3,"reasons":[],"metrics":{}}"#,
    )
    .expect("parse legacy avoid");
    assert_eq!(avoid.suitability, RewardAiSuitability::Avoid);

    let allow = parse_reward_ai_decision(
        r#"{"suitability":"allow","quote_mode":"double","exit_policy":"exit_at_markup","confidence":0.9,"reasons":[],"metrics":{}}"#,
    )
    .expect("parse legacy allow");
    assert_eq!(allow.suitability, RewardAiSuitability::Allow);
    assert_eq!(allow.quote_mode, RewardPlanQuoteMode::Double);
}
