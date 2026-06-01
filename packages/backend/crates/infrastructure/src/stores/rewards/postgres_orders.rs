async fn postgres_list_reward_orders_page(
    pool: &PgPool,
    query: &RewardOrderListQuery,
) -> Result<RewardOrderPage> {
    let search = query.search.as_deref();
    let status = query.status.map(RewardOrderStatusFilter::as_str);
    let total_items = postgres_count_reward_orders(pool, search, status).await?;
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
        WHERE ($1::text IS NULL
               OR outcome ILIKE '%' || $1 || '%'
               OR condition_id ILIKE '%' || $1 || '%'
               OR token_id ILIKE '%' || $1 || '%')
          AND ($2::text IS NULL
               OR ($2 = 'open' AND status IN ('planned', 'open'))
               OR ($2 = 'filled' AND status = 'filled')
               OR ($2 = 'cancelled' AND status = 'cancelled')
               OR ($2 = 'exit_pending' AND status = 'exit_pending'))
        ORDER BY {}
        LIMIT $3
        OFFSET $4
        "#,
        reward_order_order_by(query),
    );

    let rows = sqlx::query(&sql)
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
    search: Option<&str>,
    status: Option<&str>,
) -> Result<usize> {
    let total: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM reward_managed_orders
        WHERE ($1::text IS NULL
               OR outcome ILIKE '%' || $1 || '%'
               OR condition_id ILIKE '%' || $1 || '%'
               OR token_id ILIKE '%' || $1 || '%')
          AND ($2::text IS NULL
               OR ($2 = 'open' AND status IN ('planned', 'open'))
               OR ($2 = 'filled' AND status = 'filled')
               OR ($2 = 'cancelled' AND status = 'cancelled')
               OR ($2 = 'exit_pending' AND status = 'exit_pending'))
        "#,
    )
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
