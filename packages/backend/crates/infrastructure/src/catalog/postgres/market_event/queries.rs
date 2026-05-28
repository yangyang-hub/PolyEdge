impl PostgresMarketEventStore {
async fn market_event_list_markets(&self, filters: &MarketListFilters) -> Result<Vec<MarketView>> {
        let order_column = match filters.sort_by {
            MarketSortField::Volume24h => "m.volume_24h",
            MarketSortField::UpdatedAt => "m.updated_at",
        };
        let order_dir = match filters.sort_order {
            SortOrder::Asc => "ASC",
            SortOrder::Desc => "DESC",
        };
        let sql = format!(
            r#"
            SELECT
              m.id,
              m.question,
              m.category,
              m.status,
              m.best_bid,
              m.best_ask,
              m.mid_price,
              m.volume_24h,
              m.ambiguity_level,
              m.tradability_status,
              r.resolution_source,
              r.edge_case_notes,
              m.polymarket_condition_id,
              m.polymarket_yes_asset_id,
              m.polymarket_no_asset_id,
              m.updated_at,
              m.version
            FROM markets m
            INNER JOIN market_resolution_rules r ON r.market_id = m.id
            WHERE ($1::TEXT IS NULL OR m.status = $1)
              AND ($2::TEXT IS NULL OR m.tradability_status = $2)
              AND ($3::TEXT IS NULL OR m.category = $3)
            ORDER BY {order_column} {order_dir}, m.id ASC
            LIMIT $4 OFFSET $5
            "#,
        );
        let rows = sqlx::query(&sql)
            .bind(filters.status.map(MarketStatus::as_str))
            .bind(filters.tradability_status.map(TradabilityStatus::as_str))
            .bind(filters.category.as_deref())
            .bind(i64::from(filters.limit))
            .bind(i64::from(filters.offset))
            .fetch_all(&self.pool)
            .await
            .map_err(|error| {
                db_error(
                    "POSTGRES_QUERY_FAILED",
                    format!("failed to list markets: {error}"),
                )
            })?;

        rows.iter().map(parse_market_row).collect()
    }

async fn market_event_count_markets(&self, filters: &MarketListFilters) -> Result<i64> {
        let row = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT COUNT(*)
            FROM markets m
            WHERE ($1::TEXT IS NULL OR m.status = $1)
              AND ($2::TEXT IS NULL OR m.tradability_status = $2)
              AND ($3::TEXT IS NULL OR m.category = $3)
            "#,
        )
        .bind(filters.status.map(MarketStatus::as_str))
        .bind(filters.tradability_status.map(TradabilityStatus::as_str))
        .bind(filters.category.as_deref())
        .fetch_one(&self.pool)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_QUERY_FAILED",
                format!("failed to count markets: {error}"),
            )
        })?;

        Ok(row)
    }

async fn market_event_get_market(&self, market_id: &str) -> Result<Option<MarketView>> {
        let row = sqlx::query(
            r#"
            SELECT
              m.id,
              m.question,
              m.category,
              m.status,
              m.best_bid,
              m.best_ask,
              m.mid_price,
              m.volume_24h,
              m.ambiguity_level,
              m.tradability_status,
              r.resolution_source,
              r.edge_case_notes,
              m.polymarket_condition_id,
              m.polymarket_yes_asset_id,
              m.polymarket_no_asset_id,
              m.updated_at,
              m.version
            FROM markets m
            INNER JOIN market_resolution_rules r ON r.market_id = m.id
            WHERE m.id = $1
            "#,
        )
        .bind(market_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_QUERY_FAILED",
                format!("failed to fetch market {market_id}: {error}"),
            )
        })?;

        row.as_ref().map(parse_market_row).transpose()
    }

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

async fn market_event_list_events(&self, filters: &EventListFilters) -> Result<Vec<EventView>> {
        let rows = sqlx::query(
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
              e.id,
              e.source,
              e.summary,
              e.relevance_score,
              e.confidence,
              e.status,
              e.reason_trace,
              e.created_at,
              e.updated_at,
              e.version
            ORDER BY e.updated_at DESC, e.id ASC
            LIMIT $2
            "#,
        )
        .bind(filters.status.map(EventStatus::as_str))
        .bind(i64::from(filters.limit))
        .fetch_all(&self.pool)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_QUERY_FAILED",
                format!("failed to list events: {error}"),
            )
        })?;

        rows.iter().map(parse_event_row).collect()
    }

async fn market_event_list_evidences(&self, filters: &EvidenceListFilters) -> Result<Vec<EvidenceView>> {
        let rows = sqlx::query(
            r#"
            SELECT
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
              version
            FROM evidences
            WHERE ($1::TEXT IS NULL OR market_id = $1)
              AND ($2::TEXT IS NULL OR event_id = $2)
              AND ($3::TEXT IS NULL OR status = $3)
            ORDER BY created_at DESC, id ASC
            LIMIT $4
            "#,
        )
        .bind(filters.market_id.as_deref())
        .bind(filters.event_id.as_deref())
        .bind(filters.status.map(EvidenceStatus::as_str))
        .bind(i64::from(filters.limit))
        .fetch_all(&self.pool)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_QUERY_FAILED",
                format!("failed to list evidences: {error}"),
            )
        })?;

        rows.iter().map(parse_evidence_row).collect()
    }

async fn market_event_list_signals(&self, filters: &SignalListFilters) -> Result<Vec<SignalView>> {
        let rows = sqlx::query(
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
            WHERE ($1::TEXT IS NULL OR s.market_id = $1)
              AND ($2::TEXT IS NULL OR s.event_id = $2)
              AND ($3::TEXT IS NULL OR s.lifecycle_state = $3)
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
            ORDER BY s.updated_at DESC, s.id ASC
            LIMIT $4
            "#,
        )
        .bind(filters.market_id.as_deref())
        .bind(filters.event_id.as_deref())
        .bind(filters.lifecycle_state.map(SignalLifecycleState::as_str))
        .bind(i64::from(filters.limit))
        .fetch_all(&self.pool)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_QUERY_FAILED",
                format!("failed to list signals: {error}"),
            )
        })?;

        rows.iter().map(parse_signal_row).collect()
    }

async fn market_event_list_probability_estimates(
        &self,
        filters: &ProbabilityEstimateListFilters,
    ) -> Result<Vec<ProbabilityEstimateView>> {
        let rows = sqlx::query(
            r#"
            SELECT
              id,
              market_id,
              event_id,
              signal_id,
              prior_price,
              posterior_price,
              fair_price,
              market_price,
              edge,
              confidence,
              time_horizon,
              model_version,
              reason_codes_json,
              evidence_count,
              created_at
            FROM probability_estimates
            WHERE ($1::TEXT IS NULL OR market_id = $1)
              AND ($2::TEXT IS NULL OR event_id = $2)
              AND ($3::TEXT IS NULL OR signal_id = $3)
            ORDER BY created_at DESC, id ASC
            LIMIT $4
            "#,
        )
        .bind(filters.market_id.as_deref())
        .bind(filters.event_id.as_deref())
        .bind(filters.signal_id.as_deref())
        .bind(i64::from(filters.limit))
        .fetch_all(&self.pool)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_QUERY_FAILED",
                format!("failed to list probability estimates: {error}"),
            )
        })?;

        rows.iter().map(parse_probability_estimate_row).collect()
    }

async fn market_event_list_signal_transitions(
        &self,
        filters: &SignalTransitionListFilters,
    ) -> Result<Vec<SignalTransitionView>> {
        let rows = sqlx::query(
            r#"
            SELECT
              id,
              signal_id,
              from_state,
              to_state,
              trigger_type,
              trigger_payload_json,
              created_at
            FROM signal_transitions
            WHERE signal_id = $1
            ORDER BY created_at DESC, id ASC
            LIMIT $2
            "#,
        )
        .bind(&filters.signal_id)
        .bind(i64::from(filters.limit))
        .fetch_all(&self.pool)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_QUERY_FAILED",
                format!("failed to list signal transitions: {error}"),
            )
        })?;

        rows.iter().map(parse_signal_transition_row).collect()
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

async fn market_event_list_positions(&self, filters: &PositionListFilters) -> Result<Vec<PositionView>> {
        let rows = sqlx::query(
            r#"
            SELECT
              id,
              market_id,
              connector_name,
              account_id,
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
}
