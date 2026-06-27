use super::*;

#[test]
fn reward_info_risk_confidence_is_clamped_to_unit_interval() {
    let high = parse_reward_info_risk_decision(
        r#"{
            "allow_quote": true,
            "confidence": 1.7,
            "summary": "test",
            "sources": [],
            "metrics": {}
        }"#,
    )
    .expect("parse high confidence");
    assert_eq!(high.confidence, Decimal::ONE);

    let low = parse_reward_info_risk_decision(
        r#"{
            "allow_quote": false,
            "confidence": "-0.1",
            "summary": "test",
            "sources": [],
            "metrics": {}
        }"#,
    )
    .expect("parse low confidence");
    assert_eq!(low.confidence, Decimal::ZERO);
    assert_eq!(low.risk_level, RewardInfoRiskLevel::Critical);
    assert_eq!(low.risk_type, RewardInfoRiskType::Unknown);
}

#[test]
fn reward_info_risk_parse_skips_embedded_example_object() {
    let parsed = parse_reward_info_risk_decision(
        r#"Example: {"example": true}
Final:
{"allow_quote":true,"confidence":0.75,"summary":"quiet","sources":[],"metrics":{}}
"#,
    )
    .expect("parse embedded response");

    assert_eq!(parsed.risk_level, RewardInfoRiskLevel::Low);
    assert_eq!(parsed.risk_type, RewardInfoRiskType::None);
}

#[test]
fn reward_info_risk_parse_legacy_taxonomy_response() {
    let parsed = parse_reward_info_risk_decision(
        r#"{"risk_level":"unknown","risk_type":"unknown","directional_risk":"unclear","resolution_imminent":false,"expected_event_at":null,"confidence":0.2,"summary":"unclear","sources":[],"metrics":{}}"#,
    )
    .expect("parse legacy response");

    assert_eq!(parsed.risk_level, RewardInfoRiskLevel::Unknown);
    assert_eq!(parsed.confidence, Decimal::from_str("0.2").unwrap());
}

#[test]
fn reward_info_risk_parse_accepts_markdown_fence() {
    let parsed = parse_reward_info_risk_decision(
        r#"```json
{"allow_quote":false,"confidence":0.2,"summary":"unclear","sources":[],"metrics":{}}
```"#,
    )
    .expect("parse fenced response");

    assert_eq!(parsed.risk_level, RewardInfoRiskLevel::Critical);
    assert_eq!(parsed.confidence, Decimal::from_str("0.2").unwrap());
}

#[test]
fn reward_info_risk_batch_parse_drops_unknown_and_duplicate_items() {
    let parsed = parse_reward_info_risk_batch_decision(
        r#"{
            "risks": [
                {"condition_id":"c1","allow_quote":true,"confidence":0.8,"summary":"quiet","sources":[],"metrics":{}},
                {"condition_id":"c1","allow_quote":false,"confidence":0.9,"summary":"duplicate","sources":[],"metrics":{}},
                {"condition_id":"unknown","allow_quote":false,"confidence":0.7,"summary":"extra","sources":[],"metrics":{}}
            ]
        }"#,
        &["c1".to_string(), "c2".to_string()],
    )
    .expect("parse batch response");

    assert_eq!(parsed.len(), 1);
    assert_eq!(parsed[0].condition_id, "c1");
    assert_eq!(parsed[0].decision.risk_level, RewardInfoRiskLevel::Low);
}

#[test]
fn reward_info_risk_batch_parse_keeps_valid_items_when_one_item_is_bad() {
    let parsed = parse_reward_info_risk_batch_decision(
        r#"{
            "risks": [
                {"condition_id":"bad","risk_level":"not-a-level","risk_type":"none","directional_risk":"unclear","resolution_imminent":false,"expected_event_at":null,"confidence":0.8,"summary":"bad","sources":[],"metrics":{}},
                {"condition_id":"good","allow_quote":false,"confidence":"0.6","summary":"scheduled","sources":[],"metrics":{}}
            ]
        }"#,
        &["bad".to_string(), "good".to_string()],
    )
    .expect("parse partially valid batch response");

    assert_eq!(parsed.len(), 1);
    assert_eq!(parsed[0].condition_id, "good");
    assert_eq!(parsed[0].decision.risk_level, RewardInfoRiskLevel::Critical);
}
