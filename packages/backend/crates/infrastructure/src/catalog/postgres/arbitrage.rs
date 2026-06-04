use super::*;

#[async_trait]
impl ArbitrageStore for PostgresMarketEventStore {
    async fn start_arbitrage_scan(&self, scan: &ArbitrageScanView) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO arbitrage_scans (
              id,
              started_at,
              finished_at,
              market_count,
              snapshot_count,
              opportunity_count,
              scanner_version,
              metadata_json,
              trace_id
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            "#,
        )
        .bind(&scan.id)
        .bind(scan.started_at)
        .bind(scan.finished_at)
        .bind(i64::from(scan.market_count))
        .bind(i64::from(scan.snapshot_count))
        .bind(i64::from(scan.opportunity_count))
        .bind(&scan.scanner_version)
        .bind(Json(scan.metadata.clone()))
        .bind(&scan.trace_id)
        .execute(&self.pool)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_INSERT_FAILED",
                format!("failed to insert arbitrage scan {}: {error}", scan.id),
            )
        })?;

        Ok(())
    }

    async fn complete_arbitrage_scan(
        &self,
        scan_id: &str,
        finished_at: OffsetDateTime,
        market_count: u32,
        snapshot_count: u32,
        opportunity_count: u32,
    ) -> Result<ArbitrageScanView> {
        let row = sqlx::query(
            r#"
            UPDATE arbitrage_scans
            SET
              finished_at = $2,
              market_count = $3,
              snapshot_count = $4,
              opportunity_count = $5
            WHERE id = $1
            RETURNING
              id,
              started_at,
              finished_at,
              market_count,
              snapshot_count,
              opportunity_count,
              scanner_version,
              metadata_json,
              trace_id
            "#,
        )
        .bind(scan_id)
        .bind(finished_at)
        .bind(i64::from(market_count))
        .bind(i64::from(snapshot_count))
        .bind(i64::from(opportunity_count))
        .fetch_optional(&self.pool)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_UPDATE_FAILED",
                format!("failed to complete arbitrage scan {scan_id}: {error}"),
            )
        })?;

        row.as_ref()
            .map(parse_arbitrage_scan_row)
            .transpose()?
            .ok_or_else(|| {
                AppError::not_found(
                    "ARBITRAGE_SCAN_NOT_FOUND",
                    format!("arbitrage scan was not found: {scan_id}"),
                )
            })
    }

    async fn record_market_book_snapshot(&self, snapshot: &MarketBookSnapshotView) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO market_book_snapshots (
              id,
              scan_id,
              connector_name,
              market_id,
              yes_asset_id,
              no_asset_id,
              yes_bid,
              yes_ask,
              yes_bid_size,
              yes_ask_size,
              no_bid,
              no_ask,
              no_bid_size,
              no_ask_size,
              observed_at,
              raw_payload_json,
              trace_id
            )
            VALUES (
              $1, $2, $3, $4, $5, $6, $7, $8, $9, $10,
              $11, $12, $13, $14, $15, $16, $17
            )
            ON CONFLICT (id) DO UPDATE
            SET
              connector_name = EXCLUDED.connector_name,
              market_id = EXCLUDED.market_id,
              yes_asset_id = EXCLUDED.yes_asset_id,
              no_asset_id = EXCLUDED.no_asset_id,
              yes_bid = EXCLUDED.yes_bid,
              yes_ask = EXCLUDED.yes_ask,
              yes_bid_size = EXCLUDED.yes_bid_size,
              yes_ask_size = EXCLUDED.yes_ask_size,
              no_bid = EXCLUDED.no_bid,
              no_ask = EXCLUDED.no_ask,
              no_bid_size = EXCLUDED.no_bid_size,
              no_ask_size = EXCLUDED.no_ask_size,
              observed_at = EXCLUDED.observed_at,
              raw_payload_json = EXCLUDED.raw_payload_json,
              trace_id = EXCLUDED.trace_id
            "#,
        )
        .bind(&snapshot.id)
        .bind(&snapshot.scan_id)
        .bind(&snapshot.connector_name)
        .bind(&snapshot.market_id)
        .bind(&snapshot.yes_asset_id)
        .bind(&snapshot.no_asset_id)
        .bind(snapshot.yes_bid.map(Probability::value))
        .bind(snapshot.yes_ask.map(Probability::value))
        .bind(snapshot.yes_bid_size.value())
        .bind(snapshot.yes_ask_size.value())
        .bind(snapshot.no_bid.map(Probability::value))
        .bind(snapshot.no_ask.map(Probability::value))
        .bind(snapshot.no_bid_size.value())
        .bind(snapshot.no_ask_size.value())
        .bind(snapshot.observed_at)
        .bind(Json(snapshot.raw_payload.clone()))
        .bind(&snapshot.trace_id)
        .execute(&self.pool)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_UPSERT_FAILED",
                format!(
                    "failed to record market book snapshot {}: {error}",
                    snapshot.id
                ),
            )
        })?;

        Ok(())
    }

    async fn record_arbitrage_opportunity(
        &self,
        opportunity: &ArbitrageOpportunityView,
    ) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO arbitrage_opportunities (
              id,
              scan_id,
              market_id,
              opportunity_type,
              status,
              gross_edge,
              price_sum,
              capacity,
              yes_price,
              no_price,
              yes_size,
              no_size,
              observed_at,
              reason_codes_json,
              analysis_payload_json,
              trace_id
            )
            VALUES (
              $1, $2, $3, $4, $5, $6, $7, $8,
              $9, $10, $11, $12, $13, $14, $15, $16
            )
            ON CONFLICT (id) DO UPDATE
            SET
              status = EXCLUDED.status,
              gross_edge = EXCLUDED.gross_edge,
              price_sum = EXCLUDED.price_sum,
              capacity = EXCLUDED.capacity,
              yes_price = EXCLUDED.yes_price,
              no_price = EXCLUDED.no_price,
              yes_size = EXCLUDED.yes_size,
              no_size = EXCLUDED.no_size,
              observed_at = EXCLUDED.observed_at,
              reason_codes_json = EXCLUDED.reason_codes_json,
              analysis_payload_json = EXCLUDED.analysis_payload_json,
              trace_id = EXCLUDED.trace_id
            "#,
        )
        .bind(&opportunity.id)
        .bind(&opportunity.scan_id)
        .bind(&opportunity.market_id)
        .bind(opportunity.opportunity_type.as_str())
        .bind(opportunity.status.as_str())
        .bind(opportunity.gross_edge.value())
        .bind(opportunity.price_sum)
        .bind(opportunity.capacity.value())
        .bind(opportunity.yes_price.value())
        .bind(opportunity.no_price.value())
        .bind(opportunity.yes_size.value())
        .bind(opportunity.no_size.value())
        .bind(opportunity.observed_at)
        .bind(Json(opportunity.reason_codes.clone()))
        .bind(Json(opportunity.analysis_payload.clone()))
        .bind(&opportunity.trace_id)
        .execute(&self.pool)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_UPSERT_FAILED",
                format!(
                    "failed to record arbitrage opportunity {}: {error}",
                    opportunity.id
                ),
            )
        })?;

        Ok(())
    }

    async fn record_arbitrage_opportunity_validation(
        &self,
        validation: &ArbitrageOpportunityValidationView,
    ) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO arbitrage_opportunity_validations (
              id,
              opportunity_id,
              status,
              gross_edge,
              net_edge,
              fee_estimate,
              slippage_buffer,
              validated_capacity,
              book_age_ms,
              reason_codes_json,
              validation_payload_json,
              validated_at,
              trace_id
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
            ON CONFLICT (id) DO UPDATE
            SET
              status = EXCLUDED.status,
              gross_edge = EXCLUDED.gross_edge,
              net_edge = EXCLUDED.net_edge,
              fee_estimate = EXCLUDED.fee_estimate,
              slippage_buffer = EXCLUDED.slippage_buffer,
              validated_capacity = EXCLUDED.validated_capacity,
              book_age_ms = EXCLUDED.book_age_ms,
              reason_codes_json = EXCLUDED.reason_codes_json,
              validation_payload_json = EXCLUDED.validation_payload_json,
              validated_at = EXCLUDED.validated_at,
              trace_id = EXCLUDED.trace_id
            "#,
        )
        .bind(&validation.id)
        .bind(&validation.opportunity_id)
        .bind(validation.status.as_str())
        .bind(validation.gross_edge.value())
        .bind(validation.net_edge.value())
        .bind(validation.fee_estimate.value())
        .bind(validation.slippage_buffer.value())
        .bind(validation.validated_capacity.value())
        .bind(i64::try_from(validation.book_age_ms).unwrap_or(i64::MAX))
        .bind(Json(validation.reason_codes.clone()))
        .bind(Json(validation.validation_payload.clone()))
        .bind(validation.validated_at)
        .bind(&validation.trace_id)
        .execute(&self.pool)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_UPSERT_FAILED",
                format!(
                    "failed to record arbitrage opportunity validation {}: {error}",
                    validation.id
                ),
            )
        })?;

        Ok(())
    }

    async fn expire_arbitrage_opportunities(
        &self,
        observed_before: OffsetDateTime,
        trace_id: &str,
    ) -> Result<Vec<ArbitrageOpportunityView>> {
        let rows = sqlx::query(
            r#"
            UPDATE arbitrage_opportunities
            SET
              status = 'expired',
              trace_id = $2
            WHERE observed_at < $1
              AND status <> 'expired'
            RETURNING
              id,
              scan_id,
              market_id,
              opportunity_type,
              status,
              gross_edge,
              price_sum,
              capacity,
              yes_price,
              no_price,
              yes_size,
              no_size,
              observed_at,
              reason_codes_json,
              analysis_payload_json,
              trace_id,
              NULL::TEXT AS validation_id,
              NULL::TEXT AS validation_status,
              NULL::NUMERIC AS validation_gross_edge,
              NULL::NUMERIC AS validation_net_edge,
              NULL::NUMERIC AS validation_fee_estimate,
              NULL::NUMERIC AS validation_slippage_buffer,
              NULL::NUMERIC AS validation_validated_capacity,
              NULL::BIGINT AS validation_book_age_ms,
              NULL::JSONB AS validation_reason_codes_json,
              NULL::JSONB AS validation_payload_json,
              NULL::TIMESTAMPTZ AS validation_validated_at,
              NULL::TEXT AS validation_trace_id
            "#,
        )
        .bind(observed_before)
        .bind(trace_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_UPDATE_FAILED",
                format!("failed to expire arbitrage opportunities: {error}"),
            )
        })?;

        rows.iter().map(parse_arbitrage_opportunity_row).collect()
    }

    async fn list_arbitrage_scans(
        &self,
        _filters: &ArbitrageScanListFilters,
        page: &PageQuery,
    ) -> Result<Paginated<ArbitrageScanView>> {
        let (_page_num, page_size) = page.validated();
        let offset = page.offset();
        let limit = i64::from(page_size);

        let (total, rows) = tokio::try_join!(
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM arbitrage_scans")
                .fetch_one(&self.pool),
            sqlx::query(
                r#"
                SELECT
                  id, started_at, finished_at, market_count, snapshot_count,
                  opportunity_count, scanner_version, metadata_json, trace_id
                FROM arbitrage_scans
                ORDER BY started_at DESC, id ASC
                LIMIT $1 OFFSET $2
                "#,
            )
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.pool),
        )
        .map_err(|error| {
            db_error(
                "POSTGRES_QUERY_FAILED",
                format!("failed to list arbitrage scans: {error}"),
            )
        })?;

        let items: Result<Vec<_>> = rows.iter().map(parse_arbitrage_scan_row).collect();
        Ok(Paginated::new(items?, page, total))
    }

    async fn list_arbitrage_opportunities(
        &self,
        filters: &ArbitrageOpportunityListFilters,
        page: &PageQuery,
    ) -> Result<Paginated<ArbitrageOpportunityView>> {
        let (_page_num, page_size) = page.validated();
        let offset = page.offset();
        let limit = i64::from(page_size);

        let where_clause = r#"
            WHERE ($1::TEXT IS NULL OR o.market_id = $1)
              AND ($2::TEXT IS NULL OR o.opportunity_type = $2)
              AND ($3::TEXT IS NULL OR o.status = $3)
              AND (
                $4::TEXT IS NULL
                OR ($4 = 'unvalidated' AND v.id IS NULL)
                OR ($4 <> 'unvalidated' AND v.status = $4)
              )
              AND ($5::NUMERIC IS NULL OR v.net_edge >= $5)
              AND ($6::TIMESTAMPTZ IS NULL OR o.observed_at >= $6)
              AND (NOT $7::BOOL OR o.status <> 'expired')
        "#;

        let from_clause = r#"
            FROM arbitrage_opportunities o
            LEFT JOIN LATERAL (
              SELECT id, opportunity_id, status, gross_edge, net_edge, fee_estimate,
                     slippage_buffer, validated_capacity, book_age_ms, reason_codes_json,
                     validation_payload_json, validated_at, trace_id
              FROM arbitrage_opportunity_validations
              WHERE opportunity_id = o.id
              ORDER BY validated_at DESC, id ASC
              LIMIT 1
            ) v ON TRUE
        "#;

        let market_id = filters.market_id.as_deref();
        let opp_type = filters
            .opportunity_type
            .map(ArbitrageOpportunityType::as_str);
        let status = filters.status.map(ArbitrageOpportunityStatus::as_str);
        let val_status = filters
            .validation_status
            .map(ArbitrageValidationStatus::as_str);
        let min_edge = filters.min_net_edge.map(Edge::value);
        let after = filters.observed_after;
        let active = filters.active_only;

        let count_sql = format!("SELECT COUNT(*) {from_clause} {where_clause}");
        let data_sql = format!(
            r#"SELECT o.id, o.scan_id, o.market_id, o.opportunity_type, o.status,
              o.gross_edge, o.price_sum, o.capacity, o.yes_price, o.no_price,
              o.yes_size, o.no_size, o.observed_at, o.reason_codes_json,
              o.analysis_payload_json, o.trace_id,
              v.id AS validation_id, v.status AS validation_status,
              v.gross_edge AS validation_gross_edge, v.net_edge AS validation_net_edge,
              v.fee_estimate AS validation_fee_estimate,
              v.slippage_buffer AS validation_slippage_buffer,
              v.validated_capacity AS validation_validated_capacity,
              v.book_age_ms AS validation_book_age_ms,
              v.reason_codes_json AS validation_reason_codes_json,
              v.validation_payload_json AS validation_payload_json,
              v.validated_at AS validation_validated_at,
              v.trace_id AS validation_trace_id
            {from_clause} {where_clause}
            ORDER BY o.observed_at DESC, o.id ASC
            LIMIT $8 OFFSET $9"#
        );

        let (total, rows) = tokio::try_join!(
            sqlx::query_scalar::<_, i64>(&count_sql)
                .bind(market_id)
                .bind(opp_type)
                .bind(status)
                .bind(val_status)
                .bind(min_edge)
                .bind(after)
                .bind(active)
                .fetch_one(&self.pool),
            sqlx::query(&data_sql)
                .bind(market_id)
                .bind(opp_type)
                .bind(status)
                .bind(val_status)
                .bind(min_edge)
                .bind(after)
                .bind(active)
                .bind(limit)
                .bind(offset)
                .fetch_all(&self.pool),
        )
        .map_err(|error| {
            db_error(
                "POSTGRES_QUERY_FAILED",
                format!("failed to list arbitrage opportunities: {error}"),
            )
        })?;

        let items: Result<Vec<_>> = rows.iter().map(parse_arbitrage_opportunity_row).collect();
        Ok(Paginated::new(items?, page, total))
    }

    async fn record_arbitrage_analysis_run(
        &self,
        analysis: &ArbitrageAnalysisRunView,
    ) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO arbitrage_analysis_runs (
              id,
              generated_at,
              lookback_hours,
              opportunity_count,
              market_count,
              summary_payload_json,
              trace_id
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            ON CONFLICT (id) DO UPDATE
            SET
              generated_at = EXCLUDED.generated_at,
              lookback_hours = EXCLUDED.lookback_hours,
              opportunity_count = EXCLUDED.opportunity_count,
              market_count = EXCLUDED.market_count,
              summary_payload_json = EXCLUDED.summary_payload_json,
              trace_id = EXCLUDED.trace_id
            "#,
        )
        .bind(&analysis.id)
        .bind(analysis.generated_at)
        .bind(i64::from(analysis.lookback_hours))
        .bind(i64::from(analysis.opportunity_count))
        .bind(i64::from(analysis.market_count))
        .bind(Json(analysis.summary_payload.clone()))
        .bind(&analysis.trace_id)
        .execute(&self.pool)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_UPSERT_FAILED",
                format!(
                    "failed to record arbitrage analysis run {}: {error}",
                    analysis.id
                ),
            )
        })?;

        Ok(())
    }

    async fn list_arbitrage_analysis_runs(
        &self,
        _filters: &ArbitrageAnalysisRunListFilters,
        page: &PageQuery,
    ) -> Result<Paginated<ArbitrageAnalysisRunView>> {
        let (_page_num, page_size) = page.validated();
        let offset = page.offset();
        let limit = i64::from(page_size);

        let (total, rows) = tokio::try_join!(
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM arbitrage_analysis_runs")
                .fetch_one(&self.pool),
            sqlx::query(
                r#"
                SELECT id, generated_at, lookback_hours, opportunity_count,
                       market_count, summary_payload_json, trace_id
                FROM arbitrage_analysis_runs
                ORDER BY generated_at DESC, id ASC
                LIMIT $1 OFFSET $2
                "#,
            )
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.pool),
        )
        .map_err(|error| {
            db_error(
                "POSTGRES_QUERY_FAILED",
                format!("failed to list arbitrage analysis runs: {error}"),
            )
        })?;

        let items: Result<Vec<_>> = rows.iter().map(parse_arbitrage_analysis_run_row).collect();
        Ok(Paginated::new(items?, page, total))
    }

    async fn record_arbitrage_event(
        &self,
        event: &ArbitrageEventView,
    ) -> Result<ArbitrageEventView> {
        let row = sqlx::query(
            r#"
            INSERT INTO arbitrage_events (
              id,
              event_type,
              resource_type,
              resource_id,
              payload_json,
              occurred_at,
              trace_id
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            ON CONFLICT (id) DO UPDATE
            SET
              event_type = EXCLUDED.event_type,
              resource_type = EXCLUDED.resource_type,
              resource_id = EXCLUDED.resource_id,
              payload_json = EXCLUDED.payload_json,
              occurred_at = EXCLUDED.occurred_at,
              trace_id = EXCLUDED.trace_id
            RETURNING
              sequence,
              id,
              event_type,
              resource_type,
              resource_id,
              payload_json,
              occurred_at,
              trace_id
            "#,
        )
        .bind(&event.id)
        .bind(event.event_type.as_str())
        .bind(&event.resource_type)
        .bind(&event.resource_id)
        .bind(Json(event.payload.clone()))
        .bind(event.occurred_at)
        .bind(&event.trace_id)
        .fetch_one(&self.pool)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_UPSERT_FAILED",
                format!("failed to record arbitrage event {}: {error}", event.id),
            )
        })?;

        parse_arbitrage_event_row(&row)
    }

    async fn list_arbitrage_events(
        &self,
        filters: &ArbitrageEventListFilters,
        page: &PageQuery,
    ) -> Result<Paginated<ArbitrageEventView>> {
        let after_sequence = filters
            .after_sequence
            .map(|sequence| {
                i64::try_from(sequence).map_err(|error| {
                    AppError::invalid_input(
                        "ARBITRAGE_EVENT_SEQUENCE_OUT_OF_RANGE",
                        format!("arbitrage event sequence does not fit i64: {error}"),
                    )
                })
            })
            .transpose()?;
        let (_page_num, page_size) = page.validated();
        let offset = page.offset();
        let limit = i64::from(page_size);

        let (total, rows) = tokio::try_join!(
            sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM arbitrage_events WHERE ($1::BIGINT IS NULL OR sequence > $1)"
            )
            .bind(after_sequence)
            .fetch_one(&self.pool),
            sqlx::query(
                r#"
                SELECT sequence, id, event_type, resource_type, resource_id,
                       payload_json, occurred_at, trace_id
                FROM arbitrage_events
                WHERE ($1::BIGINT IS NULL OR sequence > $1)
                ORDER BY sequence ASC
                LIMIT $2 OFFSET $3
                "#,
            )
            .bind(after_sequence)
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.pool),
        )
        .map_err(|error| {
            db_error(
                "POSTGRES_QUERY_FAILED",
                format!("failed to list arbitrage events: {error}"),
            )
        })?;

        let items: Result<Vec<_>> = rows.iter().map(parse_arbitrage_event_row).collect();
        Ok(Paginated::new(items?, page, total))
    }

    async fn prune_arbitrage_events(&self, occurred_before: OffsetDateTime) -> Result<u64> {
        let result = sqlx::query(
            r#"
            DELETE FROM arbitrage_events
            WHERE occurred_at < $1
            "#,
        )
        .bind(occurred_before)
        .execute(&self.pool)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_DELETE_FAILED",
                format!("failed to prune arbitrage events: {error}"),
            )
        })?;

        Ok(result.rows_affected())
    }
}
