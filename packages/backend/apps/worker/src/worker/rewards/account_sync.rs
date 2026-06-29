/// Refresh the externally authoritative Polymarket balance and position snapshot.
///
/// A failed CLOB balance request preserves the local balance unless the funding
/// wallet pUSD balance can be read from Polygon. A failed position request preserves
/// the stored positions; a successful position request, including an empty response,
/// atomically replaces all positions for the rewards account.
const REWARD_ACCOUNT_SYNC_FILL_GRACE: TimeDuration = TimeDuration::seconds(120);

#[derive(Debug, Clone, Copy)]
struct RewardAccountSyncPolicy {
    managed_scoring: bool,
    open_orders: bool,
    account_snapshot: bool,
    close_absent_buy_orders: bool,
}

impl RewardAccountSyncPolicy {
    fn full() -> Self {
        Self {
            managed_scoring: true,
            open_orders: true,
            account_snapshot: true,
            close_absent_buy_orders: true,
        }
    }

    fn any(self) -> bool {
        self.managed_scoring || self.open_orders || self.account_snapshot
    }
}

async fn sync_external_account_state(
    state: &AppState,
    connector: &LivePolymarketConnector,
    cycle_account: &mut RewardAccountState,
    cycle_positions: &mut Vec<RewardPosition>,
    cycle_orders: &mut Vec<ManagedRewardOrder>,
    trace_id: &str,
    refresh_account_snapshot: bool,
    close_absent_buy_orders: bool,
) {
    let mut policy = RewardAccountSyncPolicy::full();
    policy.close_absent_buy_orders = close_absent_buy_orders;
    sync_external_account_state_with_policy(
        state,
        connector,
        cycle_account,
        cycle_positions,
        cycle_orders,
        trace_id,
        refresh_account_snapshot,
        policy,
    )
    .await;
}

async fn sync_external_account_state_with_policy(
    state: &AppState,
    connector: &LivePolymarketConnector,
    cycle_account: &mut RewardAccountState,
    cycle_positions: &mut Vec<RewardPosition>,
    cycle_orders: &mut Vec<ManagedRewardOrder>,
    trace_id: &str,
    refresh_account_snapshot: bool,
    policy: RewardAccountSyncPolicy,
) {
    if policy.managed_scoring {
        sync_managed_reward_scoring(
            state,
            connector,
            cycle_account,
            cycle_orders,
            trace_id,
        )
        .await;
    }

    if policy.open_orders {
        sync_external_open_order_state(
            state,
            connector,
            cycle_account,
            cycle_orders,
            trace_id,
            policy.close_absent_buy_orders,
        )
        .await;
    }

    if !policy.account_snapshot
        || !refresh_account_snapshot
        || !external_account_sync_allowed(state, cycle_account, trace_id).await
    {
        return;
    }

    let mut synced_account = cycle_account.clone();
    synced_account.reserved_usd = Decimal::ZERO;
    let mut balance_updated = false;
    match connector.refresh_balance().await {
        Ok(balance) => {
            // CLOB balance-allowance returns the balance in 6-decimal fixed-point
            // units (e.g. 100 USDC → "100000000").  Convert to human-readable USD
            // so the value is consistent with the on-chain fallback path and the
            // database NUMERIC(18,4) column.
            synced_account.available_usd =
                (balance.balance / Decimal::from(1_000_000_u64)).round_dp(4);
            balance_updated = true;
        }
        Err(error) => {
            warn!(error = %error, "polymarket balance query failed");
        }
    }

    synced_account.external_buy_notional = cycle_account.external_buy_notional;

    let settings = &state.settings.polymarket;
    let wallet_address =
        polymarket_funding_wallet_address(&settings.account_id, settings.funder.as_deref());
    let wallet_address_updated = synced_account.wallet_address != wallet_address;
    if wallet_address_updated {
        synced_account.wallet_address.clone_from(&wallet_address);
    }

    if let Some(wallet_address) = wallet_address.as_deref() {
        match PolymarketChainConnector::new(&settings.polygon_rpc_url) {
            Ok(chain_connector) => match chain_connector.fetch_pusd_balance(wallet_address).await {
                Ok(wallet_balance)
                    if !balance_updated
                        || (synced_account.available_usd == Decimal::ZERO
                            && wallet_balance > Decimal::ZERO) =>
                {
                    if balance_updated {
                        warn!(
                            trace_id,
                            wallet_address,
                            clob_balance = %synced_account.available_usd,
                            wallet_balance = %wallet_balance,
                            "CLOB balance is zero while the funding wallet has pUSD; using on-chain balance for rewards snapshot"
                        );
                    }
                    synced_account.available_usd = wallet_balance;
                    balance_updated = true;
                }
                Ok(_) => {}
                Err(error) => {
                    warn!(
                        trace_id,
                        wallet_address,
                        error = %error,
                        "failed to query funding wallet pUSD balance"
                    );
                }
            },
            Err(error) => {
                warn!(
                    trace_id,
                    wallet_address,
                    error = %error,
                    "failed to create Polygon chain connector"
                );
            }
        }
    }

    let position_snapshot = match wallet_address.as_deref() {
        None => {
            warn!("Polymarket funding wallet not configured, skipping external position sync");
            None
        }
        Some(wallet_address) => match PolymarketDataApiConnector::new(&settings.data_api_host) {
            Ok(data_connector) => {
                match data_connector.fetch_wallet_positions(wallet_address).await {
                    Ok(raw_positions) => {
                        let mut positions = raw_positions
                            .into_iter()
                            .filter(|position| position.size > Decimal::ZERO)
                            .map(|position| {
                                polymarket_position_to_reward(
                                    &synced_account.account_id,
                                    &position,
                                )
                            })
                            .collect::<Vec<_>>();
                        positions.sort_by(|left, right| left.token_id.cmp(&right.token_id));
                        Some(positions)
                    }
                    Err(error) => {
                        warn!(error = %error, "failed to fetch live Polymarket positions during sync");
                        None
                    }
                }
            }
            Err(error) => {
                warn!(error = %error, "failed to create Polymarket Data API connector during sync");
                None
            }
        },
    };

    if !balance_updated && position_snapshot.is_none() && !wallet_address_updated {
        return;
    }

    synced_account.updated_at = OffsetDateTime::now_utc();
    match state
        .reward_bot_service
        .apply_account_sync(&synced_account, position_snapshot.as_deref(), trace_id)
        .await
    {
        Ok(()) => {
            *cycle_account = synced_account;
            if let Some(positions) = position_snapshot {
                *cycle_positions = positions;
            }
            plan_external_inventory_original_price_exits(
                state,
                cycle_account,
                cycle_positions,
                cycle_orders,
                trace_id,
            )
            .await;
        }
        Err(error) => {
            warn!(error = %error, "failed to persist external account sync outcome");
        }
    }
}

async fn plan_external_inventory_original_price_exits(
    state: &AppState,
    account: &mut RewardAccountState,
    positions: &[RewardPosition],
    cycle_orders: &mut Vec<ManagedRewardOrder>,
    trace_id: &str,
) {
    let planned = external_inventory_original_price_exit_updates(
        &account.account_id,
        positions,
        cycle_orders,
        trace_id,
    );
    if planned.is_empty() {
        return;
    }

    let (orders, events): (Vec<_>, Vec<_>) = planned.into_iter().unzip();
    match persist_live_reward_updates(
        state,
        account,
        Vec::new(), // positions were just persisted by apply_account_sync
        orders.clone(),
        Vec::new(),
        events,
        &RewardBotRunReport::default(),
        trace_id,
    )
    .await
    {
        Ok(()) => cycle_orders.extend(orders),
        Err(error) => {
            warn!(
                error = %error,
                "failed to persist external inventory original-price exit intents"
            );
        }
    }
}

fn external_inventory_original_price_exit_updates(
    account_id: &str,
    positions: &[RewardPosition],
    open_orders: &[ManagedRewardOrder],
    trace_id: &str,
) -> Vec<(ManagedRewardOrder, RewardRiskEvent)> {
    let mut covered_sell_tokens = open_orders
        .iter()
        .filter(|order| {
            order.account_id == account_id
                && order.side == RewardOrderSide::Sell
                && order.status.is_open_like()
        })
        .map(|order| order.token_id.clone())
        .collect::<HashSet<_>>();
    let mut updates = Vec::new();
    for position in positions {
        if position.account_id != account_id
            || position.token_id.trim().is_empty()
            || position.size <= Decimal::ZERO
            || position.avg_price <= Decimal::ZERO
            || !covered_sell_tokens.insert(position.token_id.clone())
        {
            continue;
        }
        let size = position
            .size
            .round_dp_with_strategy(2, RoundingStrategy::ToZero);
        if size <= Decimal::ZERO {
            continue;
        }
        let price = ceil_reward_price_to_tick(Decimal::min(
            Decimal::from_parts(99, 0, 0, false, 2),
            position.avg_price,
        ));
        if price <= Decimal::ZERO {
            continue;
        }
        let now = OffsetDateTime::now_utc();
        let sequence = updates.len();
        let order = ManagedRewardOrder {
            id: format!(
                "rewinvexit_{}_{}_{}",
                now.unix_timestamp_nanos(),
                sequence,
                trace_id.trim_start_matches("trc_")
            ),
            account_id: position.account_id.clone(),
            condition_id: position.condition_id.clone(),
            token_id: position.token_id.clone(),
            outcome: position.outcome.clone(),
            side: RewardOrderSide::Sell,
            price,
            size,
            strategy_bucket: RewardStrategyBucket::None,
            strategy_profile: RewardStrategyProfile::Standard,
            external_order_id: None,
            status: ManagedRewardOrderStatus::ExitPending,
            scoring: false,
            reason: "external inventory original-price exit".to_string(),
            filled_size: Decimal::ZERO,
            reward_earned: Decimal::ZERO,
            last_scored_at: None,
            created_at: now,
            updated_at: now,
        };
        let event = reward_live_event(
            &order,
            "reward_live_inventory_exit_planned",
            RewardRiskSeverity::Info,
            "planned original-price sell exit for detected rewards inventory",
            json!({
                "token_id": position.token_id,
                "size": size,
                "avg_price": position.avg_price,
                "price": price,
                "post_only": true,
                "trace_id": trace_id,
            }),
        );
        updates.push((order, event));
    }
    updates
}

async fn sync_external_open_order_state(
    state: &AppState,
    connector: &LivePolymarketConnector,
    account: &mut RewardAccountState,
    cycle_orders: &mut Vec<ManagedRewardOrder>,
    trace_id: &str,
    close_absent_buy_orders: bool,
) {
    let open_orders = match connector.list_open_orders().await {
        Ok(open_orders) => open_orders,
        Err(error) => {
            warn!(error = %error, "failed to list Polymarket open orders for managed order sync");
            return;
        }
    };
    let open_order_ids = open_orders
        .iter()
        .filter(|order| external_open_order_counts_as_active(order))
        .map(|order| order.id.as_str())
        .collect::<HashSet<_>>();

    let previous_external_buy_notional = account.external_buy_notional;
    let previous_unmanaged_external_buy_notional = account.unmanaged_external_buy_notional;
    account.external_buy_notional = external_open_buy_notional(&open_orders);

    let adopted = match adopt_external_open_reward_buy_orders(
        state,
        account,
        cycle_orders,
        &open_orders,
        trace_id,
    )
    .await
    {
        Ok(adopted) => adopted,
        Err(error) => {
            warn!(
                error = %error,
                "failed to adopt external Polymarket open rewards buy orders"
            );
            Vec::new()
        }
    };

    let closed = close_managed_orders_absent_from_open_snapshot_if_reliable(
        cycle_orders,
        &open_orders,
        trace_id,
        close_absent_buy_orders,
    );
    for (order, _) in &closed {
        if let Some(current) = cycle_orders.iter_mut().find(|current| current.id == order.id) {
            *current = order.clone();
        }
    }
    cycle_orders.retain(|order| order.status.is_open_like());

    // Freeze the snapshot-time external (non-managed) buy occupancy. Both
    // `external_buy_notional` (above) and `managed_external_open_buy_notional`
    // derive from this same CLOB snapshot, so their difference is the true
    // external occupancy and stays stable between snapshots. Funding precheck
    // reads this frozen value instead of recomputing external(stale) -
    // managed(now), which used to spike whenever managed buys were cancelled
    // between snapshots and made eligible_markets oscillate to 0.
    account.unmanaged_external_buy_notional =
        (account.external_buy_notional - managed_external_open_buy_notional(&cycle_orders))
            .max(Decimal::ZERO);

    if adopted.is_empty() && closed.is_empty() {
        if account.external_buy_notional != previous_external_buy_notional
            || account.unmanaged_external_buy_notional
                != previous_unmanaged_external_buy_notional
        {
            account.updated_at = OffsetDateTime::now_utc();
            if let Err(error) = state
                .reward_bot_service
                .apply_account_sync(account, None, trace_id)
                .await
            {
                warn!(
                    error = %error,
                    "failed to persist external open-order notional sync"
                );
            }
        }
        state
            .reward_bot_service
            .record_external_open_order_count(observed_managed_external_open_order_count(
                cycle_orders,
                &open_order_ids,
            ))
            .await;
        return;
    }

    let closed_count = closed.len();
    let (orders, events): (Vec<_>, Vec<_>) = adopted.into_iter().chain(closed).unzip();
    if let Err(error) = persist_live_reward_updates(
        state,
        account,
        Vec::new(),
        orders,
        Vec::new(),
        events,
        &RewardBotRunReport {
            cancelled_orders: closed_count,
            ..RewardBotRunReport::default()
        },
        trace_id,
    )
    .await
    {
        warn!(error = %error, "failed to persist managed open-order snapshot sync");
    }
    state
        .reward_bot_service
        .record_external_open_order_count(observed_managed_external_open_order_count(
            cycle_orders,
            &open_order_ids,
        ))
        .await;
}

#[derive(Debug, Clone)]
struct RewardOpenOrderTokenMatch {
    condition_id: String,
    token_id: String,
    outcome: String,
}

fn external_open_buy_notional(open_orders: &[PolymarketOpenOrder]) -> Decimal {
    open_orders
        .iter()
        .filter(|order| {
            order.side == PolymarketTokenOrderSide::Buy
                && external_open_order_counts_as_active(order)
        })
        .map(|order| {
            (order.price * (order.original_size - order.size_matched).max(Decimal::ZERO))
                .round_dp(4)
        })
        .sum()
}

fn external_open_order_counts_as_active(open_order: &PolymarketOpenOrder) -> bool {
    external_open_order_remaining_size(open_order) > Decimal::ZERO
        && polymarket_open_order_status_counts_as_active(&open_order.status)
}

fn polymarket_open_order_status_counts_as_active(status: &str) -> bool {
    let normalized = status
        .trim()
        .chars()
        .filter(|ch| !matches!(ch, ' ' | '_' | '-'))
        .collect::<String>()
        .to_ascii_lowercase();
    !matches!(
        normalized.as_str(),
        "filled" | "matched" | "canceled" | "cancelled" | "expired"
    )
}

async fn adopt_external_open_reward_buy_orders(
    state: &AppState,
    account: &RewardAccountState,
    cycle_orders: &mut Vec<ManagedRewardOrder>,
    open_orders: &[PolymarketOpenOrder],
    trace_id: &str,
) -> Result<Vec<(ManagedRewardOrder, RewardRiskEvent)>> {
    let markets = state.reward_bot_service.list_active_reward_markets().await?;
    let token_index = reward_open_order_token_index(&markets);
    if token_index.is_empty() {
        return Ok(Vec::new());
    }

    let mut known_external_order_ids = cycle_orders
        .iter()
        .filter_map(|order| order.external_order_id.clone())
        .collect::<HashSet<_>>();
    let mut adopted = Vec::new();

    for open_order in open_orders {
        if open_order.side != PolymarketTokenOrderSide::Buy
            || known_external_order_ids.contains(&open_order.id)
        {
            continue;
        }

        let token_key = normalize_reward_open_order_lookup_key(&open_order.asset_id);
        let Some(token_match) = token_index.get(&token_key) else {
            continue;
        };
        if !external_open_order_matches_reward_market(open_order, token_match) {
            continue;
        }

        let existing = state
            .reward_bot_service
            .get_managed_order_by_external_order_id(&open_order.id)
            .await?;
        if existing
            .as_ref()
            .is_some_and(|order| order.account_id != account.account_id)
        {
            warn!(
                external_order_id = %open_order.id,
                account_id = %account.account_id,
                "skipping external rewards open-order adoption because the local order belongs to another account"
            );
            continue;
        }

        let Some((order, event)) = build_external_open_reward_buy_order_adoption(
            &account.account_id,
            token_match,
            open_order,
            existing,
            OffsetDateTime::now_utc(),
            trace_id,
        ) else {
            continue;
        };

        known_external_order_ids.insert(open_order.id.clone());
        cycle_orders.push(order.clone());
        adopted.push((order, event));
    }

    Ok(adopted)
}

fn reward_open_order_token_index(
    markets: &[RewardMarket],
) -> HashMap<String, RewardOpenOrderTokenMatch> {
    let mut index = HashMap::new();
    let mut duplicated = HashSet::new();

    for market in markets.iter().filter(|market| market.active) {
        for token in &market.tokens {
            let token_key = normalize_reward_open_order_lookup_key(&token.token_id);
            if token_key.is_empty() {
                continue;
            }
            let token_match = RewardOpenOrderTokenMatch {
                condition_id: market.condition_id.clone(),
                token_id: token.token_id.clone(),
                outcome: token.outcome.clone(),
            };
            if index.insert(token_key.clone(), token_match).is_some() {
                duplicated.insert(token_key);
            }
        }
    }

    for token_key in duplicated {
        index.remove(&token_key);
    }

    index
}

fn build_external_open_reward_buy_order_adoption(
    account_id: &str,
    token_match: &RewardOpenOrderTokenMatch,
    open_order: &PolymarketOpenOrder,
    existing: Option<ManagedRewardOrder>,
    now: OffsetDateTime,
    trace_id: &str,
) -> Option<(ManagedRewardOrder, RewardRiskEvent)> {
    if open_order.side != PolymarketTokenOrderSide::Buy
        || external_open_order_remaining_size(open_order) <= Decimal::ZERO
        || !external_open_order_matches_reward_market(open_order, token_match)
    {
        return None;
    }

    let (mut order, event_type, message, previous_status) = match existing {
        Some(existing) if existing.status.is_open_like() => return None,
        Some(existing) if existing.status == ManagedRewardOrderStatus::Filled => return None,
        Some(existing) if existing.side != RewardOrderSide::Buy => return None,
        Some(existing) => {
            let previous_status = existing.status.as_str().to_string();
            (
                existing,
                "reward_live_external_open_order_reopened",
                "Reopened a local rewards order because it is still open on Polymarket.",
                Some(previous_status),
            )
        }
        None => (
            new_adopted_external_reward_buy_order(account_id, token_match, open_order, now),
            "reward_live_external_open_order_adopted",
            "Adopted an existing Polymarket rewards buy order from the open-order snapshot.",
            None,
        ),
    };

    order.account_id = account_id.to_string();
    order.condition_id = token_match.condition_id.clone();
    order.token_id = token_match.token_id.clone();
    order.outcome = token_match.outcome.clone();
    order.side = RewardOrderSide::Buy;
    order.price = open_order.price;
    order.size = open_order.original_size;
    order.external_order_id = Some(open_order.id.clone());
    order.status = ManagedRewardOrderStatus::Open;
    order.scoring = false;
    order.reason = message.to_string();
    order.filled_size = order
        .filled_size
        .max(Decimal::ZERO)
        .min(open_order.original_size);
    order.updated_at = now;

    let event = external_open_reward_buy_adoption_event(
        &order,
        token_match,
        open_order,
        event_type,
        message,
        previous_status,
        trace_id,
    );

    Some((order, event))
}

fn new_adopted_external_reward_buy_order(
    account_id: &str,
    token_match: &RewardOpenOrderTokenMatch,
    open_order: &PolymarketOpenOrder,
    now: OffsetDateTime,
) -> ManagedRewardOrder {
    ManagedRewardOrder {
        id: format!(
            "rewadopt_{}_{}",
            now.unix_timestamp_nanos(),
            sanitize_reward_id_fragment(&open_order.id)
        ),
        account_id: account_id.to_string(),
        condition_id: token_match.condition_id.clone(),
        token_id: token_match.token_id.clone(),
        outcome: token_match.outcome.clone(),
        side: RewardOrderSide::Buy,
        price: open_order.price,
        size: open_order.original_size,
        strategy_bucket: RewardStrategyBucket::Standard,
        strategy_profile: RewardStrategyProfile::Standard,
        external_order_id: Some(open_order.id.clone()),
        status: ManagedRewardOrderStatus::Open,
        scoring: false,
        reason: String::new(),
        filled_size: Decimal::ZERO,
        reward_earned: Decimal::ZERO,
        last_scored_at: None,
        created_at: open_order.created_at,
        updated_at: now,
    }
}

fn external_open_reward_buy_adoption_event(
    order: &ManagedRewardOrder,
    token_match: &RewardOpenOrderTokenMatch,
    open_order: &PolymarketOpenOrder,
    event_type: &str,
    message: &str,
    previous_status: Option<String>,
    trace_id: &str,
) -> RewardRiskEvent {
    reward_live_event(
        order,
        event_type,
        RewardRiskSeverity::Warning,
        message,
        json!({
            "trace_id": trace_id,
            "external_order_id": open_order.id,
            "token_id": open_order.asset_id,
            "condition_id": token_match.condition_id,
            "price": open_order.price,
            "original_size": open_order.original_size,
            "size_matched": open_order.size_matched,
            "remaining_size": external_open_order_remaining_size(open_order),
            "previous_status": previous_status,
        }),
    )
}

fn external_open_order_matches_reward_market(
    open_order: &PolymarketOpenOrder,
    token_match: &RewardOpenOrderTokenMatch,
) -> bool {
    let market_key = normalize_reward_open_order_lookup_key(&open_order.market);
    market_key.is_empty()
        || market_key == normalize_reward_open_order_lookup_key(&token_match.condition_id)
}

fn external_open_order_remaining_size(open_order: &PolymarketOpenOrder) -> Decimal {
    (open_order.original_size - open_order.size_matched).max(Decimal::ZERO)
}

fn normalize_reward_open_order_lookup_key(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

fn close_managed_orders_absent_from_open_snapshot(
    orders: &[ManagedRewardOrder],
    open_orders: &[PolymarketOpenOrder],
    trace_id: &str,
) -> Vec<(ManagedRewardOrder, RewardRiskEvent)> {
    let open_order_ids = open_orders
        .iter()
        .filter(|order| external_open_order_counts_as_active(order))
        .map(|order| order.id.as_str())
        .collect::<HashSet<_>>();

    orders
        .iter()
        .filter_map(|order| {
            let external_order_id = order.external_order_id.as_deref()?;
            if !managed_reward_order_can_close_from_open_snapshot(order, external_order_id)
                || open_order_ids.contains(external_order_id)
            {
                return None;
            }

            let mut closed = order.clone();
            closed.status = ManagedRewardOrderStatus::Cancelled;
            closed.scoring = false;
            closed.reason = "live rewards buy order is no longer present in Polymarket open orders; local remainder closed".to_string();
            closed.updated_at = OffsetDateTime::now_utc();
            let event = reward_live_event(
                &closed,
                "reward_live_order_missing_from_open_orders_closed",
                RewardRiskSeverity::Info,
                closed.reason.clone(),
                json!({
                    "trace_id": trace_id,
                    "external_order_id": external_order_id,
                    "token_id": closed.token_id.clone(),
                    "remaining_size": (closed.size - closed.filled_size).max(Decimal::ZERO),
                }),
            );
            Some((closed, event))
        })
        .collect()
}

fn close_managed_orders_absent_from_open_snapshot_if_reliable(
    orders: &[ManagedRewardOrder],
    open_orders: &[PolymarketOpenOrder],
    trace_id: &str,
    reconciliation_reliable: bool,
) -> Vec<(ManagedRewardOrder, RewardRiskEvent)> {
    if !reconciliation_reliable {
        return Vec::new();
    }
    close_managed_orders_absent_from_open_snapshot(orders, open_orders, trace_id)
}

fn observed_managed_external_open_order_count(
    orders: &[ManagedRewardOrder],
    open_order_ids: &HashSet<&str>,
) -> usize {
    orders
        .iter()
        .filter(|order| {
            reward_order_counts_as_external_open(order)
                && order
                    .external_order_id
                    .as_deref()
                    .is_some_and(|external_order_id| open_order_ids.contains(external_order_id))
        })
        .count()
}

fn managed_reward_order_can_close_from_open_snapshot(
    order: &ManagedRewardOrder,
    external_order_id: &str,
) -> bool {
    order.status.is_open_like()
        && order.side == RewardOrderSide::Buy
        && !is_internal_reward_order_id(external_order_id)
        && (!is_stuck_reconciliation_order(order)
            || live_cancel_accepted_awaiting_final_reconciliation(order))
}

fn live_cancel_accepted_awaiting_final_reconciliation(order: &ManagedRewardOrder) -> bool {
    order
        .reason
        .contains("cancel accepted; awaiting final reconciliation")
}

/// Query Polymarket for today's account-level reward total and persist it.
/// Always runs regardless of fill state — the value is an authoritative daily
/// total from Polymarket, not a locally accumulated figure, so there is no
/// double-counting risk.
async fn sync_reward_earnings(
    state: &AppState,
    connector: &LivePolymarketConnector,
    cycle_account: &mut RewardAccountState,
    trace_id: &str,
) {
    match connector.reward_earnings_today_usd().await {
        Ok(reward_earned_usd) if reward_earned_usd != cycle_account.reward_earned_usd => {
            let previous = cycle_account.reward_earned_usd;
            cycle_account.reward_earned_usd = reward_earned_usd;
            let event = new_risk_event(
                Some(cycle_account.account_id.clone()),
                None,
                None,
                "reward_live_reward_earnings_synced",
                RewardRiskSeverity::Info,
                "Synced today's Polymarket maker reward total.",
                json!({
                    "date": OffsetDateTime::now_utc().date().to_string(),
                    "previous_reward_earned_usd": previous,
                    "reward_earned_usd": reward_earned_usd,
                }),
            );
            if let Err(error) = persist_live_reward_updates(
                state,
                cycle_account,
                Vec::new(),
                Vec::new(),
                Vec::new(),
                vec![event],
                &RewardBotRunReport {
                    reward_accrued: (reward_earned_usd - previous).max(Decimal::ZERO),
                    ..RewardBotRunReport::default()
                },
                trace_id,
            )
            .await
            {
                warn!(error = %error, "failed to persist Polymarket reward earnings sync");
            }
        }
        Ok(_) => {}
        Err(error) => {
            warn!(error = %error, "failed to query Polymarket reward earnings");
        }
    }
}

async fn sync_managed_reward_scoring(
    state: &AppState,
    connector: &LivePolymarketConnector,
    account: &mut RewardAccountState,
    orders: &mut [ManagedRewardOrder],
    trace_id: &str,
) {
    let config = match state.reward_bot_service.read_config().await {
        Ok(config) => config,
        Err(error) => {
            warn!(error = %error, "failed to load rewards config before scoring sync");
            return;
        }
    };
    let now = OffsetDateTime::now_utc();
    let scoring_interval = TimeDuration::seconds(config.min_scoring_check_sec as i64);
    let due = orders
        .iter()
        .enumerate()
        .filter_map(|(index, order)| {
            let external_order_id = order.external_order_id.as_ref()?;
            let should_check = should_check_managed_reward_scoring(order, now, scoring_interval);
            should_check.then(|| (index, external_order_id.clone()))
        })
        .collect::<Vec<_>>();

    if due.is_empty() {
        return;
    }

    let mut scoring_by_order = HashMap::new();
    for chunk in due.chunks(100) {
        let order_ids = chunk
            .iter()
            .map(|(_, order_id)| order_id.clone())
            .collect::<Vec<_>>();
        match connector.orders_scoring(&order_ids).await {
            Ok(scoring) => scoring_by_order.extend(scoring),
            Err(error) => {
                warn!(error = %error, "failed to query managed rewards order scoring");
                return;
            }
        }
    }

    let mut updates = Vec::new();
    let mut events = Vec::new();
    for (index, external_order_id) in due {
        let Some(scoring) = scoring_by_order.get(&external_order_id).copied() else {
            continue;
        };
        let order = &mut orders[index];
        let changed = apply_managed_reward_scoring_observation(order, scoring, now);
        updates.push(order.clone());
        if changed {
            events.push(reward_live_event(
                order,
                if scoring {
                    "reward_live_order_scoring_started"
                } else {
                    "reward_live_order_scoring_stopped"
                },
                RewardRiskSeverity::Info,
                if scoring {
                    "managed rewards order is now scoring"
                } else {
                    "managed rewards order is no longer scoring"
                },
                json!({ "scoring": scoring }),
            ));
        }
    }

    if updates.is_empty() {
        return;
    }
    if let Err(error) = persist_live_reward_updates(
        state,
        account,
        Vec::new(),
        updates,
        Vec::new(),
        events,
        &RewardBotRunReport::default(),
        trace_id,
    )
    .await
    {
        warn!(error = %error, "failed to persist managed rewards scoring sync");
    }
}

fn should_check_managed_reward_scoring(
    order: &ManagedRewardOrder,
    now: OffsetDateTime,
    scoring_interval: TimeDuration,
) -> bool {
    order.side == RewardOrderSide::Buy
        && order.status.is_open_like()
        && !is_stuck_reconciliation_order(order)
        && order
            .last_scored_at
            .is_none_or(|last_scored_at| last_scored_at + scoring_interval <= now)
}

fn apply_managed_reward_scoring_observation(
    order: &mut ManagedRewardOrder,
    scoring: bool,
    checked_at: OffsetDateTime,
) -> bool {
    let changed = order.scoring != scoring;
    order.scoring = scoring;
    order.last_scored_at = Some(checked_at);
    changed
}

fn polymarket_funding_wallet_address(
    account_id: &str,
    funder: Option<&str>,
) -> Option<String> {
    funder
        .and_then(|value| {
            let normalized = value.trim();
            (!normalized.is_empty()).then(|| normalized.to_string())
        })
        .or_else(|| {
            let normalized = account_id.trim();
            (!normalized.is_empty()).then(|| normalized.to_string())
        })
}

async fn external_account_sync_allowed(
    state: &AppState,
    account: &RewardAccountState,
    trace_id: &str,
) -> bool {
    let now = OffsetDateTime::now_utc();
    match state
        .reward_bot_service
        .latest_reward_fill_at(&account.account_id)
        .await
    {
        Ok(Some(fill_at)) if !account_sync_is_outside_fill_grace(Some(fill_at), now) => {
            debug!(
                trace_id,
                fill_at = %fill_at,
                grace_secs = REWARD_ACCOUNT_SYNC_FILL_GRACE.whole_seconds(),
                "skipping external account replacement while a confirmed fill propagates"
            );
            false
        }
        Ok(_) => true,
        Err(error) => {
            warn!(
                trace_id,
                error = %error,
                "failed to check recent rewards fills; preserving local account state"
            );
            false
        }
    }
}

fn polymarket_position_to_reward(
    account_id: &str,
    position: &PolymarketWalletPosition,
) -> RewardPosition {
    RewardPosition {
        account_id: account_id.to_string(),
        condition_id: position.condition_id.clone(),
        token_id: position.asset.clone(),
        outcome: position.outcome.clone(),
        size: position.size,
        avg_price: position.avg_price,
        realized_pnl: position.realized_pnl,
        updated_at: OffsetDateTime::now_utc(),
    }
}

fn can_refresh_external_account_after_order_sync(report: &RewardBotRunReport) -> bool {
    report.filled_orders == 0
}

fn account_sync_is_outside_fill_grace(
    latest_fill_at: Option<OffsetDateTime>,
    now: OffsetDateTime,
) -> bool {
    latest_fill_at.is_none_or(|fill_at| now >= fill_at + REWARD_ACCOUNT_SYNC_FILL_GRACE)
}
