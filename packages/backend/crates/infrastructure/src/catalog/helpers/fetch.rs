async fn fetch_market_by_id(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    market_id: &str,
) -> Result<Option<MarketView>> {
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
    .fetch_optional(&mut **transaction)
    .await
    .map_err(|error| {
        db_error(
            "POSTGRES_QUERY_FAILED",
            format!("failed to fetch market {market_id}: {error}"),
        )
    })?;

    row.as_ref().map(parse_market_row).transpose()
}

async fn fetch_evidences_for_signal(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    market_id: &str,
    event_id: &str,
) -> Result<Vec<EvidenceView>> {
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
        WHERE market_id = $1
          AND event_id = $2
        ORDER BY created_at DESC, id ASC
        "#,
    )
    .bind(market_id)
    .bind(event_id)
    .fetch_all(&mut **transaction)
    .await
    .map_err(|error| {
        db_error(
            "POSTGRES_QUERY_FAILED",
            format!("failed to fetch evidences for {market_id}/{event_id}: {error}"),
        )
    })?;

    rows.iter().map(parse_evidence_row).collect()
}

async fn fetch_source_health_adjustment_for_event(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    event_id: &str,
) -> Result<Option<SourceHealthAdjustment>> {
    let row = sqlx::query(
        r#"
        SELECT
          e.source,
          nsh.health_score
        FROM events e
        LEFT JOIN news_source_health nsh ON nsh.source = e.source
        WHERE e.id = $1
        "#,
    )
    .bind(event_id)
    .fetch_optional(&mut **transaction)
    .await
    .map_err(|error| {
        db_error(
            "POSTGRES_QUERY_FAILED",
            format!("failed to fetch source health for event {event_id}: {error}"),
        )
    })?;

    let Some(row) = row else {
        return Ok(None);
    };
    let health_score: Option<Decimal> = decode_column(&row, "health_score")?;
    let Some(health_score) = health_score else {
        return Ok(None);
    };

    Ok(Some(SourceHealthAdjustment {
        source: decode_column(&row, "source")?,
        health_score: Probability::new(health_score).map_err(|error| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("failed to decode event source health_score: {error}"),
            )
        })?,
    }))
}
