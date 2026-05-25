#[cfg(test)]
mod tests {
    use super::{
        ArbitrageAnalysisSummary, ArbitrageOpportunityStatus, ArbitrageOpportunityType,
        ArbitrageValidationConfig, ArbitrageValidationStatus, MarketBookSnapshotView,
        build_arbitrage_analysis, detect_arbitrage_opportunities, validate_arbitrage_opportunity,
    };
    use polyedge_domain::{Edge, Probability, Quantity, Result};
    use rust_decimal::Decimal;
    use serde_json::json;
    use time::{Duration, OffsetDateTime};

    fn probability(units: i64, scale: u32) -> Probability {
        Probability::new(Decimal::new(units, scale)).expect("probability")
    }

    fn quantity(units: i64) -> Quantity {
        Quantity::new(Decimal::from(units)).expect("quantity")
    }

    fn snapshot() -> MarketBookSnapshotView {
        MarketBookSnapshotView {
            id: "book_1".to_string(),
            scan_id: "scan_1".to_string(),
            connector_name: "fixture".to_string(),
            market_id: "mkt_1".to_string(),
            yes_asset_id: Some("yes".to_string()),
            no_asset_id: Some("no".to_string()),
            yes_bid: Some(probability(60, 2)),
            yes_ask: Some(probability(44, 2)),
            yes_bid_size: quantity(7),
            yes_ask_size: quantity(11),
            no_bid: Some(probability(43, 2)),
            no_ask: Some(probability(53, 2)),
            no_bid_size: quantity(9),
            no_ask_size: quantity(13),
            observed_at: OffsetDateTime::UNIX_EPOCH,
            raw_payload: json!({}),
            trace_id: "trc_1".to_string(),
        }
    }

    #[test]
    fn detect_arbitrage_opportunities_finds_buy_and_sell_dislocations() -> Result<()> {
        let opportunities = detect_arbitrage_opportunities(&snapshot())?;

        assert_eq!(opportunities.len(), 2);
        assert_eq!(
            opportunities[0].opportunity_type,
            ArbitrageOpportunityType::BinaryBuyBoth
        );
        assert_eq!(opportunities[0].gross_edge, Edge::new(Decimal::new(3, 2))?);
        assert_eq!(opportunities[0].capacity, quantity(11));
        assert_eq!(
            opportunities[1].opportunity_type,
            ArbitrageOpportunityType::BinarySellBoth
        );
        assert_eq!(opportunities[1].gross_edge, Edge::new(Decimal::new(3, 2))?);
        assert_eq!(opportunities[1].capacity, quantity(7));

        Ok(())
    }

    #[test]
    fn build_arbitrage_analysis_groups_markets_and_types() -> Result<()> {
        let observed_at = OffsetDateTime::UNIX_EPOCH + Duration::seconds(60);
        let opportunities = detect_arbitrage_opportunities(&snapshot())?
            .into_iter()
            .map(|draft| super::ArbitrageOpportunityView {
                id: format!("opp_{}", draft.opportunity_type.as_str()),
                scan_id: "scan_1".to_string(),
                market_id: "mkt_1".to_string(),
                opportunity_type: draft.opportunity_type,
                status: ArbitrageOpportunityStatus::Observed,
                gross_edge: draft.gross_edge,
                price_sum: draft.price_sum,
                capacity: draft.capacity,
                yes_price: draft.yes_price,
                no_price: draft.no_price,
                yes_size: draft.yes_size,
                no_size: draft.no_size,
                observed_at,
                reason_codes: draft.reason_codes,
                analysis_payload: draft.analysis_payload,
                trace_id: "trc_1".to_string(),
                validation: None,
            })
            .collect::<Vec<_>>();

        let summary: ArbitrageAnalysisSummary =
            build_arbitrage_analysis(&opportunities, 24, observed_at);

        assert_eq!(summary.opportunity_count, 2);
        assert_eq!(summary.market_count, 1);
        assert_eq!(summary.type_counts.len(), 2);
        assert_eq!(summary.top_markets.len(), 1);
        assert_eq!(summary.top_markets[0].market_id, "mkt_1");
        assert_eq!(summary.top_markets[0].opportunity_count, 2);

        Ok(())
    }

    #[test]
    fn arbitrage_event_payload_timestamps_are_rfc3339_strings() -> Result<()> {
        let started_at = OffsetDateTime::UNIX_EPOCH + Duration::seconds(1);
        let finished_at = started_at + Duration::seconds(2);
        let scan = super::ArbitrageScanView {
            id: "scan_1".to_string(),
            started_at,
            finished_at: Some(finished_at),
            market_count: 1,
            snapshot_count: 1,
            opportunity_count: 1,
            scanner_version: "v1".to_string(),
            metadata: json!({ "book_source": "fixture" }),
            trace_id: "trc_1".to_string(),
        };

        let scan_payload = super::scan_payload(&scan);
        assert!(scan_payload["started_at"].is_string());
        assert!(scan_payload["finished_at"].is_string());

        let snapshot = snapshot();
        let draft = detect_arbitrage_opportunities(&snapshot)?
            .into_iter()
            .next()
            .expect("opportunity");
        let opportunity = super::ArbitrageOpportunityView {
            id: "opp_binary_buy_both".to_string(),
            scan_id: snapshot.scan_id.clone(),
            market_id: snapshot.market_id.clone(),
            opportunity_type: draft.opportunity_type,
            status: ArbitrageOpportunityStatus::Observed,
            gross_edge: draft.gross_edge,
            price_sum: draft.price_sum,
            capacity: draft.capacity,
            yes_price: draft.yes_price,
            no_price: draft.no_price,
            yes_size: draft.yes_size,
            no_size: draft.no_size,
            observed_at: snapshot.observed_at,
            reason_codes: draft.reason_codes,
            analysis_payload: draft.analysis_payload,
            trace_id: "trc_1".to_string(),
            validation: None,
        };
        let opportunity_payload = super::opportunity_payload(&opportunity);
        assert!(opportunity_payload["observed_at"].is_string());

        let config = ArbitrageValidationConfig {
            max_book_age_ms: 5_000,
            min_gross_edge: Edge::new(Decimal::new(1, 2))?,
            min_capacity: quantity(5),
            fee_buffer: Edge::new(Decimal::new(5, 3))?,
            slippage_buffer: Edge::new(Decimal::new(5, 3))?,
        };
        let validation = validate_arbitrage_opportunity(
            &opportunity,
            &snapshot,
            &config,
            snapshot.observed_at + Duration::milliseconds(50),
            "trc_1",
        )?;
        let validation_payload = super::validation_payload(&validation);
        assert!(validation_payload["validated_at"].is_string());

        let analysis_payload = super::analysis_payload(&super::ArbitrageAnalysisRunView {
            id: "arb_analysis_1".to_string(),
            generated_at: finished_at,
            lookback_hours: 24,
            opportunity_count: 1,
            market_count: 1,
            summary_payload: json!({ "generated_at": "1970-01-01T00:00:03Z" }),
            trace_id: "trc_1".to_string(),
        });
        assert!(analysis_payload["generated_at"].is_string());

        Ok(())
    }

    #[test]
    fn validate_arbitrage_opportunity_applies_buffers_and_capacity_rules() -> Result<()> {
        let snapshot = snapshot();
        let observed_at = snapshot.observed_at + Duration::seconds(1);
        let draft = detect_arbitrage_opportunities(&snapshot)?
            .into_iter()
            .next()
            .expect("opportunity");
        let opportunity = super::ArbitrageOpportunityView {
            id: "opp_binary_buy_both".to_string(),
            scan_id: snapshot.scan_id.clone(),
            market_id: snapshot.market_id.clone(),
            opportunity_type: draft.opportunity_type,
            status: ArbitrageOpportunityStatus::Observed,
            gross_edge: draft.gross_edge,
            price_sum: draft.price_sum,
            capacity: draft.capacity,
            yes_price: draft.yes_price,
            no_price: draft.no_price,
            yes_size: draft.yes_size,
            no_size: draft.no_size,
            observed_at: snapshot.observed_at,
            reason_codes: draft.reason_codes,
            analysis_payload: draft.analysis_payload,
            trace_id: "trc_1".to_string(),
            validation: None,
        };
        let config = ArbitrageValidationConfig {
            max_book_age_ms: 5_000,
            min_gross_edge: Edge::new(Decimal::new(1, 2))?,
            min_capacity: quantity(5),
            fee_buffer: Edge::new(Decimal::new(5, 3))?,
            slippage_buffer: Edge::new(Decimal::new(5, 3))?,
        };

        let validation =
            validate_arbitrage_opportunity(&opportunity, &snapshot, &config, observed_at, "trc_1")?;

        assert_eq!(validation.status, ArbitrageValidationStatus::Valid);
        assert_eq!(validation.net_edge, Edge::new(Decimal::new(2, 2))?);
        assert_eq!(validation.validated_capacity, quantity(11));

        let stale_validation = validate_arbitrage_opportunity(
            &opportunity,
            &snapshot,
            &config,
            observed_at + Duration::seconds(10),
            "trc_1",
        )?;

        assert_eq!(
            stale_validation.status,
            ArbitrageValidationStatus::StaleBook
        );

        Ok(())
    }

    #[test]
    fn validate_arbitrage_opportunity_marks_price_moved_when_current_book_loses_edge() -> Result<()>
    {
        let discovery_snapshot = snapshot();
        let observed_at = discovery_snapshot.observed_at + Duration::seconds(1);
        let draft = detect_arbitrage_opportunities(&discovery_snapshot)?
            .into_iter()
            .find(|draft| draft.opportunity_type == ArbitrageOpportunityType::BinaryBuyBoth)
            .expect("buy-both opportunity");
        let opportunity = super::ArbitrageOpportunityView {
            id: "opp_binary_buy_both".to_string(),
            scan_id: discovery_snapshot.scan_id.clone(),
            market_id: discovery_snapshot.market_id.clone(),
            opportunity_type: draft.opportunity_type,
            status: ArbitrageOpportunityStatus::Observed,
            gross_edge: draft.gross_edge,
            price_sum: draft.price_sum,
            capacity: draft.capacity,
            yes_price: draft.yes_price,
            no_price: draft.no_price,
            yes_size: draft.yes_size,
            no_size: draft.no_size,
            observed_at: discovery_snapshot.observed_at,
            reason_codes: draft.reason_codes,
            analysis_payload: draft.analysis_payload,
            trace_id: "trc_1".to_string(),
            validation: None,
        };
        let validation_snapshot = MarketBookSnapshotView {
            id: "book_1_validation".to_string(),
            scan_id: discovery_snapshot.scan_id.clone(),
            connector_name: discovery_snapshot.connector_name.clone(),
            market_id: discovery_snapshot.market_id.clone(),
            yes_asset_id: discovery_snapshot.yes_asset_id.clone(),
            no_asset_id: discovery_snapshot.no_asset_id.clone(),
            yes_bid: Some(probability(49, 2)),
            yes_ask: Some(probability(50, 2)),
            yes_bid_size: quantity(7),
            yes_ask_size: quantity(11),
            no_bid: Some(probability(49, 2)),
            no_ask: Some(probability(51, 2)),
            no_bid_size: quantity(9),
            no_ask_size: quantity(13),
            observed_at,
            raw_payload: json!({ "fixture": "price_moved" }),
            trace_id: "trc_1".to_string(),
        };
        let config = ArbitrageValidationConfig {
            max_book_age_ms: 5_000,
            min_gross_edge: Edge::new(Decimal::new(1, 2))?,
            min_capacity: quantity(5),
            fee_buffer: Edge::new(Decimal::new(5, 3))?,
            slippage_buffer: Edge::new(Decimal::new(5, 3))?,
        };

        let validation = validate_arbitrage_opportunity(
            &opportunity,
            &validation_snapshot,
            &config,
            observed_at + Duration::milliseconds(50),
            "trc_1",
        )?;

        assert_eq!(validation.status, ArbitrageValidationStatus::PriceMoved);
        assert_eq!(validation.gross_edge, Edge::new(Decimal::ZERO)?);
        assert_eq!(validation.validated_capacity, quantity(0));
        assert!(
            validation
                .reason_codes
                .contains(&"opportunity_no_longer_present_in_latest_book".to_string())
        );
        assert_eq!(
            validation.validation_payload["snapshot_id"],
            json!("book_1_validation")
        );

        Ok(())
    }
}
