#[cfg(test)]
mod tests {
    use super::{
        RewardBookLevel, RewardBotConfig, RewardMarket, RewardOrderBook, RewardToken,
        build_reward_quote_plans, decimal,
    };
    use std::collections::HashMap;
    use time::OffsetDateTime;
    use rust_decimal::Decimal;

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

    use super::{
        ManagedRewardOrder, ManagedRewardOrderStatus, PostFillStrategy, RewardAccountState,
        RewardOrderSide, run_reward_simulation_tick, simulate_fill,
    };

    fn sample_market() -> RewardMarket {
        RewardMarket {
            condition_id: "cond_sim".to_string(),
            question: "Will the event happen?".to_string(),
            market_slug: "event".to_string(),
            event_slug: "event".to_string(),
            image: String::new(),
            rewards_max_spread: decimal("8"),
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
        }
    }

    fn open_buy(token: &str, outcome: &str, price: &str, size: &str) -> ManagedRewardOrder {
        let now = OffsetDateTime::now_utc();
        ManagedRewardOrder {
            id: format!("seed_{token}"),
            account_id: "reward_simulator".to_string(),
            condition_id: "cond_sim".to_string(),
            token_id: token.to_string(),
            outcome: outcome.to_string(),
            side: RewardOrderSide::Buy,
            price: decimal(price),
            size: decimal(size),
            external_order_id: Some(format!("sim_seed_{token}")),
            status: ManagedRewardOrderStatus::Open,
            scoring: true,
            reason: "seed".to_string(),
            filled_size: Decimal::ZERO,
            reward_earned: Decimal::ZERO,
            last_scored_at: None,
            created_at: now,
            updated_at: now,
        }
    }

    fn fresh_account() -> RewardAccountState {
        RewardAccountState::fresh(
            "reward_simulator",
            decimal("1000"),
            OffsetDateTime::now_utc(),
        )
    }

    #[test]
    fn places_two_sided_quotes_and_reserves_capital() {
        let config = RewardBotConfig::default();
        let outcome = run_reward_simulation_tick(
            &config,
            fresh_account(),
            Vec::new(),
            Vec::new(),
            &[sample_market()],
            &HashMap::new(),
            60,
            "trc_test",
        );

        assert_eq!(outcome.report.simulated_orders, 2);
        let open: Vec<_> = outcome
            .orders
            .iter()
            .filter(|order| order.status == ManagedRewardOrderStatus::Open)
            .collect();
        assert_eq!(open.len(), 2);
        assert!(outcome.account.reserved_usd > Decimal::ZERO);
        assert!(outcome.account.available_usd < config.account_capital_usd);
        // Reserved + available should still equal the starting capital (no fills/rewards yet,
        // since there is no book to cross and rewards need a prior resting period).
        assert_eq!(
            outcome.account.reserved_usd + outcome.account.available_usd,
            config.account_capital_usd + outcome.account.reward_earned_usd
        );
    }

    #[test]
    fn accrues_polymarket_rewards_for_resting_two_sided_quotes() {
        let config = RewardBotConfig {
            fill_rate_per_tick: decimal("0"),
            ..RewardBotConfig::default()
        };
        let seeds = vec![
            open_buy("yes", "YES", "0.51", "20"),
            open_buy("no", "NO", "0.47", "20"),
        ];
        let outcome = run_reward_simulation_tick(
            &config,
            fresh_account(),
            seeds,
            Vec::new(),
            &[sample_market()],
            &HashMap::new(),
            86_400,
            "trc_test",
        );

        assert!(outcome.account.reward_earned_usd > Decimal::ZERO);
        assert!(outcome.report.reward_accrued > Decimal::ZERO);
        assert!(outcome.report.filled_orders == 0);
    }

    #[test]
    fn cancels_quotes_that_drift_out_of_band() {
        let config = RewardBotConfig {
            fill_rate_per_tick: decimal("0"),
            ..RewardBotConfig::default()
        };
        // A stale resting buy far below the fresh midpoint should be cancelled.
        let seeds = vec![open_buy("yes", "YES", "0.30", "20")];
        let outcome = run_reward_simulation_tick(
            &config,
            fresh_account(),
            seeds,
            Vec::new(),
            &[sample_market()],
            &HashMap::new(),
            60,
            "trc_test",
        );

        let drifted = outcome
            .orders
            .iter()
            .find(|order| order.id == "seed_yes")
            .expect("seed order present");
        assert_eq!(drifted.status, ManagedRewardOrderStatus::Cancelled);
        assert!(
            outcome
                .events
                .iter()
                .any(|event| event.event_type == "reward_order_cancelled")
        );
    }

    #[test]
    fn competing_book_depth_reduces_reward_share() {
        // No fill (asks sit above our bids); fresh books carry competitor depth
        // inside the reward band, which should shrink our reward versus the
        // book-less fallback path.
        let config = RewardBotConfig {
            fill_rate_per_tick: decimal("0"),
            ..RewardBotConfig::default()
        };
        let seeds = || {
            vec![
                open_buy("yes", "YES", "0.51", "20"),
                open_buy("no", "NO", "0.47", "20"),
            ]
        };
        let now = OffsetDateTime::now_utc();

        let without_book = run_reward_simulation_tick(
            &config,
            fresh_account(),
            seeds(),
            Vec::new(),
            &[sample_market()],
            &HashMap::new(),
            86_400,
            "trc_test",
        );

        // A deep competing YES book around the midpoint.
        let mut books = HashMap::new();
        books.insert(
            "yes".to_string(),
            RewardOrderBook {
                token_id: "yes".to_string(),
                bids: vec![RewardBookLevel {
                    price: decimal("0.51"),
                    size: decimal("500"),
                }],
                asks: vec![RewardBookLevel {
                    price: decimal("0.53"),
                    size: decimal("500"),
                }],
                observed_at: now,
            },
        );
        let with_book = run_reward_simulation_tick(
            &config,
            fresh_account(),
            seeds(),
            Vec::new(),
            &[sample_market()],
            &books,
            86_400,
            "trc_test",
        );

        assert!(with_book.account.reward_earned_usd > Decimal::ZERO);
        assert!(
            with_book.account.reward_earned_usd < without_book.account.reward_earned_usd,
            "observed competitor depth should reduce our reward share"
        );
        assert!(
            with_book
                .events
                .iter()
                .filter(|event| event.event_type == "reward_accrued")
                .any(|event| event.metadata.get("competition_source")
                    == Some(&serde_json::Value::String("observed_book".to_string())))
        );
    }

    #[test]
    fn fill_model_crosses_and_touches() {
        let order = open_buy("yes", "YES", "0.51", "20");

        // Best ask at or below our bid always fills.
        let crossed = simulate_fill(
            &order,
            Some(decimal("0.49")),
            Some(decimal("0.50")),
            true,
            0.99,
            &RewardBotConfig::default(),
        );
        assert_eq!(crossed, Some(decimal("20")));

        // Merely touching fills only when the random draw beats the fill rate.
        let config = RewardBotConfig {
            fill_rate_per_tick: decimal("0.5"),
            ..RewardBotConfig::default()
        };
        let touched = simulate_fill(&order, Some(decimal("0.50")), Some(decimal("0.52")), true, 0.1, &config);
        assert!(touched.is_some());
        let missed = simulate_fill(&order, Some(decimal("0.50")), Some(decimal("0.52")), true, 0.9, &config);
        assert!(missed.is_none());
    }

    #[test]
    fn exit_order_is_placed_after_a_fill() {
        let config = RewardBotConfig {
            post_fill_strategy: PostFillStrategy::ExitAtMarkup,
            cancel_on_fill: false,
            ..RewardBotConfig::default()
        };
        // Seed a resting YES buy and a NO buy; a fresh YES book that crosses the YES bid fills it.
        let seeds = vec![
            open_buy("yes", "YES", "0.51", "20"),
            open_buy("no", "NO", "0.47", "20"),
        ];
        let now = OffsetDateTime::now_utc();
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
                    price: decimal("0.50"),
                    size: decimal("100"),
                }],
                observed_at: now,
            },
        );

        let outcome = run_reward_simulation_tick(
            &config,
            fresh_account(),
            seeds,
            Vec::new(),
            &[sample_market()],
            &books,
            60,
            "trc_test",
        );

        assert!(outcome.report.filled_orders >= 1);
        assert!(
            outcome
                .fills
                .iter()
                .any(|fill| fill.side == RewardOrderSide::Buy)
        );
        assert!(
            outcome
                .orders
                .iter()
                .any(|order| order.status == ManagedRewardOrderStatus::ExitPending),
            "expected an exit order after the fill"
        );
    }
}
