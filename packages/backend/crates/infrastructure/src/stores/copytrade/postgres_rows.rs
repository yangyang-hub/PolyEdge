fn copytrade_wallet_from_row(row: &sqlx::postgres::PgRow) -> Result<TrackedWallet> {
    let status_raw: String = row.try_get("status").map_err(postgres_decode_error)?;
    let sizing_raw: Option<String> = row
        .try_get("sizing_override")
        .map_err(postgres_decode_error)?;
    let sizing_override = sizing_raw.and_then(|s| CopySizingMode::from_str(&s).ok());
    Ok(TrackedWallet {
        address: row.try_get("address").map_err(postgres_decode_error)?,
        label: row.try_get("label").map_err(postgres_decode_error)?,
        status: TrackedWalletStatus::from_str(&status_raw)?,
        sizing_override,
        max_exposure_override: row
            .try_get("max_exposure_override")
            .map_err(postgres_decode_error)?,
        added_at: row.try_get("added_at").map_err(postgres_decode_error)?,
        updated_at: row.try_get("updated_at").map_err(postgres_decode_error)?,
        analysis: WalletAnalysisStats {
            trades_window: row.try_get("trades_window").map_err(postgres_decode_error)?,
            volume_window_usd: row
                .try_get("volume_window_usd")
                .map_err(postgres_decode_error)?,
            realized_pnl_window: row
                .try_get("realized_pnl_window")
                .map_err(postgres_decode_error)?,
            win_rate: row.try_get("win_rate").map_err(postgres_decode_error)?,
            roi: row.try_get("roi").map_err(postgres_decode_error)?,
            avg_trade_usd: row.try_get("avg_trade_usd").map_err(postgres_decode_error)?,
            markets_traded: row
                .try_get("markets_traded")
                .map_err(postgres_decode_error)?,
            last_active_at: row
                .try_get("last_active_at")
                .map_err(postgres_decode_error)?,
            last_analyzed_at: row
                .try_get("last_analyzed_at")
                .map_err(postgres_decode_error)?,
        },
    })
}

fn copytrade_source_trade_from_row(row: &sqlx::postgres::PgRow) -> Result<SourceTrade> {
    let side_raw: String = row.try_get("side").map_err(postgres_decode_error)?;
    Ok(SourceTrade {
        id: row.try_get("id").map_err(postgres_decode_error)?,
        wallet_address: row
            .try_get("wallet_address")
            .map_err(postgres_decode_error)?,
        condition_id: row
            .try_get("condition_id")
            .map_err(postgres_decode_error)?,
        token_id: row.try_get("token_id").map_err(postgres_decode_error)?,
        outcome: row.try_get("outcome").map_err(postgres_decode_error)?,
        side: CopyOrderSide::from_str(&side_raw)?,
        price: row.try_get("price").map_err(postgres_decode_error)?,
        size: row.try_get("size").map_err(postgres_decode_error)?,
        usd_size: row.try_get("usd_size").map_err(postgres_decode_error)?,
        title: row.try_get("title").map_err(postgres_decode_error)?,
        source_tx_hash: row
            .try_get("source_tx_hash")
            .map_err(postgres_decode_error)?,
        source_timestamp: row
            .try_get("source_timestamp")
            .map_err(postgres_decode_error)?,
        observed_at: row.try_get("observed_at").map_err(postgres_decode_error)?,
        copied: row.try_get("copied").map_err(postgres_decode_error)?,
        decision_reason: row
            .try_get("decision_reason")
            .map_err(postgres_decode_error)?,
    })
}

fn copytrade_event_from_row(row: &sqlx::postgres::PgRow) -> Result<CopyEvent> {
    let severity_raw: String = row.try_get("severity").map_err(postgres_decode_error)?;
    let metadata: Json<Value> = row
        .try_get("metadata_json")
        .map_err(postgres_decode_error)?;
    Ok(CopyEvent {
        id: row.try_get("id").map_err(postgres_decode_error)?,
        wallet_address: row
            .try_get("wallet_address")
            .map_err(postgres_decode_error)?,
        condition_id: row
            .try_get("condition_id")
            .map_err(postgres_decode_error)?,
        event_type: row.try_get("event_type").map_err(postgres_decode_error)?,
        severity: CopyEventSeverity::from_str(&severity_raw)?,
        message: row.try_get("message").map_err(postgres_decode_error)?,
        metadata: metadata.0,
        created_at: row.try_get("created_at").map_err(postgres_decode_error)?,
    })
}

fn copytrade_control_command_from_row(row: &sqlx::postgres::PgRow) -> Result<CopyControlCommand> {
    let action_raw: String = row.try_get("action").map_err(postgres_decode_error)?;
    let status_raw: String = row.try_get("status").map_err(postgres_decode_error)?;
    Ok(CopyControlCommand {
        id: row.try_get("id").map_err(postgres_decode_error)?,
        action: CopyControlAction::from_str(&action_raw)?,
        account_id: row.try_get("account_id").map_err(postgres_decode_error)?,
        reason: row.try_get("reason").map_err(postgres_decode_error)?,
        status: CopyControlCommandStatus::from_str(&status_raw)?,
        requested_at: row.try_get("requested_at").map_err(postgres_decode_error)?,
        started_at: row.try_get("started_at").map_err(postgres_decode_error)?,
        completed_at: row.try_get("completed_at").map_err(postgres_decode_error)?,
        trace_id: row.try_get("trace_id").map_err(postgres_decode_error)?,
        error: row.try_get("error").map_err(postgres_decode_error)?,
    })
}
