use super::*;

fn test_market(rewards_min_size: Decimal) -> RewardMarket {
    RewardMarket {
        condition_id: "cond_budget".to_string(),
        question: "Budget allocation market".to_string(),
        market_slug: "budget-allocation-market".to_string(),
        event_slug: "budget-allocation-event".to_string(),
        image: String::new(),
        rewards_max_spread: decimal("8"),
        rewards_min_size,
        total_daily_rate: decimal("50"),
        tokens: vec![
            RewardToken {
                token_id: "yes_budget".to_string(),
                outcome: "Yes".to_string(),
                price: None,
            },
            RewardToken {
                token_id: "no_budget".to_string(),
                outcome: "No".to_string(),
                price: None,
            },
        ],
        active: true,
        updated_at: OffsetDateTime::now_utc(),
    }
}

fn test_books() -> HashMap<String, RewardOrderBook> {
    let now = OffsetDateTime::now_utc();
    [
        RewardOrderBook {
            token_id: "yes_budget".to_string(),
            bids: vec![RewardBookLevel {
                price: decimal("0.77"),
                size: decimal("1000"),
            }],
            asks: vec![RewardBookLevel {
                price: decimal("0.78"),
                size: decimal("1000"),
            }],
            observed_at: now,
        },
        RewardOrderBook {
            token_id: "no_budget".to_string(),
            bids: vec![RewardBookLevel {
                price: decimal("0.22"),
                size: decimal("1000"),
            }],
            asks: vec![RewardBookLevel {
                price: decimal("0.23"),
                size: decimal("1000"),
            }],
            observed_at: now,
        },
    ]
    .into_iter()
    .map(|book| (book.token_id.clone(), book))
    .collect()
}

#[test]
fn combined_market_budget_can_satisfy_asymmetric_minimum_sizes() {
    let config = RewardBotConfig {
        per_market_usd: decimal("20"),
        quote_size_usd: decimal("10"),
        min_market_score: Decimal::ZERO,
        ..RewardBotConfig::default()
    };

    let plan = build_reward_quote_plan(&test_market(decimal("20")), &test_books(), &config);

    assert!(plan.eligible, "{}", plan.reason);
    assert_eq!(plan.legs.len(), 2);
    assert!(plan.legs.iter().all(|leg| leg.size >= decimal("20")));
    assert!(
        plan.legs
            .iter()
            .fold(Decimal::ZERO, |sum, leg| sum + leg.price * leg.size)
            <= config.per_market_usd
    );
}

#[test]
fn combined_market_budget_rejects_unaffordable_minimum_sizes() {
    let config = RewardBotConfig {
        per_market_usd: decimal("20"),
        quote_size_usd: decimal("10"),
        min_market_score: Decimal::ZERO,
        ..RewardBotConfig::default()
    };

    let plan = build_reward_quote_plan(&test_market(decimal("50")), &test_books(), &config);

    assert!(!plan.eligible);
    assert_eq!(
        plan.reason,
        "per-market budget cannot satisfy rewards minimum size"
    );
    assert!(plan.legs.is_empty());
}
