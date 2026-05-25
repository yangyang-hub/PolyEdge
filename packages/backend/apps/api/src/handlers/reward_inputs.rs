async fn fetch_reward_bot_inputs(
    state: &AppState,
    _trace_id: &str,
) -> polyedge_domain::Result<(Vec<RewardMarket>, HashMap<String, RewardOrderBook>)> {
    let config = state.reward_bot_service.read_config().await?;
    let connector = PolymarketRewardsConnector::new(&state.settings.polymarket.clob_host)?;
    let markets = connector
        .fetch_current_markets()
        .await?
        .into_iter()
        .map(reward_market_from_connector)
        .collect::<Vec<_>>();
    let token_ids = select_reward_book_token_ids(&markets, &config);
    let books = connector
        .fetch_order_books(&token_ids)
        .await?
        .into_iter()
        .map(reward_order_book_from_connector)
        .map(|book| (book.token_id.clone(), book))
        .collect::<HashMap<_, _>>();

    Ok((markets, books))
}
