async fn drain_execution_queue_for_connector(state: &AppState, connector_name: &str) {
    match drain_execution_queue(state, Some(connector_name.to_string()), task_limit(state)).await {
        Ok(report) => info!(
            connector_name,
            scanned = report.scanned,
            submitted = report.submitted,
            failed = report.failed,
            "completed worker execution queue drain",
        ),
        Err(error) => {
            warn!(connector_name, error = %error, "worker execution queue drain failed");
        }
    }
}

fn task_limit(state: &AppState) -> Option<u16> {
    Some(state.settings.worker.task_limit)
}

fn polymarket_enabled(state: &AppState) -> bool {
    state.settings.polymarket.mode != PolymarketConnectorMode::Disabled
}

async fn drain_execution_queue(
    state: &AppState,
    connector_name: Option<String>,
    limit: Option<u16>,
) -> Result<ExecutionDrainReport> {
    let connector_name = connector_name.unwrap_or_else(|| PAPER_EXECUTOR_NAME.to_string());
    let candidates = state
        .execution_service
        .list_dispatch_candidates(DispatchExecutionListFilters::new(
            Some(connector_name.clone()),
            limit,
        )?)
        .await?;
    let mut report = ExecutionDrainReport {
        scanned: candidates.len(),
        ..ExecutionDrainReport::default()
    };

    match connector_name.as_str() {
        PAPER_EXECUTOR_NAME => {
            let executor = PaperExecutor::new();
            for candidate in candidates {
                dispatch_candidate(state, &executor, candidate)
                    .await
                    .map(|submitted| {
                        if submitted {
                            report.submitted += 1;
                        } else {
                            report.failed += 1;
                        }
                    })?;
            }
        }
        POLYMARKET_CONNECTOR_NAME => {
            ensure_polymarket_enabled(state)?;
            match state.settings.polymarket.mode {
                PolymarketConnectorMode::Mock => {
                    let connector = MockPolymarketConnector::new();
                    for candidate in candidates {
                        dispatch_polymarket_candidate(state, &connector, candidate)
                            .await
                            .map(|submitted| {
                                if submitted {
                                    report.submitted += 1;
                                } else {
                                    report.failed += 1;
                                }
                            })?;
                    }
                }
                PolymarketConnectorMode::Live => {
                    let connector = build_live_polymarket_connector(state).await?;
                    for candidate in candidates {
                        dispatch_live_polymarket_candidate(state, &connector, candidate)
                            .await
                            .map(|submitted| {
                                if submitted {
                                    report.submitted += 1;
                                } else {
                                    report.failed += 1;
                                }
                            })?;
                    }
                }
                PolymarketConnectorMode::Disabled => unreachable!("disabled handled above"),
            }
        }
        other => {
            return Err(AppError::invalid_input(
                "WORKER_CONNECTOR_UNSUPPORTED",
                format!("worker does not support connector_name={other}"),
            ));
        }
    }

    Ok(report)
}
