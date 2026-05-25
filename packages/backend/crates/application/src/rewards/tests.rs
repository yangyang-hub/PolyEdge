#[cfg(test)]
mod tests {
    use super::{
        RewardBookLevel, RewardBotConfig, RewardMarket, RewardOrderBook, RewardToken,
        build_reward_quote_plans, decimal,
    };
    use std::collections::HashMap;
    use time::OffsetDateTime;

    #[test]
    fn quote_plan_uses_fallback_prices_in_dry_run() {
        let market = RewardMarket {
            condition_id: "cond_1".to_string(),
            question: "Will the event happen?".to_string(),
            market_slug: "event".to_string(),
            event_slug: "event".to_string(),
            image: String::new(),
            rewards_max_spread: decimal("800"),
            rewards_min_size: decimal("5"),
            total_daily_rate: decimal("25"),
            tokens: vec![
                RewardToken {
                    token_id: "yes".to_string(),
                    outcome: "YES".to_string(),
                    price: Some(decimal("0.52")),
                },
                RewardToken {
                    token_id: "no".to_string(),
                    outcome: "NO".to_string(),
                    price: Some(decimal("0.48")),
                },
            ],
            active: true,
            updated_at: OffsetDateTime::now_utc(),
        };

        let plans =
            build_reward_quote_plans(&[market], &HashMap::new(), &RewardBotConfig::default());

        assert_eq!(plans.len(), 1);
        assert!(plans[0].eligible);
        assert_eq!(plans[0].legs.len(), 2);
        assert_eq!(plans[0].legs[0].price, decimal("0.51"));
        assert_eq!(plans[0].legs[1].price, decimal("0.47"));
    }

    #[test]
    fn quote_plan_avoids_touching_best_ask() {
        let now = OffsetDateTime::now_utc();
        let market = RewardMarket {
            condition_id: "cond_2".to_string(),
            question: "Will the event happen?".to_string(),
            market_slug: "event".to_string(),
            event_slug: "event".to_string(),
            image: String::new(),
            rewards_max_spread: decimal("8"),
            rewards_min_size: decimal("1"),
            total_daily_rate: decimal("25"),
            tokens: vec![
                RewardToken {
                    token_id: "yes".to_string(),
                    outcome: "YES".to_string(),
                    price: Some(decimal("0.52")),
                },
                RewardToken {
                    token_id: "no".to_string(),
                    outcome: "NO".to_string(),
                    price: Some(decimal("0.48")),
                },
            ],
            active: true,
            updated_at: now,
        };
        let mut books = HashMap::new();
        books.insert(
            "yes".to_string(),
            RewardOrderBook {
                token_id: "yes".to_string(),
                bids: vec![RewardBookLevel {
                    price: decimal("0.51"),
                    size: decimal("100"),
                }],
                asks: vec![RewardBookLevel {
                    price: decimal("0.51"),
                    size: decimal("100"),
                }],
                observed_at: now,
            },
        );

        let config = RewardBotConfig {
            quote_edge_cents: decimal("0"),
            ..RewardBotConfig::default()
        };
        let plans = build_reward_quote_plans(&[market], &books, &config);

        assert!(!plans[0].eligible);
        assert_eq!(plans[0].reason, "YES bid would touch best ask");
    }
}
