/// Gamma currently emits most datetimes as RFC3339, while `gameStartTime` can
/// use a space separator and a short numeric offset such as `+00`. Normalize
/// that documented live shape before the strict RFC3339 parse.
fn parse_gamma_datetime(value: Option<&str>) -> Option<OffsetDateTime> {
    let value = value?.trim();
    if value.is_empty() {
        return None;
    }
    if let Ok(parsed) = OffsetDateTime::parse(value, &Rfc3339) {
        return Some(parsed);
    }

    let mut normalized = value.replacen(' ', "T", 1);
    let bytes = normalized.as_bytes();
    if bytes.len() >= 3 {
        let offset_start = bytes.len() - 3;
        let offset = &bytes[offset_start..];
        if matches!(offset[0], b'+' | b'-')
            && offset[1].is_ascii_digit()
            && offset[2].is_ascii_digit()
        {
            normalized.push_str(":00");
        }
    }

    OffsetDateTime::parse(&normalized, &Rfc3339).ok()
}

fn gamma_scheduled_events(raw: &RawGammaMarket) -> Vec<PolymarketGammaScheduledEvent> {
    let market_id = raw.id.trim();
    let sports_market_type = normalize_optional_text(raw.sports_market_type.clone());
    let game_start_at = parse_gamma_datetime(raw.game_start_time.as_deref());
    let single_event = raw.events.len() == 1;
    let mut scheduled_events = Vec::new();
    let mut market_game_start_consumed = false;

    for event in &raw.events {
        let gamma_event_id = parse_gamma_event_id(event.id.as_ref());
        let event_start_at = parse_gamma_datetime(event.start_time.as_deref());
        let finished_at = parse_gamma_datetime(event.finished_timestamp.as_deref());
        let is_sports = sports_market_type.is_some() || event.game_id.is_some();
        let applicable_game_start = if single_event { game_start_at } else { None };

        let (start_at, start_source, conflicting) =
            resolve_gamma_event_start(applicable_game_start, event_start_at, is_sports);
        if applicable_game_start.is_some() && is_sports {
            market_game_start_consumed = true;
        }
        if start_at.is_none() && !conflicting && finished_at.is_none() {
            continue;
        }

        let Some(event_key) = gamma_event_key(market_id, gamma_event_id.as_deref(), single_event)
        else {
            // An ordinal or timestamp would not survive upstream reordering or
            // rescheduling, so ambiguous events without a stable id stay out of
            // the scheduled-event model.
            continue;
        };
        let status = if conflicting {
            GammaScheduleStatus::Conflicting
        } else if finished_at.is_some() {
            GammaScheduleStatus::Finished
        } else {
            GammaScheduleStatus::Scheduled
        };

        scheduled_events.push(PolymarketGammaScheduledEvent {
            event_key,
            gamma_event_id,
            title: normalize_optional_text(event.title.clone()),
            kind: if is_sports {
                GammaScheduledEventKind::Sports
            } else {
                GammaScheduledEventKind::OtherStructured
            },
            status,
            start_at,
            start_source,
            finished_at,
            sports_market_type: sports_market_type.clone(),
            game_id: event.game_id,
            series_slug: normalize_optional_text(event.series_slug.clone()),
        });
    }

    if let Some(start_at) = game_start_at
        && !market_game_start_consumed
    {
        // `gameStartTime` alone is an explicit timestamp, but it is not enough
        // to classify a market as sports. Only a structured sports market type
        // makes this market-level fallback enforceable as a sports occurrence.
        scheduled_events.push(PolymarketGammaScheduledEvent {
            event_key: format!("market:{market_id}:game"),
            gamma_event_id: None,
            title: None,
            kind: if sports_market_type.is_some() {
                GammaScheduledEventKind::Sports
            } else {
                GammaScheduledEventKind::OtherStructured
            },
            status: GammaScheduleStatus::Scheduled,
            start_at: Some(start_at),
            start_source: Some(GammaEventStartSource::GameStartTime),
            finished_at: None,
            sports_market_type,
            game_id: None,
            series_slug: None,
        });
    }

    scheduled_events
}

fn resolve_gamma_event_start(
    game_start_at: Option<OffsetDateTime>,
    event_start_at: Option<OffsetDateTime>,
    is_sports: bool,
) -> (
    Option<OffsetDateTime>,
    Option<GammaEventStartSource>,
    bool,
) {
    match (game_start_at, event_start_at) {
        (Some(game_start_at), Some(event_start_at)) if is_sports => {
            let delta = (game_start_at - event_start_at).whole_seconds().abs();
            if delta <= GAMMA_EVENT_START_AGREEMENT_TOLERANCE_SECS {
                (
                    Some(event_start_at),
                    Some(GammaEventStartSource::Corroborated),
                    false,
                )
            } else {
                (None, None, true)
            }
        }
        (Some(game_start_at), _) if is_sports => (
            Some(game_start_at),
            Some(GammaEventStartSource::GameStartTime),
            false,
        ),
        (_, Some(event_start_at)) => (
            Some(event_start_at),
            Some(GammaEventStartSource::EventStartTime),
            false,
        ),
        _ => (None, None, false),
    }
}

fn gamma_event_key(
    market_id: &str,
    gamma_event_id: Option<&str>,
    single_event: bool,
) -> Option<String> {
    gamma_event_id
        .map(|event_id| format!("event:{event_id}"))
        .or_else(|| single_event.then(|| format!("market:{market_id}:event")))
}

fn parse_gamma_event_id(value: Option<&JsonValue>) -> Option<String> {
    match value? {
        JsonValue::String(value) => normalize_optional_text(Some(value.clone())),
        JsonValue::Number(value) => Some(value.to_string()),
        _ => None,
    }
}
