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
