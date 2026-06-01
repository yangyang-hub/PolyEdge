#[cfg(test)]
mod tests {
    use super::{
        RewardBookLevel, RewardBotConfig, RewardMarket, RewardOrderBook, RewardToken,
        build_reward_quote_plans, decimal, select_reward_quote_candidate_markets,
    };
    use rust_decimal::Decimal;
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

    #[test]
    fn quote_plan_caps_leg_notional_to_simulated_capital() {
        let config = RewardBotConfig {
            account_capital_usd: decimal("200"),
            per_market_usd: decimal("1000"),
            quote_size_usd: decimal("1000"),
            ..RewardBotConfig::default()
        };
        let plans = build_reward_quote_plans(&[sample_market()], &HashMap::new(), &config);

        assert!(plans[0].eligible);
        assert!(plans[0]
            .legs
            .iter()
            .all(|leg| leg.notional_usd <= config.account_capital_usd));
    }

    #[test]
    fn reward_candidate_filter_runs_before_book_selection() {
        let mut low_reward = sample_market();
        low_reward.condition_id = "low_reward".to_string();
        low_reward.total_daily_rate = decimal("0.25");

        let mut inactive = sample_market();
        inactive.condition_id = "inactive".to_string();
        inactive.active = false;

        let mut valid = sample_market();
        valid.condition_id = "valid".to_string();

        let config = RewardBotConfig {
            min_daily_reward: decimal("1"),
            ..RewardBotConfig::default()
        };
        let candidates = select_reward_quote_candidate_markets(
            &[low_reward, inactive, valid.clone()],
            &config,
        );

        assert_eq!(candidates, vec![valid]);
    }

    use super::{
        ManagedRewardOrder, ManagedRewardOrderStatus, PostFillStrategy, RewardAccountState,
        RewardOrderSide, run_reward_simulation_tick, run_reconcile_tick, simulate_fill,
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

    fn sample_market_with_suffix(suffix: &str) -> RewardMarket {
        let mut market = sample_market();
        market.condition_id = format!("cond_sim_{suffix}");
        market.market_slug = format!("event-{suffix}");
        market.tokens[0].token_id = format!("yes_{suffix}");
        market.tokens[1].token_id = format!("no_{suffix}");
        market
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

    fn empty_fresh_books() -> HashMap<String, RewardOrderBook> {
        let now = OffsetDateTime::now_utc();
        HashMap::from([
            (
                "yes".to_string(),
                RewardOrderBook {
                    token_id: "yes".to_string(),
                    bids: Vec::new(),
                    asks: Vec::new(),
                    observed_at: now,
                },
            ),
            (
                "no".to_string(),
                RewardOrderBook {
                    token_id: "no".to_string(),
                    bids: Vec::new(),
                    asks: Vec::new(),
                    observed_at: now,
                },
            ),
        ])
    }

    #[test]
    fn places_two_sided_quotes_without_hard_reserving_capital() {
        let config = RewardBotConfig::default();
        let outcome = run_reward_simulation_tick(
            &config,
            fresh_account(),
            Vec::new(),
            Vec::new(),
            &[sample_market()],
            &HashMap::new(),
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
        assert_eq!(outcome.account.reserved_usd, Decimal::ZERO);
        assert_eq!(outcome.account.available_usd, config.account_capital_usd);
    }

    #[test]
    fn pooled_capital_can_quote_multiple_markets_above_cash_balance() {
        let config = RewardBotConfig {
            account_capital_usd: decimal("200"),
            per_market_usd: decimal("400"),
            quote_size_usd: decimal("200"),
            max_markets: 3,
            max_open_orders: 12,
            max_global_position_usd: Decimal::ZERO,
            ..RewardBotConfig::default()
        };
        let account = RewardAccountState::fresh(
            "reward_simulator",
            config.account_capital_usd,
            OffsetDateTime::now_utc(),
        );
        let markets = vec![
            sample_market_with_suffix("1"),
            sample_market_with_suffix("2"),
            sample_market_with_suffix("3"),
        ];
        let outcome = run_reward_simulation_tick(
            &config,
            account,
            Vec::new(),
            Vec::new(),
            &markets,
            &HashMap::new(),
            &HashMap::new(),
            60,
            "trc_test",
        );

        assert_eq!(outcome.report.simulated_orders, 6);
        let open_notional = outcome
            .orders
            .iter()
            .filter(|order| order.status == ManagedRewardOrderStatus::Open)
            .map(|order| (order.price * order.size).round_dp(4))
            .sum::<Decimal>();
        assert!(open_notional > config.account_capital_usd);
        assert_eq!(outcome.account.available_usd, config.account_capital_usd);
        assert_eq!(outcome.account.reserved_usd, Decimal::ZERO);
    }

    #[test]
    fn legacy_open_order_reserve_is_released_on_next_tick() {
        let config = RewardBotConfig {
            fill_rate_per_tick: Decimal::ZERO,
            ..RewardBotConfig::default()
        };
        let mut account = fresh_account();
        account.available_usd = decimal("800");
        account.reserved_usd = decimal("200");

        let outcome = run_reward_simulation_tick(
            &config,
            account,
            vec![open_buy("yes", "YES", "0.51", "20")],
            Vec::new(),
            &[sample_market()],
            &HashMap::new(),
            &HashMap::new(),
            60,
            "trc_test",
        );

        assert_eq!(outcome.account.available_usd, decimal("1000"));
        assert_eq!(outcome.account.reserved_usd, Decimal::ZERO);
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
            &empty_fresh_books(),
            &HashMap::new(),
            86_400,
            "trc_test",
        );

        assert!(outcome.account.reward_earned_usd > Decimal::ZERO);
        assert!(outcome.report.reward_accrued > Decimal::ZERO);
        assert!(outcome.report.filled_orders == 0);
    }

    #[test]
    fn missing_books_do_not_fill_or_accrue_rewards() {
        let config = RewardBotConfig {
            fill_rate_per_tick: decimal("1"),
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
            &HashMap::new(),
            86_400,
            "trc_test",
        );

        assert_eq!(outcome.account.reward_earned_usd, Decimal::ZERO);
        assert_eq!(outcome.report.reward_accrued, Decimal::ZERO);
        assert_eq!(outcome.report.filled_orders, 0);
    }

    #[test]
    fn zero_limits_place_no_orders() {
        let config = RewardBotConfig {
            max_markets: 0,
            max_open_orders: 0,
            ..RewardBotConfig::default()
        };
        let outcome = run_reward_simulation_tick(
            &config,
            fresh_account(),
            Vec::new(),
            Vec::new(),
            &[sample_market()],
            &HashMap::new(),
            &HashMap::new(),
            60,
            "trc_test",
        );

        assert_eq!(outcome.report.simulated_orders, 0);
        assert!(outcome.orders.is_empty());
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

        let without_competition = run_reward_simulation_tick(
            &config,
            fresh_account(),
            seeds(),
            Vec::new(),
            &[sample_market()],
            &empty_fresh_books(),
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
        books.insert(
            "no".to_string(),
            RewardOrderBook {
                token_id: "no".to_string(),
                bids: Vec::new(),
                asks: Vec::new(),
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
            &HashMap::new(),
            86_400,
            "trc_test",
        );

        assert!(with_book.account.reward_earned_usd > Decimal::ZERO);
        assert!(
            with_book.account.reward_earned_usd < without_competition.account.reward_earned_usd,
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
        let touched = simulate_fill(
            &order,
            Some(decimal("0.50")),
            Some(decimal("0.52")),
            true,
            0.1,
            &config,
        );
        assert!(touched.is_some());
        let missed = simulate_fill(
            &order,
            Some(decimal("0.50")),
            Some(decimal("0.52")),
            true,
            0.9,
            &config,
        );
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
            &HashMap::new(),
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

    // ---- Risk control tests ----

    #[test]
    fn risk_check_disabled_by_default() {
        // With all risk fields at 0, no risk cancel should trigger.
        let config = RewardBotConfig::default();
        assert_eq!(config.min_depth_usd, Decimal::ZERO);
        assert_eq!(config.cancel_bid_rank, 0);
        assert_eq!(config.depth_drop_pct, Decimal::ZERO);
        assert_eq!(config.fill_velocity_usd, Decimal::ZERO);
        assert_eq!(config.mass_cancel_pct, Decimal::ZERO);
        assert_eq!(config.requote_interval_sec, 0);
    }

    #[test]
    fn min_depth_cancel_thins_book() {
        // Set min_depth_usd = $1000. A book with only $50 above our price should trigger cancel.
        let config = RewardBotConfig {
            min_depth_usd: decimal("1000"),
            ..RewardBotConfig::default()
        };
        let now = OffsetDateTime::now_utc();
        let mut books = HashMap::new();
        books.insert(
            "yes".to_string(),
            RewardOrderBook {
                token_id: "yes".to_string(),
                bids: vec![
                    RewardBookLevel { price: decimal("0.52"), size: decimal("100") },
                    RewardBookLevel { price: decimal("0.51"), size: decimal("100") },
                ],
                asks: vec![RewardBookLevel { price: decimal("0.55"), size: decimal("100") }],
                observed_at: now,
            },
        );
        let seeds = vec![open_buy("yes", "YES", "0.51", "20")];
        let outcome = run_reward_simulation_tick(
            &config,
            fresh_account(),
            seeds,
            Vec::new(),
            &[sample_market()],
            &books,
            &HashMap::new(),
            60,
            "trc_test",
        );
        assert!(
            outcome.report.risk_cancelled_orders >= 1
                || outcome.report.cancelled_orders >= 1,
            "expected cancel due to thin book"
        );
    }

    #[test]
    fn bid_rank_cancel_on_promotion() {
        // cancel_bid_rank = 2: cancel when order is at bid-1 or bid-2.
        // Our order at 0.51 is bid-1 (only one level) → should cancel.
        let config = RewardBotConfig {
            cancel_bid_rank: 2,
            ..RewardBotConfig::default()
        };
        let now = OffsetDateTime::now_utc();
        let mut books = HashMap::new();
        books.insert(
            "yes".to_string(),
            RewardOrderBook {
                token_id: "yes".to_string(),
                bids: vec![RewardBookLevel { price: decimal("0.51"), size: decimal("100") }],
                asks: vec![RewardBookLevel { price: decimal("0.55"), size: decimal("100") }],
                observed_at: now,
            },
        );
        let seeds = vec![open_buy("yes", "YES", "0.51", "20")];
        let outcome = run_reward_simulation_tick(
            &config,
            fresh_account(),
            seeds,
            Vec::new(),
            &[sample_market()],
            &books,
            &HashMap::new(),
            60,
            "trc_test",
        );
        assert!(
            outcome.report.risk_cancelled_orders >= 1
                || outcome.report.cancelled_orders >= 1,
            "expected cancel due to bid rank promotion"
        );
    }

    #[test]
    fn requote_age_cancel_after_interval() {
        // requote_interval_sec = 10, requote_jitter_sec = 0.
        // An order created 60s ago should be cancelled.
        let config = RewardBotConfig {
            requote_interval_sec: 10,
            requote_jitter_sec: 0,
            ..RewardBotConfig::default()
        };
        let now = OffsetDateTime::now_utc();
        let mut books = HashMap::new();
        books.insert(
            "yes".to_string(),
            RewardOrderBook {
                token_id: "yes".to_string(),
                bids: vec![
                    RewardBookLevel { price: decimal("0.55"), size: decimal("200") },
                    RewardBookLevel { price: decimal("0.52"), size: decimal("200") },
                    RewardBookLevel { price: decimal("0.51"), size: decimal("100") },
                ],
                asks: vec![RewardBookLevel { price: decimal("0.58"), size: decimal("100") }],
                observed_at: now,
            },
        );

        // Create an order that was created 60 seconds ago.
        let mut old_order = open_buy("yes", "YES", "0.51", "20");
        old_order.created_at = now - time::Duration::seconds(60);

        let outcome = run_reward_simulation_tick(
            &config,
            fresh_account(),
            vec![old_order],
            Vec::new(),
            &[sample_market()],
            &books,
            &HashMap::new(),
            60,
            "trc_test",
        );
        assert!(
            outcome.report.risk_cancelled_orders >= 1
                || outcome.report.cancelled_orders >= 1,
            "expected cancel due to requote age"
        );
    }

    #[test]
    fn reconcile_tick_uses_existing_plans() {
        // run_reconcile_tick should NOT rebuild plans; it uses the supplied ones.
        let config = RewardBotConfig::default();
        let plans = build_reward_quote_plans(&[sample_market()], &HashMap::new(), &config);
        let plan_count = plans.len();

        let outcome = run_reconcile_tick(
            &config,
            fresh_account(),
            Vec::new(),
            Vec::new(),
            plans.clone(),
            vec![sample_market()],
            &HashMap::new(),
            &HashMap::new(),
            60,
            "trc_test",
        );

        assert_eq!(outcome.plans.len(), plan_count);
        assert_eq!(outcome.plans[0].score, plans[0].score);
    }
}
