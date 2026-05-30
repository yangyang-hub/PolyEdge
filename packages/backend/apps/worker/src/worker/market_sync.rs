async fn sync_markets_once(state: &AppState, trace_id: &str) -> Result<MarketSyncReport> {
    let connector =
        PolymarketGammaConnector::new(&state.settings.polymarket.gamma_host)?;
    let limit = state.settings.arbitrage.scan_limit;
    let gamma_markets = connector.fetch_markets(limit).await?;
    let views: Vec<MarketView> = gamma_markets
        .into_iter()
        .map(gamma_market_to_view)
        .collect();
    let fetched = views.len();
    let upserted = state
        .market_event_service
        .upsert_markets(&views, trace_id)
        .await?;

    // Sync reward markets from CLOB rewards API into reward_markets table.
    let rewards_connector =
        PolymarketRewardsConnector::new(&state.settings.polymarket.clob_host)?;
    let reward_markets_raw = rewards_connector.fetch_current_markets().await?;
    let reward_raw_count = reward_markets_raw.len();
    let reward_markets: Vec<RewardMarket> = reward_markets_raw
        .into_iter()
        .map(reward_market_from_connector)
        .collect();
    let reward_enriched_count = reward_markets.len();
    state
        .reward_bot_service
        .upsert_reward_markets(&reward_markets)
        .await?;

    info!(
        trace_id = %trace_id,
        general_fetched = fetched,
        general_upserted = upserted,
        reward_raw = reward_raw_count,
        reward_upserted = reward_enriched_count,
        "synced general and reward markets",
    );

    Ok(MarketSyncReport { fetched, upserted })
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
