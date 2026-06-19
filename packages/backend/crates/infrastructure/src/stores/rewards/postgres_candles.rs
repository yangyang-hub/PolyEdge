async fn postgres_record_reward_market_candle_sample(
    pool: &PgPool,
    sample: &RewardMarketCandleSample,
) -> Result<()> {
    sqlx::query(
        r#"
        WITH matching_market AS (
            SELECT rm.condition_id,
                   token.value->>'outcome' AS outcome
            FROM reward_markets rm
            CROSS JOIN LATERAL jsonb_array_elements(rm.tokens_json) AS token(value)
            WHERE rm.active = true
              AND token.value->>'token_id' = $1
            ORDER BY rm.updated_at DESC
            LIMIT 1
        )
        INSERT INTO reward_market_candles (
            token_id,
            condition_id,
            outcome,
            interval_sec,
            bucket_start,
            open,
            high,
            low,
            close,
            best_bid_close,
            best_ask_close,
            spread_cents_close,
            sample_count,
            close_observed_at,
            updated_at
        )
        SELECT $1,
               condition_id,
               outcome,
               $2,
               $3,
               $4,
               $4,
               $4,
               $4,
               $5,
               $6,
               $7,
               1,
               $8,
               now()
        FROM matching_market
        ON CONFLICT (token_id, interval_sec, bucket_start) DO UPDATE
        SET high = GREATEST(reward_market_candles.high, EXCLUDED.close),
            low = LEAST(reward_market_candles.low, EXCLUDED.close),
            close = EXCLUDED.close,
            best_bid_close = EXCLUDED.best_bid_close,
            best_ask_close = EXCLUDED.best_ask_close,
            spread_cents_close = EXCLUDED.spread_cents_close,
            sample_count = reward_market_candles.sample_count + 1,
            close_observed_at = EXCLUDED.close_observed_at,
            updated_at = now()
        WHERE EXCLUDED.close_observed_at >= reward_market_candles.close_observed_at
        "#,
    )
    .bind(&sample.token_id)
    .bind(sample.interval_sec)
    .bind(sample.bucket_start)
    .bind(sample.midpoint)
    .bind(sample.best_bid)
    .bind(sample.best_ask)
    .bind(sample.spread_cents)
    .bind(sample.observed_at)
    .execute(pool)
    .await
    .map_err(|error| {
        db_error(
            "POSTGRES_UPSERT_FAILED",
            format!("failed to upsert reward market candle: {error}"),
        )
    })?;
    Ok(())
}

async fn postgres_list_recent_reward_market_candles(
    pool: &PgPool,
    condition_id: &str,
    interval_sec: i32,
    limit_per_token: u16,
) -> Result<Vec<RewardMarketCandle>> {
    let rows = sqlx::query(
        r#"
        WITH ranked AS (
            SELECT token_id,
                   condition_id,
                   outcome,
                   interval_sec,
                   bucket_start,
                   open,
                   high,
                   low,
                   close,
                   best_bid_close,
                   best_ask_close,
                   spread_cents_close,
                   sample_count,
                   close_observed_at,
                   updated_at,
                   row_number() OVER (
                       PARTITION BY token_id
                       ORDER BY bucket_start DESC
                   ) AS row_rank
            FROM reward_market_candles
            WHERE condition_id = $1
              AND interval_sec = $2
        )
        SELECT token_id,
               condition_id,
               outcome,
               interval_sec,
               bucket_start,
               open,
               high,
               low,
               close,
               best_bid_close,
               best_ask_close,
               spread_cents_close,
               sample_count,
               close_observed_at,
               updated_at
        FROM ranked
        WHERE row_rank <= $3
        ORDER BY token_id ASC, bucket_start ASC
        "#,
    )
    .bind(condition_id)
    .bind(interval_sec)
    .bind(i64::from(limit_per_token.max(1)))
    .fetch_all(pool)
    .await
    .map_err(|error| {
        db_error(
            "POSTGRES_QUERY_FAILED",
            format!("failed to query reward market candles: {error}"),
        )
    })?;

    rows.into_iter()
        .map(reward_market_candle_from_row)
        .collect()
}

fn reward_market_candle_from_row(row: sqlx::postgres::PgRow) -> Result<RewardMarketCandle> {
    Ok(RewardMarketCandle {
        token_id: row.try_get("token_id").map_err(postgres_decode_error)?,
        condition_id: row.try_get("condition_id").map_err(postgres_decode_error)?,
        outcome: row.try_get("outcome").map_err(postgres_decode_error)?,
        interval_sec: row.try_get("interval_sec").map_err(postgres_decode_error)?,
        bucket_start: row.try_get("bucket_start").map_err(postgres_decode_error)?,
        open: row.try_get("open").map_err(postgres_decode_error)?,
        high: row.try_get("high").map_err(postgres_decode_error)?,
        low: row.try_get("low").map_err(postgres_decode_error)?,
        close: row.try_get("close").map_err(postgres_decode_error)?,
        best_bid_close: row
            .try_get("best_bid_close")
            .map_err(postgres_decode_error)?,
        best_ask_close: row
            .try_get("best_ask_close")
            .map_err(postgres_decode_error)?,
        spread_cents_close: row
            .try_get("spread_cents_close")
            .map_err(postgres_decode_error)?,
        sample_count: row.try_get("sample_count").map_err(postgres_decode_error)?,
        close_observed_at: row
            .try_get("close_observed_at")
            .map_err(postgres_decode_error)?,
        updated_at: row.try_get("updated_at").map_err(postgres_decode_error)?,
    })
}
