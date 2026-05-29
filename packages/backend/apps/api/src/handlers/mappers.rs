fn market_to_contract(market: MarketView) -> MarketData {
    MarketData {
        id: market.id,
        slug: market.slug,
        question: market.question,
        category: market.category,
        status: market.status,
        best_bid: market.best_bid,
        best_ask: market.best_ask,
        mid_price: market.mid_price,
        volume_24h: market.volume_24h,
        ambiguity_level: market.ambiguity_level,
        tradability_status: market.tradability_status,
        resolution_source: market.resolution_source,
        edge_case_notes: market.edge_case_notes,
        polymarket_condition_id: market.polymarket_condition_id,
        polymarket_yes_asset_id: market.polymarket_yes_asset_id,
        polymarket_no_asset_id: market.polymarket_no_asset_id,
        updated_at: market.updated_at,
        version: market.version,
    }
}

fn event_to_contract(event: EventView) -> EventData {
    EventData {
        id: event.id,
        source: event.source,
        summary: event.summary,
        relevance_score: event.relevance_score,
        confidence: event.confidence,
        status: event.status,
        related_market_ids: event.related_market_ids,
        reason_trace: event.reason_trace,
        created_at: event.created_at,
        updated_at: event.updated_at,
        version: event.version,
    }
}

fn news_source_health_to_contract(source: NewsSourceHealthView) -> NewsSourceHealthData {
    NewsSourceHealthData {
        source: source.source,
        source_type: source.source_type,
        enabled: source.enabled,
        reliability: source.reliability,
        last_success_at: source.last_success_at,
        last_error_at: source.last_error_at,
        consecutive_failures: source.consecutive_failures,
        items_fetched: source.items_fetched,
        items_inserted: source.items_inserted,
        items_deduped: source.items_deduped,
        health_score: source.health_score,
        last_error: source.last_error,
        updated_at: source.updated_at,
    }
}

fn news_raw_event_to_contract(event: NewsRawEventView) -> NewsRawEventData {
    NewsRawEventData {
        id: event.id,
        source: event.source,
        source_type: event.source_type,
        external_id: event.external_id,
        title: event.title,
        url: event.url,
        author: event.author,
        published_at: event.published_at,
        event_time: event.event_time,
        hash: event.hash,
        raw_payload: event.raw_payload,
        ingested_at: event.ingested_at,
        trace_id: event.trace_id,
    }
}

fn evidence_to_contract(evidence: EvidenceView) -> EvidenceData {
    EvidenceData {
        id: evidence.id,
        market_id: evidence.market_id,
        event_id: evidence.event_id,
        direction: evidence.direction,
        strength: evidence.strength,
        source_reliability: evidence.source_reliability,
        novelty: evidence.novelty,
        resolution_relevance: evidence.resolution_relevance,
        status: evidence.status,
        expires_at: evidence.expires_at,
        created_at: evidence.created_at,
        updated_at: evidence.updated_at,
        version: evidence.version,
    }
}

fn signal_to_contract(signal: SignalView) -> SignalData {
    SignalData {
        id: signal.id,
        market_id: signal.market_id,
        event_id: signal.event_id,
        action: signal.action,
        side: signal.side,
        market_price: signal.market_price,
        fair_price: signal.fair_price,
        edge: signal.edge,
        confidence: signal.confidence,
        lifecycle_state: signal.lifecycle_state,
        reason: signal.reason,
        risk_decision: signal.risk_decision,
        evidence_ids: signal.evidence_ids,
        approved_by_user_id: signal.approved_by_user_id,
        approved_at: signal.approved_at,
        rejected_by_user_id: signal.rejected_by_user_id,
        rejected_at: signal.rejected_at,
        updated_at: signal.updated_at,
        version: signal.version,
    }
}

fn order_draft_to_contract(order_draft: OrderDraftView) -> OrderDraftData {
    OrderDraftData {
        id: order_draft.id,
        signal_id: order_draft.signal_id,
        signal_version: order_draft.signal_version,
        market_id: order_draft.market_id,
        connector_name: order_draft.connector_name,
        side: order_draft.side,
        limit_price: order_draft.limit_price,
        quantity: order_draft.quantity,
        notional: order_draft.notional,
        status: order_draft.status,
        created_by_user_id: order_draft.created_by_user_id,
        created_at: order_draft.created_at,
        external_order_id: order_draft.external_order_id,
        submitted_at: order_draft.submitted_at,
        failure_code: order_draft.failure_code,
        failure_message: order_draft.failure_message,
        updated_at: order_draft.updated_at,
        version: order_draft.version,
    }
}

fn execution_request_to_contract(execution_request: ExecutionRequestView) -> ExecutionRequestData {
    ExecutionRequestData {
        id: execution_request.id,
        signal_id: execution_request.signal_id,
        signal_version: execution_request.signal_version,
        order_draft_id: execution_request.order_draft_id,
        connector_name: execution_request.connector_name,
        mode: console_runtime_mode(execution_request.mode),
        requested_by_user_id: execution_request.requested_by_user_id,
        status: execution_request.status,
        reason: execution_request.reason,
        created_at: execution_request.created_at,
        external_order_id: execution_request.external_order_id,
        submitted_at: execution_request.submitted_at,
        failure_code: execution_request.failure_code,
        failure_message: execution_request.failure_message,
        updated_at: execution_request.updated_at,
        version: execution_request.version,
    }
}

fn order_to_contract(order: OrderView) -> OrderData {
    OrderData {
        id: order.id,
        signal_id: order.signal_id,
        execution_request_id: order.execution_request_id,
        order_draft_id: order.order_draft_id,
        market_id: order.market_id,
        connector_name: order.connector_name,
        account_id: order.account_id,
        external_order_id: order.external_order_id,
        side: order.side,
        limit_price: order.limit_price,
        quantity: order.quantity,
        filled_quantity: order.filled_quantity,
        avg_fill_price: order.avg_fill_price,
        status: order.status,
        submitted_at: order.submitted_at,
        updated_at: order.updated_at,
        version: order.version,
    }
}

fn trade_to_contract(trade: TradeView) -> TradeData {
    TradeData {
        id: trade.id,
        order_id: trade.order_id,
        signal_id: trade.signal_id,
        market_id: trade.market_id,
        connector_name: trade.connector_name,
        external_trade_id: trade.external_trade_id,
        side: trade.side,
        price: trade.price,
        quantity: trade.quantity,
        fee: trade.fee,
        executed_at: trade.executed_at,
    }
}

fn position_to_contract(position: PositionView) -> PositionData {
    PositionData {
        id: position.id,
        market_id: position.market_id,
        connector_name: position.connector_name,
        account_id: position.account_id,
        side: position.side,
        net_quantity: position.net_quantity,
        avg_cost: position.avg_cost,
        mark_price: position.mark_price,
        unrealized_pnl: position.unrealized_pnl,
        realized_pnl: position.realized_pnl,
        updated_at: position.updated_at,
        version: position.version,
    }
}

fn risk_state_to_contract(
    risk_state: RiskStateView,
    environment: String,
    policy: &RiskPolicy,
    open_alerts_override: Option<u32>,
) -> polyedge_domain::Result<RiskStateData> {
    Ok(RiskStateData {
        id: "risk_state_global".to_string(),
        mode: console_runtime_mode(risk_state.mode),
        environment,
        kill_switch: risk_state.kill_switch,
        daily_pnl: risk_state.daily_pnl,
        gross_exposure: risk_state.gross_exposure,
        net_exposure: risk_state.net_exposure,
        open_alerts: open_alerts_override.unwrap_or(risk_state.open_alerts),
        daily_loss_limit: policy.max_daily_loss,
        daily_loss_used: daily_loss_used(&risk_state)?,
        updated_at: risk_state.updated_at,
        version: risk_state.version,
    })
}

fn probability_estimate_to_contract(estimate: ProbabilityEstimateView) -> ProbabilityEstimateData {
    ProbabilityEstimateData {
        id: estimate.id,
        market_id: estimate.market_id,
        event_id: estimate.event_id,
        signal_id: estimate.signal_id,
        prior_price: estimate.prior_price,
        posterior_price: estimate.posterior_price,
        fair_price: estimate.fair_price,
        market_price: estimate.market_price,
        edge: estimate.edge,
        confidence: estimate.confidence,
        time_horizon: estimate.time_horizon,
        model_version: estimate.model_version,
        reason_codes: estimate.reason_codes,
        evidence_count: estimate.evidence_count,
        created_at: estimate.created_at,
    }
}

fn arbitrage_scan_to_contract(scan: ArbitrageScanView) -> ArbitrageScanData {
    ArbitrageScanData {
        id: scan.id,
        started_at: scan.started_at,
        finished_at: scan.finished_at,
        market_count: scan.market_count,
        snapshot_count: scan.snapshot_count,
        opportunity_count: scan.opportunity_count,
        scanner_version: scan.scanner_version,
        metadata: scan.metadata,
        trace_id: scan.trace_id,
    }
}

fn arbitrage_opportunity_to_contract(
    opportunity: ArbitrageOpportunityView,
) -> ArbitrageOpportunityData {
    ArbitrageOpportunityData {
        id: opportunity.id,
        scan_id: opportunity.scan_id,
        market_id: opportunity.market_id,
        opportunity_type: opportunity.opportunity_type.as_str().to_string(),
        status: opportunity.status.as_str().to_string(),
        gross_edge: opportunity.gross_edge,
        price_sum: opportunity.price_sum.to_string(),
        capacity: opportunity.capacity,
        yes_price: opportunity.yes_price,
        no_price: opportunity.no_price,
        yes_size: opportunity.yes_size,
        no_size: opportunity.no_size,
        observed_at: opportunity.observed_at,
        reason_codes: opportunity.reason_codes,
        analysis_payload: opportunity.analysis_payload,
        trace_id: opportunity.trace_id,
        validation: opportunity.validation.map(arbitrage_validation_to_contract),
    }
}

fn arbitrage_validation_to_contract(
    validation: ArbitrageOpportunityValidationView,
) -> ArbitrageOpportunityValidationData {
    ArbitrageOpportunityValidationData {
        id: validation.id,
        opportunity_id: validation.opportunity_id,
        status: validation.status.as_str().to_string(),
        gross_edge: validation.gross_edge,
        net_edge: validation.net_edge,
        fee_estimate: validation.fee_estimate,
        slippage_buffer: validation.slippage_buffer,
        validated_capacity: validation.validated_capacity,
        book_age_ms: validation.book_age_ms,
        reason_codes: validation.reason_codes,
        validation_payload: validation.validation_payload,
        validated_at: validation.validated_at,
        trace_id: validation.trace_id,
    }
}

fn arbitrage_analysis_run_to_contract(
    analysis: ArbitrageAnalysisRunView,
) -> ArbitrageAnalysisRunData {
    ArbitrageAnalysisRunData {
        id: analysis.id,
        generated_at: analysis.generated_at,
        lookback_hours: analysis.lookback_hours,
        opportunity_count: analysis.opportunity_count,
        market_count: analysis.market_count,
        summary_payload: analysis.summary_payload,
        trace_id: analysis.trace_id,
    }
}

fn signal_transition_to_contract(transition: SignalTransitionView) -> SignalTransitionData {
    SignalTransitionData {
        id: transition.id,
        signal_id: transition.signal_id,
        from_state: transition.from_state,
        to_state: transition.to_state,
        trigger_type: transition.trigger_type,
        trigger_payload: transition.trigger_payload,
        created_at: transition.created_at,
    }
}

fn recompute_signal_to_contract(
    result: polyedge_application::RecomputeSignalResult,
    replayed: bool,
) -> RecomputeSignalData {
    RecomputeSignalData {
        signal: signal_to_contract(result.signal),
        estimate: probability_estimate_to_contract(result.estimate),
        transition: result.transition.map(signal_transition_to_contract),
        replayed,
    }
}

fn risk_state_to_contract_for_state(
    state: &AppState,
    risk_state: RiskStateView,
) -> polyedge_domain::Result<RiskStateData> {
    risk_state_to_contract(
        risk_state,
        state.settings.runtime.environment.clone(),
        state.risk_service.policy(),
        None,
    )
}

fn execution_submission_to_contract(
    receipt: ExecutionSubmissionReceipt,
    replayed: bool,
    state: &AppState,
) -> polyedge_domain::Result<SubmitExecutionData> {
    Ok(SubmitExecutionData {
        order_draft: order_draft_to_contract(receipt.order_draft),
        execution_request: execution_request_to_contract(receipt.execution_request),
        risk_state: risk_state_to_contract_for_state(state, receipt.risk_state)?,
        replayed,
    })
}

fn kill_switch_to_contract(
    receipt: KillSwitchReceipt,
    replayed: bool,
    state: &AppState,
) -> polyedge_domain::Result<KillSwitchData> {
    Ok(KillSwitchData {
        risk_state: risk_state_to_contract_for_state(state, receipt.risk_state)?,
        replayed,
    })
}

fn connector_order_status_to_contract(
    order: OrderView,
    replayed: bool,
) -> ConnectorOrderStatusCallbackData {
    ConnectorOrderStatusCallbackData {
        order: order_to_contract(order),
        replayed,
    }
}

fn connector_trade_fill_to_contract(
    result: ExecutionFillResult,
    risk_state: RiskStateView,
    replayed: bool,
    state: &AppState,
) -> polyedge_domain::Result<ConnectorTradeFillCallbackData> {
    Ok(ConnectorTradeFillCallbackData {
        order: order_to_contract(result.order),
        trade: trade_to_contract(result.trade),
        position: position_to_contract(result.position),
        risk_state: risk_state_to_contract_for_state(state, risk_state)?,
        replayed,
    })
}
