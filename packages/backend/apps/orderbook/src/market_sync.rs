use polyedge_application::{MarketView, RewardMarket, RewardToken};
use polyedge_connectors::{PolymarketGammaConnector, PolymarketGammaMarket, PolymarketRewardMarket, PolymarketRewardsConnector};
use polyedge_domain::Result;
use polyedge_infrastructure::AppState;
use tracing::info;

pub struct MarketSyncReport {
    pub general_upserted: usize,
    pub reward_upserted: usize,
}

/// Sync general markets from Gamma API and reward markets from CLOB API
/// into the Postgres database.
pub async fn sync_markets_once(state: &AppState, trace_id: &str) -> Result<MarketSyncReport> {
    // 1. General markets from Gamma API.
    let connector =
        PolymarketGammaConnector::new(&state.settings.polymarket.gamma_host)?;
    let page_size = state.settings.arbitrage.scan_limit;
    let gamma_markets = connector.fetch_markets(page_size).await?;
    let views: Vec<MarketView> = gamma_markets
        .into_iter()
        .map(gamma_market_to_view)
        .collect();
    let general_upserted = state
        .market_event_service
        .upsert_markets(&views, trace_id)
        .await?;

    // 2. Reward markets from CLOB rewards API.
    let rewards_connector =
        PolymarketRewardsConnector::new(&state.settings.polymarket.clob_host)?;
    let reward_markets_raw = rewards_connector.fetch_current_markets().await?;
    let reward_markets: Vec<RewardMarket> = reward_markets_raw
        .into_iter()
        .map(reward_market_from_connector)
        .collect();
    let reward_upserted = reward_markets.len();
    state
        .reward_bot_service
        .upsert_reward_markets(&reward_markets)
        .await?;

    info!(
        trace_id = %trace_id,
        general_upserted,
        reward_upserted,
        "synced general and reward markets",
    );

    Ok(MarketSyncReport {
        general_upserted,
        reward_upserted,
    })
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
        image: market.image,
        rewards_max_spread: market.rewards_max_spread,
        rewards_min_size: market.rewards_min_size,
        total_daily_rate: market.total_daily_rate,
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
