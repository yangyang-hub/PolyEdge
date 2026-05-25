#[cfg(test)]
mod tests {
    use super::*;

    fn signal_view(record: &FixtureSignalRecord) -> SignalView {
        SignalView {
            id: record.id.clone(),
            market_id: record.market_id.clone(),
            event_id: record.event_id.clone(),
            action: record.action,
            side: record.side,
            market_price: record.market_price,
            fair_price: record.fair_price,
            edge: record.edge,
            confidence: record.confidence,
            lifecycle_state: record.lifecycle_state,
            reason: record.reason.clone(),
            risk_decision: record.risk_decision.clone(),
            evidence_ids: record.evidence_ids.clone(),
            approved_by_user_id: record.approved_by_user_id.clone(),
            approved_at: record.approved_at,
            rejected_by_user_id: record.rejected_by_user_id.clone(),
            rejected_at: record.rejected_at,
            updated_at: record.updated_at,
            version: record.version,
        }
    }

    fn market_view(record: &FixtureMarketRecord) -> MarketView {
        MarketView {
            id: record.id.clone(),
            question: record.question.clone(),
            category: record.category.clone(),
            status: record.status,
            best_bid: record.best_bid,
            best_ask: record.best_ask,
            mid_price: record.mid_price,
            volume_24h: record.volume_24h,
            ambiguity_level: record.ambiguity_level,
            tradability_status: record.tradability_status,
            resolution_source: record.resolution_source.clone(),
            edge_case_notes: record.edge_case_notes.clone(),
            polymarket_condition_id: record.polymarket_condition_id.clone(),
            polymarket_yes_asset_id: record.polymarket_yes_asset_id.clone(),
            polymarket_no_asset_id: record.polymarket_no_asset_id.clone(),
            updated_at: record.updated_at,
            version: record.version,
        }
    }

    fn evidence_view(record: &FixtureEvidenceRecord) -> EvidenceView {
        EvidenceView {
            id: record.id.clone(),
            market_id: record.market_id.clone(),
            event_id: record.event_id.clone(),
            direction: record.direction,
            strength: record.strength,
            source_reliability: record.source_reliability,
            novelty: record.novelty,
            resolution_relevance: record.resolution_relevance,
            status: record.status,
            expires_at: record.expires_at,
            created_at: record.created_at,
            updated_at: record.updated_at,
            version: record.version,
        }
    }

    #[test]
    fn market_filters_reject_zero_limit() {
        let result = MarketListFilters::new(None, None, Some(0));
        assert!(result.is_err());
    }

    #[test]
    fn signal_transition_filters_require_signal_id() {
        let result = SignalTransitionListFilters::new("   ", None);
        assert!(result.is_err());
    }

    #[test]
    fn demo_fixture_bundle_contains_full_chain_records() {
        let bundle = demo_fixture_bundle();
        assert_eq!(bundle.markets.len(), 4);
        assert_eq!(bundle.events.len(), 4);
        assert_eq!(bundle.evidences.len(), 4);
        assert_eq!(bundle.signals.len(), 4);
    }

    #[test]
    fn recompute_draft_keeps_negative_manual_review_signal_in_new_state() {
        let bundle = demo_fixture_bundle();
        let signal = bundle
            .signals
            .iter()
            .find(|signal| signal.id == "sig_2412")
            .expect("fixture signal");
        let market = bundle
            .markets
            .iter()
            .find(|market| market.id == signal.market_id)
            .expect("fixture market");
        let evidences: Vec<_> = bundle
            .evidences
            .iter()
            .filter(|evidence| {
                evidence.market_id == signal.market_id && evidence.event_id == signal.event_id
            })
            .map(evidence_view)
            .collect();
        let draft = build_recompute_signal_draft(
            &signal_view(signal),
            &market_view(market),
            &evidences,
            "manual refresh",
            "est_test",
        )
        .expect("recompute draft");

        assert_eq!(draft.next_signal.side, SignalSide::No);
        assert_eq!(draft.next_signal.lifecycle_state, SignalLifecycleState::New);
        assert!(draft.transition.is_none());
    }

    #[test]
    fn recompute_draft_discounts_degraded_event_source_health() {
        let bundle = demo_fixture_bundle();
        let signal = bundle
            .signals
            .iter()
            .find(|signal| signal.id == "sig_2412")
            .expect("fixture signal");
        let market = bundle
            .markets
            .iter()
            .find(|market| market.id == signal.market_id)
            .expect("fixture market");
        let evidences: Vec<_> = bundle
            .evidences
            .iter()
            .filter(|evidence| {
                evidence.market_id == signal.market_id && evidence.event_id == signal.event_id
            })
            .map(evidence_view)
            .collect();
        let signal = signal_view(signal);
        let market = market_view(market);

        let baseline = build_recompute_signal_draft(
            &signal,
            &market,
            &evidences,
            "baseline recompute",
            "est_baseline",
        )
        .expect("baseline recompute draft");
        let degraded = build_recompute_signal_draft_with_source_health(
            &signal,
            &market,
            &evidences,
            "degraded recompute",
            Some(&SourceHealthAdjustment {
                source: "reuters".to_string(),
                health_score: probability("0.20"),
            }),
            "est_degraded",
        )
        .expect("degraded recompute draft");

        assert!(degraded.estimate.edge.value().abs() < baseline.estimate.edge.value().abs());
        assert!(degraded.estimate.confidence < baseline.estimate.confidence);
        assert!(
            degraded
                .estimate
                .reason_codes
                .contains(&"source_health_degraded".to_string())
        );
        assert!(
            !degraded
                .estimate
                .reason_codes
                .contains(&"official_source".to_string())
        );
    }
}
