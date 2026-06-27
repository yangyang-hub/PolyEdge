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
        r#"{"allow_quote":true,"confidence":0.82,"reasons":["pricing acceptable for ttl"],"metrics":{"edge":"ok"}}"#,
    )
    .expect("parse binary response");

    assert_eq!(parsed.suitability, RewardAiSuitability::Allow);
    assert_eq!(parsed.quote_mode, RewardPlanQuoteMode::Double);
    assert_eq!(parsed.confidence, Decimal::from_str("0.82").unwrap());
    assert_eq!(parsed.reasons, vec!["pricing acceptable for ttl"]);
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

#[test]
fn reward_ai_batch_parse_full_array() {
    let items = parse_reward_ai_batch_decision(
        r#"{"advisories":[
                {"condition_id":"c1","allow_quote":true,"confidence":0.8,"reasons":[],"metrics":{}},
                {"condition_id":"c2","allow_quote":false,"confidence":0.4,"reasons":[],"metrics":{}}
            ]}"#,
        &["c1".to_string(), "c2".to_string()],
    )
    .expect("parse full batch array");
    assert_eq!(items.len(), 2);
    assert_eq!(items[0].condition_id, "c1");
    assert_eq!(items[0].decision.suitability, RewardAiSuitability::Allow);
    assert_eq!(items[1].condition_id, "c2");
    assert_eq!(items[1].decision.suitability, RewardAiSuitability::Avoid);
}

#[test]
fn reward_ai_batch_parse_matches_by_condition_id_regardless_of_order() {
    let items = parse_reward_ai_batch_decision(
        r#"{"advisories":[
                {"condition_id":"c2","suitability":"avoid","quote_mode":"none","exit_policy":"flatten_immediately","confidence":0.2,"reasons":[],"metrics":{}},
                {"condition_id":"c1","suitability":"allow","quote_mode":"double","exit_policy":"exit_at_markup","confidence":0.9,"reasons":[],"metrics":{}}
            ]}"#,
        &["c1".to_string(), "c2".to_string()],
    )
    .expect("parse reordered batch");
    assert_eq!(items.len(), 2);
    let by_id: std::collections::HashMap<&str, &RewardAiAdvisoryDecision> = items
        .iter()
        .map(|item| (item.condition_id.as_str(), &item.decision))
        .collect();
    assert_eq!(by_id["c1"].suitability, RewardAiSuitability::Allow);
    assert_eq!(by_id["c2"].suitability, RewardAiSuitability::Avoid);
}

#[test]
fn reward_ai_batch_parse_drops_unknown_and_duplicate_condition_ids() {
    let items = parse_reward_ai_batch_decision(
        r#"{"advisories":[
                {"condition_id":"c1","suitability":"allow","quote_mode":"double","exit_policy":"exit_at_markup","confidence":0.7,"reasons":[],"metrics":{}},
                {"condition_id":"typo","suitability":"allow","quote_mode":"double","exit_policy":"exit_at_markup","confidence":0.7,"reasons":[],"metrics":{}},
                {"condition_id":"c1","suitability":"watch","quote_mode":"none","exit_policy":"flatten_immediately","confidence":0.3,"reasons":[],"metrics":{}}
            ]}"#,
        &["c1".to_string()],
    )
    .expect("parse batch with unknown and duplicate condition ids");
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].condition_id, "c1");
    assert_eq!(items[0].decision.suitability, RewardAiSuitability::Allow);
}

#[test]
fn reward_ai_batch_parse_returns_partial_when_one_market_omitted() {
    // The omitted market is simply absent; the caller retries it via the single-request fallback.
    let result = parse_reward_ai_batch_decision(
        r#"{"advisories":[
                {"condition_id":"c1","suitability":"allow","quote_mode":"double","exit_policy":"exit_at_markup","confidence":0.7,"reasons":[],"metrics":{}}
            ]}"#,
        &["c1".to_string(), "c2".to_string()],
    )
    .expect("parse partial batch");
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].condition_id, "c1");
}

#[test]
fn reward_ai_batch_parse_single_object_fallback_for_one_market() {
    let items = parse_reward_ai_batch_decision(
        r#"{"suitability":"allow","quote_mode":"double","exit_policy":"exit_at_markup","confidence":0.8,"reasons":[],"metrics":{}}"#,
        &["only".to_string()],
    )
    .expect("parse single object fallback");
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].condition_id, "only");
    assert_eq!(
        items[0].decision.confidence,
        Decimal::from_str("0.8").unwrap()
    );
}

#[test]
fn reward_ai_batch_parse_rejects_single_object_when_multiple_markets_expected() {
    let result = parse_reward_ai_batch_decision(
        r#"{"suitability":"allow","quote_mode":"double","exit_policy":"exit_at_markup","confidence":0.8,"reasons":[],"metrics":{}}"#,
        &["c1".to_string(), "c2".to_string()],
    );
    assert!(result.is_err());
}
