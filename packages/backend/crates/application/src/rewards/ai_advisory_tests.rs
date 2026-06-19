use super::*;

fn advisory_book_test_market() -> RewardMarket {
    RewardMarket {
        condition_id: "cond_advisory_books".to_string(),
        question: "Are all legs quoted?".to_string(),
        market_slug: "advisory-books".to_string(),
        event_slug: "advisory-books-event".to_string(),
        category: "politics".to_string(),
        image: String::new(),
        rewards_max_spread: decimal("4.5"),
        rewards_min_size: decimal("100"),
        total_daily_rate: decimal("50"),
        liquidity_usd: decimal("10000"),
        volume_24h_usd: decimal("25000"),
        market_spread_cents: decimal("1"),
        end_at: Some(
            OffsetDateTime::from_unix_timestamp(1_785_000_000).expect("valid timestamp")
                + TimeDuration::days(30),
        ),
        ambiguity_level: "low".to_string(),
        market_synced_at: Some(OffsetDateTime::from_unix_timestamp(1_785_000_000).expect("valid timestamp")),
        tokens: vec![
            RewardToken {
                token_id: "token_yes_advisory".to_string(),
                outcome: "Yes".to_string(),
                price: Some(decimal("0.55")),
            },
            RewardToken {
                token_id: "token_no_advisory".to_string(),
                outcome: "No".to_string(),
                price: Some(decimal("0.45")),
            },
        ],
        active: true,
        updated_at: OffsetDateTime::from_unix_timestamp(1_785_000_000).expect("valid timestamp"),
    }
}

fn book_with(token_id: &str, bids: usize, asks: usize) -> RewardOrderBook {
    RewardOrderBook {
        token_id: token_id.to_string(),
        bids: (0..bids)
            .map(|_| RewardBookLevel {
                price: decimal("0.50"),
                size: decimal("100"),
            })
            .collect(),
        asks: (0..asks)
            .map(|_| RewardBookLevel {
                price: decimal("0.52"),
                size: decimal("100"),
            })
            .collect(),
        observed_at: OffsetDateTime::from_unix_timestamp(1_785_000_000).expect("valid timestamp"),
    }
}

fn advisory_books(yes_bids: usize, yes_asks: usize, no_bids: usize, no_asks: usize) -> HashMap<String, RewardOrderBook> {
    [
        book_with("token_yes_advisory", yes_bids, yes_asks),
        book_with("token_no_advisory", no_bids, no_asks),
    ]
    .into_iter()
    .map(|book| (book.token_id.clone(), book))
    .collect()
}

#[test]
fn reward_market_books_available_when_both_legs_populated() {
    let market = advisory_book_test_market();
    let books = advisory_books(2, 2, 1, 1);
    assert!(reward_market_books_available(&market, &books));
}

#[test]
fn reward_market_books_unavailable_when_leg_missing() {
    let market = advisory_book_test_market();
    // Only the YES leg is present.
    let books = advisory_books(2, 2, 0, 0);
    assert!(!reward_market_books_available(&market, &books));
}

#[test]
fn reward_market_books_unavailable_when_bids_empty() {
    let market = advisory_book_test_market();
    let books = advisory_books(0, 2, 1, 1);
    assert!(!reward_market_books_available(&market, &books));
}

#[test]
fn reward_market_books_unavailable_when_asks_empty() {
    let market = advisory_book_test_market();
    let books = advisory_books(2, 0, 1, 1);
    assert!(!reward_market_books_available(&market, &books));
}

#[test]
fn reward_market_books_unavailable_when_market_has_no_tokens() {
    let mut market = advisory_book_test_market();
    market.tokens.clear();
    let books = advisory_books(2, 2, 1, 1);
    assert!(!reward_market_books_available(&market, &books));
}
