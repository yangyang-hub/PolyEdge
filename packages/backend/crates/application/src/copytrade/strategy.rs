/// Compute the copy size for a source trade based on the configured sizing mode.
/// Pure function — no I/O. Called only after `check_skip_reasons` returns `None`.
///
/// `source_portfolio_usd` is the source wallet's total portfolio value (sum of
/// position market values), used by `MirrorPortfolioWeight`. Pass `Decimal::ZERO`
/// when it is unknown (the mode then falls back to a small fixed allocation).
pub fn compute_copy_size(
    config: &CopyTradeConfig,
    source_trade: &SourceTrade,
    source_position: Option<&WalletPositionInput>,
    source_portfolio_usd: Decimal,
    account: &CopyAccountState,
    our_position: Option<&CopyPosition>,
) -> CopyDecision {
    let sizing = config.sizing_mode;

    // Never sell what we do not hold locally — otherwise the simulation ledger
    // would mint phantom proceeds for a position that was never funded.
    if source_trade.side == CopyOrderSide::Sell
        && our_position.is_none_or(|pos| pos.size <= Decimal::ZERO)
    {
        return CopyDecision {
            copy: false,
            reason: "no_local_position_to_sell".into(),
            size: Decimal::ZERO,
            price: source_trade.price,
        };
    }

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
            // Mirror the weight this market holds inside the source wallet's
            // portfolio: weight = market_value / total_portfolio_value, then
            // target = our_capital × weight.
            let Some(source_pos) = source_position else {
                // No source position data — fall back to a small allocation.
                let fallback_size = config.fixed_usd_per_trade.min(account.available_usd)
                    / source_trade.price.max(Decimal::from_str_exact("0.01").expect("valid"));
                return CopyDecision {
                    copy: true,
                    reason: "mirror_weight_no_source_position_fallback".into(),
                    size: fallback_size,
                    price: source_trade.price,
                };
            };
            // Use live price when available, else cost basis, to value the leg.
            let mark_price = if source_pos.cur_price > Decimal::ZERO {
                source_pos.cur_price
            } else {
                source_pos.avg_price
            };
            let market_value = source_pos.size * mark_price;
            if source_portfolio_usd <= Decimal::ZERO || market_value <= Decimal::ZERO {
                // Unknown portfolio total — fall back to a small allocation
                // rather than the degenerate weight=1 (all-in) result.
                let fallback_size = config.fixed_usd_per_trade.min(account.available_usd)
                    / source_trade.price.max(Decimal::from_str_exact("0.01").expect("valid"));
                return CopyDecision {
                    copy: true,
                    reason: "mirror_weight_no_portfolio_total_fallback".into(),
                    size: fallback_size,
                    price: source_trade.price,
                };
            }
            let weight = (market_value / source_portfolio_usd).min(Decimal::ONE);
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
