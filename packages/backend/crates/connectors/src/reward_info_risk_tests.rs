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
    assert_eq!(low.risk_level, RewardInfoRiskLevel::Unknown);
    assert_eq!(low.risk_type, RewardInfoRiskType::Unknown);
    assert_eq!(low.action, RewardProviderAction::StopNew);
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

    assert_eq!(parsed.risk_level, RewardInfoRiskLevel::Unknown);
    assert_eq!(parsed.confidence, Decimal::from_str("0.2").unwrap());
}

#[test]
fn reward_info_risk_parse_v2_directional_cancel_with_evidence() {
    let parsed = parse_reward_info_risk_decision(
        r#"{
            "action":"cancel_yes",
            "risk_level":"critical",
            "risk_type":"official_result",
            "directional_risk":"yes",
            "resolution_imminent":true,
            "expected_event_at":null,
            "confidence":0.94,
            "summary":"official result published",
            "sources":[{"url":"https://example.com/result","title":"Official result","published_at":"2026-07-10T00:00:00Z","snippet":"Result"}],
            "metrics":{}
        }"#,
    )
    .expect("parse v2 response");

    assert_eq!(parsed.action, RewardProviderAction::CancelYes);
    assert_eq!(parsed.risk_level, RewardInfoRiskLevel::Critical);
    assert_eq!(parsed.sources.len(), 1);
}
