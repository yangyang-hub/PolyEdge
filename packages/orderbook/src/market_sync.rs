use polyedge_application::{MarketUpsertOptions, MarketView, RewardMarket, RewardToken};
use polyedge_connectors::{
    PolymarketGammaConnector, PolymarketGammaMarket, PolymarketRewardMarket,
    PolymarketRewardsConnector,
};
use polyedge_domain::{AppError, Result};
use polyedge_infrastructure::AppState;
use rust_decimal::Decimal;
use sqlx::Row;

pub struct PriorityMarketSyncReport {
    pub condition_ids: usize,
    pub fetched: usize,
    pub upserted: usize,
}

static MARKET_UPSERT_GATE: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
const GENERAL_MARKET_SYNC_PAGE_SIZE: u16 = 100;

pub async fn sync_general_markets_once(
    state: &AppState,
    trace_id: &str,
    upsert_options: MarketUpsertOptions,
) -> Result<usize> {
    let connector = PolymarketGammaConnector::new(&state.settings.polymarket.gamma_host)?;
    let gamma_markets = connector
        .fetch_markets(GENERAL_MARKET_SYNC_PAGE_SIZE)
        .await?;
    let views: Vec<MarketView> = gamma_markets
        .into_iter()
        .map(gamma_market_to_view)
        .collect();
    let _guard = MARKET_UPSERT_GATE.lock().await;
    state
        .market_event_service
        .upsert_markets_with_options(&views, trace_id, upsert_options)
        .await
}

pub async fn sync_reward_markets_once(state: &AppState) -> Result<usize> {
    let rewards_connector = PolymarketRewardsConnector::new(&state.settings.polymarket.clob_host)?;
    let reward_markets_raw = rewards_connector.fetch_current_markets().await?;
    let reward_markets: Vec<RewardMarket> = reward_markets_raw
        .into_iter()
        .map(reward_market_from_connector)
        .collect();
    if reward_markets.is_empty() {
        return Err(AppError::dependency_unavailable(
            "POLYMARKET_REWARDS_MARKETS_EMPTY",
            "refusing to replace reward catalog with an empty snapshot",
        ));
    }
    let reward_upserted = reward_markets.len();
    state
        .reward_bot_service
        .upsert_reward_markets(&reward_markets)
        .await?;
    Ok(reward_upserted)
}

pub async fn sync_priority_markets_once(
    state: &AppState,
    trace_id: &str,
    max_condition_ids: usize,
    reward_candidate_stale_minutes: u64,
) -> Result<PriorityMarketSyncReport> {
    let condition_ids =
        collect_priority_condition_ids(state, max_condition_ids, reward_candidate_stale_minutes)
            .await;
    if condition_ids.is_empty() {
        return Ok(PriorityMarketSyncReport {
            condition_ids: 0,
            fetched: 0,
            upserted: 0,
        });
    }

    let connector = PolymarketGammaConnector::new(&state.settings.polymarket.gamma_host)?;
    let gamma_markets = connector
        .fetch_markets_by_condition_ids(&condition_ids)
        .await?;
    let fetched = gamma_markets.len();
    let views: Vec<MarketView> = gamma_markets
        .into_iter()
        .map(gamma_market_to_view)
        .collect();
    let _guard = MARKET_UPSERT_GATE.lock().await;
    let upserted = state
        .market_event_service
        .upsert_markets(&views, trace_id)
        .await?;

    Ok(PriorityMarketSyncReport {
        condition_ids: condition_ids.len(),
        fetched,
        upserted,
    })
}

async fn collect_priority_condition_ids(
    state: &AppState,
    max_condition_ids: usize,
    reward_candidate_stale_minutes: u64,
) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    let mut condition_ids = Vec::new();

    match registered_token_condition_ids(state, max_condition_ids.saturating_mul(2)).await {
        Ok(registered) => {
            for condition_id in registered {
                push_condition_id(
                    &mut condition_ids,
                    &mut seen,
                    condition_id,
                    max_condition_ids,
                );
            }
        }
        Err(error) => {
            tracing::warn!(error = %error, "failed to map registered orderbook tokens to markets");
        }
    }

    if condition_ids.len() < max_condition_ids {
        let remaining = max_condition_ids.saturating_sub(condition_ids.len());
        match state
            .reward_bot_service
            .list_priority_reward_condition_ids(reward_candidate_stale_minutes, remaining)
            .await
        {
            Ok(reward_condition_ids) => {
                for condition_id in reward_condition_ids {
                    push_condition_id(
                        &mut condition_ids,
                        &mut seen,
                        condition_id,
                        max_condition_ids,
                    );
                }
            }
            Err(error) => {
                tracing::warn!(error = %error, "failed to list priority rewards markets");
            }
        }
    }

    if condition_ids.len() < max_condition_ids {
        match active_reward_catalog_condition_ids(state, max_condition_ids).await {
            Ok(reward_condition_ids) => {
                for condition_id in reward_condition_ids {
                    push_condition_id(
                        &mut condition_ids,
                        &mut seen,
                        condition_id,
                        max_condition_ids,
                    );
                }
            }
            Err(error) => {
                tracing::warn!(
                    error = %error,
                    "failed to list active rewards catalog fallback markets"
                );
            }
        }
    }

    condition_ids
}

async fn registered_token_condition_ids(
    state: &AppState,
    max_token_ids: usize,
) -> Result<Vec<String>> {
    let token_ids = state
        .orderbook_registry
        .list_all_tokens()
        .await
        .into_iter()
        .take(max_token_ids)
        .collect::<Vec<_>>();
    if token_ids.is_empty() {
        return Ok(Vec::new());
    }

    let Some(pool) = state.dependencies.postgres.as_ref() else {
        return Ok(Vec::new());
    };

    let rows = sqlx::query(
        r#"
        SELECT token_id, condition_id
        FROM (
            SELECT polymarket_yes_asset_id AS token_id,
                   polymarket_condition_id AS condition_id
            FROM markets
            WHERE polymarket_yes_asset_id = ANY($1)
              AND polymarket_yes_asset_id IS NOT NULL
              AND polymarket_condition_id IS NOT NULL
            UNION ALL
            SELECT polymarket_no_asset_id AS token_id,
                   polymarket_condition_id AS condition_id
            FROM markets
            WHERE polymarket_no_asset_id = ANY($1)
              AND polymarket_no_asset_id IS NOT NULL
              AND polymarket_condition_id IS NOT NULL
        ) refs
        "#,
    )
    .bind(&token_ids)
    .fetch_all(pool)
    .await
    .map_err(|error| {
        AppError::dependency_unavailable(
            "POSTGRES_QUERY_FAILED",
            format!("failed to map orderbook token ids to Gamma condition ids: {error}"),
        )
    })?;

    let mut token_to_condition = std::collections::HashMap::new();
    for row in rows {
        let token_id: String = row.try_get("token_id").map_err(postgres_decode_error)?;
        let condition_id: String = row.try_get("condition_id").map_err(postgres_decode_error)?;
        token_to_condition.insert(token_id, condition_id);
    }

    let mut seen = std::collections::HashSet::new();
    let mut condition_ids = Vec::new();
    for token_id in token_ids {
        let Some(condition_id) = token_to_condition.get(&token_id) else {
            continue;
        };
        if seen.insert(condition_id.clone()) {
            condition_ids.push(condition_id.clone());
        }
    }

    Ok(condition_ids)
}

async fn active_reward_catalog_condition_ids(
    state: &AppState,
    max_condition_ids: usize,
) -> Result<Vec<String>> {
    if max_condition_ids == 0 {
        return Ok(Vec::new());
    }

    let Some(pool) = state.dependencies.postgres.as_ref() else {
        return Ok(Vec::new());
    };

    let rows = sqlx::query(
        r#"
        SELECT condition_id
        FROM reward_markets
        WHERE active = true
          AND rewards_max_spread > 0
          AND jsonb_array_length(tokens_json) = 2
        ORDER BY total_daily_rate DESC, updated_at DESC
        LIMIT $1
        "#,
    )
    .bind(i64::try_from(max_condition_ids).unwrap_or(i64::MAX))
    .fetch_all(pool)
    .await
    .map_err(|error| {
        AppError::dependency_unavailable(
            "POSTGRES_QUERY_FAILED",
            format!("failed to list active rewards catalog condition ids: {error}"),
        )
    })?;

    rows.into_iter()
        .map(|row| row.try_get("condition_id").map_err(postgres_decode_error))
        .collect()
}

fn push_condition_id(
    condition_ids: &mut Vec<String>,
    seen: &mut std::collections::HashSet<String>,
    condition_id: String,
    max_condition_ids: usize,
) {
    if condition_ids.len() >= max_condition_ids {
        return;
    }
    let condition_id = condition_id.trim();
    if condition_id.is_empty() || !seen.insert(condition_id.to_string()) {
        return;
    }
    condition_ids.push(condition_id.to_string());
}

fn postgres_decode_error(error: sqlx::Error) -> AppError {
    AppError::dependency_unavailable(
        "POSTGRES_DECODE_FAILED",
        format!("failed to decode Postgres row: {error}"),
    )
}

fn gamma_market_to_view(market: PolymarketGammaMarket) -> MarketView {
    MarketView {
        id: market.id,
        slug: market.slug,
        question: market.question,
        category: market.category,
        status: market.status,
        best_bid: market.best_bid,
        best_ask: market.best_ask,
        mid_price: market.mid_price,
        volume_24h: market.volume_24h,
        liquidity_usd: market.liquidity_usd,
        end_at: market.end_at,
        ambiguity_level: market.ambiguity_level,
        tradability_status: market.tradability_status,
        resolution_source: market.resolution_source,
        edge_case_notes: market.edge_case_notes,
        polymarket_condition_id: Some(market.condition_id),
        polymarket_yes_asset_id: Some(market.yes_asset_id),
        polymarket_no_asset_id: Some(market.no_asset_id),
        updated_at: market.updated_at,
        version: market.version,
    }
}

fn reward_market_from_connector(market: PolymarketRewardMarket) -> RewardMarket {
    RewardMarket {
        condition_id: market.condition_id,
        question: market.question,
        market_slug: market.market_slug,
        event_slug: market.event_slug,
        category: String::new(),
        image: market.image,
        rewards_max_spread: market.rewards_max_spread,
        rewards_min_size: market.rewards_min_size,
        total_daily_rate: market.total_daily_rate,
        liquidity_usd: Decimal::ZERO,
        volume_24h_usd: Decimal::ZERO,
        market_spread_cents: Decimal::ZERO,
        end_at: None,
        ambiguity_level: "unknown".to_string(),
        market_synced_at: None,
        tokens: market
            .tokens
            .into_iter()
            .map(|token| RewardToken {
                token_id: token.token_id,
                outcome: token.outcome,
                price: token.price,
            })
            .collect(),
        active: market.active,
        updated_at: market.updated_at,
    }
}
