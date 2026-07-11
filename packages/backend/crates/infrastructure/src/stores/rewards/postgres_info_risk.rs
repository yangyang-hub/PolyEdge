async fn postgres_latest_market_info_risk(
    pool: &PgPool,
    request: &RewardInfoRiskAssessmentRequest,
    now: OffsetDateTime,
) -> Result<Option<RewardMarketInfoRisk>> {
    let row = sqlx::query(
        r#"
        SELECT condition_id,
               provider,
               request_format,
               model,
               query_hash,
               input_hash,
               action,
               risk_level,
               risk_type,
               directional_risk,
               resolution_imminent,
               expected_event_at,
               confidence,
               summary,
               sources_json,
               metrics_json,
               created_at,
               expires_at
        FROM reward_market_info_risks
        WHERE condition_id = $1
          AND provider = $2
          AND request_format = $3
          AND model = $4
          AND input_hash = $5
          AND expires_at > $6
        ORDER BY expires_at DESC, created_at DESC
        LIMIT 1
        "#,
    )
    .bind(&request.condition_id)
    .bind(request.provider.as_str())
    .bind(request.request_format.as_str())
    .bind(&request.model)
    .bind(&request.input_hash)
    .bind(now)
    .fetch_optional(pool)
    .await
    .map_err(|error| {
        db_error(
            "POSTGRES_QUERY_FAILED",
            format!("failed to query reward market info risk: {error}"),
        )
    })?;

    row.as_ref().map(reward_market_info_risk_from_row).transpose()
}

async fn postgres_latest_market_info_risks(
    pool: &PgPool,
    condition_ids: &[String],
    now: OffsetDateTime,
) -> Result<Vec<RewardMarketInfoRisk>> {
    if condition_ids.is_empty() {
        return Ok(Vec::new());
    }
    let rows = sqlx::query(
        r#"
        SELECT DISTINCT ON (condition_id)
               condition_id,
               provider,
               request_format,
               model,
               query_hash,
               input_hash,
               action,
               risk_level,
               risk_type,
               directional_risk,
               resolution_imminent,
               expected_event_at,
               confidence,
               summary,
               sources_json,
               metrics_json,
               created_at,
               expires_at
        FROM reward_market_info_risks
        WHERE condition_id = ANY($1)
          AND expires_at > $2
        ORDER BY condition_id, expires_at DESC, created_at DESC
        "#,
    )
    .bind(condition_ids)
    .bind(now)
    .fetch_all(pool)
    .await
    .map_err(|error| {
        db_error(
            "POSTGRES_QUERY_FAILED",
            format!("failed to query reward market info risks: {error}"),
        )
    })?;

    rows.iter().map(reward_market_info_risk_from_row).collect()
}

async fn postgres_save_market_info_risk(
    pool: &PgPool,
    risk: &RewardMarketInfoRisk,
) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO reward_market_info_risks (
          condition_id,
          provider,
          request_format,
          model,
          query_hash,
          input_hash,
          action,
          risk_level,
          risk_type,
          directional_risk,
          resolution_imminent,
          expected_event_at,
          confidence,
          summary,
          sources_json,
          metrics_json,
          created_at,
          expires_at
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18)
        "#,
    )
    .bind(&risk.condition_id)
    .bind(risk.provider.as_str())
    .bind(risk.request_format.as_str())
    .bind(&risk.model)
    .bind(&risk.query_hash)
    .bind(&risk.input_hash)
    .bind(risk.action.as_str())
    .bind(risk.risk_level.as_str())
    .bind(risk.risk_type.as_str())
    .bind(risk.directional_risk.as_str())
    .bind(risk.resolution_imminent)
    .bind(risk.expected_event_at)
    .bind(risk.confidence)
    .bind(&risk.summary)
    .bind(Json(json!(risk.sources)))
    .bind(Json(risk.metrics.clone()))
    .bind(risk.created_at)
    .bind(risk.expires_at)
    .execute(pool)
    .await
    .map_err(|error| {
        db_error(
            "POSTGRES_INSERT_FAILED",
            format!("failed to insert reward market info risk: {error}"),
        )
    })?;
    Ok(())
}
