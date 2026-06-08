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
    trace_id: &str,
) {
    if !external_account_sync_allowed(state, cycle_account, trace_id).await {
        return;
    }

    let mut synced_account = cycle_account.clone();
    let mut balance_updated = false;
    match connector.refresh_balance().await {
        Ok(balance) => {
            synced_account.available_usd = balance.balance;
            balance_updated = true;
        }
        Err(error) => {
            warn!(error = %error, "polymarket balance query failed");
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
