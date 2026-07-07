async fn postgres_latest_market_maker_fair_values(
    pool: &PgPool,
    condition_ids: &[String],
    model_version: &str,
    now: OffsetDateTime,
) -> Result<Vec<RewardMarketMakerFairValue>> {
    if condition_ids.is_empty() {
        return Ok(Vec::new());
    }

    let rows = sqlx::query(
        r#"
        SELECT DISTINCT ON (condition_id)
               id,
               condition_id,
               token_id,
               fair_yes_low,
               fair_yes_mid,
               fair_yes_high,
               market_implied,
               base_rate,
               confidence,
               uncertainty_cents,
               sample_count,
               bucket_key,
               fallback_level,
               model_version,
               input_hash,
               reason_codes,
               live_eligible,
               computed_at,
               expires_at
        FROM reward_market_fair_values
        WHERE condition_id = ANY($1)
          AND model_version = $2
          AND expires_at > $3
        ORDER BY condition_id, live_eligible DESC, computed_at DESC
        "#,
    )
    .bind(condition_ids)
    .bind(model_version)
    .bind(now)
    .fetch_all(pool)
    .await
    .map_err(|error| {
        db_error(
            "POSTGRES_QUERY_FAILED",
            format!("failed to query reward market maker fair values: {error}"),
        )
    })?;

    rows.iter().map(reward_market_maker_fair_value_from_row).collect()
}

async fn postgres_record_market_maker_decisions(
    pool: &PgPool,
    decisions: &[RewardMarketMakerDecision],
) -> Result<()> {
    if decisions.is_empty() {
        return Ok(());
    }

    for chunk in decisions.chunks(REWARD_UPSERT_BATCH_SIZE) {
        let cols = 26usize;
        let placeholders = chunk
            .iter()
            .enumerate()
            .map(|(i, _)| {
                let base = i * cols;
                let params = (1..=cols)
                    .map(|j| format!("${}", base + j))
                    .collect::<Vec<_>>();
                format!("({})", params.join(", "))
            })
            .collect::<Vec<_>>()
            .join(", ");

        let sql = format!(
            r#"
            INSERT INTO reward_market_maker_decisions (
                id,
                run_id,
                account_id,
                condition_id,
                token_id,
                outcome,
                side,
                strategy_mode,
                decision_type,
                decision_status,
                target_price,
                target_size,
                target_notional_usd,
                fair_value_id,
                reward_ev_id,
                pricing_edge_cents,
                reward_ev_cents,
                exit_cost_cents,
                adverse_selection_cost_cents,
                inventory_penalty_cents,
                uncertainty_buffer_cents,
                total_ev_cents,
                max_profitable_bid,
                reason_codes,
                inputs_hash,
                created_at
            )
            VALUES {placeholders}
            ON CONFLICT (id) DO UPDATE
            SET decision_type = EXCLUDED.decision_type,
                decision_status = EXCLUDED.decision_status,
                target_price = EXCLUDED.target_price,
                target_size = EXCLUDED.target_size,
                target_notional_usd = EXCLUDED.target_notional_usd,
                fair_value_id = EXCLUDED.fair_value_id,
                reward_ev_id = EXCLUDED.reward_ev_id,
                pricing_edge_cents = EXCLUDED.pricing_edge_cents,
                reward_ev_cents = EXCLUDED.reward_ev_cents,
                exit_cost_cents = EXCLUDED.exit_cost_cents,
                adverse_selection_cost_cents = EXCLUDED.adverse_selection_cost_cents,
                inventory_penalty_cents = EXCLUDED.inventory_penalty_cents,
                uncertainty_buffer_cents = EXCLUDED.uncertainty_buffer_cents,
                total_ev_cents = EXCLUDED.total_ev_cents,
                max_profitable_bid = EXCLUDED.max_profitable_bid,
                reason_codes = EXCLUDED.reason_codes,
                inputs_hash = EXCLUDED.inputs_hash,
                created_at = EXCLUDED.created_at
            "#,
        );

        let mut query = sqlx::query(&sql);
        for decision in chunk {
            query = query
                .bind(&decision.id)
                .bind(&decision.run_id)
                .bind(&decision.account_id)
                .bind(&decision.condition_id)
                .bind(&decision.token_id)
                .bind(&decision.outcome)
                .bind(decision.side.as_str())
                .bind(decision.strategy_mode.as_str())
                .bind(decision.decision_type.as_str())
                .bind(decision.decision_status.as_str())
                .bind(decision.target_price)
                .bind(decision.target_size)
                .bind(decision.target_notional_usd)
                .bind(decision.fair_value_id)
                .bind(decision.reward_ev_id)
                .bind(decision.pricing_edge_cents)
                .bind(decision.reward_ev_cents)
                .bind(decision.exit_cost_cents)
                .bind(decision.adverse_selection_cost_cents)
                .bind(decision.inventory_penalty_cents)
                .bind(decision.uncertainty_buffer_cents)
                .bind(decision.total_ev_cents)
                .bind(decision.max_profitable_bid)
                .bind(Json(decision.reason_codes.clone()))
                .bind(&decision.inputs_hash)
                .bind(decision.created_at);
        }

        query.execute(pool).await.map_err(|error| {
            db_error(
                "POSTGRES_UPSERT_FAILED",
                format!(
                    "failed to upsert reward market maker decisions (chunk size {}): {error}",
                    chunk.len()
                ),
            )
        })?;
    }

    Ok(())
}

fn reward_market_maker_fair_value_from_row(
    row: &sqlx::postgres::PgRow,
) -> Result<RewardMarketMakerFairValue> {
    let reason_codes: Json<Vec<String>> = row.try_get("reason_codes").map_err(postgres_decode_error)?;
    let fallback_level_raw: i16 = row
        .try_get("fallback_level")
        .map_err(postgres_decode_error)?;
    Ok(RewardMarketMakerFairValue {
        id: row.try_get("id").map_err(postgres_decode_error)?,
        condition_id: row.try_get("condition_id").map_err(postgres_decode_error)?,
        token_id: row.try_get("token_id").map_err(postgres_decode_error)?,
        fair_yes_low: row.try_get("fair_yes_low").map_err(postgres_decode_error)?,
        fair_yes_mid: row.try_get("fair_yes_mid").map_err(postgres_decode_error)?,
        fair_yes_high: row.try_get("fair_yes_high").map_err(postgres_decode_error)?,
        market_implied: row.try_get("market_implied").map_err(postgres_decode_error)?,
        base_rate: row.try_get("base_rate").map_err(postgres_decode_error)?,
        confidence: row.try_get("confidence").map_err(postgres_decode_error)?,
        uncertainty_cents: row
            .try_get("uncertainty_cents")
            .map_err(postgres_decode_error)?,
        sample_count: i64_count_to_u64(row.try_get("sample_count").map_err(postgres_decode_error)?),
        bucket_key: row.try_get("bucket_key").map_err(postgres_decode_error)?,
        fallback_level: u8::try_from(fallback_level_raw).unwrap_or(0),
        model_version: row.try_get("model_version").map_err(postgres_decode_error)?,
        input_hash: row.try_get("input_hash").map_err(postgres_decode_error)?,
        reason_codes: reason_codes.0,
        live_eligible: row.try_get("live_eligible").map_err(postgres_decode_error)?,
        computed_at: row.try_get("computed_at").map_err(postgres_decode_error)?,
        expires_at: row.try_get("expires_at").map_err(postgres_decode_error)?,
    })
}
