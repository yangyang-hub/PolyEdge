/// Refresh the externally authoritative Polymarket balance and position snapshot.
///
/// A failed balance request preserves the local balance. A failed position request
/// preserves the stored positions; a successful position request, including an
/// empty response, atomically replaces all positions for the rewards account.
async fn sync_external_account_state(
    state: &AppState,
    connector: &LivePolymarketConnector,
    cycle_account: &mut RewardAccountState,
    cycle_positions: &mut Vec<RewardPosition>,
    trace_id: &str,
) {
    let mut balance_updated = false;
    match connector.balance().await {
        Ok(balance) => {
            cycle_account.available_usd = balance.balance;
            balance_updated = true;
        }
        Err(error) => {
            warn!(error = %error, "failed to fetch live Polymarket balance during sync");
        }
    }

    let settings = &state.settings.polymarket;
    let position_snapshot = if settings.account_id.trim().is_empty() {
        warn!("Polymarket account_id not configured, skipping external position sync");
        None
    } else {
        match PolymarketDataApiConnector::new(&settings.data_api_host) {
            Ok(data_connector) => {
                match data_connector.fetch_wallet_positions(&settings.account_id).await {
                    Ok(raw_positions) => {
                        let mut positions = raw_positions
                            .into_iter()
                            .filter(|position| position.size > Decimal::ZERO)
                            .map(|position| {
                                polymarket_position_to_reward(
                                    &cycle_account.account_id,
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
        }
    };

    if !balance_updated && position_snapshot.is_none() {
        return;
    }

    if let Some(positions) = position_snapshot.as_ref() {
        cycle_positions.clone_from(positions);
    }
    cycle_account.updated_at = OffsetDateTime::now_utc();
    if let Err(error) = state
        .reward_bot_service
        .apply_account_sync(cycle_account, position_snapshot.as_deref(), trace_id)
        .await
    {
        warn!(error = %error, "failed to persist external account sync outcome");
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
