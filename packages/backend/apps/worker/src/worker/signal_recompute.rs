#[derive(Debug, Default)]
struct SignalRecomputeReport {
    scanned: usize,
    recomputed: usize,
    skipped: usize,
    failed: usize,
}

async fn recompute_all_signals(
    state: &AppState,
    limit: Option<u16>,
    trace_id: &str,
) -> Result<SignalRecomputeReport> {
    let signals = state
        .market_event_service
        .list_signals(SignalListFilters::new(None, None, None, limit)?, &PageQuery { page: 1, page_size: limit.unwrap_or(200), sort_order: None })
        .await?
        .data;

    let mut report = SignalRecomputeReport {
        scanned: signals.len(),
        ..SignalRecomputeReport::default()
    };

    for signal in signals {
        if signal.evidence_ids.is_empty() {
            report.skipped += 1;
            continue;
        }

        match state
            .market_event_service
            .recompute_signal(&signal.id, "auto_recompute", trace_id)
            .await
        {
            Ok(_) => {
                report.recomputed += 1;
            }
            Err(error) => {
                report.failed += 1;
                warn!(
                    signal_id = %signal.id,
                    trace_id = %trace_id,
                    error = %error,
                    "auto recompute signal failed",
                );
            }
        }
    }

    Ok(report)
}
