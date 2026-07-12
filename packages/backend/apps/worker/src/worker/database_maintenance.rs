async fn run_database_maintenance_once(state: &AppState) -> Result<DatabaseMaintenanceReport> {
    state
        .database_maintenance_service
        .prune_history(OffsetDateTime::now_utc())
        .await
}

fn log_database_maintenance_report(report: DatabaseMaintenanceReport, message: &'static str) {
    info!(
        total_deleted = report.total_deleted(),
        idempotency_keys_deleted = report.idempotency_keys_deleted,
        outbox_events_deleted = report.outbox_events_deleted,
        external_event_dedup_deleted = report.external_event_dedup_deleted,
        llm_calls_deleted = report.llm_calls_deleted,
        raw_events_deleted = report.raw_events_deleted,
        reward_market_advisories_deleted = report.reward_market_advisories_deleted,
        reward_market_info_risks_deleted = report.reward_market_info_risks_deleted,
        reward_market_candles_deleted = report.reward_market_candles_deleted,
        reward_fair_value_history_deleted = report.reward_fair_value_history_deleted,
        reward_strategy_runs_deleted = report.reward_strategy_runs_deleted,
        reward_order_transitions_deleted = report.reward_order_transitions_deleted,
        reward_risk_events_deleted = report.reward_risk_events_deleted,
        reward_control_commands_deleted = report.reward_control_commands_deleted,
        audit_logs_deleted = report.audit_logs_deleted,
        mode_transitions_deleted = report.mode_transitions_deleted,
        "{message}",
    );
}
