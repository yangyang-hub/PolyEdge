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
        liquidity_usd: market.liquidity_usd,
        end_at: market.end_at,
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
        mode: execution_request.mode,
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
        mode: risk_state.mode,
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

fn daily_loss_used(risk_state: &RiskStateView) -> polyedge_domain::Result<UsdAmount> {
    let daily_pnl = risk_state.daily_pnl.value();
    if daily_pnl < Decimal::ZERO {
        return UsdAmount::new(-daily_pnl);
    }
    UsdAmount::new(Decimal::ZERO)
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
