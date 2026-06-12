/// Refresh the externally authoritative Polymarket balance and position snapshot.
///
/// A failed CLOB balance request preserves the local balance unless the funding
/// wallet pUSD balance can be read from Polygon. A failed position request preserves
/// the stored positions; a successful position request, including an empty response,
/// atomically replaces all positions for the rewards account.
const REWARD_ACCOUNT_SYNC_FILL_GRACE: TimeDuration = TimeDuration::seconds(120);

async fn sync_external_account_state(
    state: &AppState,
    connector: &LivePolymarketConnector,
    cycle_account: &mut RewardAccountState,
    cycle_positions: &mut Vec<RewardPosition>,
    cycle_orders: &mut Vec<ManagedRewardOrder>,
    trace_id: &str,
) {
    sync_managed_reward_scoring(
        state,
        connector,
        cycle_account,
        cycle_orders,
        trace_id,
    )
    .await;

    if !external_account_sync_allowed(state, cycle_account, trace_id).await {
        return;
    }

    let mut synced_account = cycle_account.clone();
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

    // Sync all active buy orders on Polymarket (including orders not managed by
    // this bot) so the placement pre-check can avoid "not enough balance" rejections.
    match connector.list_open_orders().await {
        Ok(open_orders) => {
            let buy_notional: Decimal = open_orders
                .iter()
                .filter(|o| o.side == PolymarketTokenOrderSide::Buy)
                .map(|o| (o.price * (o.original_size - o.size_matched).max(Decimal::ZERO)).round_dp(4))
                .sum();
            synced_account.external_buy_notional = buy_notional;
        }
        Err(error) => {
            warn!(error = %error, "failed to list Polymarket open orders for notional sync");
        }
    }

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
        }
        Err(error) => {
            warn!(error = %error, "failed to persist external account sync outcome");
        }
    }
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
            let should_check = order.side == RewardOrderSide::Buy
                && order.status.is_open_like()
                && order
                    .last_scored_at
                    .is_none_or(|last_scored_at| last_scored_at + scoring_interval <= now);
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
        let changed = order.scoring != scoring;
        order.scoring = scoring;
        order.last_scored_at = Some(now);
        order.updated_at = now;
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
