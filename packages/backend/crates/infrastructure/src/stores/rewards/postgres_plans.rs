// Quote-plan server-side pagination: COUNT + filtered/sorted/paged SELECT.

async fn postgres_count_quote_plans(pool: &PgPool) -> Result<RewardQuotePlanCounts> {
    let row = sqlx::query(
        r#"
        SELECT COUNT(*) AS total,
               COUNT(*) FILTER (WHERE eligible) AS eligible,
               COUNT(*) FILTER (WHERE quote_readiness = 'ready_to_quote') AS ready_to_quote,
               COUNT(*) FILTER (WHERE quote_readiness = 'waiting_orderbook') AS waiting_orderbook,
               COUNT(*) FILTER (WHERE quote_readiness = 'provider_pending') AS provider_pending,
               COUNT(*) FILTER (WHERE NOT eligible
                                AND reason_code = 'ai_pending') AS blocker_ai_pending,
               COUNT(*) FILTER (WHERE NOT eligible
                                AND reason_code = 'info_risk_pending') AS blocker_info_risk_pending,
               COUNT(*) FILTER (WHERE NOT eligible
                                AND reason_code = 'ai_stop_new') AS blocker_ai_stop_new,
               COUNT(*) FILTER (WHERE NOT eligible
                                AND reason_code = 'provider_size') AS blocker_provider_size,
               COUNT(*) FILTER (WHERE NOT eligible
                                AND reason_code = 'info_risk') AS blocker_info_risk,
               COUNT(*) FILTER (WHERE NOT eligible
                                AND reason_code = 'event_window') AS blocker_event_window,
               COUNT(*) FILTER (WHERE NOT eligible
                                AND reason_code = 'fair_value') AS blocker_fair_value,
               COUNT(*) FILTER (WHERE NOT eligible
                                AND reason_code = 'competition') AS blocker_competition,
               COUNT(*) FILTER (WHERE NOT eligible
                                AND reason_code = 'funding') AS blocker_funding,
               COUNT(*) FILTER (WHERE NOT eligible
                                AND reason_code = 'maker_budget') AS blocker_maker_budget,
               COUNT(*) FILTER (WHERE NOT eligible
                                AND reason_code = 'inventory_headroom') AS blocker_inventory_headroom,
               COUNT(*) FILTER (WHERE NOT eligible
                                AND reason_code = 'live_validation') AS blocker_live_validation,
               COUNT(*) FILTER (WHERE NOT eligible
                                AND reason_code NOT IN (
                                    'ai_pending',
                                    'info_risk_pending',
                                    'ai_stop_new',
                                    'provider_size',
                                    'info_risk',
                                    'event_window',
                                    'fair_value',
                                    'competition',
                                    'funding',
                                    'maker_budget',
                                    'inventory_headroom',
                                    'live_validation'
                                )) AS blocker_other
        FROM reward_quote_plans
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
            ai_stop_new: postgres_count_to_usize(&row, "blocker_ai_stop_new")?,
            provider_size: postgres_count_to_usize(&row, "blocker_provider_size")?,
            info_risk: postgres_count_to_usize(&row, "blocker_info_risk")?,
            event_window: postgres_count_to_usize(&row, "blocker_event_window")?,
            fair_value: postgres_count_to_usize(&row, "blocker_fair_value")?,
            competition: postgres_count_to_usize(&row, "blocker_competition")?,
            funding: postgres_count_to_usize(&row, "blocker_funding")?,
            maker_budget: postgres_count_to_usize(&row, "blocker_maker_budget")?,
            inventory_headroom: postgres_count_to_usize(&row, "blocker_inventory_headroom")?,
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
        RewardQuotePlanSortField::SelectionScore => "selection_score".to_string(),
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
