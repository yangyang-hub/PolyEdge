const SMART_MONEY_CANDIDATE_SOURCE: &str = "copytrade_tracked";
const SMART_MONEY_LEADERBOARD_SOURCE: &str = "polymarket_leaderboard";
const SMART_MONEY_TRADE_SOURCE_ACTIVITY: &str = "polymarket_data_api_activity";
const SMART_MONEY_CLOSED_POSITION_LIMIT: u16 = 50;
const SMART_MONEY_CLOSED_POSITION_PAGES: u32 = 3;
const SMART_MONEY_TRADE_LIMIT: u16 = 100;
const SMART_MONEY_LEADERBOARD_LIMIT: u16 = 50;
const SMART_MONEY_EXCLUDED_CANDIDATE_LIMIT: u16 = 500;
const SMART_MONEY_MAX_WALLETS_PER_SCAN: usize = 50;
const SMART_MONEY_SIGNAL_TRADE_LIMIT: u16 = 200;
const SMART_MONEY_SIGNAL_ADVISORY_SIGNAL_LIMIT: u16 = 100;
const SMART_MONEY_SIGNAL_ADVISORY_CONTEXT_LIMIT: u16 = 500;
const SMART_MONEY_SIGNAL_ADVISORY_TTL_SEC: u64 = 900;
const SMART_MONEY_SIGNAL_ADVISORY_LLM_TASK_TYPE: &str = "smart_signal_advisory";
const SMART_MONEY_SIGNAL_ADVISORY_PROMPT_VERSION: &str = "smart_signal_advisory_schema_v1";

#[derive(Debug, Default)]
struct SmartMoneyWalletInputs {
    activities: Vec<PolymarketWalletActivity>,
    positions: Vec<PolymarketWalletPosition>,
    closed_positions: Vec<PolymarketClosedPosition>,
    trades: Vec<PolymarketTrade>,
}

#[derive(Debug, Clone)]
struct SmartMoneyWalletScanTarget {
    address: String,
    source: String,
}

async fn run_smart_money_once(
    state: &AppState,
    trace_id: &str,
) -> Result<SmartMoneyRunReport> {
    let config = state.smart_money_service.read_config().await?;
    if !config.discovery_enabled {
        let signal_report = generate_smart_money_signals(state, &config, trace_id).await?;
        let advisory_report =
            prepare_smart_money_signal_advisory_requests(state, &config, trace_id).await?;
        let snapshot = state.smart_money_service.snapshot().await?;
        return Ok(SmartMoneyRunReport {
            signal_trades_scanned: signal_report.trades_scanned,
            signals_generated: signal_report.signals_generated,
            signal_decisions_recorded: signal_report.decisions_recorded,
            observe_signals: signal_report.observe_signals,
            rejected_signals: signal_report.rejected_signals,
            signal_advisory_candidates: advisory_report.candidates,
            signal_advisory_cache_hits: advisory_report.cache_hits,
            signal_advisory_requests_built: advisory_report.requests_built,
            signal_advisory_provider_requests: advisory_report.provider_requests,
            signal_advisory_provider_saved: advisory_report.provider_saved,
            signal_advisory_provider_failures: advisory_report.provider_failures,
            candidates: snapshot.status.candidates,
            profiles: snapshot.status.profiles,
            scored_wallets: snapshot.status.scored_wallets,
            recent_trades: snapshot.status.recent_trades,
            recent_signals: snapshot.status.recent_signals,
            ..SmartMoneyRunReport::default()
        });
    }

    let connector =
        PolymarketDataApiConnector::new(&state.settings.polymarket.data_api_host)?;
    let excluded_wallets = load_smart_money_excluded_wallets(state).await?;
    let mut report = SmartMoneyRunReport::default();
    report.leaderboard_candidates_seeded = seed_smart_money_leaderboard_candidates(
        state,
        &connector,
        &config,
        &excluded_wallets,
        trace_id,
    )
    .await?;
    report.candidates_seeded += report.leaderboard_candidates_seeded;

    let max_wallets = smart_money_wallet_scan_limit(state);
    let mut scan_targets = Vec::new();
    let mut seen_wallets = HashSet::new();

    let wallets = state.copytrade_service.snapshot().await?.wallets;
    let active_wallets = wallets
        .into_iter()
        .filter(|wallet| wallet.status == TrackedWalletStatus::Active);

    for wallet in active_wallets {
        if is_smart_money_excluded_wallet(&excluded_wallets, &wallet.address) {
            continue;
        }
        state
            .smart_money_service
            .upsert_candidate(
                &wallet.address,
                SMART_MONEY_CANDIDATE_SOURCE,
                Some("tracked copytrade wallet".to_string()),
                json!({
                    "label": wallet.label,
                    "copytrade_status": wallet.status.as_str(),
                    "copytrade_added_at": wallet.added_at,
                    "copytrade_updated_at": wallet.updated_at,
                    "source": SMART_MONEY_CANDIDATE_SOURCE
                }),
            )
            .await?;
        report.candidates_seeded += 1;

        push_smart_money_scan_target(
            &mut scan_targets,
            &mut seen_wallets,
            &excluded_wallets,
            max_wallets,
            wallet.address,
            SMART_MONEY_CANDIDATE_SOURCE,
        );
    }

    report.smart_candidates_scanned = append_smart_money_candidate_scan_targets(
        state,
        &mut scan_targets,
        &mut seen_wallets,
        &excluded_wallets,
        max_wallets,
    )
    .await?;

    report.wallets_scanned = scan_targets.len();

    for target in scan_targets {
        let inputs = fetch_smart_money_wallet_inputs(state, &connector, &target.address).await;
        if inputs.activities.is_empty()
            && inputs.positions.is_empty()
            && inputs.closed_positions.is_empty()
            && inputs.trades.is_empty()
        {
            warn!(
                trace_id = %trace_id,
                wallet = %target.address,
                source = %target.source,
                "skipped smart money profile update because no Data API wallet sample was available",
            );
            continue;
        }

        let profile = build_smart_money_profile(&target.address, &inputs);
        state
            .smart_money_service
            .save_profile_and_score(profile)
            .await?;
        report.profiles_updated += 1;
        report.scores_updated += 1;

        let trades = build_smart_money_trades(&target.address, &inputs.activities);
        let recorded = state.smart_money_service.record_trades(&trades).await?;
        report.trades_recorded += recorded;
    }

    let signal_report = generate_smart_money_signals(state, &config, trace_id).await?;
    report.signal_trades_scanned = signal_report.trades_scanned;
    report.signals_generated = signal_report.signals_generated;
    report.signal_decisions_recorded = signal_report.decisions_recorded;
    report.observe_signals = signal_report.observe_signals;
    report.rejected_signals = signal_report.rejected_signals;
    let advisory_report =
        prepare_smart_money_signal_advisory_requests(state, &config, trace_id).await?;
    report.signal_advisory_candidates = advisory_report.candidates;
    report.signal_advisory_cache_hits = advisory_report.cache_hits;
    report.signal_advisory_requests_built = advisory_report.requests_built;
    report.signal_advisory_provider_requests = advisory_report.provider_requests;
    report.signal_advisory_provider_saved = advisory_report.provider_saved;
    report.signal_advisory_provider_failures = advisory_report.provider_failures;

    let snapshot = state.smart_money_service.snapshot().await?;
    info!(
        trace_id = %trace_id,
        enabled = config.enabled,
        mode = config.mode.as_str(),
        wallets_scanned = report.wallets_scanned,
        candidates_seeded = report.candidates_seeded,
        leaderboard_candidates_seeded = report.leaderboard_candidates_seeded,
        smart_candidates_scanned = report.smart_candidates_scanned,
        profiles_updated = report.profiles_updated,
        scores_updated = report.scores_updated,
        trades_recorded = report.trades_recorded,
        signal_trades_scanned = report.signal_trades_scanned,
        signals_generated = report.signals_generated,
        signal_decisions_recorded = report.signal_decisions_recorded,
        observe_signals = report.observe_signals,
        rejected_signals = report.rejected_signals,
        signal_advisory_candidates = report.signal_advisory_candidates,
        signal_advisory_cache_hits = report.signal_advisory_cache_hits,
        signal_advisory_requests_built = report.signal_advisory_requests_built,
        signal_advisory_provider_requests = report.signal_advisory_provider_requests,
        signal_advisory_provider_saved = report.signal_advisory_provider_saved,
        signal_advisory_provider_failures = report.signal_advisory_provider_failures,
        candidates = snapshot.status.candidates,
        profiles = snapshot.status.profiles,
        scored_wallets = snapshot.status.scored_wallets,
        recent_trades = snapshot.status.recent_trades,
        recent_signals = snapshot.status.recent_signals,
        "smart money intelligence scan completed",
    );
    report.candidates = snapshot.status.candidates;
    report.profiles = snapshot.status.profiles;
    report.scored_wallets = snapshot.status.scored_wallets;
    report.recent_trades = snapshot.status.recent_trades;
    report.recent_signals = snapshot.status.recent_signals;
    Ok(report)
}

async fn run_smart_money_if_enabled(
    state: &AppState,
    trace_id: &str,
) -> Result<Option<SmartMoneyRunReport>> {
    let config = state.smart_money_service.read_config().await?;
    if !config.enabled {
        return Ok(None);
    }

    run_smart_money_once(state, trace_id).await.map(Some)
}

async fn poll_smart_money(
    state: &AppState,
    max_cycles: Option<usize>,
) -> Result<SmartMoneyRunReport> {
    let mut total = SmartMoneyRunReport::default();
    let mut cycles = 0usize;
    let interval = Duration::from_secs(state.settings.worker.smart_money_interval_secs.max(60));

    loop {
        let trace_id = new_trace_id();
        match run_smart_money_if_enabled(state, &trace_id).await? {
            Some(report) => {
                accumulate_smart_money_report(&mut total, &report);
                info!(
                    trace_id = %trace_id,
                    cycle = cycles + 1,
                    wallets_scanned = report.wallets_scanned,
                    candidates_seeded = report.candidates_seeded,
                    leaderboard_candidates_seeded = report.leaderboard_candidates_seeded,
                    smart_candidates_scanned = report.smart_candidates_scanned,
                    profiles_updated = report.profiles_updated,
                    scores_updated = report.scores_updated,
                    trades_recorded = report.trades_recorded,
                    signal_advisory_candidates = report.signal_advisory_candidates,
                    signal_advisory_cache_hits = report.signal_advisory_cache_hits,
                    signal_advisory_requests_built = report.signal_advisory_requests_built,
                    signal_advisory_provider_requests = report.signal_advisory_provider_requests,
                    signal_advisory_provider_saved = report.signal_advisory_provider_saved,
                    signal_advisory_provider_failures = report.signal_advisory_provider_failures,
                    "completed smart money polling cycle",
                );
            }
            None => {
                debug!(
                    trace_id = %trace_id,
                    cycle = cycles + 1,
                    "skipped smart money polling cycle because Smart Money config is disabled",
                );
            }
        }
        cycles += 1;

        if max_cycles.is_some_and(|limit| cycles >= limit) {
            break;
        }

        tokio::select! {
            () = tokio::time::sleep(interval) => {}
            shutdown = tokio::signal::ctrl_c() => {
                if let Err(error) = shutdown {
                    warn!(error = %error, "failed to listen for ctrl-c during smart money polling");
                }
                break;
            }
        }
    }

    Ok(total)
}

fn accumulate_smart_money_report(total: &mut SmartMoneyRunReport, report: &SmartMoneyRunReport) {
    total.wallets_scanned += report.wallets_scanned;
    total.candidates_seeded += report.candidates_seeded;
    total.leaderboard_candidates_seeded += report.leaderboard_candidates_seeded;
    total.smart_candidates_scanned += report.smart_candidates_scanned;
    total.profiles_updated += report.profiles_updated;
    total.scores_updated += report.scores_updated;
    total.trades_recorded += report.trades_recorded;
    total.signal_trades_scanned += report.signal_trades_scanned;
    total.signals_generated += report.signals_generated;
    total.signal_decisions_recorded += report.signal_decisions_recorded;
    total.observe_signals += report.observe_signals;
    total.rejected_signals += report.rejected_signals;
    total.signal_advisory_candidates += report.signal_advisory_candidates;
    total.signal_advisory_cache_hits += report.signal_advisory_cache_hits;
    total.signal_advisory_requests_built += report.signal_advisory_requests_built;
    total.signal_advisory_provider_requests += report.signal_advisory_provider_requests;
    total.signal_advisory_provider_saved += report.signal_advisory_provider_saved;
    total.signal_advisory_provider_failures += report.signal_advisory_provider_failures;
    total.candidates = report.candidates;
    total.profiles = report.profiles;
    total.scored_wallets = report.scored_wallets;
    total.recent_trades = report.recent_trades;
    total.recent_signals = report.recent_signals;
}

async fn seed_smart_money_leaderboard_candidates(
    state: &AppState,
    connector: &PolymarketDataApiConnector,
    config: &SmartMoneyConfig,
    excluded_wallets: &HashSet<String>,
    trace_id: &str,
) -> Result<usize> {
    let entries = match connector
        .fetch_leaderboard(SMART_MONEY_LEADERBOARD_LIMIT, 0)
        .await
    {
        Ok(entries) => entries,
        Err(error) => {
            warn!(
                trace_id = %trace_id,
                error = %error,
                "failed to fetch smart money leaderboard candidates from Polymarket Data API"
            );
            return Ok(0);
        }
    };

    let mut seeded = 0usize;
    for entry in entries.into_iter().filter(|entry| {
        entry.pnl > Decimal::ZERO && entry.vol >= config.min_total_volume_usd
    }) {
        if is_smart_money_excluded_wallet(excluded_wallets, &entry.proxy_wallet) {
            continue;
        }
        state
            .smart_money_service
            .upsert_candidate(
                &entry.proxy_wallet,
                SMART_MONEY_LEADERBOARD_SOURCE,
                Some(format!(
                    "Polymarket leaderboard rank {} with pnl {} and volume {}",
                    entry.rank, entry.pnl, entry.vol
                )),
                smart_money_leaderboard_raw(&entry),
            )
            .await?;
        seeded += 1;
    }

    Ok(seeded)
}

async fn append_smart_money_candidate_scan_targets(
    state: &AppState,
    scan_targets: &mut Vec<SmartMoneyWalletScanTarget>,
    seen_wallets: &mut HashSet<String>,
    excluded_wallets: &HashSet<String>,
    max_wallets: usize,
) -> Result<usize> {
    let mut appended = 0usize;
    let statuses = [
        SmartWalletCandidateStatus::Tracked,
        SmartWalletCandidateStatus::Watch,
        SmartWalletCandidateStatus::Candidate,
    ];

    for status in statuses {
        if scan_targets.len() >= max_wallets {
            break;
        }
        let remaining = max_wallets.saturating_sub(scan_targets.len());
        let limit = remaining.min(usize::from(u16::MAX)) as u16;
        let candidates = state
            .smart_money_service
            .list_candidates(Some(status), Some(limit))
            .await?;
        for candidate in candidates {
            if push_smart_money_scan_target(
                scan_targets,
                seen_wallets,
                excluded_wallets,
                max_wallets,
                candidate.wallet_address,
                candidate.source,
            ) {
                appended += 1;
            }
            if scan_targets.len() >= max_wallets {
                break;
            }
        }
    }

    Ok(appended)
}

fn push_smart_money_scan_target(
    scan_targets: &mut Vec<SmartMoneyWalletScanTarget>,
    seen_wallets: &mut HashSet<String>,
    excluded_wallets: &HashSet<String>,
    max_wallets: usize,
    address: String,
    source: impl Into<String>,
) -> bool {
    if scan_targets.len() >= max_wallets {
        return false;
    }
    let normalized = address.trim().to_lowercase();
    if normalized.is_empty()
        || excluded_wallets.contains(&normalized)
        || !seen_wallets.insert(normalized.clone())
    {
        return false;
    }
    scan_targets.push(SmartMoneyWalletScanTarget {
        address: normalized,
        source: source.into(),
    });
    true
}

async fn load_smart_money_excluded_wallets(state: &AppState) -> Result<HashSet<String>> {
    let mut excluded = HashSet::new();
    for status in [
        SmartWalletCandidateStatus::Blocked,
        SmartWalletCandidateStatus::Rejected,
    ] {
        for candidate in state
            .smart_money_service
            .list_candidates(Some(status), Some(SMART_MONEY_EXCLUDED_CANDIDATE_LIMIT))
            .await?
        {
            excluded.insert(candidate.wallet_address.trim().to_lowercase());
        }
    }
    Ok(excluded)
}

fn is_smart_money_excluded_wallet(excluded_wallets: &HashSet<String>, address: &str) -> bool {
    excluded_wallets.contains(&address.trim().to_lowercase())
}

fn smart_money_wallet_scan_limit(state: &AppState) -> usize {
    usize::from(task_limit(state).unwrap_or(SMART_MONEY_MAX_WALLETS_PER_SCAN as u16))
        .clamp(1, SMART_MONEY_MAX_WALLETS_PER_SCAN)
}

fn smart_money_leaderboard_raw(entry: &PolymarketLeaderboardEntry) -> Value {
    json!({
        "source": SMART_MONEY_LEADERBOARD_SOURCE,
        "rank": entry.rank,
        "proxy_wallet": entry.proxy_wallet,
        "user_name": entry.user_name,
        "volume_usd": entry.vol,
        "pnl_usd": entry.pnl,
        "profile_image": entry.profile_image,
        "x_username": entry.x_username,
        "verified_badge": entry.verified_badge
    })
}

async fn generate_smart_money_signals(
    state: &AppState,
    config: &SmartMoneyConfig,
    trace_id: &str,
) -> Result<polyedge_application::SmartSignalGenerationReport> {
    let limit = task_limit(state)
        .unwrap_or(SMART_MONEY_SIGNAL_TRADE_LIMIT)
        .max(1);
    let trades = state
        .smart_money_service
        .list_signal_candidate_trades(Some(limit))
        .await?;
    if trades.is_empty() {
        return Ok(polyedge_application::SmartSignalGenerationReport::default());
    }

    let token_ids = trades
        .iter()
        .filter_map(|trade| trade.token_id.as_ref())
        .filter(|token_id| !token_id.trim().is_empty())
        .cloned()
        .collect::<HashSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let quotes_by_token = if token_ids.is_empty() {
        HashMap::new()
    } else {
        match state
            .orderbook_cache
            .get_books_with_max_age(&token_ids, config.max_signal_age_ms)
            .await
        {
            Ok(books) => smart_money_signal_quotes_from_books(&books),
            Err(error) => {
                warn!(
                    trace_id = %trace_id,
                    error = %error,
                    token_count = token_ids.len(),
                    "skipped smart money signal generation because orderbook cache read failed"
                );
                return Ok(polyedge_application::SmartSignalGenerationReport {
                    trades_scanned: trades.len(),
                    ..polyedge_application::SmartSignalGenerationReport::default()
                });
            }
        }
    };

    let report = state
        .smart_money_service
        .generate_signals_from_trades(&trades, &quotes_by_token)
        .await?;
    info!(
        trace_id = %trace_id,
        trades_scanned = report.trades_scanned,
        signals_generated = report.signals_generated,
        decisions_recorded = report.decisions_recorded,
        observe_signals = report.observe_signals,
        rejected_signals = report.rejected_signals,
        quotes = quotes_by_token.len(),
        "completed smart money deterministic signal generation",
    );
    Ok(report)
}

fn smart_money_signal_quotes_from_books(
    books: &[CachedOrderBook],
) -> HashMap<String, SmartSignalBookQuote> {
    books
        .iter()
        .map(|book| {
            (
                book.token_id.clone(),
                SmartSignalBookQuote {
                    token_id: book.token_id.clone(),
                    best_bid: smart_money_best_bid(book),
                    best_ask: smart_money_best_ask(book),
                    bid_depth_usd: smart_money_best_side_depth_usd(&book.bids, smart_money_best_bid(book)),
                    ask_depth_usd: smart_money_best_side_depth_usd(&book.asks, smart_money_best_ask(book)),
                },
            )
        })
        .collect()
}

fn smart_money_best_bid(book: &CachedOrderBook) -> Option<Decimal> {
    book.bids
        .iter()
        .filter(|level| level.price > Decimal::ZERO && level.size > Decimal::ZERO)
        .map(|level| level.price)
        .max()
}

fn smart_money_best_ask(book: &CachedOrderBook) -> Option<Decimal> {
    book.asks
        .iter()
        .filter(|level| level.price > Decimal::ZERO && level.size > Decimal::ZERO)
        .map(|level| level.price)
        .min()
}

fn smart_money_best_side_depth_usd(
    levels: &[polyedge_application::CachedBookLevel],
    best_price: Option<Decimal>,
) -> Decimal {
    let Some(best_price) = best_price else {
        return Decimal::ZERO;
    };
    levels
        .iter()
        .filter(|level| level.price == best_price && level.size > Decimal::ZERO)
        .map(|level| level.price * level.size)
        .sum()
}

async fn fetch_smart_money_wallet_inputs(
    state: &AppState,
    connector: &PolymarketDataApiConnector,
    address: &str,
) -> SmartMoneyWalletInputs {
    let activities = match connector
        .fetch_wallet_activity(address, state.settings.copytrade.wallet_activity_limit)
        .await
    {
        Ok(activities) => activities,
        Err(error) => {
            warn!(
                wallet = %address,
                error = %error,
                "failed to fetch smart money wallet activity from Polymarket Data API"
            );
            Vec::new()
        }
    };

    let positions = match connector.fetch_wallet_positions(address).await {
        Ok(positions) => positions,
        Err(error) => {
            warn!(
                wallet = %address,
                error = %error,
                "failed to fetch smart money wallet positions from Polymarket Data API"
            );
            Vec::new()
        }
    };

    let mut closed_positions = Vec::new();
    for page in 0..SMART_MONEY_CLOSED_POSITION_PAGES {
        let offset = page.saturating_mul(u32::from(SMART_MONEY_CLOSED_POSITION_LIMIT));
        match connector
            .fetch_closed_positions(address, SMART_MONEY_CLOSED_POSITION_LIMIT, offset)
            .await
        {
            Ok(page_positions) => {
                let fetched = page_positions.len();
                closed_positions.extend(page_positions);
                if fetched < usize::from(SMART_MONEY_CLOSED_POSITION_LIMIT) {
                    break;
                }
            }
            Err(error) => {
                warn!(
                    wallet = %address,
                    page = page,
                    error = %error,
                    "failed to fetch smart money closed positions from Polymarket Data API"
                );
                break;
            }
        }
    }

    let trades = match connector.fetch_trades(address, SMART_MONEY_TRADE_LIMIT, 0).await {
        Ok(trades) => trades,
        Err(error) => {
            warn!(
                wallet = %address,
                error = %error,
                "failed to fetch smart money trades from Polymarket Data API"
            );
            Vec::new()
        }
    };

    SmartMoneyWalletInputs {
        activities,
        positions,
        closed_positions,
        trades,
    }
}

include!("smart_money/advisory.rs");
include!("smart_money/profile.rs");
