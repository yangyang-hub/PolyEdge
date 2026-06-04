// Quote-plan server-side pagination: COUNT + filtered/sorted/paged SELECT.

async fn postgres_count_quote_plans(pool: &PgPool) -> Result<(usize, usize)> {
    let row = sqlx::query(
        r#"
        SELECT COUNT(*) AS total,
               COUNT(*) FILTER (WHERE eligible = true) AS eligible
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

    let total: i64 = row.try_get("total").map_err(postgres_decode_error)?;
    let eligible: i64 = row.try_get("eligible").map_err(postgres_decode_error)?;
    Ok((total.max(0) as usize, eligible.max(0) as usize))
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
            let plan: Json<RewardQuotePlan> = row
                .try_get("quote_plan_json")
                .map_err(postgres_decode_error)?;
            Ok(plan.0)
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
