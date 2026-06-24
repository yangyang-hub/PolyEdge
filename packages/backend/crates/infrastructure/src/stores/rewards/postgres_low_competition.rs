async fn postgres_record_low_competition_observations(
    pool: &PgPool,
    observations: &[RewardLowCompetitionObservation],
) -> Result<()> {
    if observations.is_empty() {
        return Ok(());
    }

    let mut transaction = pool.begin().await.map_err(|error| {
        db_error(
            "POSTGRES_TRANSACTION_BEGIN_FAILED",
            format!("failed to begin low-competition observation transaction: {error}"),
        )
    })?;

    for observation in observations {
        sqlx::query(
            r#"
            INSERT INTO reward_low_competition_observations (
              id,
              account_id,
              condition_id,
              market_slug,
              question,
              observed_at,
              mode,
              planned_notional_usd,
              competition_probe_notional_usd,
              qualified_competition_usd,
              competition_share_bps,
              competition_multiple,
              estimated_reward_per_100_usd_day,
              competition_density,
              account_effective_available_usd,
              low_competition_open_buy_notional_usd,
              low_competition_open_buy_notional_usd_after_plan,
              condition_buy_notional_usd_after_plan,
              account_allocation_bps,
              market_allocation_bps,
              exit_depth_usd,
              exit_slippage_cents,
              midpoint_range_cents,
              top_of_book_flip_count,
              sample_count,
              sample_insufficient,
              eligible_for_low_competition,
              final_eligible,
              ai_blocked,
              info_risk_blocked,
              standard_plan_overlap,
              rejection_reasons,
              created_at
            )
            VALUES (
              $1, $2, $3, $4, $5, $6, $7, $8,
              $9, $10, $11, $12, $13, $14, $15, $16,
              $17, $18, $19, $20, $21, $22, $23, $24,
              $25, $26, $27, $28, $29, $30, $31, $32, $33
            )
            ON CONFLICT (id) DO NOTHING
            "#,
        )
        .bind(&observation.id)
        .bind(&observation.account_id)
        .bind(&observation.condition_id)
        .bind(&observation.market_slug)
        .bind(&observation.question)
        .bind(observation.observed_at)
        .bind(observation.mode.as_str())
        .bind(observation.planned_notional_usd)
        .bind(observation.competition_probe_notional_usd)
        .bind(observation.qualified_competition_usd)
        .bind(observation.competition_share_bps)
        .bind(observation.competition_multiple)
        .bind(observation.estimated_reward_per_100_usd_day)
        .bind(observation.competition_density)
        .bind(observation.account_effective_available_usd)
        .bind(observation.low_competition_open_buy_notional_usd)
        .bind(observation.low_competition_open_buy_notional_usd_after_plan)
        .bind(observation.condition_buy_notional_usd_after_plan)
        .bind(observation.account_allocation_bps)
        .bind(observation.market_allocation_bps)
        .bind(observation.exit_depth_usd)
        .bind(observation.exit_slippage_cents)
        .bind(observation.midpoint_range_cents)
        .bind(optional_u64_to_i64(observation.top_of_book_flip_count)?)
        .bind(u64_to_i64(observation.sample_count)?)
        .bind(observation.sample_insufficient)
        .bind(observation.eligible_for_low_competition)
        .bind(observation.final_eligible)
        .bind(observation.ai_blocked)
        .bind(observation.info_risk_blocked)
        .bind(observation.standard_plan_overlap)
        .bind(Json(observation.rejection_reasons.clone()))
        .bind(observation.created_at)
        .execute(&mut *transaction)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_INSERT_FAILED",
                format!("failed to insert low-competition observation: {error}"),
            )
        })?;
    }

    transaction.commit().await.map_err(|error| {
        db_error(
            "POSTGRES_TRANSACTION_COMMIT_FAILED",
            format!("failed to commit low-competition observation transaction: {error}"),
        )
    })?;
    Ok(())
}

async fn postgres_list_low_competition_observations(
    pool: &PgPool,
    account_id: &str,
    since: OffsetDateTime,
    limit: u16,
) -> Result<Vec<RewardLowCompetitionObservation>> {
    let rows = sqlx::query(
        r#"
        SELECT id,
               account_id,
               condition_id,
               market_slug,
               question,
               observed_at,
               mode,
               planned_notional_usd,
               competition_probe_notional_usd,
               qualified_competition_usd,
               competition_share_bps,
               competition_multiple,
               estimated_reward_per_100_usd_day,
               competition_density,
               account_effective_available_usd,
               low_competition_open_buy_notional_usd,
               low_competition_open_buy_notional_usd_after_plan,
               condition_buy_notional_usd_after_plan,
               account_allocation_bps,
               market_allocation_bps,
               exit_depth_usd,
               exit_slippage_cents,
               midpoint_range_cents,
               top_of_book_flip_count,
               sample_count,
               sample_insufficient,
               eligible_for_low_competition,
               final_eligible,
               ai_blocked,
               info_risk_blocked,
               standard_plan_overlap,
               rejection_reasons,
               created_at
        FROM reward_low_competition_observations
        WHERE account_id = $1
          AND observed_at >= $2
        ORDER BY observed_at DESC
        LIMIT $3
        "#,
    )
    .bind(account_id)
    .bind(since)
    .bind(i64::from(limit))
    .fetch_all(pool)
    .await
    .map_err(|error| {
        db_error(
            "POSTGRES_QUERY_FAILED",
            format!("failed to query low-competition observations: {error}"),
        )
    })?;

    rows.iter().map(low_competition_observation_from_row).collect()
}

fn low_competition_observation_from_row(
    row: &sqlx::postgres::PgRow,
) -> Result<RewardLowCompetitionObservation> {
    let mode: String = row.try_get("mode").map_err(postgres_decode_error)?;
    let top_of_book_flip_count: Option<i64> = row
        .try_get("top_of_book_flip_count")
        .map_err(postgres_decode_error)?;
    let sample_count: i64 = row.try_get("sample_count").map_err(postgres_decode_error)?;
    let rejection_reasons: Json<Vec<String>> = row
        .try_get("rejection_reasons")
        .map_err(postgres_decode_error)?;

    Ok(RewardLowCompetitionObservation {
        id: row.try_get("id").map_err(postgres_decode_error)?,
        account_id: row.try_get("account_id").map_err(postgres_decode_error)?,
        condition_id: row.try_get("condition_id").map_err(postgres_decode_error)?,
        market_slug: row.try_get("market_slug").map_err(postgres_decode_error)?,
        question: row.try_get("question").map_err(postgres_decode_error)?,
        observed_at: row.try_get("observed_at").map_err(postgres_decode_error)?,
        mode: RewardLowCompetitionMode::from_str(&mode)?,
        planned_notional_usd: row
            .try_get("planned_notional_usd")
            .map_err(postgres_decode_error)?,
        competition_probe_notional_usd: row
            .try_get("competition_probe_notional_usd")
            .map_err(postgres_decode_error)?,
        qualified_competition_usd: row
            .try_get("qualified_competition_usd")
            .map_err(postgres_decode_error)?,
        competition_share_bps: row
            .try_get("competition_share_bps")
            .map_err(postgres_decode_error)?,
        competition_multiple: row
            .try_get("competition_multiple")
            .map_err(postgres_decode_error)?,
        estimated_reward_per_100_usd_day: row
            .try_get("estimated_reward_per_100_usd_day")
            .map_err(postgres_decode_error)?,
        competition_density: row
            .try_get("competition_density")
            .map_err(postgres_decode_error)?,
        account_effective_available_usd: row
            .try_get("account_effective_available_usd")
            .map_err(postgres_decode_error)?,
        low_competition_open_buy_notional_usd: row
            .try_get("low_competition_open_buy_notional_usd")
            .map_err(postgres_decode_error)?,
        low_competition_open_buy_notional_usd_after_plan: row
            .try_get("low_competition_open_buy_notional_usd_after_plan")
            .map_err(postgres_decode_error)?,
        condition_buy_notional_usd_after_plan: row
            .try_get("condition_buy_notional_usd_after_plan")
            .map_err(postgres_decode_error)?,
        account_allocation_bps: row
            .try_get("account_allocation_bps")
            .map_err(postgres_decode_error)?,
        market_allocation_bps: row
            .try_get("market_allocation_bps")
            .map_err(postgres_decode_error)?,
        exit_depth_usd: row
            .try_get("exit_depth_usd")
            .map_err(postgres_decode_error)?,
        exit_slippage_cents: row
            .try_get("exit_slippage_cents")
            .map_err(postgres_decode_error)?,
        midpoint_range_cents: row
            .try_get("midpoint_range_cents")
            .map_err(postgres_decode_error)?,
        top_of_book_flip_count: optional_i64_to_u64(top_of_book_flip_count)?,
        sample_count: i64_to_u64(sample_count)?,
        sample_insufficient: row
            .try_get("sample_insufficient")
            .map_err(postgres_decode_error)?,
        eligible_for_low_competition: row
            .try_get("eligible_for_low_competition")
            .map_err(postgres_decode_error)?,
        final_eligible: row
            .try_get("final_eligible")
            .map_err(postgres_decode_error)?,
        ai_blocked: row.try_get("ai_blocked").map_err(postgres_decode_error)?,
        info_risk_blocked: row
            .try_get("info_risk_blocked")
            .map_err(postgres_decode_error)?,
        standard_plan_overlap: row
            .try_get("standard_plan_overlap")
            .map_err(postgres_decode_error)?,
        rejection_reasons: rejection_reasons.0,
        created_at: row.try_get("created_at").map_err(postgres_decode_error)?,
    })
}

fn optional_u64_to_i64(value: Option<u64>) -> Result<Option<i64>> {
    value.map(u64_to_i64).transpose()
}

fn u64_to_i64(value: u64) -> Result<i64> {
    i64::try_from(value).map_err(|error| {
        db_error(
            "POSTGRES_ENCODE_FAILED",
            format!("failed to encode unsigned count as bigint: {error}"),
        )
    })
}

fn optional_i64_to_u64(value: Option<i64>) -> Result<Option<u64>> {
    value.map(i64_to_u64).transpose()
}

fn i64_to_u64(value: i64) -> Result<u64> {
    u64::try_from(value).map_err(|error| {
        db_error(
            "POSTGRES_DECODE_FAILED",
            format!("failed to decode bigint as unsigned count: {error}"),
        )
    })
}
