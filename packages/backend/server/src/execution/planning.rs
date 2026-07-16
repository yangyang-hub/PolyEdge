pub fn stable_idempotency_key(
    wallet_id: i64,
    strategy_version_id: i64,
    slot_id: i64,
    generation: i64,
    action: ActionKind,
) -> String {
    format!(
        "wallet:{wallet_id}:version:{strategy_version_id}:slot:{slot_id}:generation:{generation}:action:{}",
        action.as_str()
    )
}

pub fn build_target(
    slot: &StrategyQuoteSlot,
    token_id: &str,
    book: &CachedOrderBook,
    now: OffsetDateTime,
    freshness_ms: i64,
) -> Result<Option<TargetOrder>> {
    if !book.is_fresh_at(now, freshness_ms) {
        return Ok(None);
    }
    let price = match slot.pricing_mode {
        QuotePricingMode::Fixed => slot.fixed_price.ok_or_else(|| {
            AppError::conflict(
                "EXECUTION_FIXED_PRICE_MISSING",
                "fixed slot has no fixed_price",
            )
        })?,
        QuotePricingMode::BookRank => {
            let rank = slot.book_rank.ok_or_else(|| {
                AppError::conflict(
                    "EXECUTION_BOOK_RANK_MISSING",
                    "book-rank slot has no book_rank",
                )
            })?;
            if rank <= 0 {
                return Err(AppError::invalid_input(
                    "EXECUTION_BOOK_RANK_INVALID",
                    "book rank must be positive",
                ));
            }
            let index = usize::try_from(rank - 1).map_err(|_| {
                AppError::invalid_input(
                    "EXECUTION_BOOK_RANK_INVALID",
                    "book rank does not fit usize",
                )
            })?;
            let Some(level) = book.bids.get(index) else {
                return Ok(None);
            };
            level.price + slot.price_offset
        }
    };
    let rounded_price = price.round_dp(2);
    if rounded_price < slot.minimum_price
        || rounded_price > slot.maximum_price
        || slot.quantity <= Decimal::ZERO
    {
        return Ok(None);
    }
    if slot.post_only && book.best_ask().is_some_and(|ask| rounded_price >= ask) {
        return Ok(None);
    }
    Ok(Some(TargetOrder {
        slot: slot.clone(),
        token_id: token_id.to_string(),
        price: rounded_price,
        quantity: slot.quantity,
        post_only: slot.post_only,
    }))
}

pub fn compare_target(
    target: &TargetOrder,
    actual: Option<&ManagedOrder>,
    now: OffsetDateTime,
    version: &StrategyVersion,
) -> ReconcileDecision {
    let Some(actual) = actual else {
        return ReconcileDecision {
            action: DesiredAction::Place,
            reason: "MISSING_ORDER",
        };
    };
    if actual.status == polyedge_domain::ManagedOrderStatus::Unknown {
        return ReconcileDecision {
            action: DesiredAction::Blocked,
            reason: "ORDER_UNKNOWN",
        };
    }
    if actual.price.round_dp(2) == target.price.round_dp(2) && actual.quantity == target.quantity {
        return ReconcileDecision {
            action: DesiredAction::Keep,
            reason: "TARGET_MATCH",
        };
    }
    let elapsed = (now - actual.updated_at).whole_milliseconds();
    let required = if target.price < actual.price {
        version.downward_reprice_confirm_ms
    } else {
        version.upward_reprice_confirm_ms
    };
    if elapsed < i128::from(required) || elapsed < i128::from(version.reprice_cooldown_ms) {
        return ReconcileDecision {
            action: DesiredAction::Keep,
            reason: "REPRICE_COOLDOWN",
        };
    }
    ReconcileDecision {
        action: DesiredAction::Replace,
        reason: "TARGET_CHANGED",
    }
}

fn risk_snapshot(context: &ExecutionContext) -> RiskSnapshot {
    RiskSnapshot {
        open_orders: context
            .managed_orders
            .iter()
            .filter(|order| {
                matches!(
                    order.status,
                    polyedge_domain::ManagedOrderStatus::Planned
                        | polyedge_domain::ManagedOrderStatus::Submitting
                        | polyedge_domain::ManagedOrderStatus::Open
                        | polyedge_domain::ManagedOrderStatus::PartiallyFilled
                        | polyedge_domain::ManagedOrderStatus::CancelPending
                        | polyedge_domain::ManagedOrderStatus::Unknown
                )
            })
            .count() as i64,
        open_buy_notional: context.account_state.open_buy_notional,
        market_position_notional: context.market_position_notional,
        total_position_notional: context.account_state.total_position_notional,
        available_collateral: context.account_state.available_collateral,
    }
}

fn ensure_risk_budget_snapshot(
    policy: &WalletRiskPolicy,
    snapshot: &RiskSnapshot,
    target: &TargetOrder,
) -> Result<()> {
    let notional = target.price * target.quantity;
    if snapshot.open_orders >= policy.max_open_orders {
        return Err(AppError::conflict(
            "EXECUTION_MAX_OPEN_ORDERS",
            "wallet open-order limit reached",
        ));
    }
    if notional > policy.max_order_notional || notional > policy.max_open_buy_notional {
        return Err(AppError::conflict(
            "EXECUTION_ORDER_NOTIONAL_LIMIT",
            "target order exceeds wallet notional limit",
        ));
    }
    if snapshot.open_buy_notional + notional > policy.max_open_buy_notional {
        return Err(AppError::conflict(
            "EXECUTION_OPEN_BUY_LIMIT",
            "target order exceeds wallet open-buy budget",
        ));
    }
    if snapshot.market_position_notional + notional > policy.max_market_position_notional {
        return Err(AppError::conflict(
            "EXECUTION_MARKET_POSITION_LIMIT",
            "target order exceeds wallet market-position budget",
        ));
    }
    if snapshot.total_position_notional + notional > policy.max_total_position_notional {
        return Err(AppError::conflict(
            "EXECUTION_POSITION_LIMIT",
            "target order exceeds wallet position budget",
        ));
    }
    if snapshot.available_collateral < notional {
        return Err(AppError::conflict(
            "EXECUTION_COLLATERAL_INSUFFICIENT",
            "wallet available collateral is insufficient",
        ));
    }
    Ok(())
}

fn apply_place_to_risk(snapshot: &mut RiskSnapshot, target: &TargetOrder) {
    let notional = target.price * target.quantity;
    snapshot.open_orders += 1;
    snapshot.open_buy_notional += notional;
    snapshot.available_collateral -= notional;
}

fn apply_cancel_to_risk(snapshot: &mut RiskSnapshot, order: &ManagedOrder) {
    if order.status == polyedge_domain::ManagedOrderStatus::Filled
        || order.status == polyedge_domain::ManagedOrderStatus::Cancelled
    {
        return;
    }
    let notional = order.price * (order.quantity - order.filled_quantity).max(Decimal::ZERO);
    snapshot.open_orders = snapshot.open_orders.saturating_sub(1);
    snapshot.open_buy_notional = (snapshot.open_buy_notional - notional).max(Decimal::ZERO);
    snapshot.available_collateral += notional;
}
