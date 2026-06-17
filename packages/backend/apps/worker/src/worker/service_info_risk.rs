fn maybe_spawn_reward_info_risk_task(
    state: &AppState,
    shutdown_rx: watch::Receiver<bool>,
    handles: &mut Vec<JoinHandle<()>>,
) {
    let settings = &state.settings.worker;
    if !settings.poll_reward_info_risks {
        info!(
            "worker reward info risk polling is disabled; set POLYEDGE_WORKER__POLL_REWARD_INFO_RISKS=true"
        );
        return;
    }
    if !state.settings.rewards.enabled {
        warn!(
            "worker poll-reward-info-risks is enabled but rewards bot is disabled; set POLYEDGE_REWARDS__ENABLED=true"
        );
        return;
    }

    let job_state = state.clone();
    let interval_secs = state.settings.rewards.info_risk_interval_secs;
    let initial_delay_secs = interval_secs.max(30);
    info!(
        interval_secs,
        initial_delay_secs,
        web_search_enabled = state.settings.rewards.info_risk_web_search_enabled,
        "spawning worker reward info risk polling task",
    );
    handles.push(tokio::spawn(async move {
        let mut shutdown_rx = shutdown_rx;
        if wait_for_worker_interval(&mut shutdown_rx, initial_delay_secs).await {
            info!(job = "poll-reward-info-risks", "worker interval job stopped");
            return;
        }

        loop {
            if *shutdown_rx.borrow() {
                break;
            }

            let trace_id = new_trace_id();
            match scan_reward_info_risks_once(&job_state, &trace_id).await {
                Ok(report) => info!(
                    trace_id = %trace_id,
                    candidates = report.candidates,
                    cache_hits = report.cache_hits,
                    requested = report.requested,
                    saved = report.saved,
                    failures = report.failures,
                    skipped_missing_market = report.skipped_missing_market,
                    applied_plans = report.applied_plans,
                    "completed worker reward info risk cycle",
                ),
                Err(error) => {
                    warn!(trace_id = %trace_id, error = %error, "worker reward info risk cycle failed");
                }
            }

            if wait_for_worker_interval(&mut shutdown_rx, interval_secs).await {
                break;
            }
        }

        info!(job = "poll-reward-info-risks", "worker interval job stopped");
    }));
}
