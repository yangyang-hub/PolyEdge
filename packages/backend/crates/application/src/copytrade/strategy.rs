/// Compute the copy size for a source trade based on the configured sizing mode.
/// Pure function — no I/O. Called only after `check_skip_reasons` returns `None`.
pub fn compute_copy_size(
    config: &CopyTradeConfig,
    source_trade: &SourceTrade,
    source_position: Option<&WalletPositionInput>,
    account: &CopyAccountState,
    our_position: Option<&CopyPosition>,
) -> CopyDecision {
    let sizing = config.sizing_mode;

    let target_usd = match sizing {
        CopySizingMode::FixedUsd => {
            // Fixed notional per trade, bounded by available cash.
            config.fixed_usd_per_trade.min(account.available_usd)
        }
        CopySizingMode::ProportionalToSource => {
            // Our size = source size × factor, bounded by available cash.
            let target = source_trade.usd_size * config.proportional_factor;
            target.min(account.available_usd)
        }
        CopySizingMode::CapitalRatio => {
            // Allocate a fixed fraction of our capital per trade.
            let target = account.capital_usd * config.capital_ratio;
            target.min(account.available_usd)
        }
        CopySizingMode::MirrorPortfolioWeight => {
            // Mirror the source wallet's portfolio weight in this market.
            // Approximate: source portfolio value ≈ sum of their position
            // cur_prices × sizes + available (which we don't have). Instead,
            // use the position's cost basis as a proxy for their total
            // allocation and compute our target proportionally.
            let Some(source_pos) = source_position else {
                // No source position data — fall back to a small allocation.
                let fallback_size =
                    config.fixed_usd_per_trade.min(account.available_usd) / source_trade.price.max(Decimal::from_str_exact("0.01").expect("valid"));
                return CopyDecision {
                    copy: true,
                    reason: "mirror_weight_no_source_position_fallback".into(),
                    size: fallback_size,
                    price: source_trade.price,
                };
            };
            let source_cost = source_pos.size * source_pos.avg_price;
            let source_portfolio_approx = source_cost.max(Decimal::ONE);
            let weight = source_cost / source_portfolio_approx;
            let target_usd = account.capital_usd * weight;
            target_usd.min(account.available_usd)
        }
    };

    if target_usd <= Decimal::ZERO {
        return CopyDecision {
            copy: false,
            reason: "no_available_capital".into(),
            size: Decimal::ZERO,
            price: source_trade.price,
        };
    }

    // Convert USD target to token size.
    let price = source_trade.price.max(decimal("0.0001"));
    let mut size = target_usd / price;

    // Reduce by already-held position if copying the same direction.
    if let Some(our_pos) = our_position {
        if source_trade.side == CopyOrderSide::Buy && our_pos.size > Decimal::ZERO {
            // Already long; reduce to keep total position within the target.
            let already_invested = our_pos.size * our_pos.avg_price;
            let remaining_target = (target_usd - already_invested).max(Decimal::ZERO);
            size = remaining_target / price;
        }
        if source_trade.side == CopyOrderSide::Sell {
            // Can only sell what we hold.
            size = size.min(our_pos.size);
        }
    }

    if size <= Decimal::ZERO {
        return CopyDecision {
            copy: false,
            reason: format!("sizing_mode_{sizing:?}_results_in_zero_size"),
            size: Decimal::ZERO,
            price: source_trade.price,
        };
    }

    CopyDecision {
        copy: true,
        reason: format!("sizing_mode_{}", sizing.as_str()),
        size,
        price: source_trade.price,
    }
}
