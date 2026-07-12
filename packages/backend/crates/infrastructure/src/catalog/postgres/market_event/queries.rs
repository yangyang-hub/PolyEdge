impl PostgresMarketEventStore {
    async fn market_event_get_signal(&self, signal_id: &str) -> Result<Option<SignalView>> {
        let row = sqlx::query(
            r#"
            SELECT
              s.id,
              s.market_id,
              s.event_id,
              s.action,
              s.side,
              s.market_price,
              s.fair_price,
              s.edge,
              s.confidence,
              s.lifecycle_state,
              s.reason,
              s.risk_decision,
              s.approved_by_user_id,
              s.approved_at,
              s.rejected_by_user_id,
              s.rejected_at,
              s.updated_at,
              s.version,
              COALESCE(
                array_agg(sel.evidence_id ORDER BY sel.evidence_id)
                FILTER (WHERE sel.evidence_id IS NOT NULL),
                '{}'::TEXT[]
              ) AS evidence_ids
            FROM signals s
            LEFT JOIN signal_evidence_links sel ON sel.signal_id = s.id
            WHERE s.id = $1
            GROUP BY
              s.id,
              s.market_id,
              s.event_id,
              s.action,
              s.side,
              s.market_price,
              s.fair_price,
              s.edge,
              s.confidence,
              s.lifecycle_state,
              s.reason,
              s.risk_decision,
              s.approved_by_user_id,
              s.approved_at,
              s.rejected_by_user_id,
              s.rejected_at,
              s.updated_at,
              s.version
            "#,
        )
        .bind(signal_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_QUERY_FAILED",
                format!("failed to fetch signal {signal_id}: {error}"),
            )
        })?;

        row.as_ref().map(parse_signal_row).transpose()
    }

    async fn market_event_list_events(
        &self,
        filters: &EventListFilters,
        page: &PageQuery,
    ) -> Result<Paginated<EventView>> {
        let offset = page.offset();
        let limit = i64::from(page.validated().1);

        let (total_count, rows) = tokio::try_join!(
            sqlx::query_scalar::<_, i64>(
                r#"
                SELECT COUNT(DISTINCT e.id)
                FROM events e
                LEFT JOIN event_market_links eml ON eml.event_id = e.id
                WHERE ($1::TEXT IS NULL OR e.status = $1)
                "#,
            )
            .bind(filters.status.map(EventStatus::as_str))
            .fetch_one(&self.pool),
            sqlx::query(
                r#"
                SELECT
                  e.id,
                  e.source,
                  e.summary,
                  e.relevance_score,
                  e.confidence,
                  e.status,
                  e.reason_trace,
                  e.created_at,
                  e.updated_at,
                  e.version,
                  COALESCE(
                    array_agg(eml.market_id ORDER BY eml.market_id)
                    FILTER (WHERE eml.market_id IS NOT NULL),
                    '{}'::TEXT[]
                  ) AS related_market_ids
                FROM events e
                LEFT JOIN event_market_links eml ON eml.event_id = e.id
                WHERE ($1::TEXT IS NULL OR e.status = $1)
                GROUP BY
                  e.id, e.source, e.summary, e.relevance_score,
                  e.confidence, e.status, e.reason_trace,
                  e.created_at, e.updated_at, e.version
                ORDER BY e.updated_at DESC, e.id ASC
                LIMIT $2 OFFSET $3
                "#,
            )
            .bind(filters.status.map(EventStatus::as_str))
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.pool),
        )
        .map_err(|error| {
            db_error(
                "POSTGRES_QUERY_FAILED",
                format!("failed to list events: {error}"),
            )
        })?;

        let items: Result<Vec<_>> = rows.iter().map(parse_event_row).collect();
        Ok(Paginated::new(items?, page, total_count))
    }

    async fn market_event_list_evidences(
        &self,
        filters: &EvidenceListFilters,
        page: &PageQuery,
    ) -> Result<Paginated<EvidenceView>> {
        let offset = page.offset();
        let limit = i64::from(page.validated().1);

        let (total_count, rows) = tokio::try_join!(
            sqlx::query_scalar::<_, i64>(
                r#"
                SELECT COUNT(*) FROM evidences
                WHERE ($1::TEXT IS NULL OR market_id = $1)
                  AND ($2::TEXT IS NULL OR event_id = $2)
                  AND ($3::TEXT IS NULL OR status = $3)
                "#,
            )
            .bind(filters.market_id.as_deref())
            .bind(filters.event_id.as_deref())
            .bind(filters.status.map(EvidenceStatus::as_str))
            .fetch_one(&self.pool),
            sqlx::query(
                r#"
                SELECT
                  id, market_id, event_id, direction, strength,
                  source_reliability, novelty, resolution_relevance,
                  status, expires_at, created_at, updated_at, version
                FROM evidences
                WHERE ($1::TEXT IS NULL OR market_id = $1)
                  AND ($2::TEXT IS NULL OR event_id = $2)
                  AND ($3::TEXT IS NULL OR status = $3)
                ORDER BY created_at DESC, id ASC
                LIMIT $4 OFFSET $5
                "#,
            )
            .bind(filters.market_id.as_deref())
            .bind(filters.event_id.as_deref())
            .bind(filters.status.map(EvidenceStatus::as_str))
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.pool),
        )
        .map_err(|error| {
            db_error(
                "POSTGRES_QUERY_FAILED",
                format!("failed to list evidences: {error}"),
            )
        })?;

        let items: Result<Vec<_>> = rows.iter().map(parse_evidence_row).collect();
        Ok(Paginated::new(items?, page, total_count))
    }

    async fn market_event_list_signals(
        &self,
        filters: &SignalListFilters,
        page: &PageQuery,
    ) -> Result<Paginated<SignalView>> {
        let offset = page.offset();
        let limit = i64::from(page.validated().1);

        let (total_count, rows) = tokio::try_join!(
            sqlx::query_scalar::<_, i64>(
                r#"
                SELECT COUNT(DISTINCT s.id)
                FROM signals s
                LEFT JOIN signal_evidence_links sel ON sel.signal_id = s.id
                WHERE ($1::TEXT IS NULL OR s.market_id = $1)
                  AND ($2::TEXT IS NULL OR s.event_id = $2)
                  AND ($3::TEXT IS NULL OR s.lifecycle_state = $3)
                "#,
            )
            .bind(filters.market_id.as_deref())
            .bind(filters.event_id.as_deref())
            .bind(filters.lifecycle_state.map(SignalLifecycleState::as_str))
            .fetch_one(&self.pool),
            sqlx::query(
                r#"
                SELECT
                  s.id, s.market_id, s.event_id, s.action, s.side,
                  s.market_price, s.fair_price, s.edge, s.confidence,
                  s.lifecycle_state, s.reason, s.risk_decision,
                  s.approved_by_user_id, s.approved_at,
                  s.rejected_by_user_id, s.rejected_at,
                  s.updated_at, s.version,
                  COALESCE(
                    array_agg(sel.evidence_id ORDER BY sel.evidence_id)
                    FILTER (WHERE sel.evidence_id IS NOT NULL),
                    '{}'::TEXT[]
                  ) AS evidence_ids
                FROM signals s
                LEFT JOIN signal_evidence_links sel ON sel.signal_id = s.id
                WHERE ($1::TEXT IS NULL OR s.market_id = $1)
                  AND ($2::TEXT IS NULL OR s.event_id = $2)
                  AND ($3::TEXT IS NULL OR s.lifecycle_state = $3)
                GROUP BY
                  s.id, s.market_id, s.event_id, s.action, s.side,
                  s.market_price, s.fair_price, s.edge, s.confidence,
                  s.lifecycle_state, s.reason, s.risk_decision,
                  s.approved_by_user_id, s.approved_at,
                  s.rejected_by_user_id, s.rejected_at,
                  s.updated_at, s.version
                ORDER BY s.updated_at DESC, s.id ASC
                LIMIT $4 OFFSET $5
                "#,
            )
            .bind(filters.market_id.as_deref())
            .bind(filters.event_id.as_deref())
            .bind(filters.lifecycle_state.map(SignalLifecycleState::as_str))
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.pool),
        )
        .map_err(|error| {
            db_error(
                "POSTGRES_QUERY_FAILED",
                format!("failed to list signals: {error}"),
            )
        })?;

        let items: Result<Vec<_>> = rows.iter().map(parse_signal_row).collect();
        Ok(Paginated::new(items?, page, total_count))
    }

    async fn market_event_list_probability_estimates(
        &self,
        filters: &ProbabilityEstimateListFilters,
        page: &PageQuery,
    ) -> Result<Paginated<ProbabilityEstimateView>> {
        let order_dir = match page.sort_order() {
            SortOrder::Asc => "ASC",
            SortOrder::Desc => "DESC",
        };
        let offset = page.offset();
        let limit = i64::from(page.validated().1);

        let data_sql = format!(
            r#"
            SELECT
              id, market_id, event_id, signal_id, prior_price, posterior_price,
              fair_price, market_price, edge, confidence, time_horizon,
              model_version, reason_codes_json, evidence_count, created_at
            FROM probability_estimates
            WHERE ($1::TEXT IS NULL OR market_id = $1)
              AND ($2::TEXT IS NULL OR event_id = $2)
              AND ($3::TEXT IS NULL OR signal_id = $3)
            ORDER BY created_at {order_dir}, id ASC
            LIMIT $4 OFFSET $5
            "#,
        );

        let (total_count, rows) = tokio::try_join!(
            sqlx::query_scalar::<_, i64>(
                r#"
                SELECT COUNT(*) FROM probability_estimates
                WHERE ($1::TEXT IS NULL OR market_id = $1)
                  AND ($2::TEXT IS NULL OR event_id = $2)
                  AND ($3::TEXT IS NULL OR signal_id = $3)
                "#,
            )
            .bind(filters.market_id.as_deref())
            .bind(filters.event_id.as_deref())
            .bind(filters.signal_id.as_deref())
            .fetch_one(&self.pool),
            sqlx::query(&data_sql)
                .bind(filters.market_id.as_deref())
                .bind(filters.event_id.as_deref())
                .bind(filters.signal_id.as_deref())
                .bind(limit)
                .bind(offset)
                .fetch_all(&self.pool),
        )
        .map_err(|error| {
            db_error(
                "POSTGRES_QUERY_FAILED",
                format!("failed to list probability estimates: {error}"),
            )
        })?;

        let items: Vec<ProbabilityEstimateView> = rows
            .iter()
            .map(parse_probability_estimate_row)
            .collect::<Result<_>>()?;
        Ok(Paginated::new(items, page, total_count))
    }

    async fn market_event_list_signal_transitions(
        &self,
        filters: &SignalTransitionListFilters,
        page: &PageQuery,
    ) -> Result<Paginated<SignalTransitionView>> {
        let order_dir = match page.sort_order() {
            SortOrder::Asc => "ASC",
            SortOrder::Desc => "DESC",
        };
        let offset = page.offset();
        let limit = i64::from(page.validated().1);

        let data_sql = format!(
            r#"
            SELECT
              id, signal_id, from_state, to_state, trigger_type,
              trigger_payload_json, created_at
            FROM signal_transitions
            WHERE signal_id = $1
            ORDER BY created_at {order_dir}, id ASC
            LIMIT $2 OFFSET $3
            "#,
        );

        let (total_count, rows) = tokio::try_join!(
            sqlx::query_scalar::<_, i64>(
                r#"
                SELECT COUNT(*) FROM signal_transitions
                WHERE signal_id = $1
                "#,
            )
            .bind(&filters.signal_id)
            .fetch_one(&self.pool),
            sqlx::query(&data_sql)
                .bind(&filters.signal_id)
                .bind(limit)
                .bind(offset)
                .fetch_all(&self.pool),
        )
        .map_err(|error| {
            db_error(
                "POSTGRES_QUERY_FAILED",
                format!("failed to list signal transitions: {error}"),
            )
        })?;

        let items: Vec<SignalTransitionView> = rows
            .iter()
            .map(parse_signal_transition_row)
            .collect::<Result<_>>()?;
        Ok(Paginated::new(items, page, total_count))
    }

    async fn market_event_list_order_drafts(
        &self,
        filters: &OrderDraftListFilters,
    ) -> Result<Vec<OrderDraftView>> {
        let rows = sqlx::query(
            r#"
            SELECT
              id,
              signal_id,
              signal_version,
              market_id,
              connector_name,
              side,
              limit_price,
              quantity,
              notional,
              status,
              created_by_user_id,
              external_order_id,
              submitted_at,
              failure_code,
              failure_message,
              created_at,
              updated_at,
              version
            FROM order_drafts
            WHERE ($1::TEXT IS NULL OR signal_id = $1)
              AND ($2::TEXT IS NULL OR connector_name = $2)
              AND ($3::TEXT IS NULL OR status = $3)
            ORDER BY created_at DESC, id ASC
            LIMIT $4
            "#,
        )
        .bind(filters.signal_id.as_deref())
        .bind(filters.connector_name.as_deref())
        .bind(filters.status.map(OrderDraftStatus::as_str))
        .bind(i64::from(filters.limit))
        .fetch_all(&self.pool)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_QUERY_FAILED",
                format!("failed to list order drafts: {error}"),
            )
        })?;

        rows.iter().map(parse_order_draft_row).collect()
    }

    async fn market_event_list_execution_requests(
        &self,
        filters: &ExecutionRequestListFilters,
    ) -> Result<Vec<ExecutionRequestView>> {
        let rows = sqlx::query(
            r#"
            SELECT
              id,
              signal_id,
              signal_version,
              order_draft_id,
              connector_name,
              mode,
              requested_by_user_id,
              status,
              reason,
              external_order_id,
              submitted_at,
              failure_code,
              failure_message,
              created_at,
              updated_at,
              version
            FROM execution_requests
            WHERE ($1::TEXT IS NULL OR signal_id = $1)
              AND ($2::TEXT IS NULL OR connector_name = $2)
              AND ($3::TEXT IS NULL OR status = $3)
            ORDER BY created_at DESC, id ASC
            LIMIT $4
            "#,
        )
        .bind(filters.signal_id.as_deref())
        .bind(filters.connector_name.as_deref())
        .bind(filters.status.map(ExecutionRequestStatus::as_str))
        .bind(i64::from(filters.limit))
        .fetch_all(&self.pool)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_QUERY_FAILED",
                format!("failed to list execution requests: {error}"),
            )
        })?;

        rows.iter().map(parse_execution_request_row).collect()
    }

    async fn market_event_get_order_by_external_ref(
        &self,
        connector_name: &str,
        external_order_id: &str,
    ) -> Result<OrderView> {
        let row = sqlx::query(
            r#"
            SELECT
              id,
              signal_id,
              execution_request_id,
              order_draft_id,
              market_id,
              connector_name,
              account_id,
              external_order_id,
              side,
              limit_price,
              quantity,
              filled_quantity,
              avg_fill_price,
              status,
              submitted_at,
              updated_at,
              version
            FROM orders
            WHERE connector_name = $1
              AND external_order_id = $2
            LIMIT 1
            "#,
        )
        .bind(connector_name)
        .bind(external_order_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_QUERY_FAILED",
                format!(
                    "failed to load order for connector={} external_order_id={}: {error}",
                    connector_name, external_order_id
                ),
            )
        })?
        .ok_or_else(|| {
            AppError::not_found(
                "ORDER_NOT_FOUND",
                format!(
                    "order was not found for connector={} external_order_id={}",
                    connector_name, external_order_id
                ),
            )
        })?;

        parse_order_row(&row)
    }

    async fn market_event_list_orders(&self, filters: &OrderListFilters) -> Result<Vec<OrderView>> {
        let rows = sqlx::query(
            r#"
            SELECT
              id,
              signal_id,
              execution_request_id,
              order_draft_id,
              market_id,
              connector_name,
              account_id,
              external_order_id,
              side,
              limit_price,
              quantity,
              filled_quantity,
              avg_fill_price,
              status,
              submitted_at,
              updated_at,
              version
            FROM orders
            WHERE ($1::TEXT IS NULL OR signal_id = $1)
              AND ($2::TEXT IS NULL OR market_id = $2)
              AND ($3::TEXT IS NULL OR connector_name = $3)
              AND ($4::TEXT IS NULL OR status = $4)
            ORDER BY updated_at DESC, id ASC
            LIMIT $5
            "#,
        )
        .bind(filters.signal_id.as_deref())
        .bind(filters.market_id.as_deref())
        .bind(filters.connector_name.as_deref())
        .bind(filters.status.map(OrderStatus::as_str))
        .bind(i64::from(filters.limit))
        .fetch_all(&self.pool)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_QUERY_FAILED",
                format!("failed to list orders: {error}"),
            )
        })?;

        rows.iter().map(parse_order_row).collect()
    }

    async fn market_event_list_active_order_market_ids(
        &self,
        connector_name: &str,
        limit: usize,
    ) -> Result<Vec<String>> {
        let rows = sqlx::query_scalar::<_, String>(
            r#"
            SELECT market_id
            FROM orders
            WHERE connector_name = $1
              AND status IN ('submitted', 'open', 'partially_filled')
            GROUP BY market_id
            ORDER BY MAX(updated_at) DESC, market_id ASC
            LIMIT $2
            "#,
        )
        .bind(connector_name)
        .bind(i64::try_from(limit).unwrap_or(i64::MAX))
        .fetch_all(&self.pool)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_QUERY_FAILED",
                format!("failed to list active order market ids: {error}"),
            )
        })?;
        Ok(rows)
    }

    async fn market_event_list_trades(&self, filters: &TradeListFilters) -> Result<Vec<TradeView>> {
        let rows = sqlx::query(
            r#"
            SELECT
              id,
              order_id,
              signal_id,
              market_id,
              connector_name,
              external_trade_id,
              side,
              price,
              quantity,
              fee,
              executed_at
            FROM trades
            WHERE ($1::TEXT IS NULL OR order_id = $1)
              AND ($2::TEXT IS NULL OR signal_id = $2)
              AND ($3::TEXT IS NULL OR market_id = $3)
              AND ($4::TEXT IS NULL OR connector_name = $4)
            ORDER BY executed_at DESC, id ASC
            LIMIT $5
            "#,
        )
        .bind(filters.order_id.as_deref())
        .bind(filters.signal_id.as_deref())
        .bind(filters.market_id.as_deref())
        .bind(filters.connector_name.as_deref())
        .bind(i64::from(filters.limit))
        .fetch_all(&self.pool)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_QUERY_FAILED",
                format!("failed to list trades: {error}"),
            )
        })?;

        rows.iter().map(parse_trade_row).collect()
    }

    async fn market_event_list_positions(
        &self,
        filters: &PositionListFilters,
    ) -> Result<Vec<PositionView>> {
        let rows = sqlx::query(
            r#"
            SELECT
              id,
              market_id,
              connector_name,
              side,
              net_quantity,
              avg_cost,
              mark_price,
              unrealized_pnl,
              realized_pnl,
              updated_at,
              version
            FROM positions
            WHERE ($1::TEXT IS NULL OR market_id = $1)
              AND ($2::TEXT IS NULL OR connector_name = $2)
              AND ($3::TEXT IS NULL OR side = $3)
            ORDER BY updated_at DESC, id ASC
            LIMIT $4
            "#,
        )
        .bind(filters.market_id.as_deref())
        .bind(filters.connector_name.as_deref())
        .bind(filters.side.map(SignalSide::as_str))
        .bind(i64::from(filters.limit))
        .fetch_all(&self.pool)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_QUERY_FAILED",
                format!("failed to list positions: {error}"),
            )
        })?;

        rows.iter().map(parse_position_row).collect()
    }

    async fn market_event_count_order_drafts(
        &self,
        filters: &OrderDraftListFilters,
    ) -> Result<i64> {
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM order_drafts WHERE ($1::TEXT IS NULL OR signal_id = $1) AND ($2::TEXT IS NULL OR connector_name = $2) AND ($3::TEXT IS NULL OR status = $3)")
            .bind(filters.signal_id.as_deref()).bind(filters.connector_name.as_deref()).bind(filters.status.map(OrderDraftStatus::as_str))
            .fetch_one(&self.pool).await.map_err(|e| db_error("POSTGRES_QUERY_FAILED", format!("failed to count order drafts: {e}")))
    }
    async fn market_event_list_order_drafts_paginated(
        &self,
        filters: &OrderDraftListFilters,
        page: &PageQuery,
    ) -> Result<Paginated<OrderDraftView>> {
        let (_, ps) = page.validated();
        let off = page.offset();
        let (rows, total) = tokio::try_join!(
            sqlx::query("SELECT id,signal_id,signal_version,market_id,connector_name,side,limit_price,quantity,notional,status,created_by_user_id,external_order_id,submitted_at,failure_code,failure_message,created_at,updated_at,version FROM order_drafts WHERE ($1::TEXT IS NULL OR signal_id=$1) AND ($2::TEXT IS NULL OR connector_name=$2) AND ($3::TEXT IS NULL OR status=$3) ORDER BY created_at DESC, id ASC LIMIT $4 OFFSET $5")
                .bind(filters.signal_id.as_deref()).bind(filters.connector_name.as_deref()).bind(filters.status.map(OrderDraftStatus::as_str)).bind(i64::from(ps)).bind(off).fetch_all(&self.pool),
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM order_drafts WHERE ($1::TEXT IS NULL OR signal_id=$1) AND ($2::TEXT IS NULL OR connector_name=$2) AND ($3::TEXT IS NULL OR status=$3)")
                .bind(filters.signal_id.as_deref()).bind(filters.connector_name.as_deref()).bind(filters.status.map(OrderDraftStatus::as_str)).fetch_one(&self.pool)
        ).map_err(|e| db_error("POSTGRES_QUERY_FAILED", format!("failed to list order drafts: {e}")))?;
        Ok(Paginated::new(
            rows.iter()
                .map(parse_order_draft_row)
                .collect::<Result<_>>()?,
            page,
            total,
        ))
    }
    async fn market_event_count_execution_requests(
        &self,
        filters: &ExecutionRequestListFilters,
    ) -> Result<i64> {
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM execution_requests WHERE ($1::TEXT IS NULL OR signal_id=$1) AND ($2::TEXT IS NULL OR connector_name=$2) AND ($3::TEXT IS NULL OR status=$3)")
            .bind(filters.signal_id.as_deref()).bind(filters.connector_name.as_deref()).bind(filters.status.map(ExecutionRequestStatus::as_str))
            .fetch_one(&self.pool).await.map_err(|e| db_error("POSTGRES_QUERY_FAILED", format!("failed to count execution requests: {e}")))
    }
    async fn market_event_list_execution_requests_paginated(
        &self,
        filters: &ExecutionRequestListFilters,
        page: &PageQuery,
    ) -> Result<Paginated<ExecutionRequestView>> {
        let (_, ps) = page.validated();
        let off = page.offset();
        let (rows, total) = tokio::try_join!(
            sqlx::query("SELECT id,signal_id,signal_version,order_draft_id,connector_name,mode,requested_by_user_id,status,reason,external_order_id,submitted_at,failure_code,failure_message,created_at,updated_at,version FROM execution_requests WHERE ($1::TEXT IS NULL OR signal_id=$1) AND ($2::TEXT IS NULL OR connector_name=$2) AND ($3::TEXT IS NULL OR status=$3) ORDER BY created_at DESC, id ASC LIMIT $4 OFFSET $5")
                .bind(filters.signal_id.as_deref()).bind(filters.connector_name.as_deref()).bind(filters.status.map(ExecutionRequestStatus::as_str)).bind(i64::from(ps)).bind(off).fetch_all(&self.pool),
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM execution_requests WHERE ($1::TEXT IS NULL OR signal_id=$1) AND ($2::TEXT IS NULL OR connector_name=$2) AND ($3::TEXT IS NULL OR status=$3)")
                .bind(filters.signal_id.as_deref()).bind(filters.connector_name.as_deref()).bind(filters.status.map(ExecutionRequestStatus::as_str)).fetch_one(&self.pool)
        ).map_err(|e| db_error("POSTGRES_QUERY_FAILED", format!("failed to list execution requests: {e}")))?;
        Ok(Paginated::new(
            rows.iter()
                .map(parse_execution_request_row)
                .collect::<Result<_>>()?,
            page,
            total,
        ))
    }
    async fn market_event_count_orders(&self, filters: &OrderListFilters) -> Result<i64> {
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM orders WHERE ($1::TEXT IS NULL OR signal_id=$1) AND ($2::TEXT IS NULL OR market_id=$2) AND ($3::TEXT IS NULL OR connector_name=$3) AND ($4::TEXT IS NULL OR status=$4)")
            .bind(filters.signal_id.as_deref()).bind(filters.market_id.as_deref()).bind(filters.connector_name.as_deref()).bind(filters.status.map(OrderStatus::as_str))
            .fetch_one(&self.pool).await.map_err(|e| db_error("POSTGRES_QUERY_FAILED", format!("failed to count orders: {e}")))
    }
    async fn market_event_list_orders_paginated(
        &self,
        filters: &OrderListFilters,
        page: &PageQuery,
    ) -> Result<Paginated<OrderView>> {
        let (_, ps) = page.validated();
        let off = page.offset();
        let (rows, total) = tokio::try_join!(
            sqlx::query("SELECT id,signal_id,execution_request_id,order_draft_id,market_id,connector_name,account_id,external_order_id,side,limit_price,quantity,filled_quantity,avg_fill_price,status,submitted_at,updated_at,version FROM orders WHERE ($1::TEXT IS NULL OR signal_id=$1) AND ($2::TEXT IS NULL OR market_id=$2) AND ($3::TEXT IS NULL OR connector_name=$3) AND ($4::TEXT IS NULL OR status=$4) ORDER BY updated_at DESC, id ASC LIMIT $5 OFFSET $6")
                .bind(filters.signal_id.as_deref()).bind(filters.market_id.as_deref()).bind(filters.connector_name.as_deref()).bind(filters.status.map(OrderStatus::as_str)).bind(i64::from(ps)).bind(off).fetch_all(&self.pool),
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM orders WHERE ($1::TEXT IS NULL OR signal_id=$1) AND ($2::TEXT IS NULL OR market_id=$2) AND ($3::TEXT IS NULL OR connector_name=$3) AND ($4::TEXT IS NULL OR status=$4)")
                .bind(filters.signal_id.as_deref()).bind(filters.market_id.as_deref()).bind(filters.connector_name.as_deref()).bind(filters.status.map(OrderStatus::as_str)).fetch_one(&self.pool)
        ).map_err(|e| db_error("POSTGRES_QUERY_FAILED", format!("failed to list orders: {e}")))?;
        Ok(Paginated::new(
            rows.iter().map(parse_order_row).collect::<Result<_>>()?,
            page,
            total,
        ))
    }
    async fn market_event_count_trades(&self, filters: &TradeListFilters) -> Result<i64> {
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM trades WHERE ($1::TEXT IS NULL OR order_id=$1) AND ($2::TEXT IS NULL OR signal_id=$2) AND ($3::TEXT IS NULL OR market_id=$3) AND ($4::TEXT IS NULL OR connector_name=$4)")
            .bind(filters.order_id.as_deref()).bind(filters.signal_id.as_deref()).bind(filters.market_id.as_deref()).bind(filters.connector_name.as_deref())
            .fetch_one(&self.pool).await.map_err(|e| db_error("POSTGRES_QUERY_FAILED", format!("failed to count trades: {e}")))
    }
    async fn market_event_list_trades_paginated(
        &self,
        filters: &TradeListFilters,
        page: &PageQuery,
    ) -> Result<Paginated<TradeView>> {
        let (_, ps) = page.validated();
        let off = page.offset();
        let (rows, total) = tokio::try_join!(
            sqlx::query("SELECT id,order_id,signal_id,market_id,connector_name,external_trade_id,side,price,quantity,fee,executed_at FROM trades WHERE ($1::TEXT IS NULL OR order_id=$1) AND ($2::TEXT IS NULL OR signal_id=$2) AND ($3::TEXT IS NULL OR market_id=$3) AND ($4::TEXT IS NULL OR connector_name=$4) ORDER BY executed_at DESC, id ASC LIMIT $5 OFFSET $6")
                .bind(filters.order_id.as_deref()).bind(filters.signal_id.as_deref()).bind(filters.market_id.as_deref()).bind(filters.connector_name.as_deref()).bind(i64::from(ps)).bind(off).fetch_all(&self.pool),
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM trades WHERE ($1::TEXT IS NULL OR order_id=$1) AND ($2::TEXT IS NULL OR signal_id=$2) AND ($3::TEXT IS NULL OR market_id=$3) AND ($4::TEXT IS NULL OR connector_name=$4)")
                .bind(filters.order_id.as_deref()).bind(filters.signal_id.as_deref()).bind(filters.market_id.as_deref()).bind(filters.connector_name.as_deref()).fetch_one(&self.pool)
        ).map_err(|e| db_error("POSTGRES_QUERY_FAILED", format!("failed to list trades: {e}")))?;
        Ok(Paginated::new(
            rows.iter().map(parse_trade_row).collect::<Result<_>>()?,
            page,
            total,
        ))
    }
    async fn market_event_count_positions(&self, filters: &PositionListFilters) -> Result<i64> {
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM positions WHERE ($1::TEXT IS NULL OR market_id=$1) AND ($2::TEXT IS NULL OR connector_name=$2) AND ($3::TEXT IS NULL OR side=$3)")
            .bind(filters.market_id.as_deref()).bind(filters.connector_name.as_deref()).bind(filters.side.map(SignalSide::as_str))
            .fetch_one(&self.pool).await.map_err(|e| db_error("POSTGRES_QUERY_FAILED", format!("failed to count positions: {e}")))
    }
    async fn market_event_list_positions_paginated(
        &self,
        filters: &PositionListFilters,
        page: &PageQuery,
    ) -> Result<Paginated<PositionView>> {
        let (_, ps) = page.validated();
        let off = page.offset();
        let (rows, total) = tokio::try_join!(
            sqlx::query("SELECT id,market_id,connector_name,account_id,side,net_quantity,avg_cost,mark_price,unrealized_pnl,realized_pnl,updated_at,version FROM positions WHERE ($1::TEXT IS NULL OR market_id=$1) AND ($2::TEXT IS NULL OR connector_name=$2) AND ($3::TEXT IS NULL OR side=$3) ORDER BY updated_at DESC, id ASC LIMIT $4 OFFSET $5")
                .bind(filters.market_id.as_deref()).bind(filters.connector_name.as_deref()).bind(filters.side.map(SignalSide::as_str)).bind(i64::from(ps)).bind(off).fetch_all(&self.pool),
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM positions WHERE ($1::TEXT IS NULL OR market_id=$1) AND ($2::TEXT IS NULL OR connector_name=$2) AND ($3::TEXT IS NULL OR side=$3)")
                .bind(filters.market_id.as_deref()).bind(filters.connector_name.as_deref()).bind(filters.side.map(SignalSide::as_str)).fetch_one(&self.pool)
        ).map_err(|e| db_error("POSTGRES_QUERY_FAILED", format!("failed to list positions: {e}")))?;
        Ok(Paginated::new(
            rows.iter().map(parse_position_row).collect::<Result<_>>()?,
            page,
            total,
        ))
    }
}
