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
    assert_eq!(low.action, RewardProviderAction::StopNew);
}

#[test]
fn reward_ai_parse_binary_allow_quote_decision() {
    let parsed = parse_reward_ai_decision(
        r#"{"allow_quote":true,"confidence":0.82,"strategy_hint":{"quote_mode":"single_yes","bid_rank":2,"max_condition_notional_usd":12.5},"reasons":["pricing acceptable for ttl"],"metrics":{"edge":"ok"}}"#,
    )
    .expect("parse binary response");

    assert_eq!(parsed.action, RewardProviderAction::Allow);
    assert_eq!(parsed.confidence, Decimal::from_str("0.82").unwrap());
    assert_eq!(parsed.reasons, vec!["pricing acceptable for ttl"]);
    assert!(parsed.metrics.pointer("/strategy_hint").is_none());
}

#[test]
fn reward_ai_parse_v2_bounded_risk_adjustment() {
    let parsed = parse_reward_ai_decision(
        r#"{"action":"reduce","size_multiplier":0.45,"edge_buffer_cents":1.25,"confidence":0.84,"reasons":["structural ambiguity"],"metrics":{}}"#,
    )
    .expect("parse v2 response");

    assert_eq!(parsed.action, RewardProviderAction::Reduce);
    assert_eq!(parsed.size_multiplier, Decimal::from_str("0.45").unwrap());
    assert_eq!(parsed.edge_buffer_cents, Decimal::from_str("1.25").unwrap());
}

#[test]
fn reward_ai_v2_action_normalizes_inapplicable_modifiers() {
    let allow = parse_reward_ai_decision(
        r#"{"action":"allow","size_multiplier":0.2,"edge_buffer_cents":9,"confidence":0.9,"reasons":[],"metrics":{}}"#,
    )
    .expect("parse allow response");
    assert_eq!(allow.size_multiplier, Decimal::ONE);
    assert_eq!(allow.edge_buffer_cents, Decimal::ZERO);

    let stop = parse_reward_ai_decision(
        r#"{"action":"stop_new","size_multiplier":1,"edge_buffer_cents":9,"confidence":0.9,"reasons":[],"metrics":{}}"#,
    )
    .expect("parse stop-new response");
    assert_eq!(stop.size_multiplier, Decimal::ZERO);
    assert_eq!(stop.edge_buffer_cents, Decimal::ZERO);
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

    assert_eq!(parsed.action, RewardProviderAction::Allow);
}

#[test]
fn reward_ai_parse_accepts_json_string_payload() {
    // A legacy `suitability` payload wrapped in a JSON string is still
    // extracted, but a non-allow verdict is fail-closed: `watch` becomes
    // `stop_new` so the advisory gate blocks the market instead of letting an
    // unendorsed market through.
    let parsed = parse_reward_ai_decision(
        r#""{\"suitability\":\"watch\",\"quote_mode\":\"none\",\"exit_policy\":\"flatten_immediately\",\"confidence\":0.4,\"reasons\":[],\"metrics\":{}}""#,
    )
    .expect("parse json string payload");

    assert_eq!(parsed.action, RewardProviderAction::StopNew);
    assert_eq!(parsed.confidence, Decimal::from_str("0.4").unwrap());
}

#[test]
fn reward_ai_parse_legacy_suitability_is_fail_closed() {
    // Legacy 3-way `suitability` responses honour binary fail-closed semantics:
    // only an explicit `allow` is treated as allowed; `watch`, `avoid` and any
    // other non-allow verdicts become the canonical stop-new action.
    let watch = parse_reward_ai_decision(
        r#"{"suitability":"watch","quote_mode":"double","exit_policy":"hold_and_requote","confidence":0.68,"reasons":["uncertain"],"metrics":{}}"#,
    )
    .expect("parse legacy watch");
    assert_eq!(watch.action, RewardProviderAction::StopNew);

    let avoid = parse_reward_ai_decision(
        r#"{"suitability":"avoid","quote_mode":"none","exit_policy":"flatten_immediately","confidence":0.3,"reasons":[],"metrics":{}}"#,
    )
    .expect("parse legacy avoid");
    assert_eq!(avoid.action, RewardProviderAction::StopNew);

    let allow = parse_reward_ai_decision(
        r#"{"suitability":"allow","quote_mode":"double","exit_policy":"exit_at_markup","confidence":0.9,"reasons":[],"metrics":{}}"#,
    )
    .expect("parse legacy allow");
    assert_eq!(allow.action, RewardProviderAction::Allow);
}
