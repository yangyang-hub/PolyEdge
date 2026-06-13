impl PostgresMarketEventStore {
async fn market_event_ingest_fixture_bundle(
        &self,
        bundle: &FixtureBundle,
        trace_id: &str,
    ) -> Result<FixtureIngestionReport> {
        let mut transaction = self.pool.begin().await.map_err(|error| {
            db_error(
                "POSTGRES_TRANSACTION_BEGIN_FAILED",
                format!("failed to begin market/event ingestion transaction: {error}"),
            )
        })?;

        for market in &bundle.markets {
            sqlx::query(
                r#"
                INSERT INTO markets (
                  id,
                  question,
                  category,
                  status,
                  best_bid,
                  best_ask,
                  mid_price,
                  volume_24h,
                  liquidity_usd,
                  end_at,
                  ambiguity_level,
                  tradability_status,
                  polymarket_condition_id,
                  polymarket_yes_asset_id,
                  polymarket_no_asset_id,
                  updated_at,
                  version,
                  trace_id
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18)
                ON CONFLICT (id) DO UPDATE
                SET
                  question = EXCLUDED.question,
                  category = EXCLUDED.category,
                  status = EXCLUDED.status,
                  best_bid = EXCLUDED.best_bid,
                  best_ask = EXCLUDED.best_ask,
                  mid_price = EXCLUDED.mid_price,
                  volume_24h = EXCLUDED.volume_24h,
                  liquidity_usd = EXCLUDED.liquidity_usd,
                  end_at = EXCLUDED.end_at,
                  synced_at = now(),
                  ambiguity_level = EXCLUDED.ambiguity_level,
                  tradability_status = EXCLUDED.tradability_status,
                  polymarket_condition_id = EXCLUDED.polymarket_condition_id,
                  polymarket_yes_asset_id = EXCLUDED.polymarket_yes_asset_id,
                  polymarket_no_asset_id = EXCLUDED.polymarket_no_asset_id,
                  updated_at = EXCLUDED.updated_at,
                  version = EXCLUDED.version,
                  trace_id = EXCLUDED.trace_id
                "#,
            )
            .bind(&market.id)
            .bind(&market.question)
            .bind(&market.category)
            .bind(market.status.as_str())
            .bind(market.best_bid.value())
            .bind(market.best_ask.value())
            .bind(market.mid_price.value())
            .bind(market.volume_24h.value())
            .bind(market.liquidity_usd.value())
            .bind(market.end_at)
            .bind(market.ambiguity_level.as_str())
            .bind(market.tradability_status.as_str())
            .bind(&market.polymarket_condition_id)
            .bind(&market.polymarket_yes_asset_id)
            .bind(&market.polymarket_no_asset_id)
            .bind(market.updated_at)
            .bind(market.version)
            .bind(trace_id)
            .execute(&mut *transaction)
            .await
            .map_err(|error| {
                db_error(
                    "POSTGRES_INSERT_FAILED",
                    format!("failed to upsert market {}: {error}", market.id),
                )
            })?;

            sqlx::query(
                r#"
                INSERT INTO market_resolution_rules (
                  market_id,
                  resolution_source,
                  edge_case_notes,
                  updated_at,
                  version,
                  trace_id
                )
                VALUES ($1, $2, $3, $4, $5, $6)
                ON CONFLICT (market_id) DO UPDATE
                SET
                  resolution_source = EXCLUDED.resolution_source,
                  edge_case_notes = EXCLUDED.edge_case_notes,
                  updated_at = EXCLUDED.updated_at,
                  version = EXCLUDED.version,
                  trace_id = EXCLUDED.trace_id
                "#,
            )
            .bind(&market.id)
            .bind(&market.resolution_source)
            .bind(&market.edge_case_notes)
            .bind(market.updated_at)
            .bind(market.version)
            .bind(trace_id)
            .execute(&mut *transaction)
            .await
            .map_err(|error| {
                db_error(
                    "POSTGRES_INSERT_FAILED",
                    format!(
                        "failed to upsert market resolution rules for {}: {error}",
                        market.id
                    ),
                )
            })?;
        }

        for event in &bundle.events {
            let raw_payload = serde_json::to_value(event).map_err(|error| {
                AppError::internal(
                    "RAW_EVENT_SERIALIZE_FAILED",
                    format!("failed to serialize event fixture {}: {error}", event.id),
                )
            })?;

            let raw_event_id = if let Some(raw_event_id) = event.raw_event_id.as_deref() {
                raw_event_id.to_string()
            } else {
                let raw_event_id = format!("raw_{}", event.id);
                let hash = format!("fixture_hash_{}", event.id);

                sqlx::query(
                    r#"
                    INSERT INTO raw_events (
                      id,
                      source,
                      hash,
                      raw_payload,
                      ingested_at,
                      trace_id
                    )
                    VALUES ($1, $2, $3, $4, $5, $6)
                    ON CONFLICT (id) DO UPDATE
                    SET
                      source = EXCLUDED.source,
                      hash = EXCLUDED.hash,
                      raw_payload = EXCLUDED.raw_payload,
                      ingested_at = EXCLUDED.ingested_at,
                      trace_id = EXCLUDED.trace_id
                    "#,
                )
                .bind(&raw_event_id)
                .bind(&event.source)
                .bind(hash)
                .bind(Json(raw_payload))
                .bind(event.updated_at)
                .bind(trace_id)
                .execute(&mut *transaction)
                .await
                .map_err(|error| {
                    db_error(
                        "POSTGRES_INSERT_FAILED",
                        format!("failed to upsert raw event {}: {error}", event.id),
                    )
                })?;

                raw_event_id
            };

            sqlx::query(
                r#"
                INSERT INTO events (
                  id,
                  raw_event_id,
                  source,
                  summary,
                  relevance_score,
                  confidence,
                  status,
                  reason_trace,
                  created_at,
                  updated_at,
                  version,
                  trace_id
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
                ON CONFLICT (id) DO UPDATE
                SET
                  raw_event_id = EXCLUDED.raw_event_id,
                  source = EXCLUDED.source,
                  summary = EXCLUDED.summary,
                  relevance_score = EXCLUDED.relevance_score,
                  confidence = EXCLUDED.confidence,
                  status = EXCLUDED.status,
                  reason_trace = EXCLUDED.reason_trace,
                  created_at = EXCLUDED.created_at,
                  updated_at = EXCLUDED.updated_at,
                  version = EXCLUDED.version,
                  trace_id = EXCLUDED.trace_id
                "#,
            )
            .bind(&event.id)
            .bind(&raw_event_id)
            .bind(&event.source)
            .bind(&event.summary)
            .bind(event.relevance_score.value())
            .bind(event.confidence.value())
            .bind(event.status.as_str())
            .bind(&event.reason_trace)
            .bind(event.created_at)
            .bind(event.updated_at)
            .bind(event.version)
            .bind(trace_id)
            .execute(&mut *transaction)
            .await
            .map_err(|error| {
                db_error(
                    "POSTGRES_INSERT_FAILED",
                    format!("failed to upsert event {}: {error}", event.id),
                )
            })?;

            sqlx::query("DELETE FROM event_market_links WHERE event_id = $1")
                .bind(&event.id)
                .execute(&mut *transaction)
                .await
                .map_err(|error| {
                    db_error(
                        "POSTGRES_DELETE_FAILED",
                        format!(
                            "failed to reset event market links for {}: {error}",
                            event.id
                        ),
                    )
                })?;

            for market_id in &event.related_market_ids {
                sqlx::query(
                    r#"
                    INSERT INTO event_market_links (event_id, market_id, created_at)
                    VALUES ($1, $2, $3)
                    ON CONFLICT (event_id, market_id) DO NOTHING
                    "#,
                )
                .bind(&event.id)
                .bind(market_id)
                .bind(event.created_at)
                .execute(&mut *transaction)
                .await
                .map_err(|error| {
                    db_error(
                        "POSTGRES_INSERT_FAILED",
                        format!(
                            "failed to insert event-market link {} -> {}: {error}",
                            event.id, market_id
                        ),
                    )
                })?;
            }
        }

        for evidence in &bundle.evidences {
            sqlx::query(
                r#"
                INSERT INTO evidences (
                  id,
                  market_id,
                  event_id,
                  direction,
                  strength,
                  source_reliability,
                  novelty,
                  resolution_relevance,
                  status,
                  expires_at,
                  created_at,
                  updated_at,
                  version,
                  trace_id
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
                ON CONFLICT (id) DO UPDATE
                SET
                  market_id = EXCLUDED.market_id,
                  event_id = EXCLUDED.event_id,
                  direction = EXCLUDED.direction,
                  strength = EXCLUDED.strength,
                  source_reliability = EXCLUDED.source_reliability,
                  novelty = EXCLUDED.novelty,
                  resolution_relevance = EXCLUDED.resolution_relevance,
                  status = EXCLUDED.status,
                  expires_at = EXCLUDED.expires_at,
                  created_at = EXCLUDED.created_at,
                  updated_at = EXCLUDED.updated_at,
                  version = EXCLUDED.version,
                  trace_id = EXCLUDED.trace_id
                "#,
            )
            .bind(&evidence.id)
            .bind(&evidence.market_id)
            .bind(&evidence.event_id)
            .bind(evidence.direction.as_str())
            .bind(evidence.strength.value())
            .bind(evidence.source_reliability.value())
            .bind(evidence.novelty.value())
            .bind(evidence.resolution_relevance.value())
            .bind(evidence.status.as_str())
            .bind(evidence.expires_at)
            .bind(evidence.created_at)
            .bind(evidence.updated_at)
            .bind(evidence.version)
            .bind(trace_id)
            .execute(&mut *transaction)
            .await
            .map_err(|error| {
                db_error(
                    "POSTGRES_INSERT_FAILED",
                    format!("failed to upsert evidence {}: {error}", evidence.id),
                )
            })?;
        }

        for signal in &bundle.signals {
            sqlx::query(
                r#"
                INSERT INTO signals (
                  id,
                  market_id,
                  event_id,
                  action,
                  side,
                  market_price,
                  fair_price,
                  edge,
                  confidence,
                  lifecycle_state,
                  reason,
                  risk_decision,
                  approved_by_user_id,
                  approved_at,
                  rejected_by_user_id,
                  rejected_at,
                  updated_at,
                  version,
                  trace_id
                )
                VALUES (
                  $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17,
                  $18, $19
                )
                ON CONFLICT (id) DO UPDATE
                SET
                  market_id = EXCLUDED.market_id,
                  event_id = EXCLUDED.event_id,
                  action = EXCLUDED.action,
                  side = EXCLUDED.side,
                  market_price = EXCLUDED.market_price,
                  fair_price = EXCLUDED.fair_price,
                  edge = EXCLUDED.edge,
                  confidence = EXCLUDED.confidence,
                  lifecycle_state = EXCLUDED.lifecycle_state,
                  reason = EXCLUDED.reason,
                  risk_decision = EXCLUDED.risk_decision,
                  approved_by_user_id = EXCLUDED.approved_by_user_id,
                  approved_at = EXCLUDED.approved_at,
                  rejected_by_user_id = EXCLUDED.rejected_by_user_id,
                  rejected_at = EXCLUDED.rejected_at,
                  updated_at = EXCLUDED.updated_at,
                  version = EXCLUDED.version,
                  trace_id = EXCLUDED.trace_id
                "#,
            )
            .bind(&signal.id)
            .bind(&signal.market_id)
            .bind(&signal.event_id)
            .bind(signal.action.as_str())
            .bind(signal.side.as_str())
            .bind(signal.market_price.value())
            .bind(signal.fair_price.value())
            .bind(signal.edge.value())
            .bind(signal.confidence.value())
            .bind(signal.lifecycle_state.as_str())
            .bind(&signal.reason)
            .bind(&signal.risk_decision)
            .bind(&signal.approved_by_user_id)
            .bind(signal.approved_at)
            .bind(&signal.rejected_by_user_id)
            .bind(signal.rejected_at)
            .bind(signal.updated_at)
            .bind(signal.version)
            .bind(trace_id)
            .execute(&mut *transaction)
            .await
            .map_err(|error| {
                db_error(
                    "POSTGRES_INSERT_FAILED",
                    format!("failed to upsert signal {}: {error}", signal.id),
                )
            })?;

            sqlx::query("DELETE FROM signal_evidence_links WHERE signal_id = $1")
                .bind(&signal.id)
                .execute(&mut *transaction)
                .await
                .map_err(|error| {
                    db_error(
                        "POSTGRES_DELETE_FAILED",
                        format!(
                            "failed to reset signal evidence links for {}: {error}",
                            signal.id
                        ),
                    )
                })?;

            for evidence_id in &signal.evidence_ids {
                sqlx::query(
                    r#"
                    INSERT INTO signal_evidence_links (signal_id, evidence_id, created_at)
                    VALUES ($1, $2, $3)
                    ON CONFLICT (signal_id, evidence_id) DO NOTHING
                    "#,
                )
                .bind(&signal.id)
                .bind(evidence_id)
                .bind(signal.updated_at)
                .execute(&mut *transaction)
                .await
                .map_err(|error| {
                    db_error(
                        "POSTGRES_INSERT_FAILED",
                        format!(
                            "failed to insert signal-evidence link {} -> {}: {error}",
                            signal.id, evidence_id
                        ),
                    )
                })?;
            }
        }

        transaction.commit().await.map_err(|error| {
            db_error(
                "POSTGRES_TRANSACTION_COMMIT_FAILED",
                format!("failed to commit market/event ingestion transaction: {error}"),
            )
        })?;

        Ok(FixtureIngestionReport {
            markets_upserted: bundle.markets.len(),
            events_upserted: bundle.events.len(),
            evidences_upserted: bundle.evidences.len(),
            signals_upserted: bundle.signals.len(),
        })
    }
}
