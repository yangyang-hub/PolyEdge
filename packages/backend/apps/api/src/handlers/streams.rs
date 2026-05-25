#[derive(Debug, Clone)]
struct SseMessage {
    id: String,
    event: &'static str,
    data: Value,
}

#[derive(Clone)]
struct StreamState {
    app_state: AppState,
    channel: String,
    sequence: u64,
    emitted_ids: HashSet<String>,
    emitted_id_order: VecDeque<String>,
    last_arbitrage_sequence: Option<u64>,
}

async fn stream_channel(
    Extension(_auth): Extension<AuthContext>,
    State(state): State<AppState>,
    Path(channel): Path<String>,
    headers: HeaderMap,
) -> std::result::Result<Response, HttpError> {
    if !matches!(
        channel.as_str(),
        "signals" | "risk" | "events" | "arbitrage"
    ) {
        return Err(HttpError::with_meta(
            AppError::not_found("STREAM_CHANNEL_NOT_FOUND", "unknown stream channel"),
            "unknown",
            new_trace_id(),
        ));
    }

    let mut emitted_ids = HashSet::new();
    let mut emitted_id_order = VecDeque::new();
    let mut last_arbitrage_sequence = None;

    if let Some(last_event_id) = headers
        .get("last-event-id")
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        if channel == "arbitrage" {
            last_arbitrage_sequence = last_event_id.parse::<u64>().ok();
        }
        emitted_ids.insert(last_event_id.to_string());
        emitted_id_order.push_back(last_event_id.to_string());
    }

    let stream_state = StreamState {
        app_state: state,
        channel,
        sequence: 0,
        emitted_ids,
        emitted_id_order,
        last_arbitrage_sequence,
    };
    let event_stream = stream::unfold(stream_state, |mut stream_state| async move {
        if stream_state.sequence > 0 {
            tokio::time::sleep(Duration::from_secs(5)).await;
        }

        let chunk = match build_stream_chunk(
            &stream_state.app_state,
            &stream_state.channel,
            stream_state.sequence,
            &mut stream_state.emitted_ids,
            &mut stream_state.emitted_id_order,
            &mut stream_state.last_arbitrage_sequence,
        )
        .await
        {
            Ok(chunk) => chunk,
            Err(error) => format!(
                "event: stream.error\ndata: {}\n\n",
                json!({
                    "code": error.code(),
                    "message": error.message(),
                    "retryable": error.retryable(),
                })
            ),
        };

        stream_state.sequence += 1;
        Some((Ok::<Bytes, Infallible>(Bytes::from(chunk)), stream_state))
    });

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/event-stream; charset=utf-8")
        .header(header::CACHE_CONTROL, "no-cache, no-transform")
        .header(header::CONNECTION, "keep-alive")
        .header("x-accel-buffering", "no")
        .body(Body::from_stream(event_stream))
        .map_err(|error| {
            HttpError::with_meta(
                AppError::internal(
                    "STREAM_RESPONSE_BUILD_FAILED",
                    format!("failed to build stream response: {error}"),
                ),
                "unknown",
                new_trace_id(),
            )
        })
}

async fn build_stream_chunk(
    state: &AppState,
    channel: &str,
    sequence: u64,
    emitted_ids: &mut HashSet<String>,
    emitted_id_order: &mut VecDeque<String>,
    last_arbitrage_sequence: &mut Option<u64>,
) -> polyedge_domain::Result<String> {
    let messages = match channel {
        "signals" => signal_stream_messages(state).await?,
        "risk" => risk_stream_messages(state).await?,
        "events" => event_stream_messages(state).await?,
        "arbitrage" => arbitrage_stream_messages(state, last_arbitrage_sequence).await?,
        _ => Vec::new(),
    };
    let messages = filter_new_sse_messages(messages, emitted_ids, emitted_id_order);

    if messages.is_empty() {
        return Ok(format!(
            ": polyedge {channel} stream heartbeat {sequence}\n\n"
        ));
    }

    Ok(messages
        .iter()
        .map(format_sse_message)
        .collect::<Vec<_>>()
        .join(""))
}

fn filter_new_sse_messages(
    messages: Vec<SseMessage>,
    emitted_ids: &mut HashSet<String>,
    emitted_id_order: &mut VecDeque<String>,
) -> Vec<SseMessage> {
    messages
        .into_iter()
        .filter(|message| remember_stream_event_id(&message.id, emitted_ids, emitted_id_order))
        .collect()
}

fn remember_stream_event_id(
    event_id: &str,
    emitted_ids: &mut HashSet<String>,
    emitted_id_order: &mut VecDeque<String>,
) -> bool {
    if !emitted_ids.insert(event_id.to_string()) {
        return false;
    }

    emitted_id_order.push_back(event_id.to_string());

    while emitted_ids.len() > MAX_STREAM_EMITTED_IDS {
        let Some(oldest_id) = emitted_id_order.pop_front() else {
            break;
        };
        emitted_ids.remove(&oldest_id);
    }

    true
}

async fn signal_stream_messages(state: &AppState) -> polyedge_domain::Result<Vec<SseMessage>> {
    let markets = state
        .market_event_service
        .list_markets(MarketListFilters::new(None, None, Some(100))?)
        .await?;
    let signals = state
        .market_event_service
        .list_signals(SignalListFilters::new(None, None, None, Some(50))?)
        .await?;

    Ok(signals
        .into_iter()
        .map(|signal| {
            let market = markets.iter().find(|market| market.id == signal.market_id);
            let event = match signal.lifecycle_state.as_str() {
                "new" => "signal.created",
                "invalidated" => "signal.invalidated",
                _ => "signal.updated",
            };

            SseMessage {
                id: format!("signals:{}:{}", signal.id, signal.version),
                event,
                data: json!({
                    "signal_id": signal.id,
                    "market_id": signal.market_id,
                    "market_question": market.map(|market| market.question.clone()),
                    "context_label": market.map(|market| {
                        format!("{} / {}", market.category, market.tradability_status.as_str())
                    }),
                    "version": signal.version,
                    "lifecycle_state": signal.lifecycle_state,
                    "side": signal.side,
                    "fair_price": signal.fair_price,
                    "market_price": signal.market_price,
                    "edge": signal.edge,
                    "confidence": signal.confidence,
                    "reason": signal.reason,
                    "risk_decision": signal.risk_decision,
                    "evidence_lines": Vec::<String>::new(),
                    "updated_at": format_timestamp(signal.updated_at),
                }),
            }
        })
        .collect())
}

async fn risk_stream_messages(state: &AppState) -> polyedge_domain::Result<Vec<SseMessage>> {
    let snapshot = read_console_risk_snapshot(state).await?;
    let open_alerts = snapshot
        .alerts
        .iter()
        .filter(|alert| alert.status != AlertStatus::Contained)
        .count();
    let critical_alerts = snapshot
        .alerts
        .iter()
        .filter(|alert| alert.severity == AlertSeverity::Critical)
        .count();
    let warning_alerts = snapshot
        .alerts
        .iter()
        .filter(|alert| alert.severity == AlertSeverity::Warning)
        .count();
    let risk_state = risk_state_to_contract(
        snapshot.risk_state.clone(),
        snapshot.environment.clone(),
        state.risk_service.policy(),
        Some(open_alerts.try_into().unwrap_or(u32::MAX)),
    )?;
    let mut messages = vec![SseMessage {
        id: format!("risk:{}:{}", risk_state.mode.as_str(), risk_state.version),
        event: "risk.mode_changed",
        data: json!({
            "resource_id": risk_state.id,
            "version": risk_state.version,
            "mode": risk_state.mode,
            "environment": risk_state.environment,
            "kill_switch": risk_state.kill_switch,
            "daily_pnl": risk_state.daily_pnl,
            "gross_exposure": risk_state.gross_exposure,
            "net_exposure": risk_state.net_exposure,
            "daily_loss_limit": risk_state.daily_loss_limit,
            "daily_loss_used": risk_state.daily_loss_used,
            "open_alerts": risk_state.open_alerts,
            "critical_alerts": critical_alerts,
            "warning_alerts": warning_alerts,
            "updated_at": format_timestamp(risk_state.updated_at),
        }),
    }];

    messages.extend(snapshot.alerts.into_iter().map(|alert| {
        let alert_id = alert.id;

        SseMessage {
            id: format!("risk:alert:{}:{}", alert_id, alert.version),
            event: "risk.alerted",
            data: json!({
                "resource_id": alert_id.clone(),
                "version": alert.version,
                "alert_id": alert_id,
                "severity": alert.severity,
                "reason": alert.reason,
                "target": alert.target,
                "status": alert.status,
                "created_at": format_timestamp(alert.created_at),
                "updated_at": format_timestamp(alert.updated_at),
            }),
        }
    }));
    Ok(messages)
}

async fn event_stream_messages(state: &AppState) -> polyedge_domain::Result<Vec<SseMessage>> {
    let events = state
        .market_event_service
        .list_events(EventListFilters::new(None, Some(50))?)
        .await?;

    Ok(events
        .into_iter()
        .map(|event| SseMessage {
            id: format!("events:{}:{}", event.id, event.version),
            event: "event.created",
            data: json!({
                "event_id": event.id,
                "source": event.source,
                "summary": event.summary,
                "confidence": event.confidence,
                "created_at": format_timestamp(event.created_at),
                "version": event.version,
            }),
        })
        .collect())
}

async fn arbitrage_stream_messages(
    state: &AppState,
    last_sequence: &mut Option<u64>,
) -> polyedge_domain::Result<Vec<SseMessage>> {
    let events = state
        .arbitrage_service
        .list_events(ArbitrageEventListFilters::new(*last_sequence, Some(100))?)
        .await?;

    if let Some(sequence) = events.last().map(|event| event.sequence) {
        *last_sequence = Some(sequence);
    }

    Ok(events
        .into_iter()
        .map(|event| SseMessage {
            id: event.sequence.to_string(),
            event: event.event_type.as_str(),
            data: arbitrage_event_sse_data(event),
        })
        .collect())
}

fn arbitrage_event_sse_data(event: ArbitrageEventView) -> Value {
    let mut data = match event.payload {
        Value::Object(map) => map,
        payload => {
            let mut map = Map::new();
            map.insert("payload".to_string(), payload);
            map
        }
    };

    data.insert("sequence".to_string(), json!(event.sequence));
    data.insert("event_id".to_string(), json!(event.id));
    data.insert("event_type".to_string(), json!(event.event_type.as_str()));
    data.insert("resource_type".to_string(), json!(event.resource_type));
    data.insert("resource_id".to_string(), json!(event.resource_id));
    data.insert(
        "occurred_at".to_string(),
        json!(format_timestamp(event.occurred_at)),
    );
    data.insert("trace_id".to_string(), json!(event.trace_id));
    Value::Object(data)
}

fn daily_loss_used(risk_state: &RiskStateView) -> polyedge_domain::Result<UsdAmount> {
    let daily_pnl = risk_state.daily_pnl.value();

    if daily_pnl < Decimal::ZERO {
        return UsdAmount::new(-daily_pnl);
    }

    UsdAmount::new(Decimal::ZERO)
}

fn format_sse_message(message: &SseMessage) -> String {
    format!(
        "id: {}\nevent: {}\ndata: {}\n\n",
        message.id, message.event, message.data
    )
}

fn format_timestamp(timestamp: OffsetDateTime) -> String {
    timestamp
        .format(&Rfc3339)
        .unwrap_or_else(|_| timestamp.to_string())
}
