async fn postgres_list_reward_markets(
    store: &PostgresRewardBotStore,
    limit: u16,
) -> Result<Vec<RewardMarket>> {
    let rows = sqlx::query(
        r#"
            SELECT rm.condition_id,
                   rm.question,
                   rm.market_slug,
                   rm.event_slug,
                   rm.image,
                   m.category,
                   rm.rewards_max_spread,
                   rm.rewards_min_size,
                   rm.total_daily_rate,
                   rm.tokens_json,
                   rm.active,
                   rm.updated_at,
                   m.best_bid,
                   m.best_ask,
                   m.liquidity_usd,
                   m.volume_24h AS volume_24h_usd,
                   (m.best_ask - m.best_bid) * 100 AS market_spread_cents,
                   m.end_at,
                   m.ambiguity_level,
                   m.synced_at AS market_synced_at
            FROM reward_markets rm
            JOIN markets m
              ON m.polymarket_condition_id = rm.condition_id
            WHERE rm.active = true
              AND m.status = 'open'
              AND m.tradability_status = 'tradable'
            ORDER BY m.liquidity_usd DESC,
                     m.volume_24h DESC,
                     m.end_at DESC NULLS LAST,
                     rm.total_daily_rate DESC,
                     rm.updated_at DESC
            LIMIT $1
            "#,
    )
    .bind(i64::from(limit))
    .fetch_all(&store.pool)
    .await
    .map_err(|error| {
        db_error(
            "POSTGRES_QUERY_FAILED",
            format!("failed to query reward markets: {error}"),
        )
    })?;

    rows.iter().map(reward_market_from_row).collect()
}

async fn postgres_list_reward_candidate_markets(
    store: &PostgresRewardBotStore,
    filter: &RewardCandidateFilter,
    safety_limit: u16,
) -> Result<Vec<RewardMarket>> {
    let rows = sqlx::query(
        r#"
            SELECT rm.condition_id,
                   rm.question,
                   rm.market_slug,
                   rm.event_slug,
                   rm.image,
                   m.category,
                   rm.rewards_max_spread,
                   rm.rewards_min_size,
                   rm.total_daily_rate,
                   rm.tokens_json,
                   rm.active,
                   rm.updated_at,
                   m.best_bid,
                   m.best_ask,
                   m.liquidity_usd,
                   m.volume_24h AS volume_24h_usd,
                   (m.best_ask - m.best_bid) * 100 AS market_spread_cents,
                   m.end_at,
                   m.ambiguity_level,
                   m.synced_at AS market_synced_at
            FROM reward_markets rm
            JOIN markets m
              ON m.polymarket_condition_id = rm.condition_id
            WHERE rm.active = true
              AND m.status = 'open'
              AND m.tradability_status = 'tradable'
              AND m.ambiguity_level <> 'high'
              -- Binary rewards quoting requires exactly one YES and one NO token.
              -- Rust performs the outcome/id validation after row decoding.
              AND jsonb_array_length(rm.tokens_json) = 2
              -- Daily reward must meet threshold
              AND rm.total_daily_rate >= $1
              -- Spread must be positive (normalize treats <= 0 as invalid)
              AND rm.rewards_max_spread > 0
              -- Valid midpoint range from market best bid/ask
              AND m.best_bid > 0
              AND m.best_ask > 0
              AND m.best_bid <= m.best_ask
              AND (
                  ((m.best_bid + m.best_ask) / 2 >= $2
                   AND (m.best_bid + m.best_ask) / 2 <= $3)
                  OR (
                      $11
                      AND (
                          ((m.best_bid + m.best_ask) / 2 >= $12
                           AND (m.best_bid + m.best_ask) / 2 <= $13)
                          OR ((m.best_bid + m.best_ask) / 2 >= 1 - $13
                              AND (m.best_bid + m.best_ask) / 2 <= 1 - $12)
                      )
                  )
              )
              AND m.liquidity_usd >= $5
              AND m.volume_24h >= $6
              AND m.end_at IS NOT NULL
              AND m.end_at >= now() + ($7::BIGINT * interval '1 hour')
              AND (m.best_ask - m.best_bid) * 100 <= $8
              AND m.synced_at >= now() - ($9::BIGINT * interval '1 minute')
              AND m.synced_at <= now() + interval '5 minutes'
              -- Double YES/NO minimum-size legs require roughly
              -- rewards_min_size USD in aggregate. Auto/enforce single-side
              -- fallback needs exact orderbook prices, so Rust planner checks
              -- affordability after books are loaded.
              AND CASE
                  WHEN $4 <= 0 THEN true
                  WHEN $14 THEN true
                  ELSE rm.rewards_min_size <= $4
              END
            ORDER BY (
                       LEAST(35.0, SQRT(rm.total_daily_rate::DOUBLE PRECISION) * 10.0)
                       + LEAST(20.0, LN(1.0 + m.liquidity_usd::DOUBLE PRECISION) / LN(10.0) * 4.0)
                       + LEAST(15.0, LN(1.0 + m.volume_24h::DOUBLE PRECISION) / LN(10.0) * 3.0)
                       + LEAST(
                           10.0,
                           SQRT(EXTRACT(EPOCH FROM (m.end_at - now())) / 86400.0) * 2.0
                         )
                       + LEAST(
                           10.0,
                           LEAST(
                               rm.rewards_max_spread,
                               $10
                           )::DOUBLE PRECISION * 1.25
                         )
                     ) DESC,
                     rm.total_daily_rate DESC,
                     m.liquidity_usd DESC,
                     m.volume_24h DESC,
                     m.end_at DESC,
                     rm.updated_at DESC
            LIMIT $15
            "#,
    )
    .bind(filter.min_daily_reward)
    .bind(filter.min_midpoint)
    .bind(filter.max_midpoint)
    .bind(filter.per_market_usd)
    .bind(filter.min_market_liquidity_usd)
    .bind(filter.min_market_volume_24h_usd)
    .bind(i64::try_from(filter.min_hours_to_end).unwrap_or(i64::MAX))
    .bind(filter.max_market_spread_cents)
    .bind(i64::try_from(filter.max_market_data_age_minutes).unwrap_or(i64::MAX))
    .bind(filter.max_rewards_spread_cents)
    .bind(filter.allow_dominant_single_side)
    .bind(filter.dominant_min_probability)
    .bind(filter.dominant_max_probability)
    .bind(filter.allow_single_side_budget_fallback)
    .bind(i64::from(safety_limit))
    .fetch_all(&store.pool)
    .await
    .map_err(|error| {
        db_error(
            "POSTGRES_QUERY_FAILED",
            format!("failed to query candidate reward markets: {error}"),
        )
    })?;

    rows.iter().map(reward_market_from_row).collect()
}

async fn postgres_list_all_active_reward_markets(
    store: &PostgresRewardBotStore,
) -> Result<Vec<RewardMarket>> {
    let rows = sqlx::query(
        r#"
            SELECT condition_id,
                   question,
                   market_slug,
                   event_slug,
                   image,
                   ''::TEXT AS category,
                   rewards_max_spread,
                   rewards_min_size,
                   total_daily_rate,
                   tokens_json,
                   active,
                   updated_at,
                   NULL::NUMERIC AS best_bid,
                   NULL::NUMERIC AS best_ask,
                   0::NUMERIC AS liquidity_usd,
                   0::NUMERIC AS volume_24h_usd,
                   0::NUMERIC AS market_spread_cents,
                   NULL::TIMESTAMPTZ AS end_at,
                   'unknown'::TEXT AS ambiguity_level,
                   NULL::TIMESTAMPTZ AS market_synced_at
            FROM reward_markets
            WHERE active = true
            ORDER BY total_daily_rate DESC, updated_at DESC
            "#,
    )
    .fetch_all(&store.pool)
    .await
    .map_err(|error| {
        db_error(
            "POSTGRES_QUERY_FAILED",
            format!("failed to query all reward markets: {error}"),
        )
    })?;

    rows.iter().map(reward_market_from_row).collect()
}
