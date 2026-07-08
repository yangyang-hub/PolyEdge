const LIVE_CANCEL_RETRY_MIN_INTERVAL: TimeDuration = TimeDuration::seconds(15);
const LIVE_CANCEL_FINAL_RECONCILIATION_RETRY_AFTER: TimeDuration = TimeDuration::seconds(30);
include!("live_placement_limits.rs");
include!("live_cancel.rs");

#[allow(clippy::needless_range_loop, clippy::too_many_arguments)]
fn live_placement_orders(
    config: &RewardBotConfig,
    account: &RewardAccountState,
    plans: &mut [RewardQuotePlan],
    books: &HashMap<String, RewardOrderBook>,
    book_history: &HashMap<String, VecDeque<BookSnapshot>>,
    open_orders: &[ManagedRewardOrder],
    positions: &[RewardPosition],
    kill_switch: bool,
    trace_id: &str,
) -> (Vec<ManagedRewardOrder>, bool) {
    if (config.max_markets == 0 || config.max_open_orders == 0)
        && (!config.balanced_merge_enabled
            || config.balanced_merge_max_markets == 0
            || config.balanced_merge_max_open_orders == 0)
    {
        return (Vec::new(), false);
    }

    let mut active_markets_by_profile: HashMap<RewardStrategyProfile, HashSet<String>> =
        HashMap::new();
    for order in open_orders
        .iter()
        .filter(|order| order.status.is_open_like())
    {
        active_markets_by_profile
            .entry(order.strategy_profile)
            .or_default()
            .insert(order.condition_id.clone());
    }
    let mut orders = open_orders.to_vec();
    let mut placements = Vec::new();
    let mut plans_changed = false;
    let mut seq = 0usize;
    let available_for_new_condition = live_available_usd_after_unmanaged_external_buys(account);

    for plan_index in 0..plans.len() {
        if !plans[plan_index].eligible {
            continue;
        }
        if plans[plan_index].strategy_bucket != RewardStrategyBucket::Standard {
            plans[plan_index].strategy_bucket = RewardStrategyBucket::Standard;
            plans_changed = true;
        }
        let plan_config = config
            .config_for_strategy_bucket(plans[plan_index].strategy_bucket)
            .config_for_strategy_profile(plans[plan_index].strategy_profile);
        let max_markets = usize::from(plan_config.max_markets);
        let max_open_orders = usize::from(plan_config.max_open_orders);
        if max_markets == 0 || max_open_orders == 0 {
            continue;
        }
        let materialized = match materialize_reward_quote_plan_for_live_orderbook(
            &plans[plan_index],
            books,
            &plan_config,
        ) {
            Ok(materialized) => materialized,
            Err(reason) => {
                let now = OffsetDateTime::now_utc();
                if let Some(wait_reason) =
                    live_orderbook_wait_reason(&plan_config, &plans[plan_index], books, now)
                {
                    if mark_live_orderbook_waiting(&mut plans[plan_index], wait_reason, now) {
                        plans_changed = true;
                    }
                } else {
                    mark_live_orderbook_validation_skip(&mut plans[plan_index], reason, now);
                    plans_changed = true;
                }
                continue;
            }
        };
        if apply_live_quote_plan_materialization(
            &mut plans[plan_index],
            materialized,
            OffsetDateTime::now_utc(),
        ) {
            plans_changed = true;
        }
        if reward_quote_plan_event_window_blocks_new_buy(&plans[plan_index]) {
            if mark_live_event_window_new_buy_blocked(
                &mut plans[plan_index],
                OffsetDateTime::now_utc(),
            ) {
                plans_changed = true;
            }
            continue;
        }
        let now = OffsetDateTime::now_utc();
        if let Some(wait_reason) =
            live_orderbook_placement_wait_reason(&plan_config, &plans[plan_index].legs, books, now)
        {
            if mark_live_orderbook_waiting(&mut plans[plan_index], wait_reason, now) {
                plans_changed = true;
            }
            continue;
        }
        let plan = &plans[plan_index];
        let active_profile_markets = active_markets_by_profile
            .entry(plan.strategy_profile)
            .or_default();
        if active_profile_markets.len() >= max_markets
            && !active_profile_markets.contains(&plan.condition_id)
        {
            continue;
        }
        let existing_market_buy_notional = live_market_buy_notional(&orders, &plan.condition_id);
        let raw_budget =
            (available_for_new_condition - existing_market_buy_notional).max(Decimal::ZERO);
        // Cap the condition budget by per-leg position limits so rescaled legs
        // do not exceed max_position_usd when both are open simultaneously.
        let position_budget = live_condition_budget_capped_by_positions(
            &plan_config,
            &plan.legs,
            positions,
            raw_budget,
        );
        let condition_budget = live_condition_budget_capped_by_ai_hint(
            config,
            plan,
            existing_market_buy_notional,
            position_budget,
        );
        // Rescale legs to use available balance: single-side uses all,
        // double-side splits 50/50. Plan legs stay at minimum size for
        // snapshot and price-drift detection.
        let rescaled_legs = live_rescaled_quote_legs_for_budget(plan, condition_budget);
        let missing_plan_buy_notional =
            live_missing_plan_buy_notional(&rescaled_legs, &orders, &plan.condition_id);
        let projected_condition_buy_notional =
            existing_market_buy_notional + missing_plan_buy_notional;
        if let Some(max_condition_notional) = live_ai_hint_condition_notional_cap_exceeded(
            config,
            plan,
            projected_condition_buy_notional,
        ) {
            if missing_plan_buy_notional > Decimal::ZERO
                && !live_condition_has_active_exposure(&plan.condition_id, open_orders, positions)
                && mark_live_ai_notional_cap_skip(
                    &mut plans[plan_index],
                    existing_market_buy_notional,
                    missing_plan_buy_notional,
                    max_condition_notional,
                    OffsetDateTime::now_utc(),
                )
            {
                plans_changed = true;
            }
            continue;
        }
        // Polymarket applies its collateral validity check to the sum of all
        // open BUY orders in the same condition. Different conditions may reuse
        // the same collateral, but both YES/NO legs in one condition must fit.
        // Open BUY orders outside this system are only available as an account
        // aggregate, so reserve the unassigned portion conservatively.
        if projected_condition_buy_notional > available_for_new_condition {
            if missing_plan_buy_notional > Decimal::ZERO
                && mark_live_funding_skip(
                    &mut plans[plan_index],
                    existing_market_buy_notional,
                    missing_plan_buy_notional,
                    available_for_new_condition,
                    OffsetDateTime::now_utc(),
                )
            {
                plans_changed = true;
            }
            continue;
        }
        for leg in &rescaled_legs {
            if orders
                .iter()
                .filter(|order| {
                    order.status.is_open_like() && order.strategy_profile == plan.strategy_profile
                })
                .count()
                >= max_open_orders
            {
                continue;
            }
            if orders.iter().any(|order| {
                order.condition_id == plan.condition_id
                    && order.token_id == leg.token_id
                    && order.side == RewardOrderSide::Buy
                    && order.status.is_open_like()
            }) {
                continue;
            }
            if orders.iter().any(|order| {
                order.token_id == leg.token_id
                    && order.side == RewardOrderSide::Sell
                    && order.status.is_open_like()
            }) {
                continue;
            }
            let notional = (leg.price * leg.size).round_dp(4);
            if notional <= Decimal::ZERO {
                continue;
            }
            if live_position_over_cap(&plan_config, positions, &leg.token_id, leg.price, notional) {
                continue;
            }
            // Live maker buys intentionally do not reserve global cash until a
            // fill is observed. This cap applies to actual inventory only, so
            // the same funds can be quoted across markets while orders rest.
            if config.max_global_position_usd > Decimal::ZERO
                && live_global_inventory_notional(positions) + notional
                    > config.max_global_position_usd
            {
                continue;
            }
            seq += 1;
            let now = OffsetDateTime::now_utc();
            let order = ManagedRewardOrder {
                id: format!(
                    "rewlive_{}_{}_{}",
                    now.unix_timestamp_nanos(),
                    seq,
                    trace_id.trim_start_matches("trc_")
                ),
                account_id: account.account_id.clone(),
                condition_id: plan.condition_id.clone(),
                token_id: leg.token_id.clone(),
                outcome: leg.outcome.clone(),
                side: RewardOrderSide::Buy,
                price: leg.price,
                size: leg.size,
                strategy_bucket: plan.strategy_bucket,
                strategy_profile: plan.strategy_profile,
                exit_strategy_source: RewardExitStrategySource::Configured,
                exit_strategy_selected: None,
                exit_floor_price: None,
                exit_reselect_count: 0,
                exit_last_reselected_at: None,
                external_order_id: None,
                status: ManagedRewardOrderStatus::Planned,
                scoring: false,
                reason: "pending live post-only rewards quote".to_string(),
                filled_size: Decimal::ZERO,
                reward_earned: Decimal::ZERO,
                last_scored_at: None,
                created_at: now,
                updated_at: now,
            };
            let mut single_plan_index = HashMap::new();
            single_plan_index.insert(plan.condition_id.as_str(), plan);
            if live_cancel_reason(
                &plan_config,
                &single_plan_index,
                books,
                book_history,
                &orders,
                account,
                &order,
                now,
                kill_switch,
            )
            .is_some()
            {
                continue;
            }
            active_markets_by_profile
                .entry(plan.strategy_profile)
                .or_default()
                .insert(plan.condition_id.clone());
            orders.push(order.clone());
            placements.push(order);
        }
    }

    (placements, plans_changed)
}

fn mark_live_orderbook_validation_skip(
    plan: &mut RewardQuotePlan,
    reason: String,
    now: OffsetDateTime,
) {
    let skip_until = now + LIVE_ORDERBOOK_VALIDATION_SKIP_TTL;
    plan.eligible = false;
    plan.quote_mode = RewardPlanQuoteMode::None;
    plan.reason = format!("live orderbook validation skipped until {skip_until}: {reason}");
    plan.live_skip_until = Some(skip_until);
    plan.live_skip_reason = Some(reason);
    plan.updated_at = now;
}

fn apply_live_quote_plan_materialization(
    plan: &mut RewardQuotePlan,
    materialized: RewardLiveQuoteMaterialization,
    now: OffsetDateTime,
) -> bool {
    let changed = plan.quote_mode != materialized.quote_mode
        || plan.recommended_quote_mode != materialized.recommended_quote_mode
        || plan.book_metrics != materialized.book_metrics
        || plan.midpoint != Some(materialized.midpoint)
        || plan.legs != materialized.legs
        || !plan.eligible
        || plan.reason
            != format!(
                "eligible for live post-only {} quotes",
                materialized.quote_mode.as_str()
            )
        || plan.live_skip_until.is_some()
        || plan.live_skip_reason.is_some();

    if changed {
        plan.quote_mode = materialized.quote_mode;
        plan.recommended_quote_mode = materialized.recommended_quote_mode;
        plan.book_metrics = materialized.book_metrics;
        plan.midpoint = Some(materialized.midpoint);
        plan.legs = materialized.legs;
        plan.eligible = true;
        plan.reason = format!(
            "eligible for live post-only {} quotes",
            plan.quote_mode.as_str()
        );
        plan.live_skip_until = None;
        plan.live_skip_reason = None;
        plan.updated_at = now;
    }

    changed
}

fn mark_live_event_window_new_buy_blocked(plan: &mut RewardQuotePlan, now: OffsetDateTime) -> bool {
    let reason = plan
        .event_window
        .as_ref()
        .map(|assessment| format!("event window blocks new BUY quotes: {}", assessment.reason))
        .unwrap_or_else(|| "event window blocks new BUY quotes".to_string());
    let changed = plan.reason != reason || plan.quote_readiness != RewardQuoteReadiness::Blocked;
    if changed {
        plan.reason = reason;
        plan.quote_readiness = RewardQuoteReadiness::Blocked;
        plan.updated_at = now;
    }
    changed
}

fn mark_live_orderbook_waiting(
    plan: &mut RewardQuotePlan,
    reason: String,
    now: OffsetDateTime,
) -> bool {
    let changed = !plan.eligible
        || plan.reason != reason
        || plan.live_skip_until.is_some()
        || plan.live_skip_reason.is_some();
    if changed {
        plan.eligible = true;
        plan.reason = reason;
        plan.live_skip_until = None;
        plan.live_skip_reason = None;
        plan.updated_at = now;
    }
    changed
}

fn live_min_depth_cancel_reason(
    config: &RewardBotConfig,
    books: &HashMap<String, RewardOrderBook>,
    order: &ManagedRewardOrder,
) -> Option<String> {
    if config.min_depth_usd <= Decimal::ZERO || order.side != RewardOrderSide::Buy {
        return None;
    }
    let book = books.get(&order.token_id)?;
    let aggregate_depth_usd: Decimal = book
        .bids
        .iter()
        .filter(|level| level.price >= order.price)
        .map(|level| (level.price * level.size).round_dp(4))
        .sum();
    let own_remaining_notional = if order.external_order_id.is_some() {
        (order.price * (order.size - order.filled_size).max(Decimal::ZERO)).round_dp(4)
    } else {
        Decimal::ZERO
    };
    let depth_usd = (aggregate_depth_usd - own_remaining_notional).max(Decimal::ZERO);
    if depth_usd < config.min_depth_usd {
        Some(format!(
            "external bid depth {depth_usd} at or above order price {} is below minimum {}",
            order.price, config.min_depth_usd
        ))
    } else {
        None
    }
}

fn live_token_spread_cancel_reason(
    config: &RewardBotConfig,
    books: &HashMap<String, RewardOrderBook>,
    order: &ManagedRewardOrder,
) -> Option<String> {
    if config.max_market_spread_cents <= Decimal::ZERO || order.side != RewardOrderSide::Buy {
        return None;
    }
    let book = books.get(&order.token_id)?;
    let best_bid = book.bids.first()?.price;
    let best_ask = book.asks.first()?.price;
    if best_bid <= Decimal::ZERO || best_ask <= best_bid {
        return None;
    }
    let spread_cents = ((best_ask - best_bid) * Decimal::from(100_u64)).round_dp(4);
    if spread_cents > config.max_market_spread_cents {
        Some(format!(
            "live token spread {spread_cents}c exceeds max market spread {}c",
            config.max_market_spread_cents
        ))
    } else {
        None
    }
}

fn live_bid_rank_cancel_reason(
    config: &RewardBotConfig,
    books: &HashMap<String, RewardOrderBook>,
    order: &ManagedRewardOrder,
) -> Option<String> {
    if config.cancel_bid_rank == 0 || order.side != RewardOrderSide::Buy {
        return None;
    }
    if order.strategy_profile == RewardStrategyProfile::BalancedMerge {
        return None;
    }
    let book = books.get(&order.token_id)?;
    let rank = live_bid_rank(order.price, book)?;
    if rank <= usize::from(config.cancel_bid_rank) {
        Some(format!(
            "order promoted to bid-{rank} (cancel threshold: bid-{})",
            config.cancel_bid_rank
        ))
    } else {
        None
    }
}

fn live_bid_rank(price: Decimal, book: &RewardOrderBook) -> Option<usize> {
    let mut seen_prices = Vec::new();
    for level in &book.bids {
        if seen_prices.last() != Some(&level.price) {
            seen_prices.push(level.price);
        }
    }
    if seen_prices.is_empty() {
        return None;
    }
    Some(
        seen_prices
            .iter()
            .filter(|level_price| **level_price > price)
            .count()
            + 1,
    )
}

fn live_depth_drop_cancel_reason(
    config: &RewardBotConfig,
    books: &HashMap<String, RewardOrderBook>,
    book_history: &HashMap<String, VecDeque<BookSnapshot>>,
    order: &ManagedRewardOrder,
    now: OffsetDateTime,
) -> Option<String> {
    if config.depth_drop_pct <= Decimal::ZERO || order.side != RewardOrderSide::Buy {
        return None;
    }
    let current = books.get(&order.token_id)?;
    let old = live_snapshot_ago(
        book_history.get(&order.token_id)?,
        config.depth_drop_window_sec,
        now,
    )?;
    let old_notional = live_top_bid_notional_snapshot(old, 2);
    if old_notional <= Decimal::ZERO {
        return None;
    }
    let current_notional = live_top_bid_notional(current, 2);
    let drop_pct =
        ((old_notional - current_notional) / old_notional * Decimal::from(100_u64)).round_dp(2);
    if drop_pct >= config.depth_drop_pct {
        Some(format!(
            "top-2 bid depth dropped {drop_pct}% in {}s (threshold {}%)",
            config.depth_drop_window_sec, config.depth_drop_pct
        ))
    } else {
        None
    }
}

fn live_fill_velocity_cancel_reason(
    config: &RewardBotConfig,
    books: &HashMap<String, RewardOrderBook>,
    book_history: &HashMap<String, VecDeque<BookSnapshot>>,
    order: &ManagedRewardOrder,
    now: OffsetDateTime,
) -> Option<String> {
    if config.fill_velocity_usd <= Decimal::ZERO || order.side != RewardOrderSide::Buy {
        return None;
    }
    let current = books.get(&order.token_id)?;
    let old = live_snapshot_ago(
        book_history.get(&order.token_id)?,
        config.fill_velocity_window_sec,
        now,
    )?;
    let old_ask = live_top_ask_notional_snapshot(old, 2);
    let current_ask = live_top_ask_notional(current, 2);
    if old_ask <= current_ask {
        return None;
    }
    let decrease = old_ask - current_ask;
    if decrease >= config.fill_velocity_usd {
        Some(format!(
            "ask depth decreased ${decrease} in {}s (threshold ${})",
            config.fill_velocity_window_sec, config.fill_velocity_usd
        ))
    } else {
        None
    }
}

fn live_mass_cancel_reason(
    config: &RewardBotConfig,
    books: &HashMap<String, RewardOrderBook>,
    book_history: &HashMap<String, VecDeque<BookSnapshot>>,
    order: &ManagedRewardOrder,
    now: OffsetDateTime,
) -> Option<String> {
    if config.mass_cancel_pct <= Decimal::ZERO || order.side != RewardOrderSide::Buy {
        return None;
    }
    let current = books.get(&order.token_id)?;
    let old = live_snapshot_ago(
        book_history.get(&order.token_id)?,
        config.mass_cancel_window_sec,
        now,
    )?;
    let old_total: Decimal = old
        .bids
        .iter()
        .map(|level| (level.price * level.size).round_dp(4))
        .sum();
    if old_total <= Decimal::ZERO {
        return None;
    }
    let current_total: Decimal = current
        .bids
        .iter()
        .map(|level| (level.price * level.size).round_dp(4))
        .sum();
    let drop_pct = ((old_total - current_total) / old_total * Decimal::from(100_u64)).round_dp(2);
    if drop_pct >= config.mass_cancel_pct {
        Some(format!(
            "total bid depth dropped {drop_pct}% in {}s (threshold {}%)",
            config.mass_cancel_window_sec, config.mass_cancel_pct
        ))
    } else {
        None
    }
}

fn live_requote_age_cancel_reason(
    config: &RewardBotConfig,
    order: &ManagedRewardOrder,
    now: OffsetDateTime,
) -> Option<String> {
    if config.requote_interval_sec == 0 || order.side != RewardOrderSide::Buy {
        return None;
    }
    let age_sec = (now - order.created_at).whole_seconds().max(0) as u64;
    let threshold = config.requote_interval_sec
        + deterministic_reward_jitter(&order.id, config.requote_jitter_sec);
    if age_sec >= threshold {
        Some(format!(
            "order age {age_sec}s exceeds requote interval {threshold}s"
        ))
    } else {
        None
    }
}

fn live_snapshot_ago(
    history: &VecDeque<BookSnapshot>,
    window_sec: u64,
    now: OffsetDateTime,
) -> Option<&BookSnapshot> {
    if window_sec == 0 {
        return None;
    }
    let target = now - TimeDuration::seconds(window_sec as i64);
    history
        .iter()
        .rev()
        .find(|snapshot| snapshot.observed_at <= target)
}

fn live_top_bid_notional(book: &RewardOrderBook, depth: usize) -> Decimal {
    book.bids
        .iter()
        .take(depth)
        .map(|level| (level.price * level.size).round_dp(4))
        .sum()
}

fn live_top_ask_notional(book: &RewardOrderBook, depth: usize) -> Decimal {
    book.asks
        .iter()
        .take(depth)
        .map(|level| (level.price * level.size).round_dp(4))
        .sum()
}

fn live_top_bid_notional_snapshot(snapshot: &BookSnapshot, depth: usize) -> Decimal {
    snapshot
        .bids
        .iter()
        .take(depth)
        .map(|level| (level.price * level.size).round_dp(4))
        .sum()
}

fn live_top_ask_notional_snapshot(snapshot: &BookSnapshot, depth: usize) -> Decimal {
    snapshot
        .asks
        .iter()
        .take(depth)
        .map(|level| (level.price * level.size).round_dp(4))
        .sum()
}

fn deterministic_reward_jitter(input: &str, max_jitter: u64) -> u64 {
    if max_jitter == 0 {
        return 0;
    }
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in input.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash % (max_jitter + 1)
}

fn live_position_over_cap(
    config: &RewardBotConfig,
    positions: &[RewardPosition],
    token_id: &str,
    price: Decimal,
    candidate_notional: Decimal,
) -> bool {
    if config.max_position_usd <= Decimal::ZERO {
        return false;
    }
    let current = positions
        .iter()
        .find(|position| position.token_id == token_id && position.size > Decimal::ZERO)
        .map(|position| position.size * price)
        .unwrap_or_default();
    current + candidate_notional > config.max_position_usd
}

fn live_global_inventory_notional(positions: &[RewardPosition]) -> Decimal {
    positions
        .iter()
        .filter(|position| position.size > Decimal::ZERO)
        .map(|position| position.size * position.avg_price)
        .sum()
}

fn reward_side_to_polymarket(side: RewardOrderSide) -> PolymarketTokenOrderSide {
    match side {
        RewardOrderSide::Buy => PolymarketTokenOrderSide::Buy,
        RewardOrderSide::Sell => PolymarketTokenOrderSide::Sell,
    }
}

fn reward_live_event(
    order: &ManagedRewardOrder,
    event_type: &str,
    severity: RewardRiskSeverity,
    message: impl Into<String>,
    metadata: serde_json::Value,
) -> RewardRiskEvent {
    new_risk_event(
        Some(order.account_id.clone()),
        Some(order.condition_id.clone()),
        order.external_order_id.clone(),
        event_type,
        severity,
        message,
        metadata,
    )
}
