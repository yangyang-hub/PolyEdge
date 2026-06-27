// ── Wallet Analysis handler ──────────────────────────────────────────────────
// POST /api/v1/wallet-analysis — comprehensive analysis of any Polymarket wallet.

use polyedge_application::wallet_analysis as wa;

async fn analyze_wallet(
    Extension(auth): Extension<AuthContext>,
    State(state): State<AppState>,
    Json(payload): Json<WalletAnalysisRequest>,
) -> std::result::Result<Json<ApiResponse<WalletAnalysisData>>, HttpError> {
    let trace_id = new_trace_id();
    let address = payload.address.trim().to_string();
    if address.is_empty() {
        return Err(HttpError::with_meta(
            AppError::invalid_input(
                "WALLET_ADDRESS_REQUIRED",
                "wallet address must not be empty",
            ),
            auth.request_id.clone(),
            trace_id,
        ));
    }

    let report = fetch_and_analyze_wallet(&state, &address)
        .await
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;

    let data = report_to_dto(report);
    Ok(Json(ApiResponse::new(data, auth.request_id, trace_id)))
}

/// Fetch all data from Polymarket APIs and build the analysis report.
async fn fetch_and_analyze_wallet(
    state: &AppState,
    address: &str,
) -> Result<wa::WalletAnalysisReport, AppError> {
    let connector = PolymarketDataApiConnector::new(&state.settings.polymarket.data_api_host)?;
    let gamma_host = &state.settings.polymarket.gamma_host;
    let activity_limit = state.settings.copytrade.wallet_activity_limit;

    let profile_raw = connector.fetch_public_profile(gamma_host, address).await?;
    let leaderboard_raw = connector.fetch_leaderboard_entry(address).await?;
    let portfolio_value = connector.fetch_portfolio_value(address).await?;
    let total_markets = connector.fetch_total_markets_traded(address).await?;

    let activities_raw = connector
        .fetch_wallet_activity(address, activity_limit)
        .await
        .unwrap_or_default();
    let positions_raw = connector
        .fetch_wallet_positions(address)
        .await
        .unwrap_or_default();

    // Paginate closed positions (up to 150).
    let mut closed_raw = Vec::new();
    for page in 0..3u32 {
        let batch = connector
            .fetch_closed_positions(address, 50, page * 50)
            .await
            .unwrap_or_default();
        let batch_len = batch.len();
        closed_raw.extend(batch);
        if batch_len < 50 {
            break;
        }
    }

    // Paginate trades (up to 5000).
    let mut trades_raw = Vec::new();
    for page in 0..5u32 {
        let batch = connector
            .fetch_trades(address, 1000, page * 1000)
            .await
            .unwrap_or_default();
        let batch_len = batch.len();
        trades_raw.extend(batch);
        if batch_len < 1000 {
            break;
        }
    }

    let activities: Vec<wa::ActivityInput> = activities_raw
        .into_iter()
        .map(|a| wa::ActivityInput {
            kind: a.kind,
            side: a.side,
            asset: a.asset,
            condition_id: a.condition_id,
            outcome: a.outcome,
            title: a.title,
            price: a.price,
            size: a.size,
            usdc_size: a.usdc_size,
            timestamp: a.timestamp,
        })
        .collect();

    let open_positions: Vec<wa::OpenPositionInput> = positions_raw
        .into_iter()
        .map(|p| wa::OpenPositionInput {
            condition_id: p.condition_id,
            outcome: p.outcome,
            title: p.title,
            slug: p.slug,
            size: p.size,
            avg_price: p.avg_price,
            cur_price: p.cur_price,
            realized_pnl: p.realized_pnl,
            percent_pnl: p.percent_pnl,
        })
        .collect();

    let closed_positions: Vec<wa::ClosedPositionInput> = closed_raw
        .into_iter()
        .map(|c| wa::ClosedPositionInput {
            condition_id: c.condition_id,
            avg_price: c.avg_price,
            total_bought: c.total_bought,
            realized_pnl: c.realized_pnl,
            cur_price: c.cur_price,
            timestamp: c.timestamp,
            title: c.title,
            slug: c.slug,
            outcome: c.outcome,
            end_date: c.end_date,
        })
        .collect();

    let trades: Vec<wa::TradeInput> = trades_raw
        .into_iter()
        .map(|t| wa::TradeInput {
            side: t.side,
            asset: t.asset,
            condition_id: t.condition_id,
            size: t.size,
            price: t.price,
            timestamp: t.timestamp,
            title: t.title,
            slug: t.slug,
            outcome: t.outcome,
            transaction_hash: t.transaction_hash,
        })
        .collect();

    let (name, pseudonym, bio, x_user, image, created_at, verified) =
        if let Some(ref p) = profile_raw {
            (
                p.name.clone(),
                p.pseudonym.clone(),
                p.bio.clone(),
                p.x_username.clone(),
                p.profile_image.clone(),
                p.created_at.clone(),
                p.verified_badge,
            )
        } else {
            Default::default()
        };

    let (lb_rank, lb_vol, lb_pnl) = if let Some(ref entry) = leaderboard_raw {
        (entry.rank, entry.vol, entry.pnl)
    } else {
        (0i64, Decimal::ZERO, Decimal::ZERO)
    };

    Ok(wa::build_wallet_analysis_report(
        address,
        &name,
        &pseudonym,
        &bio,
        &x_user,
        &image,
        &created_at,
        verified,
        &activities,
        &closed_positions,
        &open_positions,
        &trades,
        lb_rank,
        lb_vol,
        lb_pnl,
        portfolio_value,
        total_markets,
    ))
}

// ── Domain → DTO mapping ─────────────────────────────────────────────────────

fn report_to_dto(r: wa::WalletAnalysisReport) -> WalletAnalysisData {
    use time::format_description::well_known::Rfc3339;
    let fmt = |t: time::OffsetDateTime| t.format(&Rfc3339).unwrap_or_default();

    WalletAnalysisData {
        profile: WalletProfileData {
            address: r.profile.address,
            name: r.profile.name,
            pseudonym: r.profile.pseudonym,
            bio: r.profile.bio,
            x_username: r.profile.x_username,
            profile_image: r.profile.profile_image,
            created_at: r.profile.created_at,
            verified_badge: r.profile.verified_badge,
            leaderboard_rank: r.profile.leaderboard_rank,
            leaderboard_volume: r.profile.leaderboard_volume.to_string(),
            leaderboard_pnl: r.profile.leaderboard_pnl.to_string(),
            portfolio_value: r.profile.portfolio_value.to_string(),
            total_markets_traded: r.profile.total_markets_traded,
        },
        pnl: WalletPnlData {
            total_realized_pnl: r.pnl.total_realized_pnl.to_string(),
            total_unrealized_pnl: r.pnl.total_unrealized_pnl.to_string(),
            total_pnl: r.pnl.total_pnl.to_string(),
            overall_roi: r.pnl.overall_roi.to_string(),
            win_rate_closed: r.pnl.win_rate_closed.to_string(),
            win_rate_open: r.pnl.win_rate_open.to_string(),
            largest_win: r.pnl.largest_win.to_string(),
            largest_loss: r.pnl.largest_loss.to_string(),
            closed_positions_count: r.pnl.closed_positions_count,
            open_positions_count: r.pnl.open_positions_count,
        },
        activity: WalletActivityData {
            total_volume_usd: r.activity.total_volume_usd.to_string(),
            total_trades: r.activity.total_trades,
            avg_trade_usd: r.activity.avg_trade_usd.to_string(),
            median_trade_usd: r.activity.median_trade_usd.to_string(),
            first_trade_at: r.activity.first_trade_at.map(&fmt),
            last_trade_at: r.activity.last_trade_at.map(&fmt),
            trading_days: r.activity.trading_days,
            avg_trades_per_day: r.activity.avg_trades_per_day.to_string(),
            buy_ratio: r.activity.buy_ratio.to_string(),
            total_buy_volume: r.activity.total_buy_volume.to_string(),
            total_sell_volume: r.activity.total_sell_volume.to_string(),
        },
        categories: r
            .categories
            .into_iter()
            .map(|c| WalletCategoryData {
                category: c.category,
                trade_count: c.trade_count,
                volume_usd: c.volume_usd.to_string(),
                pnl: c.pnl.to_string(),
                win_count: c.win_count,
                loss_count: c.loss_count,
            })
            .collect(),
        style: WalletStyleData {
            style_label: r.style.style_label,
            avg_hold_hours: r.style.avg_hold_hours.to_string(),
            trade_size_stddev: r.style.trade_size_stddev.to_string(),
            directional_bias: r.style.directional_bias.to_string(),
            preferred_price_range_low: r.style.preferred_price_range_low.to_string(),
            preferred_price_range_high: r.style.preferred_price_range_high.to_string(),
            price_concentration: r.style.price_concentration,
        },
        risk: WalletRiskData {
            max_single_market_exposure_pct: r.risk.max_single_market_exposure_pct.to_string(),
            max_drawdown_estimate: r.risk.max_drawdown_estimate.to_string(),
            avg_position_size_pct: r.risk.avg_position_size_pct.to_string(),
            diversification_score: r.risk.diversification_score.to_string(),
            concentration_label: r.risk.concentration_label,
        },
        top_markets: r
            .top_markets
            .into_iter()
            .map(|m| WalletTopMarketData {
                condition_id: m.condition_id,
                title: m.title,
                slug: m.slug,
                trade_count: m.trade_count,
                volume_usd: m.volume_usd.to_string(),
                pnl: m.pnl.to_string(),
                buy_count: m.buy_count,
                sell_count: m.sell_count,
            })
            .collect(),
        recent_trades: r
            .recent_trades
            .into_iter()
            .map(|t| WalletRecentTradeData {
                side: t.side,
                title: t.title,
                slug: t.slug,
                outcome: t.outcome,
                price: t.price.to_string(),
                size: t.size.to_string(),
                notional_usd: t.notional_usd.to_string(),
                timestamp: fmt(t.timestamp),
            })
            .collect(),
        winners: r.winners.into_iter().map(closed_to_dto).collect(),
        losers: r.losers.into_iter().map(closed_to_dto).collect(),
        recent_closed: r.recent_closed.into_iter().map(closed_to_dto).collect(),
    }
}

fn closed_to_dto(c: wa::WalletClosedPositionItem) -> WalletClosedPositionData {
    use time::format_description::well_known::Rfc3339;
    WalletClosedPositionData {
        title: c.title,
        slug: c.slug,
        outcome: c.outcome,
        avg_price: c.avg_price.to_string(),
        realized_pnl: c.realized_pnl.to_string(),
        total_bought: c.total_bought.to_string(),
        end_date: c.end_date,
        timestamp: c.timestamp.format(&Rfc3339).unwrap_or_default(),
    }
}
