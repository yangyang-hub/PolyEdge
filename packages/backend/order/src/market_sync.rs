use polyedge_application::{
    MarketUpsertOptions, MarketView, RewardEventEndPolicy, RewardEventScheduleStatus,
    RewardEventTimeConfidence, RewardEventTimePrecision, RewardEventTimeRole,
    RewardEventWindowSourceCoverage, RewardEventWindowSourceSnapshot, RewardMarket,
    RewardMarketEventWindow, RewardToken,
};
use polyedge_connectors::{
    GammaEventStartSource, GammaScheduleStatus, GammaScheduledEventKind, PolymarketGammaConnector,
    PolymarketGammaMarket, PolymarketGammaScheduledEvent, PolymarketRewardMarket,
    PolymarketRewardsConnector,
};
use polyedge_domain::{AppError, Result};
use polyedge_infrastructure::AppState;
use rust_decimal::Decimal;
use serde_json::json;
use sqlx::Row;
use time::OffsetDateTime;

pub struct PriorityMarketSyncReport {
    pub condition_ids: usize,
    pub fetched: usize,
    pub upserted: usize,
}

static MARKET_UPSERT_GATE: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
const GENERAL_MARKET_SYNC_PAGE_SIZE: u16 = 100;
const GAMMA_EVENT_WINDOW_SOURCE: &str = "gamma";
const GAMMA_EVENT_WINDOW_PRODUCER_VERSION: u32 = 2;

pub async fn sync_general_markets_once(
    state: &AppState,
    trace_id: &str,
    upsert_options: MarketUpsertOptions,
) -> Result<usize> {
    let observed_at = OffsetDateTime::now_utc();
    let connector = PolymarketGammaConnector::new(&state.settings.polymarket.gamma_host)?;
    let gamma_markets = connector
        .fetch_markets(GENERAL_MARKET_SYNC_PAGE_SIZE)
        .await?;
    let event_window_snapshot = gamma_event_window_snapshot(&gamma_markets, observed_at);
    let views: Vec<MarketView> = gamma_markets
        .iter()
        .cloned()
        .map(gamma_market_to_view)
        .collect();
    let _guard = MARKET_UPSERT_GATE.lock().await;
    let upserted = state
        .market_event_service
        .upsert_markets_with_options(&views, trace_id, upsert_options)
        .await?;
    if let Err(error) = state
        .reward_bot_service
        .replace_market_event_windows(&event_window_snapshot)
        .await
    {
        tracing::warn!(error = %error, "failed to replace Gamma reward event-window candidates");
    }
    Ok(upserted)
}

pub async fn sync_reward_markets_once(state: &AppState) -> Result<usize> {
    let rewards_connector = PolymarketRewardsConnector::new(&state.settings.polymarket.clob_host)?;
    let reward_markets_raw = rewards_connector.fetch_current_markets().await?;
    let reward_markets: Vec<RewardMarket> = reward_markets_raw
        .into_iter()
        .map(reward_market_from_connector)
        .collect();
    if reward_markets.is_empty() {
        return Err(AppError::dependency_unavailable(
            "POLYMARKET_REWARDS_MARKETS_EMPTY",
            "refusing to replace reward catalog with an empty snapshot",
        ));
    }
    let reward_upserted = reward_markets.len();
    state
        .reward_bot_service
        .upsert_reward_markets(&reward_markets)
        .await?;
    Ok(reward_upserted)
}

pub async fn sync_priority_markets_once(
    state: &AppState,
    trace_id: &str,
    max_condition_ids: usize,
    reward_candidate_stale_minutes: u64,
) -> Result<PriorityMarketSyncReport> {
    let condition_ids =
        collect_priority_condition_ids(state, max_condition_ids, reward_candidate_stale_minutes)
            .await;
    if condition_ids.is_empty() {
        return Ok(PriorityMarketSyncReport {
            condition_ids: 0,
            fetched: 0,
            upserted: 0,
        });
    }

    let observed_at = OffsetDateTime::now_utc();
    let connector = PolymarketGammaConnector::new(&state.settings.polymarket.gamma_host)?;
    let gamma_markets = connector
        .fetch_markets_by_condition_ids(&condition_ids)
        .await?;
    let fetched = gamma_markets.len();
    let event_window_snapshot = gamma_event_window_snapshot(&gamma_markets, observed_at);
    let views: Vec<MarketView> = gamma_markets
        .iter()
        .cloned()
        .map(gamma_market_to_view)
        .collect();
    let _guard = MARKET_UPSERT_GATE.lock().await;
    let upserted = state
        .market_event_service
        .upsert_markets(&views, trace_id)
        .await?;
    if let Err(error) = state
        .reward_bot_service
        .replace_market_event_windows(&event_window_snapshot)
        .await
    {
        tracing::warn!(error = %error, "failed to replace priority Gamma reward event-window candidates");
    }

    Ok(PriorityMarketSyncReport {
        condition_ids: condition_ids.len(),
        fetched,
        upserted,
    })
}

async fn collect_priority_condition_ids(
    state: &AppState,
    max_condition_ids: usize,
    reward_candidate_stale_minutes: u64,
) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    let mut condition_ids = Vec::new();

    match registered_token_condition_ids(state, max_condition_ids.saturating_mul(2)).await {
        Ok(registered) => {
            for condition_id in registered {
                push_condition_id(
                    &mut condition_ids,
                    &mut seen,
                    condition_id,
                    max_condition_ids,
                );
            }
        }
        Err(error) => {
            tracing::warn!(error = %error, "failed to map registered orderbook tokens to markets");
        }
    }

    if condition_ids.len() < max_condition_ids {
        let remaining = max_condition_ids.saturating_sub(condition_ids.len());
        match state
            .reward_bot_service
            .list_priority_reward_condition_ids(reward_candidate_stale_minutes, remaining)
            .await
        {
            Ok(reward_condition_ids) => {
                for condition_id in reward_condition_ids {
                    push_condition_id(
                        &mut condition_ids,
                        &mut seen,
                        condition_id,
                        max_condition_ids,
                    );
                }
            }
            Err(error) => {
                tracing::warn!(error = %error, "failed to list priority rewards markets");
            }
        }
    }

    if condition_ids.len() < max_condition_ids {
        match active_reward_catalog_condition_ids(state, max_condition_ids).await {
            Ok(reward_condition_ids) => {
                for condition_id in reward_condition_ids {
                    push_condition_id(
                        &mut condition_ids,
                        &mut seen,
                        condition_id,
                        max_condition_ids,
                    );
                }
            }
            Err(error) => {
                tracing::warn!(
                    error = %error,
                    "failed to list active rewards catalog fallback markets"
                );
            }
        }
    }

    condition_ids
}

async fn registered_token_condition_ids(
    state: &AppState,
    max_token_ids: usize,
) -> Result<Vec<String>> {
    let token_ids = state
        .orderbook_registry
        .list_all_tokens()
        .await
        .into_iter()
        .take(max_token_ids)
        .collect::<Vec<_>>();
    if token_ids.is_empty() {
        return Ok(Vec::new());
    }

    let Some(pool) = state.dependencies.postgres.as_ref() else {
        return Ok(Vec::new());
    };

    let rows = sqlx::query(
        r#"
        SELECT token_id, condition_id
        FROM (
            SELECT polymarket_yes_asset_id AS token_id,
                   polymarket_condition_id AS condition_id
            FROM markets
            WHERE polymarket_yes_asset_id = ANY($1)
              AND polymarket_yes_asset_id IS NOT NULL
              AND polymarket_condition_id IS NOT NULL
            UNION ALL
            SELECT polymarket_no_asset_id AS token_id,
                   polymarket_condition_id AS condition_id
            FROM markets
            WHERE polymarket_no_asset_id = ANY($1)
              AND polymarket_no_asset_id IS NOT NULL
              AND polymarket_condition_id IS NOT NULL
        ) refs
        "#,
    )
    .bind(&token_ids)
    .fetch_all(pool)
    .await
    .map_err(|error| {
        AppError::dependency_unavailable(
            "POSTGRES_QUERY_FAILED",
            format!("failed to map orderbook token ids to Gamma condition ids: {error}"),
        )
    })?;

    let mut token_to_condition = std::collections::HashMap::new();
    for row in rows {
        let token_id: String = row.try_get("token_id").map_err(postgres_decode_error)?;
        let condition_id: String = row.try_get("condition_id").map_err(postgres_decode_error)?;
        token_to_condition.insert(token_id, condition_id);
    }

    let mut seen = std::collections::HashSet::new();
    let mut condition_ids = Vec::new();
    for token_id in token_ids {
        let Some(condition_id) = token_to_condition.get(&token_id) else {
            continue;
        };
        if seen.insert(condition_id.clone()) {
            condition_ids.push(condition_id.clone());
        }
    }

    Ok(condition_ids)
}

async fn active_reward_catalog_condition_ids(
    state: &AppState,
    max_condition_ids: usize,
) -> Result<Vec<String>> {
    if max_condition_ids == 0 {
        return Ok(Vec::new());
    }

    let Some(pool) = state.dependencies.postgres.as_ref() else {
        return Ok(Vec::new());
    };

    let rows = sqlx::query(
        r#"
        SELECT condition_id
        FROM reward_markets
        WHERE active = true
          AND rewards_max_spread > 0
          AND jsonb_array_length(tokens_json) = 2
        ORDER BY total_daily_rate DESC, updated_at DESC
        LIMIT $1
        "#,
    )
    .bind(i64::try_from(max_condition_ids).unwrap_or(i64::MAX))
    .fetch_all(pool)
    .await
    .map_err(|error| {
        AppError::dependency_unavailable(
            "POSTGRES_QUERY_FAILED",
            format!("failed to list active rewards catalog condition ids: {error}"),
        )
    })?;

    rows.into_iter()
        .map(|row| row.try_get("condition_id").map_err(postgres_decode_error))
        .collect()
}

fn push_condition_id(
    condition_ids: &mut Vec<String>,
    seen: &mut std::collections::HashSet<String>,
    condition_id: String,
    max_condition_ids: usize,
) {
    if condition_ids.len() >= max_condition_ids {
        return;
    }
    let condition_id = condition_id.trim();
    if condition_id.is_empty() || !seen.insert(condition_id.to_string()) {
        return;
    }
    condition_ids.push(condition_id.to_string());
}

fn postgres_decode_error(error: sqlx::Error) -> AppError {
    AppError::dependency_unavailable(
        "POSTGRES_DECODE_FAILED",
        format!("failed to decode Postgres row: {error}"),
    )
}

fn gamma_market_to_view(market: PolymarketGammaMarket) -> MarketView {
    MarketView {
        id: market.id,
        slug: market.slug,
        question: market.question,
        category: market.category,
        status: market.status,
        best_bid: market.best_bid,
        best_ask: market.best_ask,
        mid_price: market.mid_price,
        volume_24h: market.volume_24h,
        liquidity_usd: market.liquidity_usd,
        end_at: market.resolution_deadline_at,
        ambiguity_level: market.ambiguity_level,
        tradability_status: market.tradability_status,
        resolution_source: market.resolution_source,
        edge_case_notes: market.edge_case_notes,
        polymarket_condition_id: Some(market.condition_id),
        polymarket_yes_asset_id: Some(market.yes_asset_id),
        polymarket_no_asset_id: Some(market.no_asset_id),
        updated_at: market.updated_at,
        version: market.version,
    }
}

fn gamma_event_window_snapshot(
    markets: &[PolymarketGammaMarket],
    observed_at: OffsetDateTime,
) -> RewardEventWindowSourceSnapshot {
    RewardEventWindowSourceSnapshot {
        source: GAMMA_EVENT_WINDOW_SOURCE.to_string(),
        producer_version: GAMMA_EVENT_WINDOW_PRODUCER_VERSION,
        observed_at,
        coverage: markets
            .iter()
            .map(|market| RewardEventWindowSourceCoverage {
                condition_id: market.condition_id.clone(),
                source_updated_at: Some(market.updated_at),
            })
            .collect(),
        windows: markets
            .iter()
            .flat_map(|market| gamma_market_to_event_windows(market, observed_at))
            .collect(),
    }
}

fn gamma_market_to_event_windows(
    market: &PolymarketGammaMarket,
    observed_at: OffsetDateTime,
) -> Vec<RewardMarketEventWindow> {
    market
        .scheduled_events
        .iter()
        .map(|event| gamma_scheduled_event_to_window(market, event, observed_at))
        .collect()
}

fn gamma_scheduled_event_to_window(
    market: &PolymarketGammaMarket,
    event: &PolymarketGammaScheduledEvent,
    observed_at: OffsetDateTime,
) -> RewardMarketEventWindow {
    let confidence = if market.has_reviewed_dates {
        RewardEventTimeConfidence::Medium
    } else {
        RewardEventTimeConfidence::Low
    };
    let schedule_status = match event.status {
        GammaScheduleStatus::Scheduled => RewardEventScheduleStatus::Scheduled,
        GammaScheduleStatus::Conflicting => RewardEventScheduleStatus::Conflicting,
        GammaScheduleStatus::Finished => RewardEventScheduleStatus::Finished,
    };
    let is_scheduled_sports = event.kind == GammaScheduledEventKind::Sports
        && event.status == GammaScheduleStatus::Scheduled;
    let event_end_at = match event.status {
        GammaScheduleStatus::Finished => event.finished_at,
        GammaScheduleStatus::Scheduled if event.kind == GammaScheduledEventKind::Sports => {
            market.resolution_deadline_at
        }
        GammaScheduleStatus::Scheduled | GammaScheduleStatus::Conflicting => None,
    };
    let end_policy = match event.status {
        GammaScheduleStatus::Finished => RewardEventEndPolicy::Explicit,
        GammaScheduleStatus::Scheduled if event.kind == GammaScheduledEventKind::Sports => {
            RewardEventEndPolicy::UntilMarketClosed
        }
        GammaScheduleStatus::Scheduled => RewardEventEndPolicy::Point,
        GammaScheduleStatus::Conflicting => RewardEventEndPolicy::Unknown,
    };
    let hard_gate_eligible = is_scheduled_sports && event.start_at.is_some();
    let start_source_field = event.start_source.map(|source| match source {
        GammaEventStartSource::GameStartTime => "gameStartTime".to_string(),
        GammaEventStartSource::EventStartTime => "events[].startTime".to_string(),
        GammaEventStartSource::Corroborated => "gameStartTime+events[].startTime".to_string(),
    });
    let event_type = match event.kind {
        GammaScheduledEventKind::Sports => "sports",
        GammaScheduledEventKind::OtherStructured => "other_structured",
    };

    RewardMarketEventWindow {
        condition_id: market.condition_id.clone(),
        source: GAMMA_EVENT_WINDOW_SOURCE.to_string(),
        event_key: event.event_key.clone(),
        event_type: event_type.to_string(),
        event_time_role: RewardEventTimeRole::EventOccurrence,
        schedule_status,
        time_precision: if event.status == GammaScheduleStatus::Conflicting
            || event.start_at.is_none()
        {
            RewardEventTimePrecision::Unknown
        } else {
            RewardEventTimePrecision::Exact
        },
        start_source_field,
        end_policy,
        event_start_at: event.start_at,
        event_end_at,
        confidence,
        source_url: market
            .slug
            .as_ref()
            .map(|slug| format!("https://polymarket.com/market/{slug}")),
        source_payload: json!({
            "market_id": market.id,
            "slug": market.slug,
            "gamma_event_id": event.gamma_event_id,
            "title": event.title,
            "event_key": event.event_key,
            "event_kind": event_type,
            "schedule_status": gamma_schedule_status_name(event.status),
            "start_source": gamma_event_start_source_name(event.start_source),
            "sports_market_type": event.sports_market_type,
            "game_id": event.game_id,
            "series_slug": event.series_slug,
            "lifecycle_started_at": market.lifecycle_started_at,
            "resolution_deadline_at": market.resolution_deadline_at,
            "has_reviewed_dates": market.has_reviewed_dates,
        }),
        notes: "Polymarket Gamma explicit scheduled-event candidate; lifecycle dates are excluded."
            .to_string(),
        active: true,
        hard_gate_eligible,
        producer_version: GAMMA_EVENT_WINDOW_PRODUCER_VERSION,
        source_updated_at: Some(market.updated_at),
        observed_at: Some(observed_at),
        expires_at: None,
        reviewed_by: None,
        reviewed_at: None,
        updated_at: observed_at,
    }
}

fn gamma_schedule_status_name(status: GammaScheduleStatus) -> &'static str {
    match status {
        GammaScheduleStatus::Scheduled => "scheduled",
        GammaScheduleStatus::Conflicting => "conflicting",
        GammaScheduleStatus::Finished => "finished",
    }
}

fn gamma_event_start_source_name(source: Option<GammaEventStartSource>) -> Option<&'static str> {
    source.map(|source| match source {
        GammaEventStartSource::GameStartTime => "gameStartTime",
        GammaEventStartSource::EventStartTime => "events[].startTime",
        GammaEventStartSource::Corroborated => "gameStartTime+events[].startTime",
    })
}

fn reward_market_from_connector(market: PolymarketRewardMarket) -> RewardMarket {
    RewardMarket {
        condition_id: market.condition_id,
        question: market.question,
        market_slug: market.market_slug,
        event_slug: market.event_slug,
        category: String::new(),
        image: market.image,
        rewards_max_spread: market.rewards_max_spread,
        rewards_min_size: market.rewards_min_size,
        total_daily_rate: market.total_daily_rate,
        liquidity_usd: Decimal::ZERO,
        volume_24h_usd: Decimal::ZERO,
        market_spread_cents: Decimal::ZERO,
        end_at: None,
        ambiguity_level: "unknown".to_string(),
        market_synced_at: None,
        tokens: market
            .tokens
            .into_iter()
            .map(|token| RewardToken {
                token_id: token.token_id,
                outcome: token.outcome,
                price: token.price,
            })
            .collect(),
        active: market.active,
        updated_at: market.updated_at,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use polyedge_domain::{
        AmbiguityLevel, MarketStatus, Probability, TradabilityStatus, UsdAmount,
    };
    use time::Duration as TimeDuration;

    fn test_gamma_market(
        now: OffsetDateTime,
        reviewed_dates: bool,
        scheduled_events: Vec<PolymarketGammaScheduledEvent>,
    ) -> PolymarketGammaMarket {
        let midpoint = Decimal::from_str_exact("0.50").expect("midpoint");
        PolymarketGammaMarket {
            id: "gamma-market-1".to_string(),
            slug: Some("fixture-market".to_string()),
            question: "Fixture market?".to_string(),
            category: "Sports".to_string(),
            status: MarketStatus::Open,
            best_bid: Probability::new(midpoint).expect("best bid"),
            best_ask: Probability::new(midpoint).expect("best ask"),
            mid_price: Probability::new(midpoint).expect("mid price"),
            volume_24h: UsdAmount::new(Decimal::from(1_000_u64)).expect("volume"),
            liquidity_usd: UsdAmount::new(Decimal::from(1_000_u64)).expect("liquidity"),
            lifecycle_started_at: Some(now - TimeDuration::days(30)),
            resolution_deadline_at: Some(now + TimeDuration::days(7)),
            scheduled_events,
            has_reviewed_dates: reviewed_dates,
            ambiguity_level: AmbiguityLevel::Low,
            tradability_status: TradabilityStatus::Tradable,
            resolution_source: "fixture".to_string(),
            edge_case_notes: Vec::new(),
            condition_id: "condition-1".to_string(),
            yes_asset_id: "yes-token".to_string(),
            no_asset_id: "no-token".to_string(),
            outcome_token_ids: vec!["yes-token".to_string(), "no-token".to_string()],
            outcomes: vec!["Yes".to_string(), "No".to_string()],
            outcome_prices: vec![midpoint, midpoint],
            updated_at: now,
            version: now.unix_timestamp(),
        }
    }

    fn scheduled_event(
        now: OffsetDateTime,
        event_key: &str,
        kind: GammaScheduledEventKind,
        status: GammaScheduleStatus,
    ) -> PolymarketGammaScheduledEvent {
        PolymarketGammaScheduledEvent {
            event_key: event_key.to_string(),
            gamma_event_id: Some(event_key.to_string()),
            title: Some("Fixture event".to_string()),
            kind,
            status,
            start_at: (status != GammaScheduleStatus::Conflicting)
                .then_some(now + TimeDuration::hours(6)),
            start_source: (status != GammaScheduleStatus::Conflicting)
                .then_some(GammaEventStartSource::EventStartTime),
            finished_at: (status == GammaScheduleStatus::Finished).then_some(now),
            sports_market_type: (kind == GammaScheduledEventKind::Sports)
                .then(|| "moneyline".to_string()),
            game_id: (kind == GammaScheduledEventKind::Sports).then_some(42),
            series_slug: (kind == GammaScheduledEventKind::Sports)
                .then(|| "fixture-series".to_string()),
        }
    }

    #[test]
    fn gamma_snapshot_covers_markets_without_discrete_events_for_tombstones() {
        let now = OffsetDateTime::from_unix_timestamp(1_784_000_000).expect("fixed time");
        let market = test_gamma_market(now, true, Vec::new());

        let snapshot = gamma_event_window_snapshot(&[market], now);

        assert_eq!(snapshot.source, "gamma");
        assert_eq!(snapshot.coverage[0].condition_id, "condition-1");
        assert!(snapshot.windows.is_empty());
    }

    #[test]
    fn gamma_scheduled_sports_event_is_the_only_hard_gate_shape() {
        let now = OffsetDateTime::from_unix_timestamp(1_784_000_000).expect("fixed time");
        let market = test_gamma_market(
            now,
            true,
            vec![scheduled_event(
                now,
                "event:sports-1",
                GammaScheduledEventKind::Sports,
                GammaScheduleStatus::Scheduled,
            )],
        );

        let window = gamma_event_window_snapshot(&[market], now)
            .windows
            .into_iter()
            .next()
            .expect("sports window");

        assert_eq!(window.source, "gamma");
        assert_eq!(window.event_key, "event:sports-1");
        assert_eq!(window.event_time_role, RewardEventTimeRole::EventOccurrence);
        assert_eq!(window.schedule_status, RewardEventScheduleStatus::Scheduled);
        assert_eq!(window.time_precision, RewardEventTimePrecision::Exact);
        assert_eq!(window.end_policy, RewardEventEndPolicy::UntilMarketClosed);
        assert_eq!(window.confidence, RewardEventTimeConfidence::Medium);
        assert!(window.hard_gate_eligible);
        assert_eq!(window.producer_version, 2);
    }

    #[test]
    fn gamma_other_and_conflicting_events_are_audited_but_not_hard_gated() {
        let now = OffsetDateTime::from_unix_timestamp(1_784_000_000).expect("fixed time");
        let market = test_gamma_market(
            now,
            false,
            vec![
                scheduled_event(
                    now,
                    "event:other-1",
                    GammaScheduledEventKind::OtherStructured,
                    GammaScheduleStatus::Scheduled,
                ),
                scheduled_event(
                    now,
                    "event:conflict-1",
                    GammaScheduledEventKind::Sports,
                    GammaScheduleStatus::Conflicting,
                ),
            ],
        );

        let snapshot = gamma_event_window_snapshot(&[market], now);

        assert_eq!(snapshot.windows.len(), 2);
        assert!(
            snapshot
                .windows
                .iter()
                .all(|window| !window.hard_gate_eligible)
        );
        assert_eq!(snapshot.windows[0].end_policy, RewardEventEndPolicy::Point);
        assert_eq!(
            snapshot.windows[1].schedule_status,
            RewardEventScheduleStatus::Conflicting
        );
        assert_eq!(
            snapshot.windows[1].time_precision,
            RewardEventTimePrecision::Unknown
        );
        assert_eq!(
            snapshot.windows[0].confidence,
            RewardEventTimeConfidence::Low
        );
    }

    #[test]
    fn gamma_snapshot_keeps_multiple_events_separate() {
        let now = OffsetDateTime::from_unix_timestamp(1_784_000_000).expect("fixed time");
        let market = test_gamma_market(
            now,
            true,
            vec![
                scheduled_event(
                    now,
                    "event:sports-1",
                    GammaScheduledEventKind::Sports,
                    GammaScheduleStatus::Scheduled,
                ),
                scheduled_event(
                    now,
                    "event:sports-2",
                    GammaScheduledEventKind::Sports,
                    GammaScheduleStatus::Scheduled,
                ),
            ],
        );

        let snapshot = gamma_event_window_snapshot(&[market], now);

        assert_eq!(snapshot.windows.len(), 2);
        assert_eq!(snapshot.windows[0].event_key, "event:sports-1");
        assert_eq!(snapshot.windows[1].event_key, "event:sports-2");
    }
}
