fn slot(mode: QuotePricingMode) -> StrategyQuoteSlot {
    StrategyQuoteSlot {
        id: 1,
        strategy_version_id: 1,
        slot_key: "yes-1".to_string(),
        outcome: QuoteOutcome::Yes,
        quantity: Decimal::from(10),
        pricing_mode: mode,
        fixed_price: (mode == QuotePricingMode::Fixed).then_some(Decimal::new(50, 2)),
        book_rank: (mode == QuotePricingMode::BookRank).then_some(1),
        price_offset: Decimal::ZERO,
        minimum_price: Decimal::new(1, 2),
        maximum_price: Decimal::new(99, 2),
        post_only: true,
        enabled: true,
    }
}

fn book() -> CachedOrderBook {
    let now = OffsetDateTime::now_utc();
    CachedOrderBook {
        token_id: "yes".to_string(),
        bids: vec![crate::orderbook::BookLevel {
            price: Decimal::new(40, 2),
            size: Decimal::from(10),
        }],
        asks: vec![crate::orderbook::BookLevel {
            price: Decimal::new(60, 2),
            size: Decimal::from(10),
        }],
        observed_at: now,
        confirmed_at: now,
    }
}

fn active_execution_context(now: OffsetDateTime) -> ExecutionContext {
    ExecutionContext {
        job: WalletExecutionJob {
            id: 1,
            batch_id: 1,
            owner_user_id: 1,
            wallet_id: 1,
            status: polyedge_domain::WalletExecutionJobStatus::Running,
            attempt_count: 1,
            error_code: None,
            error_message: None,
            lease_epoch: 1,
            lease_owner: Some("test".to_string()),
            lease_expires_at: Some(now + time::Duration::seconds(30)),
            created_at: now,
            updated_at: now,
        },
        wallet: WalletAccount {
            id: 1,
            owner_user_id: 1,
            name: "test".to_string(),
            signer_address: "0x1".to_string(),
            funder_address: "0x1".to_string(),
            signature_type: 0,
            status: WalletAccountStatus::Active,
            trading_enabled: true,
            created_at: now,
            updated_at: now,
        },
        subscription_id: 1,
        subscription_status: StrategySubscriptionStatus::Active,
        subscription_wallet_enabled: true,
        strategy_status: StrategyStatus::Active,
        strategy_active_from: now - time::Duration::hours(1),
        effective_active_until: now + time::Duration::hours(1),
        market_status: MarketStatus::Open,
        strategy_version: StrategyVersion {
            id: 1,
            strategy_id: 1,
            version_number: 1,
            status: StrategyVersionStatus::Published,
            book_freshness_ms: 1_000,
            downward_reprice_confirm_ms: 0,
            upward_reprice_confirm_ms: 0,
            reprice_cooldown_ms: 0,
            max_replaces_per_cycle: 1,
            published_at: Some(now),
            created_at: now,
        },
        market_id: 1,
        slots: Vec::new(),
        outcomes: HashMap::new(),
        managed_orders: Vec::new(),
        risk_policy: WalletRiskPolicy {
            wallet_id: 1,
            max_open_orders: 1,
            max_open_buy_notional: Decimal::ONE,
            max_total_position_notional: Decimal::ONE,
            max_market_position_notional: Decimal::ONE,
            max_order_notional: Decimal::ONE,
            updated_at: now,
        },
        account_state: WalletAccountState {
            wallet_id: 1,
            available_collateral: Decimal::ONE,
            reserved_collateral: Decimal::ZERO,
            open_buy_notional: Decimal::ZERO,
            total_position_notional: Decimal::ZERO,
            last_synced_at: Some(now),
            last_error: None,
            version: 1,
            updated_at: now,
        },
        market_position_notional: Decimal::ZERO,
        trading_enabled: true,
        kill_switch_locked: false,
        force_cancel_all: false,
    }
}

#[test]
fn fixed_and_book_rank_prices_are_deterministic() {
    let orderbook = book();
    let now = orderbook.confirmed_at;
    assert_eq!(
        target_price(&slot(QuotePricingMode::Fixed), &orderbook, now),
        Decimal::new(50, 2)
    );
    assert_eq!(
        target_price(&slot(QuotePricingMode::BookRank), &orderbook, now),
        Decimal::new(40, 2)
    );
}

#[test]
fn post_only_crossing_ask_is_blocked() {
    let now = OffsetDateTime::now_utc();
    let mut configured = slot(QuotePricingMode::Fixed);
    configured.fixed_price = Some(Decimal::new(70, 2));
    assert!(matches!(
        build_target(&configured, "yes", &book(), now, 1_000),
        Ok(None)
    ));
}

#[test]
fn idempotency_key_contains_all_identity_dimensions() {
    let key = stable_idempotency_key(3, 4, 5, 6, ActionKind::Replace);
    assert_eq!(
        key,
        "wallet:3:version:4:slot:5:generation:6:action:replace_order"
    );
}

#[test]
fn subscription_and_effective_window_fail_closed() {
    let now = OffsetDateTime::now_utc();
    let mut context = active_execution_context(now);
    assert!(desired_state_active_at(&context, now));

    context.subscription_status = StrategySubscriptionStatus::Paused;
    assert!(!desired_state_active_at(&context, now));

    context.subscription_status = StrategySubscriptionStatus::Active;
    context.effective_active_until = now;
    assert!(!desired_state_active_at(&context, now));
}

fn target_price(
    slot: &StrategyQuoteSlot,
    orderbook: &CachedOrderBook,
    now: OffsetDateTime,
) -> Decimal {
    match build_target(slot, "yes", orderbook, now, 1_000) {
        Ok(Some(target)) => target.price,
        _ => Decimal::ZERO,
    }
}
