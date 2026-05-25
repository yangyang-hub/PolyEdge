#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn probability_serializes_as_canonical_string() {
        let value = Probability::new(Decimal::from_str("0.500000").expect("valid decimal"))
            .expect("valid probability");

        let serialized = serde_json::to_string(&value).expect("serialize");
        assert_eq!(serialized, "\"0.5\"");
    }

    #[test]
    fn edge_rejects_out_of_range_value() {
        let edge = Edge::new(Decimal::from_str("1.5").expect("valid decimal"));
        assert!(edge.is_err());
    }

    #[test]
    fn quantity_rejects_negative_value() {
        let quantity = Quantity::new(Decimal::from_str("-1").expect("valid decimal"));
        assert!(quantity.is_err());
    }

    #[test]
    fn usd_amount_serializes_as_two_decimal_string() {
        let amount = UsdAmount::new(Decimal::from_str("125000.00").expect("valid decimal"))
            .expect("valid usd amount");

        let serialized = serde_json::to_string(&amount).expect("serialize");
        assert_eq!(serialized, "\"125000\"");
    }

    #[test]
    fn signed_usd_amount_serializes_negative_values() {
        let amount = SignedUsdAmount::new(Decimal::from_str("-125.50").expect("valid decimal"))
            .expect("valid signed usd amount");

        let serialized = serde_json::to_string(&amount).expect("serialize");
        assert_eq!(serialized, "\"-125.5\"");
    }

    #[test]
    fn tradability_status_parses_from_contract_value() {
        let status = TradabilityStatus::from_str("manual_review").expect("valid status");
        assert_eq!(status.as_str(), "manual_review");
    }

    #[test]
    fn evidence_direction_parses_from_contract_value() {
        let direction =
            EvidenceDirection::from_str("supports_no").expect("valid evidence direction");
        assert_eq!(direction.as_str(), "supports_no");
    }

    #[test]
    fn signal_lifecycle_state_rejects_unknown_value() {
        let state = SignalLifecycleState::from_str("queued");
        assert!(state.is_err());
    }

    #[test]
    fn time_horizon_parses_from_contract_value() {
        let horizon = TimeHorizon::from_str("medium").expect("valid horizon");
        assert_eq!(horizon.as_str(), "medium");
    }
}
