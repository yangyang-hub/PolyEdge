fn maybe_spawn_reward_info_risk_task(
    state: &AppState,
    shutdown_rx: watch::Receiver<bool>,
    handles: &mut Vec<JoinHandle<()>>,
) {
    let settings = &state.settings.worker;
    if !settings.poll_reward_info_risks {
        return;
    }
    if !state.settings.rewards.enabled {
        warn!(
            "worker poll-reward-info-risks is enabled but rewards bot is disabled; set POLYEDGE_REWARDS__ENABLED=true"
        );
        return;
    }

    let job_state = state.clone();
    handles.push(spawn_interval_job(
        "poll-reward-info-risks",
        state.settings.rewards.info_risk_interval_secs,
        shutdown_rx,
        move || {
            let state = job_state.clone();
            async move {
                let trace_id = new_trace_id();
                match scan_reward_info_risks_once(&state, &trace_id).await {
                    Ok(report) => info!(
                        trace_id = %trace_id,
                        candidates = report.candidates,
                        cache_hits = report.cache_hits,
                        requested = report.requested,
                        saved = report.saved,
                        applied_plans = report.applied_plans,
                        "completed worker reward info risk cycle",
                    ),
                    Err(error) => {
                        warn!(trace_id = %trace_id, error = %error, "worker reward info risk cycle failed");
                    }
                }
            }
        },
    ));
}
