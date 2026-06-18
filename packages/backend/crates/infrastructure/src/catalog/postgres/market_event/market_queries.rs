impl PostgresMarketEventStore {
    async fn market_event_list_markets(
        &self,
        filters: &MarketListFilters,
    ) -> Result<Vec<MarketView>> {
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
              m.slug,
              m.question,
              m.category,
              m.status,
              m.best_bid,
              m.best_ask,
              m.mid_price,
              m.volume_24h,
              m.liquidity_usd,
              m.end_at,
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

    async fn market_event_list_market_categories(&self) -> Result<Vec<MarketCategoryView>> {
        let rows = sqlx::query(
            r#"
            SELECT id, label, sort_order
            FROM market_categories
            ORDER BY sort_order ASC
            LIMIT 100
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_QUERY_FAILED",
                format!("failed to list market categories: {error}"),
            )
        })?;

        rows.iter()
            .map(|row| {
                Ok(MarketCategoryView {
                    id: decode_column(row, "id")?,
                    label: decode_column(row, "label")?,
                    sort_order: decode_column(row, "sort_order")?,
                })
            })
            .collect()
    }

    async fn market_event_get_market(&self, market_id: &str) -> Result<Option<MarketView>> {
        let row = sqlx::query(
            r#"
            SELECT
              m.id,
              m.slug,
              m.question,
              m.category,
              m.status,
              m.best_bid,
              m.best_ask,
              m.mid_price,
              m.volume_24h,
              m.liquidity_usd,
              m.end_at,
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

    async fn market_event_get_markets_by_ids(
        &self,
        market_ids: &[String],
    ) -> Result<Vec<MarketView>> {
        if market_ids.is_empty() {
            return Ok(Vec::new());
        }

        let rows = sqlx::query(
            r#"
            SELECT
              m.id,
              m.slug,
              m.question,
              m.category,
              m.status,
              m.best_bid,
              m.best_ask,
              m.mid_price,
              m.volume_24h,
              m.liquidity_usd,
              m.end_at,
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
            WHERE m.id = ANY($1)
            ORDER BY m.id ASC
            "#,
        )
        .bind(market_ids)
        .fetch_all(&self.pool)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_QUERY_FAILED",
                format!("failed to fetch markets by ids: {error}"),
            )
        })?;

        rows.iter().map(parse_market_row).collect()
    }
}
