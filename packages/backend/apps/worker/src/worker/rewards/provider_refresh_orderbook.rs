async fn prepare_reward_ai_provider_orderbook_batch(
    state: &AppState,
    base_books: &HashMap<String, RewardOrderBook>,
    markets_by_condition: &HashMap<String, RewardMarket>,
    condition_ids: &[String],
    trace_id: &str,
) -> Result<HashMap<String, RewardOrderBook>> {
    let token_ids =
        reward_provider_orderbook_batch_token_ids(state, markets_by_condition, condition_ids);
    state
        .orderbook_registry
        .register_tokens(REWARD_AI_PROVIDER_ORDERBOOK_SOURCE, &token_ids)
        .await?;
    info!(
        trace_id = %trace_id,
        source = REWARD_AI_PROVIDER_ORDERBOOK_SOURCE,
        markets = condition_ids.len(),
        tokens = token_ids.len(),
        "registered temporary reward AI provider orderbook batch",
    );

    let mut books = base_books.clone();
    if token_ids.is_empty() {
        return Ok(books);
    }

    for attempt in 0..REWARD_AI_PROVIDER_ORDERBOOK_WAIT_ATTEMPTS {
        let remote_books = fetch_remote_cached_orderbooks(state, &token_ids).await?;
        for cached in remote_books {
            books.insert(cached.token_id.clone(), cached_order_book_to_reward(&cached));
        }
        if reward_provider_orderbook_batch_is_ready(markets_by_condition, condition_ids, &books) {
            return Ok(books);
        }
        if attempt + 1 < REWARD_AI_PROVIDER_ORDERBOOK_WAIT_ATTEMPTS {
            tokio::time::sleep(REWARD_AI_PROVIDER_ORDERBOOK_WAIT_DELAY).await;
        }
    }

    Ok(books)
}

fn reward_provider_orderbook_batch_token_ids(
    state: &AppState,
    markets_by_condition: &HashMap<String, RewardMarket>,
    condition_ids: &[String],
) -> Vec<String> {
    let max_tokens = state.settings.orderbook_stream.max_tokens;
    if max_tokens == 0 {
        return Vec::new();
    }
    let mut seen = HashSet::new();
    let mut token_ids = Vec::new();
    for condition_id in condition_ids {
        let Some(market) = markets_by_condition.get(condition_id) else {
            continue;
        };
        for token in &market.tokens {
            if token_ids.len() >= max_tokens {
                return token_ids;
            }
            if token.token_id.trim().is_empty() || !seen.insert(token.token_id.clone()) {
                continue;
            }
            token_ids.push(token.token_id.clone());
        }
    }
    token_ids
}

fn reward_provider_orderbook_batch_is_ready(
    markets_by_condition: &HashMap<String, RewardMarket>,
    condition_ids: &[String],
    books: &HashMap<String, RewardOrderBook>,
) -> bool {
    condition_ids.iter().all(|condition_id| {
        markets_by_condition
            .get(condition_id)
            .is_some_and(|market| reward_market_books_available(market, books))
    })
}

async fn promote_reward_ai_provider_passed_market_to_eligible_source(
    state: &AppState,
    market: &RewardMarket,
    advisory: &RewardMarketAdvisory,
    trace_id: &str,
    promoted_tokens: &mut Vec<String>,
) {
    if reward_ai_advisory_blocks_quote(advisory) {
        return;
    }

    let max_tokens = state.settings.orderbook_stream.max_tokens;
    if max_tokens == 0 {
        return;
    }
    let mut tokens = match state
        .reward_bot_service
        .list_eligible_reward_book_token_ids()
        .await
    {
        Ok(tokens) => tokens,
        Err(error) => {
            warn!(
                trace_id = %trace_id,
                condition_id = %market.condition_id,
                error = %error,
                "failed to list eligible reward tokens before AI provider promotion",
            );
            Vec::new()
        }
    };
    let mut seen = tokens.iter().cloned().collect::<HashSet<_>>();
    for token_id in promoted_tokens.iter() {
        if tokens.len() >= max_tokens {
            break;
        }
        if token_id.trim().is_empty() || !seen.insert(token_id.clone()) {
            continue;
        }
        tokens.push(token_id.clone());
    }
    let mut newly_promoted = Vec::new();
    for token in &market.tokens {
        if tokens.len() >= max_tokens {
            break;
        }
        if token.token_id.trim().is_empty() || !seen.insert(token.token_id.clone()) {
            continue;
        }
        tokens.push(token.token_id.clone());
        newly_promoted.push(token.token_id.clone());
    }
    if let Err(error) = state
        .orderbook_registry
        .register_tokens("rewards_eligible", &tokens)
        .await
    {
        warn!(
            trace_id = %trace_id,
            condition_id = %market.condition_id,
            error = %error,
            "failed to promote AI-allowed reward market to eligible orderbook source",
        );
        return;
    }
    promoted_tokens.extend(newly_promoted);
    debug!(
        trace_id = %trace_id,
        condition_id = %market.condition_id,
        tokens = market.tokens.len(),
        "promoted AI-allowed reward market to eligible orderbook source",
    );
}
