use super::*;
use polyedge_application::{
    ApproveSignalCommand, ArbitrageAnalysisRunListFilters, ArbitrageScanListFilters,
    EventListFilters, EvidenceListFilters, ExecutionRequestListFilters, OrderDraftListFilters,
    OrderListFilters, PageQuery, PositionListFilters, RewardExecutionMode, SignalListFilters,
    SubmitExecutionCommand, SyncExternalOrderStatusCommand, TradeListFilters, demo_fixture_bundle,
};
use polyedge_domain::{
    ExecutionRequestStatus, OrderDraftStatus, OrderStatus, Quantity, SignalLifecycleState,
    SignalSide, SignedUsdAmount, SystemMode,
};
use polyedge_infrastructure::Settings;

fn test_state(initial_mode: SystemMode) -> AppState {
    Runtime::test_app_state(Settings::for_test(initial_mode, "test", Vec::new()))
        .expect("test app state")
}

fn test_actor(request_id: &str) -> AuthenticatedActor {
    AuthenticatedActor {
        user_id: "usr_test_operator".to_string(),
        session_id: "sess_test_operator".to_string(),
        roles: vec![UserRole::Admin],
        request_id: request_id.to_string(),
        ip: None,
        user_agent: None,
    }
}

#[tokio::test]
async fn promote_news_events_creates_market_linked_event_and_evidence() {
    let state = test_state(SystemMode::ManualConfirm);
    state
        .market_event_service
        .ingest_fixture_bundle(demo_fixture_bundle(), "trace_seed")
        .await
        .expect("seed markets");
    let source_reliability = static_probability(92, 2);

    state
        .news_ingestion_service
        .ingest_source_items(NewsIngestSourceCommand {
            source: "sec_feed".to_string(),
            source_type: "official".to_string(),
            reliability: source_reliability,
            items: vec![NewsIngestionItem {
                source: "sec_feed".to_string(),
                source_type: "official".to_string(),
                external_id: Some("entry-promote-1".to_string()),
                title: "SEC ETF calendar narrows approval window".to_string(),
                url: Some("https://example.com/sec/entry-promote-1".to_string()),
                author: None,
                published_at: Some(OffsetDateTime::UNIX_EPOCH),
                content_snippet: Some(
                    "Review window narrowed for pending ETF decisions.".to_string(),
                ),
                raw_payload: serde_json::json!({"id": "entry-promote-1"}),
            }],
            trace_id: "trc_news_ingest".to_string(),
        })
        .await
        .expect("ingest raw news");

    let report = promote_news_events(&state, Some(10), "trc_promote_news")
        .await
        .expect("promote news events");

    assert_eq!(
        report,
        NewsPromotionReport {
            scanned: 1,
            promoted: 1,
            evidences_promoted: 1,
            skipped_unmatched: 0,
        }
    );

    let page = PageQuery::default();
    let promoted_event = state
        .market_event_service
        .list_events(EventListFilters::new(None, Some(200)).expect("event filters"), &page)
        .await
        .expect("list events")
        .data
        .into_iter()
        .find(|event| event.summary == "SEC ETF calendar narrows approval window")
        .expect("promoted event");
    assert_eq!(promoted_event.source, "sec_feed");
    assert_eq!(promoted_event.status, EventStatus::Active);
    assert_eq!(promoted_event.related_market_ids, vec!["mkt_121"]);
    assert_eq!(promoted_event.confidence, source_reliability);

    let promoted_evidences = state
        .market_event_service
        .list_evidences(
            EvidenceListFilters::new(
                Some("mkt_121".to_string()),
                Some(promoted_event.id.clone()),
                None,
                Some(200),
            )
            .expect("evidence filters"),
            &page,
        )
        .await
        .expect("list evidences");
    assert_eq!(promoted_evidences.data.len(), 1);
    let promoted_evidence = &promoted_evidences.data[0];
    assert_eq!(promoted_evidence.status, EvidenceStatus::Active);
    assert_eq!(promoted_evidence.direction, EvidenceDirection::Background);
    assert_eq!(promoted_evidence.source_reliability, source_reliability);
    assert_eq!(promoted_evidence.market_id, "mkt_121");
    assert_eq!(
        promoted_evidence.event_id.as_str(),
        promoted_event.id.as_str()
    );

    let promoted_signals = state
        .market_event_service
        .list_signals(
            SignalListFilters::new(
                Some("mkt_121".to_string()),
                Some(promoted_event.id.clone()),
                None,
                Some(200),
            )
            .expect("signal filters"),
            &page,
        )
        .await
        .expect("list signals");
    assert!(promoted_signals.data.is_empty());
}

#[tokio::test]
async fn scan_arbitrage_once_records_market_snapshots_without_trade_side_effects() {
    let state = test_state(SystemMode::ManualConfirm);
    state
        .market_event_service
        .ingest_fixture_bundle(demo_fixture_bundle(), "trace_seed")
        .await
        .expect("seed markets");

    let report = scan_arbitrage_once(&state, "trc_arbitrage_scan")
        .await
        .expect("scan arbitrage");

    assert_eq!(
        report,
        ArbitrageScanRunReport {
            markets_scanned: 4,
            snapshots_recorded: 4,
            opportunities_recorded: 0,
            validations_recorded: 0,
            validation_books_refetched: 0,
            validation_book_failures: 0,
            opportunities_expired: 0,
            events_pruned: 0,
            failed_books: 0,
        }
    );

    let page = PageQuery::default();
    let scans = state
        .arbitrage_service
        .list_scans(ArbitrageScanListFilters::new().expect("scan filters"), &page)
        .await
        .expect("list scans");
    assert_eq!(scans.data.len(), 1);
    assert_eq!(scans.data[0].id, "scan_arbitrage_scan");
    assert_eq!(scans.data[0].market_count, 4);
    assert_eq!(scans.data[0].snapshot_count, 4);
    assert_eq!(scans.data[0].opportunity_count, 0);
    assert!(scans.data[0].finished_at.is_some());
}

#[tokio::test]
async fn analyze_arbitrage_opportunities_records_summary_run() {
    let state = test_state(SystemMode::ManualConfirm);
    state
        .market_event_service
        .ingest_fixture_bundle(demo_fixture_bundle(), "trace_seed")
        .await
        .expect("seed markets");
    scan_arbitrage_once(&state, "trc_arbitrage_scan")
        .await
        .expect("scan arbitrage");

    let analysis = analyze_arbitrage_opportunities(&state, 24, "trc_arbitrage_analysis")
        .await
        .expect("analyze arbitrage");

    assert_eq!(analysis.id, "arb_analysis_arbitrage_analysis");
    assert_eq!(analysis.lookback_hours, 24);
    assert_eq!(analysis.opportunity_count, 0);
    assert_eq!(analysis.market_count, 0);

    let page = PageQuery::default();
    let runs = state
        .arbitrage_service
        .list_analysis_runs(
            ArbitrageAnalysisRunListFilters::new().expect("analysis filters"),
            &page,
        )
        .await
        .expect("list analysis runs");
    assert_eq!(runs.data.len(), 1);
    assert_eq!(runs.data[0].id, analysis.id);
}

async fn seed_execution_request_for_connector(
    state: &AppState,
    quantity_units: i64,
    connector_name: &str,
) -> polyedge_application::ExecutionSubmissionReceipt {
    state
        .market_event_service
        .ingest_fixture_bundle(demo_fixture_bundle(), "trace_seed")
        .await
        .expect("seed fixtures");

    let approval = state
        .risk_service
        .approve_signal(ApproveSignalCommand {
            signal_id: "sig_2411".to_string(),
            reason: "approve fixture signal for worker dispatch test".to_string(),
            expected_version: Some(9),
            request_id: "req_approve".to_string(),
            trace_id: "trace_approve".to_string(),
            actor: test_actor("req_approve"),
        })
        .await
        .expect("approve signal");

    state
        .execution_service
        .submit_execution_request(SubmitExecutionCommand {
            signal_id: approval.signal.id.clone(),
            expected_signal_version: Some(approval.signal.version),
            limit_price: approval.signal.market_price,
            quantity: Quantity::new(quantity_units.into()).expect("quantity"),
            connector_name: Some(connector_name.to_string()),
            reason: "queue execution request for worker dispatch test".to_string(),
            request_id: "req_submit".to_string(),
            trace_id: "trace_submit".to_string(),
            actor: test_actor("req_submit"),
        })
        .await
        .expect("submit execution request")
}

async fn seed_execution_request(
    state: &AppState,
    quantity_units: i64,
) -> polyedge_application::ExecutionSubmissionReceipt {
    seed_execution_request_for_connector(state, quantity_units, PAPER_EXECUTOR_NAME).await
}

#[tokio::test]
async fn drain_execution_queue_marks_submitted_for_eligible_orders() {
    let state = test_state(SystemMode::ManualConfirm);
    let receipt = seed_execution_request(&state, 3).await;

    let report = drain_execution_queue(&state, None, Some(10))
        .await
        .expect("drain queue");

    assert_eq!(
        report,
        ExecutionDrainReport {
            scanned: 1,
            submitted: 1,
            failed: 0,
        }
    );

    let execution_request = state
        .execution_service
        .list_execution_requests(
            ExecutionRequestListFilters::new(None, None, None, Some(10)).expect("request filters"),
        )
        .await
        .expect("list execution requests")
        .into_iter()
        .find(|item| item.id == receipt.execution_request.id)
        .expect("submitted execution request");
    assert_eq!(execution_request.status, ExecutionRequestStatus::Submitted);
    assert!(
        execution_request
            .external_order_id
            .as_deref()
            .is_some_and(|value| value.starts_with("paper:mkt_120:yes:"))
    );
    assert!(execution_request.submitted_at.is_some());
    assert_eq!(execution_request.failure_code, None);
    assert_eq!(execution_request.failure_message, None);

    let order_draft = state
        .execution_service
        .list_order_drafts(
            OrderDraftListFilters::new(None, None, None, Some(10)).expect("draft filters"),
        )
        .await
        .expect("list order drafts")
        .into_iter()
        .find(|item| item.id == receipt.order_draft.id)
        .expect("submitted order draft");
    assert_eq!(order_draft.status, OrderDraftStatus::Submitted);
    assert_eq!(
        order_draft.external_order_id,
        execution_request.external_order_id
    );
    assert!(order_draft.submitted_at.is_some());

    let orders = state
        .execution_service
        .list_orders(
            OrderListFilters::new(Some("sig_2411".to_string()), None, None, None, Some(10))
                .expect("order filters"),
        )
        .await
        .expect("list orders");
    assert_eq!(orders.len(), 1);
    assert_eq!(orders[0].status, OrderStatus::Submitted);
    assert_eq!(orders[0].account_id, PAPER_ACCOUNT_ID);
    assert_eq!(
        orders[0].filled_quantity,
        Quantity::new(0.into()).expect("quantity")
    );
    assert_eq!(orders[0].avg_fill_price.api_string(), "0");
}

#[tokio::test]
async fn poll_paper_order_statuses_promotes_submitted_orders_to_open() {
    let state = test_state(SystemMode::ManualConfirm);
    seed_execution_request(&state, 3).await;
    drain_execution_queue(&state, None, Some(10))
        .await
        .expect("drain queue");

    let report = poll_paper_order_statuses(&state, None, Some(10))
        .await
        .expect("poll order statuses");

    assert_eq!(
        report,
        OrderStatusPollReport {
            scanned: 1,
            opened: 1,
        }
    );

    let orders = state
        .execution_service
        .list_orders(
            OrderListFilters::new(Some("sig_2411".to_string()), None, None, None, Some(10))
                .expect("order filters"),
        )
        .await
        .expect("list orders");
    assert_eq!(orders.len(), 1);
    assert_eq!(orders[0].status, OrderStatus::Open);
}

#[tokio::test]
async fn polymarket_worker_requires_live_credentials() {
    let state = test_state(SystemMode::ManualConfirm);

    let error = poll_polymarket_order_statuses(
        &state,
        Some(POLYMARKET_CONNECTOR_NAME.to_string()),
        Some(10),
    )
    .await
    .expect_err("polymarket connector should require a private key");

    assert_eq!(error.code(), "POLYMARKET_PRIVATE_KEY_REQUIRED");
}

#[tokio::test]
async fn sync_external_order_status_cancels_open_order_and_request() {
    let state = test_state(SystemMode::ManualConfirm);
    let receipt = seed_execution_request(&state, 3).await;
    drain_execution_queue(&state, None, Some(10))
        .await
        .expect("drain queue");
    poll_paper_order_statuses(&state, None, Some(10))
        .await
        .expect("poll order statuses");

    let order = state
        .execution_service
        .list_orders(
            OrderListFilters::new(Some("sig_2411".to_string()), None, None, None, Some(10))
                .expect("order filters"),
        )
        .await
        .expect("list orders")
        .into_iter()
        .next()
        .expect("open order");

    let canceled_order = state
        .execution_service
        .sync_external_order_status(SyncExternalOrderStatusCommand {
            connector_name: order.connector_name.clone(),
            external_order_id: order.external_order_id.clone(),
            status: OrderStatus::Canceled,
            request_id: "req_cancel_sync".to_string(),
            trace_id: "trace_cancel_sync".to_string(),
            actor: test_actor("req_cancel_sync"),
        })
        .await
        .expect("cancel order");
    assert_eq!(canceled_order.status, OrderStatus::Canceled);

    let execution_request = state
        .execution_service
        .list_execution_requests(
            ExecutionRequestListFilters::new(None, None, None, Some(10)).expect("request filters"),
        )
        .await
        .expect("list execution requests")
        .into_iter()
        .find(|item| item.id == receipt.execution_request.id)
        .expect("canceled execution request");
    assert_eq!(execution_request.status, ExecutionRequestStatus::Canceled);
}

#[tokio::test]
async fn drain_execution_queue_marks_failed_for_sub_min_notional_orders() {
    let state = test_state(SystemMode::ManualConfirm);
    let receipt = seed_execution_request(&state, 1).await;

    let report = drain_execution_queue(&state, None, Some(10))
        .await
        .expect("drain queue");

    assert_eq!(
        report,
        ExecutionDrainReport {
            scanned: 1,
            submitted: 0,
            failed: 1,
        }
    );

    let execution_request = state
        .execution_service
        .list_execution_requests(
            ExecutionRequestListFilters::new(None, None, None, Some(10)).expect("request filters"),
        )
        .await
        .expect("list execution requests")
        .into_iter()
        .find(|item| item.id == receipt.execution_request.id)
        .expect("failed execution request");
    assert_eq!(execution_request.status, ExecutionRequestStatus::Failed);
    assert_eq!(
        execution_request.failure_code.as_deref(),
        Some("PAPER_MIN_NOTIONAL_NOT_MET")
    );
    assert!(
        execution_request
            .failure_message
            .as_deref()
            .is_some_and(|value| value.contains("notional >= 1.00 USD"))
    );
    assert_eq!(execution_request.external_order_id, None);
    assert_eq!(execution_request.submitted_at, None);

    let order_draft = state
        .execution_service
        .list_order_drafts(
            OrderDraftListFilters::new(None, None, None, Some(10)).expect("draft filters"),
        )
        .await
        .expect("list order drafts")
        .into_iter()
        .find(|item| item.id == receipt.order_draft.id)
        .expect("rejected order draft");
    assert_eq!(order_draft.status, OrderDraftStatus::Rejected);
    assert_eq!(
        order_draft.failure_code.as_deref(),
        Some("PAPER_MIN_NOTIONAL_NOT_MET")
    );
    assert_eq!(order_draft.external_order_id, None);
    assert_eq!(order_draft.submitted_at, None);
}

#[tokio::test]
async fn reconcile_paper_fills_creates_order_trade_position_and_executes_signal() {
    let state = test_state(SystemMode::ManualConfirm);
    let receipt = seed_execution_request(&state, 3).await;
    drain_execution_queue(&state, None, Some(10))
        .await
        .expect("drain queue");

    let first_report = reconcile_paper_fills(&state, None, Some(10))
        .await
        .expect("reconcile fills");

    assert_eq!(
        first_report,
        FillReconciliationReport {
            scanned: 1,
            reconciled: 1,
        }
    );

    let orders = state
        .execution_service
        .list_orders(
            OrderListFilters::new(Some("sig_2411".to_string()), None, None, None, Some(10))
                .expect("order filters"),
        )
        .await
        .expect("list orders");
    assert_eq!(orders.len(), 1);
    let order = &orders[0];
    assert_eq!(order.execution_request_id, receipt.execution_request.id);
    assert_eq!(order.order_draft_id, receipt.order_draft.id);
    assert_eq!(order.account_id, PAPER_ACCOUNT_ID);
    assert_eq!(order.status, OrderStatus::PartiallyFilled);
    assert_eq!(order.side, SignalSide::Yes);
    assert_eq!(order.quantity, Quantity::new(3.into()).expect("quantity"));
    assert_eq!(
        order.filled_quantity,
        Quantity::new(1.into()).expect("quantity")
    );
    assert!(order.external_order_id.starts_with("paper:mkt_120:yes:"));

    let trades = state
        .execution_service
        .list_trades(
            TradeListFilters::new(None, Some("sig_2411".to_string()), None, None, Some(10))
                .expect("trade filters"),
        )
        .await
        .expect("list trades");
    assert_eq!(trades.len(), 1);
    assert_eq!(trades[0].order_id, order.id);
    assert_eq!(trades[0].connector_name, PAPER_EXECUTOR_NAME);
    assert!(
        trades[0]
            .external_trade_id
            .starts_with("paper-trade:mkt_120:yes:")
    );
    assert_eq!(
        trades[0].quantity,
        Quantity::new(1.into()).expect("quantity")
    );
    assert!(trades[0].external_trade_id.ends_with(":1"));

    let positions = state
        .execution_service
        .list_positions(
            PositionListFilters::new(
                Some("mkt_120".to_string()),
                Some(PAPER_EXECUTOR_NAME.to_string()),
                Some(SignalSide::Yes),
                Some(10),
            )
            .expect("position filters"),
        )
        .await
        .expect("list positions");
    assert_eq!(positions.len(), 1);
    assert_eq!(positions[0].account_id, PAPER_ACCOUNT_ID);
    assert_eq!(
        positions[0].net_quantity,
        Quantity::new(1.into()).expect("quantity")
    );
    assert_eq!(positions[0].mark_price, order.avg_fill_price);
    assert_eq!(positions[0].avg_cost, order.avg_fill_price);

    let page = PageQuery::default();
    let signals = state
        .market_event_service
        .list_signals(
            SignalListFilters::new(Some("mkt_120".to_string()), None, None, Some(10))
                .expect("signal filters"),
            &page,
        )
        .await
        .expect("list signals");
    let signal = signals
        .data
        .into_iter()
        .find(|item| item.id == "sig_2411")
        .expect("executed signal");
    assert_eq!(signal.lifecycle_state, SignalLifecycleState::Executed);

    let risk_state = state
        .risk_service
        .read_state()
        .await
        .expect("read risk state");
    assert_eq!(
        risk_state.daily_pnl,
        SignedUsdAmount::new(0.into()).expect("daily pnl")
    );
    assert_eq!(risk_state.gross_exposure.api_string(), "0.0052");
    assert_eq!(risk_state.net_exposure.api_string(), "0.0052");

    let second_report = reconcile_paper_fills(&state, None, Some(10))
        .await
        .expect("reconcile fills again");
    assert_eq!(
        second_report,
        FillReconciliationReport {
            scanned: 1,
            reconciled: 1,
        }
    );

    let orders = state
        .execution_service
        .list_orders(
            OrderListFilters::new(Some("sig_2411".to_string()), None, None, None, Some(10))
                .expect("order filters"),
        )
        .await
        .expect("list orders");
    assert_eq!(orders.len(), 1);
    let order = &orders[0];
    assert_eq!(order.status, OrderStatus::Filled);
    assert_eq!(
        order.filled_quantity,
        Quantity::new(3.into()).expect("quantity")
    );

    let trades = state
        .execution_service
        .list_trades(
            TradeListFilters::new(None, Some("sig_2411".to_string()), None, None, Some(10))
                .expect("trade filters"),
        )
        .await
        .expect("list trades");
    assert_eq!(trades.len(), 2);
    let mut trade_quantities: Vec<_> = trades
        .iter()
        .map(|trade| {
            assert_eq!(trade.order_id, order.id);
            trade.quantity.value()
        })
        .collect();
    trade_quantities.sort();
    assert_eq!(trade_quantities, vec![1.into(), 2.into()]);
    assert!(
        trades
            .iter()
            .any(|trade| trade.external_trade_id.ends_with(":3"))
    );

    let positions = state
        .execution_service
        .list_positions(
            PositionListFilters::new(
                Some("mkt_120".to_string()),
                Some(PAPER_EXECUTOR_NAME.to_string()),
                Some(SignalSide::Yes),
                Some(10),
            )
            .expect("position filters"),
        )
        .await
        .expect("list positions");
    assert_eq!(positions.len(), 1);
    assert_eq!(
        positions[0].net_quantity,
        Quantity::new(3.into()).expect("quantity")
    );

    let risk_state = state
        .risk_service
        .read_state()
        .await
        .expect("read risk state");
    assert_eq!(risk_state.gross_exposure.api_string(), "0.0156");
    assert_eq!(risk_state.net_exposure.api_string(), "0.0156");

    let third_report = reconcile_paper_fills(&state, None, Some(10))
        .await
        .expect("reconcile fills final pass");
    assert_eq!(
        third_report,
        FillReconciliationReport {
            scanned: 0,
            reconciled: 0,
        }
    );
}

fn reward_decimal(value: &str) -> Decimal {
    Decimal::from_str_exact(value).expect("decimal")
}

fn live_test_plan(now: OffsetDateTime) -> RewardQuotePlan {
    RewardQuotePlan {
        condition_id: "cond_live".to_string(),
        market_slug: "live-market".to_string(),
        question: "Will the live event happen?".to_string(),
        score: reward_decimal("50"),
        eligible: true,
        reason: "eligible".to_string(),
        midpoint: Some(reward_decimal("0.50")),
        total_daily_rate: reward_decimal("25"),
        rewards_max_spread: reward_decimal("8"),
        rewards_min_size: reward_decimal("5"),
        legs: vec![
            polyedge_application::RewardQuoteLeg {
                token_id: "yes_live".to_string(),
                outcome: "YES".to_string(),
                side: RewardOrderSide::Buy,
                price: reward_decimal("0.49"),
                size: reward_decimal("20"),
                notional_usd: reward_decimal("9.8"),
            },
            polyedge_application::RewardQuoteLeg {
                token_id: "no_live".to_string(),
                outcome: "NO".to_string(),
                side: RewardOrderSide::Buy,
                price: reward_decimal("0.49"),
                size: reward_decimal("20"),
                notional_usd: reward_decimal("9.8"),
            },
        ],
        updated_at: now,
    }
}

fn live_test_book(token_id: &str, observed_at: OffsetDateTime) -> RewardOrderBook {
    RewardOrderBook {
        token_id: token_id.to_string(),
        bids: vec![RewardBookLevel {
            price: reward_decimal("0.48"),
            size: reward_decimal("100"),
        }],
        asks: vec![RewardBookLevel {
            price: reward_decimal("0.52"),
            size: reward_decimal("100"),
        }],
        observed_at,
    }
}

fn live_test_open_order(token_id: &str) -> ManagedRewardOrder {
    let now = OffsetDateTime::now_utc();
    ManagedRewardOrder {
        id: format!("rewlive_seed_{token_id}"),
        account_id: "reward_live".to_string(),
        condition_id: "cond_live".to_string(),
        token_id: token_id.to_string(),
        outcome: "YES".to_string(),
        side: RewardOrderSide::Buy,
        price: reward_decimal("0.49"),
        size: reward_decimal("20"),
        external_order_id: Some(format!("pm_{token_id}")),
        status: ManagedRewardOrderStatus::Open,
        scoring: true,
        reason: "seed live order".to_string(),
        filled_size: Decimal::ZERO,
        reward_earned: Decimal::ZERO,
        last_scored_at: None,
        created_at: now,
        updated_at: now,
    }
}

#[test]
fn live_placement_reuses_cash_and_allows_stale_book_age_check_to_be_disabled() {
    let config = RewardBotConfig {
        execution_mode: RewardExecutionMode::Live,
        account_id: "reward_live".to_string(),
        stale_book_ms: 0,
        max_markets: 1,
        max_open_orders: 2,
        max_global_position_usd: Decimal::ZERO,
        ..RewardBotConfig::default()
    };
    let now = OffsetDateTime::now_utc();
    let old = now - TimeDuration::hours(1);
    let plan = live_test_plan(now);
    let books = HashMap::from([
        ("yes_live".to_string(), live_test_book("yes_live", old)),
        ("no_live".to_string(), live_test_book("no_live", old)),
    ]);

    let orders = live_placement_orders(
        &config,
        "reward_live",
        &[plan],
        &books,
        &[],
        &[],
        "trc_live_test",
    );

    assert_eq!(orders.len(), 2);
    assert!(orders.iter().all(|order| {
        order.side == RewardOrderSide::Buy && order.status == ManagedRewardOrderStatus::Planned
    }));
}

#[test]
fn live_cancel_candidates_cancel_when_orderbook_missing() {
    let config = RewardBotConfig {
        execution_mode: RewardExecutionMode::Live,
        account_id: "reward_live".to_string(),
        ..RewardBotConfig::default()
    };
    let plan = live_test_plan(OffsetDateTime::now_utc());
    let order = live_test_open_order("yes_live");

    let candidates =
        live_cancel_candidates(&config, &[plan], &[order], &HashMap::new(), &HashMap::new());

    assert_eq!(candidates.len(), 1);
    assert!(candidates[0].1.contains("orderbook unavailable"));
}

#[test]
fn live_placement_counts_candidate_notional_against_position_cap() {
    let config = RewardBotConfig {
        execution_mode: RewardExecutionMode::Live,
        account_id: "reward_live".to_string(),
        max_markets: 1,
        max_open_orders: 2,
        max_position_usd: Decimal::from(20_u64),
        max_global_position_usd: Decimal::ZERO,
        ..RewardBotConfig::default()
    };
    let now = OffsetDateTime::now_utc();
    let plan = live_test_plan(now);
    let books = HashMap::from([
        ("yes_live".to_string(), live_test_book("yes_live", now)),
        ("no_live".to_string(), live_test_book("no_live", now)),
    ]);
    let positions = vec![RewardPosition {
        account_id: "reward_live".to_string(),
        condition_id: "cond_live".to_string(),
        token_id: "yes_live".to_string(),
        outcome: "Yes".to_string(),
        size: Decimal::from(38_u64),
        avg_price: Decimal::from_parts(50, 0, 0, false, 2),
        realized_pnl: Decimal::ZERO,
        updated_at: now,
    }];

    let orders = live_placement_orders(
        &config,
        "reward_live",
        &[plan],
        &books,
        &[],
        &positions,
        "trc_live_test",
    );

    assert_eq!(orders.len(), 1);
    assert_eq!(orders[0].token_id, "no_live");
}
