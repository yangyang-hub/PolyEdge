// Quote-plan server-side pagination: COUNT + filtered/sorted/paged SELECT.

async fn postgres_count_quote_plans(pool: &PgPool) -> Result<RewardQuotePlanCounts> {
    let row = sqlx::query(
        r#"
        WITH plan_flags AS (
            SELECT eligible,
                   reason,
                   CASE
                       WHEN reason LIKE 'waiting for fresh orderbook data%' THEN 'waiting_orderbook'
                       WHEN eligible
                            AND quote_mode <> 'none'
                            AND has_live_legs THEN 'ready_to_quote'
                       WHEN eligible
                            AND pre_ai_eligible
                            AND (reason LIKE 'AI advisory pending:%'
                                 OR reason LIKE 'info risk pending:%'
                                 OR (quote_plan_json->>'ai_advisory_pending_since') IS NOT NULL
                                 OR (quote_plan_json->>'info_risk_pending_since') IS NOT NULL)
                           THEN 'provider_pending'
                       WHEN eligible THEN 'waiting_orderbook'
                       WHEN pre_ai_eligible
                            AND (reason LIKE 'AI advisory pending:%'
                                 OR reason LIKE 'info risk pending:%') THEN 'provider_pending'
                       ELSE 'blocked'
                   END AS readiness
            FROM (
                SELECT eligible,
                       reason,
                       COALESCE(quote_plan_json->>'quote_mode', 'none') AS quote_mode,
                       COALESCE((quote_plan_json->>'pre_ai_eligible')::boolean, false) AS pre_ai_eligible,
                       quote_plan_json,
                       jsonb_array_length(COALESCE(quote_plan_json->'legs', '[]'::jsonb)) > 0
                       AND NOT EXISTS (
                           SELECT 1
                           FROM jsonb_array_elements(COALESCE(quote_plan_json->'legs', '[]'::jsonb)) AS leg
                           WHERE COALESCE(NULLIF(leg->>'price', '')::numeric, 0) <= 0
                              OR COALESCE(NULLIF(leg->>'size', '')::numeric, 0) <= 0
                              OR COALESCE(NULLIF(leg->>'notional_usd', '')::numeric, 0) <= 0
                       ) AS has_live_legs
                FROM reward_quote_plans
            ) plans
        )
        SELECT COUNT(*) AS total,
               COUNT(*) FILTER (WHERE eligible) AS eligible,
               COUNT(*) FILTER (WHERE readiness = 'ready_to_quote') AS ready_to_quote,
               COUNT(*) FILTER (WHERE readiness = 'waiting_orderbook') AS waiting_orderbook,
               COUNT(*) FILTER (WHERE readiness = 'provider_pending') AS provider_pending,
               COUNT(*) FILTER (WHERE NOT eligible
                                AND reason LIKE 'AI advisory pending:%') AS blocker_ai_pending,
               COUNT(*) FILTER (WHERE NOT eligible
                                AND reason LIKE 'info risk pending:%') AS blocker_info_risk_pending,
               COUNT(*) FILTER (WHERE NOT eligible
                                AND reason LIKE 'AI advisory confidence%') AS blocker_ai_confidence_low,
               COUNT(*) FILTER (WHERE NOT eligible
                                AND reason LIKE 'AI advisory watch:%') AS blocker_ai_watch,
               COUNT(*) FILTER (WHERE NOT eligible
                                AND reason LIKE 'AI advisory avoid:%') AS blocker_ai_avoid,
               COUNT(*) FILTER (WHERE NOT eligible
                                AND reason LIKE 'info risk %'
                                AND reason NOT LIKE 'info risk pending:%') AS blocker_info_risk,
               0::BIGINT AS blocker_low_competition,
               COUNT(*) FILTER (WHERE NOT eligible
                                AND reason LIKE 'live funding below rewards minimum:%') AS blocker_funding,
               COUNT(*) FILTER (WHERE NOT eligible
                                AND reason LIKE 'live orderbook validation skipped until %') AS blocker_live_validation,
               COUNT(*) FILTER (
                   WHERE NOT eligible
                     AND reason NOT LIKE 'AI advisory pending:%'
                     AND reason NOT LIKE 'info risk pending:%'
                     AND reason NOT LIKE 'AI advisory confidence%'
                     AND reason NOT LIKE 'AI advisory watch:%'
                     AND reason NOT LIKE 'AI advisory avoid:%'
                     AND reason NOT LIKE 'info risk %'
                     AND reason NOT LIKE 'live funding below rewards minimum:%'
                     AND reason NOT LIKE 'live orderbook validation skipped until %'
               ) AS blocker_other
        FROM plan_flags
        "#,
    )
    .fetch_one(pool)
    .await
    .map_err(|error| {
        db_error(
            "POSTGRES_QUERY_FAILED",
            format!("failed to count reward quote plans: {error}"),
        )
    })?;

    Ok(RewardQuotePlanCounts {
        total: postgres_count_to_usize(&row, "total")?,
        eligible: postgres_count_to_usize(&row, "eligible")?,
        ready_to_quote: postgres_count_to_usize(&row, "ready_to_quote")?,
        waiting_orderbook: postgres_count_to_usize(&row, "waiting_orderbook")?,
        provider_pending: postgres_count_to_usize(&row, "provider_pending")?,
        blockers: RewardQuotePlanBlockerCounts {
            waiting_orderbook: postgres_count_to_usize(&row, "waiting_orderbook")?,
            ai_pending: postgres_count_to_usize(&row, "blocker_ai_pending")?,
            info_risk_pending: postgres_count_to_usize(&row, "blocker_info_risk_pending")?,
            ai_confidence_low: postgres_count_to_usize(&row, "blocker_ai_confidence_low")?,
            ai_watch: postgres_count_to_usize(&row, "blocker_ai_watch")?,
            ai_avoid: postgres_count_to_usize(&row, "blocker_ai_avoid")?,
            info_risk: postgres_count_to_usize(&row, "blocker_info_risk")?,
            low_competition: postgres_count_to_usize(&row, "blocker_low_competition")?,
            funding: postgres_count_to_usize(&row, "blocker_funding")?,
            live_validation: postgres_count_to_usize(&row, "blocker_live_validation")?,
            other: postgres_count_to_usize(&row, "blocker_other")?,
        },
    })
}

fn postgres_count_to_usize(row: &sqlx::postgres::PgRow, column: &str) -> Result<usize> {
    let count = row
        .try_get::<i64, _>(column)
        .map_err(postgres_decode_error)?;
    Ok(count.max(0) as usize)
}

async fn postgres_latest_quote_plan_updated_at(pool: &PgPool) -> Result<Option<OffsetDateTime>> {
    let row: Option<OffsetDateTime> =
        sqlx::query_scalar("SELECT MAX(updated_at) FROM reward_quote_plans")
            .fetch_one(pool)
            .await
            .map_err(|error| {
                db_error(
                    "POSTGRES_QUERY_FAILED",
                    format!("failed to query latest quote plan updated_at: {error}"),
                )
            })?;
    Ok(row)
}

async fn postgres_list_quote_plans_page(
    pool: &PgPool,
    query: &RewardQuotePlanListQuery,
) -> Result<RewardQuotePlanPage> {
    let search = query.search.as_deref();
    let eligible = query.eligible;
    let total_items = postgres_count_quote_plans_filtered(pool, search, eligible).await?;
    let page = query.page_for_total(total_items);
    let offset = (page.page - 1) * page.page_size;

    let sql = format!(
        r#"
        SELECT quote_plan_json
        FROM reward_quote_plans
        WHERE ($1::text IS NULL
               OR quote_plan_json::text ILIKE '%' || $1 || '%')
          AND ($2::boolean IS NULL
               OR eligible = $2)
        ORDER BY {}
        LIMIT $3
        OFFSET $4
        "#,
        quote_plan_order_by(query),
    );

    let rows = sqlx::query(&sql)
        .bind(search)
        .bind(eligible)
        .bind(page.page_size as i64)
        .bind(offset as i64)
        .fetch_all(pool)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_QUERY_FAILED",
                format!("failed to query paged reward quote plans: {error}"),
            )
        })?;

    let items = rows
        .iter()
        .map(|row| {
            let mut plan: RewardQuotePlan = row
                .try_get::<Json<RewardQuotePlan>, _>("quote_plan_json")
                .map_err(postgres_decode_error)?
                .0;
            refresh_reward_quote_plan_readiness(&mut plan);
            Ok(plan)
        })
        .collect::<Result<Vec<_>>>()?;

    Ok(RewardQuotePlanPage { items, page })
}

async fn postgres_count_quote_plans_filtered(
    pool: &PgPool,
    search: Option<&str>,
    eligible: Option<bool>,
) -> Result<usize> {
    let total: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM reward_quote_plans
        WHERE ($1::text IS NULL
               OR quote_plan_json::text ILIKE '%' || $1 || '%')
          AND ($2::boolean IS NULL
               OR eligible = $2)
        "#,
    )
    .bind(search)
    .bind(eligible)
    .fetch_one(pool)
    .await
    .map_err(|error| {
        db_error(
            "POSTGRES_QUERY_FAILED",
            format!("failed to count filtered reward quote plans: {error}"),
        )
    })?;

    Ok(total.max(0) as usize)
}

fn quote_plan_order_by(query: &RewardQuotePlanListQuery) -> String {
    let primary = match query.sort_by {
        RewardQuotePlanSortField::Score => "score".to_string(),
        RewardQuotePlanSortField::DailyReward => {
            "(quote_plan_json->>'total_daily_rate')::numeric".to_string()
        }
        RewardQuotePlanSortField::Midpoint => {
            "COALESCE((quote_plan_json->>'midpoint')::numeric, 0)".to_string()
        }
        RewardQuotePlanSortField::Eligible => "eligible".to_string(),
    };
    let dir = match query.sort_order {
        SortOrder::Asc => "ASC",
        SortOrder::Desc => "DESC",
    };
    format!("eligible DESC, {primary} {dir}, updated_at DESC")
}
