impl InMemoryMarketEventStore {
async fn market_event_ingest_fixture_bundle(
        &self,
        bundle: &FixtureBundle,
        _trace_id: &str,
    ) -> Result<FixtureIngestionReport> {
        {
            let mut markets = self.markets.write().await;
            for market in &bundle.markets {
                markets.insert(
                    market.id.clone(),
                    MarketView {
                        id: market.id.clone(),
                        slug: market.slug.clone(),
                        question: market.question.clone(),
                        category: market.category.clone(),
                        status: market.status,
                        best_bid: market.best_bid,
                        best_ask: market.best_ask,
                        mid_price: market.mid_price,
                        volume_24h: market.volume_24h,
                        ambiguity_level: market.ambiguity_level,
                        tradability_status: market.tradability_status,
                        resolution_source: market.resolution_source.clone(),
                        edge_case_notes: market.edge_case_notes.clone(),
                        polymarket_condition_id: market.polymarket_condition_id.clone(),
                        polymarket_yes_asset_id: market.polymarket_yes_asset_id.clone(),
                        polymarket_no_asset_id: market.polymarket_no_asset_id.clone(),
                        updated_at: market.updated_at,
                        version: market.version,
                    },
                );
            }
        }

        {
            let mut events = self.events.write().await;
            for event in &bundle.events {
                events.insert(
                    event.id.clone(),
                    EventView {
                        id: event.id.clone(),
                        source: event.source.clone(),
                        summary: event.summary.clone(),
                        relevance_score: event.relevance_score,
                        confidence: event.confidence,
                        status: event.status,
                        related_market_ids: event.related_market_ids.clone(),
                        reason_trace: event.reason_trace.clone(),
                        created_at: event.created_at,
                        updated_at: event.updated_at,
                        version: event.version,
                    },
                );
            }
        }

        {
            let mut evidences = self.evidences.write().await;
            for evidence in &bundle.evidences {
                evidences.insert(
                    evidence.id.clone(),
                    EvidenceView {
                        id: evidence.id.clone(),
                        market_id: evidence.market_id.clone(),
                        event_id: evidence.event_id.clone(),
                        direction: evidence.direction,
                        strength: evidence.strength,
                        source_reliability: evidence.source_reliability,
                        novelty: evidence.novelty,
                        resolution_relevance: evidence.resolution_relevance,
                        status: evidence.status,
                        expires_at: evidence.expires_at,
                        created_at: evidence.created_at,
                        updated_at: evidence.updated_at,
                        version: evidence.version,
                    },
                );
            }
        }

        {
            let mut signals = self.signals.write().await;
            for signal in &bundle.signals {
                signals.insert(
                    signal.id.clone(),
                    SignalView {
                        id: signal.id.clone(),
                        market_id: signal.market_id.clone(),
                        event_id: signal.event_id.clone(),
                        action: signal.action,
                        side: signal.side,
                        market_price: signal.market_price,
                        fair_price: signal.fair_price,
                        edge: signal.edge,
                        confidence: signal.confidence,
                        lifecycle_state: signal.lifecycle_state,
                        reason: signal.reason.clone(),
                        risk_decision: signal.risk_decision.clone(),
                        evidence_ids: signal.evidence_ids.clone(),
                        approved_by_user_id: signal.approved_by_user_id.clone(),
                        approved_at: signal.approved_at,
                        rejected_by_user_id: signal.rejected_by_user_id.clone(),
                        rejected_at: signal.rejected_at,
                        updated_at: signal.updated_at,
                        version: signal.version,
                    },
                );
            }
        }

        Ok(FixtureIngestionReport {
            markets_upserted: bundle.markets.len(),
            events_upserted: bundle.events.len(),
            evidences_upserted: bundle.evidences.len(),
            signals_upserted: bundle.signals.len(),
        })
    }
}
