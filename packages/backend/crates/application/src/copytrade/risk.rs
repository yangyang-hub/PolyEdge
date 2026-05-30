/// Risk gating: returns `Some(CopySkipReason)` if the source trade should be
/// skipped, `None` if it passes all checks. Pure function — no I/O.
pub fn check_skip_reasons(
    config: &CopyTradeConfig,
    source_trade: &SourceTrade,
    account: &CopyAccountState,
    positions: &[CopyPosition],
    open_orders: &[CopyOrder],
    wallet_exposure: Decimal,
    total_exposure: Decimal,
) -> Option<CopySkipReason> {
    // Wallet-level gate (not used here because we don't have the TrackedWallet
    // in scope — the service checks wallet_paused before calling this).

    // 1. Minimum source trade size
    if source_trade.usd_size < config.min_source_trade_usd {
        return Some(CopySkipReason::BelowMinSize);
    }

    // 2. Price range
    if source_trade.price < config.min_price || source_trade.price > config.max_price {
        return Some(CopySkipReason::PriceOutOfRange);
    }

    // 3. Sells gating
    if source_trade.side == CopyOrderSide::Sell && !config.copy_sells {
        return Some(CopySkipReason::CopySellsDisabled);
    }

    // 4. Max open copy orders
    let open_count = open_orders.iter().filter(|o| o.status.is_open_like()).count();
    if open_count >= usize::from(config.max_open_copy_orders) {
        return Some(CopySkipReason::MaxOrdersReached);
    }

    // 5. Per-market position cap
    let market_position: Decimal = positions
        .iter()
        .filter(|p| p.token_id == source_trade.token_id)
        .map(|p| p.size * p.avg_price)
        .sum();
    let market_order_pending: Decimal = open_orders
        .iter()
        .filter(|o| o.token_id == source_trade.token_id && o.status.is_open_like())
        .map(|o| o.remaining_size() * o.price)
        .sum();
    if market_position + market_order_pending >= config.max_position_per_market_usd {
        return Some(CopySkipReason::PositionCapExceeded);
    }

    // 6. Per-wallet exposure cap
    if wallet_exposure >= config.per_wallet_max_exposure_usd {
        return Some(CopySkipReason::WalletExposureCapExceeded);
    }

    // 7. Total exposure cap
    if total_exposure >= config.max_total_exposure_usd {
        return Some(CopySkipReason::TotalExposureCapExceeded);
    }

    // 8. Daily loss limit (uses daily_realized_pnl which resets at UTC date boundary)
    if account.daily_realized_pnl < -config.daily_loss_limit_usd {
        return Some(CopySkipReason::DailyLossLimit);
    }

    None
}
