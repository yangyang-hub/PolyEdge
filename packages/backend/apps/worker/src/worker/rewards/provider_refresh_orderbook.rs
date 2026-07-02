async fn prepare_reward_ai_provider_orderbook_batch(
    state: &AppState,
    base_books: &HashMap<String, RewardOrderBook>,
    markets_by_condition: &HashMap<String, RewardMarket>,
    condition_ids: &[String],
    trace_id: &str,
) -> Result<HashMap<String, RewardOrderBook>> {
    let token_ids =
        reward_provider_orderbook_batch_token_ids(state, markets_by_condition, condition_ids);
    let mut books = base_books.clone();
    if token_ids.is_empty()
        || reward_provider_orderbook_batch_is_ready(markets_by_condition, condition_ids, &books)
    {
        state
            .orderbook_registry
            .register_tokens(REWARD_AI_PROVIDER_ORDERBOOK_SOURCE, &[])
            .await?;
        return Ok(books);
    }

    let remote_books = fetch_remote_cached_orderbooks(state, &token_ids).await?;
    for cached in remote_books {
        books.insert(cached.token_id.clone(), cached_order_book_to_reward(&cached));
    }
    if reward_provider_orderbook_batch_is_ready(markets_by_condition, condition_ids, &books) {
        state
            .orderbook_registry
            .register_tokens(REWARD_AI_PROVIDER_ORDERBOOK_SOURCE, &[])
            .await?;
        return Ok(books);
    }

    let max_tokens = state.settings.orderbook_stream.max_tokens;
    let mut missing_tokens = reward_provider_orderbook_missing_token_ids(
        max_tokens,
        markets_by_condition,
        condition_ids,
        &books,
    );
    if missing_tokens.is_empty() {
        state
            .orderbook_registry
            .register_tokens(REWARD_AI_PROVIDER_ORDERBOOK_SOURCE, &[])
            .await?;
        return Ok(books);
    }

    state
        .orderbook_registry
        .register_tokens(REWARD_AI_PROVIDER_ORDERBOOK_SOURCE, &missing_tokens)
        .await?;
    info!(
        trace_id = %trace_id,
        source = REWARD_AI_PROVIDER_ORDERBOOK_SOURCE,
        markets = condition_ids.len(),
        tokens = missing_tokens.len(),
        requested_tokens = token_ids.len(),
        "registered temporary reward AI provider orderbook batch",
    );

    for attempt in 0..REWARD_AI_PROVIDER_ORDERBOOK_WAIT_ATTEMPTS {
        missing_tokens = reward_provider_orderbook_missing_token_ids(
            max_tokens,
            markets_by_condition,
            condition_ids,
            &books,
        );
        if missing_tokens.is_empty() {
            return Ok(books);
        }
        let remote_books = fetch_remote_cached_orderbooks(state, &missing_tokens).await?;
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

fn reward_provider_orderbook_missing_token_ids(
    max_tokens: usize,
    markets_by_condition: &HashMap<String, RewardMarket>,
    condition_ids: &[String],
    books: &HashMap<String, RewardOrderBook>,
) -> Vec<String> {
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
            if books
                .get(&token.token_id)
                .is_some_and(|book| !book.bids.is_empty() && !book.asks.is_empty())
            {
                continue;
            }
            token_ids.push(token.token_id.clone());
        }
    }
    token_ids
}

#[cfg(test)]
mod provider_refresh_orderbook_tests {
    use super::*;

    fn provider_orderbook_test_market() -> RewardMarket {
        let now = OffsetDateTime::from_unix_timestamp(1_785_000_000).expect("valid timestamp");
        RewardMarket {
            condition_id: "cond_provider_books".to_string(),
            question: "Will provider books be ready?".to_string(),
            market_slug: "provider-books".to_string(),
            event_slug: "provider-books-event".to_string(),
            category: "test".to_string(),
            image: String::new(),
            rewards_max_spread: Decimal::ZERO,
            rewards_min_size: Decimal::ZERO,
            total_daily_rate: Decimal::ZERO,
            liquidity_usd: Decimal::ZERO,
            volume_24h_usd: Decimal::ZERO,
            market_spread_cents: Decimal::ZERO,
            end_at: None,
            ambiguity_level: "low".to_string(),
            market_synced_at: Some(now),
            tokens: vec![
                RewardToken {
                    token_id: "token_yes_provider".to_string(),
                    outcome: "Yes".to_string(),
                    price: None,
                },
                RewardToken {
                    token_id: "token_no_provider".to_string(),
                    outcome: "No".to_string(),
                    price: None,
                },
            ],
            active: true,
            updated_at: now,
        }
    }

    fn provider_orderbook_test_book(token_id: &str, bids: usize, asks: usize) -> RewardOrderBook {
        let now = OffsetDateTime::from_unix_timestamp(1_785_000_000).expect("valid timestamp");
        RewardOrderBook {
            token_id: token_id.to_string(),
            bids: (0..bids)
                .map(|_| RewardBookLevel {
                    price: Decimal::new(50, 2),
                    size: Decimal::new(100, 0),
                })
                .collect(),
            asks: (0..asks)
                .map(|_| RewardBookLevel {
                    price: Decimal::new(52, 2),
                    size: Decimal::new(100, 0),
                })
                .collect(),
            observed_at: now,
            confirmed_at: now,
        }
    }

    fn provider_orderbook_test_market_map() -> HashMap<String, RewardMarket> {
        let market = provider_orderbook_test_market();
        HashMap::from([(market.condition_id.clone(), market)])
    }

    #[test]
    fn provider_orderbook_batch_ready_when_base_books_are_populated() {
        let markets = provider_orderbook_test_market_map();
        let condition_ids = vec!["cond_provider_books".to_string()];
        let books = HashMap::from([
            (
                "token_yes_provider".to_string(),
                provider_orderbook_test_book("token_yes_provider", 1, 1),
            ),
            (
                "token_no_provider".to_string(),
                provider_orderbook_test_book("token_no_provider", 1, 1),
            ),
        ]);

        assert!(reward_provider_orderbook_batch_is_ready(
            &markets,
            &condition_ids,
            &books
        ));
        assert!(reward_provider_orderbook_missing_token_ids(
            10,
            &markets,
            &condition_ids,
            &books
        )
        .is_empty());
    }

    #[test]
    fn provider_orderbook_missing_tokens_only_returns_unpopulated_books() {
        let markets = provider_orderbook_test_market_map();
        let condition_ids = vec!["cond_provider_books".to_string()];
        let books = HashMap::from([
            (
                "token_yes_provider".to_string(),
                provider_orderbook_test_book("token_yes_provider", 1, 1),
            ),
            (
                "token_no_provider".to_string(),
                provider_orderbook_test_book("token_no_provider", 1, 0),
            ),
        ]);

        assert_eq!(
            reward_provider_orderbook_missing_token_ids(10, &markets, &condition_ids, &books),
            vec!["token_no_provider".to_string()]
        );
    }
}
