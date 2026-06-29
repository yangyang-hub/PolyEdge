async fn postgres_list_reward_orders_page(
    pool: &PgPool,
    query: &RewardOrderListQuery,
) -> Result<RewardOrderPage> {
    let account_id = query.account_id.as_str();
    let search = query.search.as_deref();
    let status = query.status.map(RewardOrderStatusFilter::as_str);
    let total_items = postgres_count_reward_orders(pool, account_id, search, status).await?;
    let page = query.page_for_total(total_items);
    let offset = (page.page - 1) * page.page_size;

    let sql = format!(
        r#"
        SELECT id,
               account_id,
               condition_id,
               token_id,
               outcome,
               side,
               price,
               size,
               strategy_bucket,
               strategy_profile,
               external_order_id,
               status,
               scoring,
               reason,
               filled_size,
               reward_earned,
               last_scored_at,
               created_at,
               updated_at
        FROM reward_managed_orders
        WHERE account_id = $1
          AND ($2::text IS NULL
               OR outcome ILIKE '%' || $2 || '%'
               OR condition_id ILIKE '%' || $2 || '%'
               OR token_id ILIKE '%' || $2 || '%')
          AND ($3::text IS NULL
               OR ($3 = 'open' AND status IN ('planned', 'open'))
               OR ($3 = 'filled' AND (status = 'filled' OR filled_size > 0))
               OR ($3 = 'cancelled' AND status = 'cancelled')
               OR ($3 = 'exit_pending' AND status = 'exit_pending'))
        ORDER BY {}
        LIMIT $4
        OFFSET $5
        "#,
        reward_order_order_by(query),
    );

    let rows = sqlx::query(&sql)
        .bind(account_id)
        .bind(search)
        .bind(status)
        .bind(page.page_size as i64)
        .bind(offset as i64)
        .fetch_all(pool)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_QUERY_FAILED",
                format!("failed to query paged reward managed orders: {error}"),
            )
        })?;

    let items = rows
        .iter()
        .map(reward_order_from_row)
        .collect::<Result<Vec<_>>>()?;

    Ok(RewardOrderPage { items, page })
}

async fn postgres_count_reward_orders(
    pool: &PgPool,
    account_id: &str,
    search: Option<&str>,
    status: Option<&str>,
) -> Result<usize> {
    let total: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM reward_managed_orders
        WHERE account_id = $1
          AND ($2::text IS NULL
               OR outcome ILIKE '%' || $2 || '%'
               OR condition_id ILIKE '%' || $2 || '%'
               OR token_id ILIKE '%' || $2 || '%')
          AND ($3::text IS NULL
               OR ($3 = 'open' AND status IN ('planned', 'open'))
               OR ($3 = 'filled' AND (status = 'filled' OR filled_size > 0))
               OR ($3 = 'cancelled' AND status = 'cancelled')
               OR ($3 = 'exit_pending' AND status = 'exit_pending'))
        "#,
    )
    .bind(account_id)
    .bind(search)
    .bind(status)
    .fetch_one(pool)
    .await
    .map_err(|error| {
        db_error(
            "POSTGRES_QUERY_FAILED",
            format!("failed to count reward managed orders: {error}"),
        )
    })?;

    Ok(total.max(0) as usize)
}

fn reward_order_order_by(query: &RewardOrderListQuery) -> &'static str {
    match (query.sort_by, query.sort_order) {
        (RewardOrderSortField::Price, SortOrder::Asc) => {
            "CASE WHEN status IN ('planned', 'open', 'exit_pending') THEN 0 ELSE 1 END, price ASC, updated_at DESC"
        }
        (RewardOrderSortField::Price, SortOrder::Desc) => {
            "CASE WHEN status IN ('planned', 'open', 'exit_pending') THEN 0 ELSE 1 END, price DESC, updated_at DESC"
        }
        (RewardOrderSortField::Size, SortOrder::Asc) => {
            "CASE WHEN status IN ('planned', 'open', 'exit_pending') THEN 0 ELSE 1 END, size ASC, updated_at DESC"
        }
        (RewardOrderSortField::Size, SortOrder::Desc) => {
            "CASE WHEN status IN ('planned', 'open', 'exit_pending') THEN 0 ELSE 1 END, size DESC, updated_at DESC"
        }
        (RewardOrderSortField::Status, SortOrder::Asc) => {
            "CASE WHEN status IN ('planned', 'open', 'exit_pending') THEN 0 ELSE 1 END, status ASC, updated_at DESC"
        }
        (RewardOrderSortField::Status, SortOrder::Desc) => {
            "CASE WHEN status IN ('planned', 'open', 'exit_pending') THEN 0 ELSE 1 END, status DESC, updated_at DESC"
        }
    }
}
