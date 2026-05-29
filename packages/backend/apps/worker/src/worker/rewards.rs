async fn run_reward_bot_once(state: &AppState, trace_id: &str) -> Result<RewardBotRunReport> {
    let (markets, books) = fetch_reward_bot_inputs(state).await?;
    state
        .reward_bot_service
        .run_simulation(markets, books, trace_id)
        .await
}

async fn poll_reward_bot(
    state: &AppState,
    max_cycles: Option<usize>,
) -> Result<RewardBotRunReport> {
    let mut total = RewardBotRunReport {
        markets_scanned: 0,
        books_fetched: 0,
        plans_built: 0,
        eligible_plans: 0,
        simulated_orders: 0,
        cancelled_orders: 0,
        filled_orders: 0,
        reward_accrued: rust_decimal::Decimal::ZERO,
    };
    let mut cycles = 0usize;
    let interval = Duration::from_secs(state.settings.rewards.poll_interval_secs.max(1));

    loop {
        let trace_id = new_trace_id();
        let report = run_reward_bot_once(state, &trace_id).await?;
        total.markets_scanned += report.markets_scanned;
        total.books_fetched += report.books_fetched;
        total.plans_built += report.plans_built;
        total.eligible_plans += report.eligible_plans;
        total.simulated_orders += report.simulated_orders;
        total.cancelled_orders += report.cancelled_orders;
        total.filled_orders += report.filled_orders;
        total.reward_accrued += report.reward_accrued;
        cycles += 1;

        info!(
            trace_id = %trace_id,
            cycle = cycles,
            markets_scanned = report.markets_scanned,
            books_fetched = report.books_fetched,
            plans_built = report.plans_built,
            eligible_plans = report.eligible_plans,
            simulated_orders = report.simulated_orders,
            cancelled_orders = report.cancelled_orders,
            "completed reward bot polling cycle",
        );

        if max_cycles.is_some_and(|limit| cycles >= limit) {
            break;
        }

        tokio::select! {
            () = tokio::time::sleep(interval) => {}
            shutdown = tokio::signal::ctrl_c() => {
                if let Err(error) = shutdown {
                    warn!(error = %error, "failed to listen for ctrl-c during reward bot polling");
                }
                break;
            }
        }
    }

    Ok(total)
}

async fn fetch_reward_bot_inputs(
    state: &AppState,
) -> Result<(Vec<RewardMarket>, HashMap<String, RewardOrderBook>)> {
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

fn reward_order_book_from_connector(book: PolymarketRewardOrderBook) -> RewardOrderBook {
    RewardOrderBook {
        token_id: book.token_id,
        bids: book
            .bids
            .into_iter()
            .map(|level| RewardBookLevel {
                price: level.price,
                size: level.size,
            })
            .collect(),
        asks: book
            .asks
            .into_iter()
            .map(|level| RewardBookLevel {
                price: level.price,
                size: level.size,
            })
            .collect(),
        observed_at: book.observed_at,
    }
}
