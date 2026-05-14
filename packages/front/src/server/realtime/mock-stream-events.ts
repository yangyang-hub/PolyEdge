import "server-only";

import { humanizeSnakeCase } from "@/lib/formatters";
import type {
  ArbitrageStreamPayload,
  ConsoleEventStreamPayload,
  RealtimeChannel,
  RiskStreamPayload,
  SignalStreamPayload,
} from "@/lib/contracts/realtime";
import {
  approvalFixtures,
  arbitrageAnalysisRunFixtures,
  arbitrageOpportunityFixtures,
  arbitrageScanFixtures,
  eventFixtures,
  marketFixtures,
  riskAlertFixtures,
  riskStateFixture,
  signalFixtures,
} from "@/lib/server/polyedge-mock-data";

export type MockStreamEvent = {
  id: string;
  type: string;
  data: SignalStreamPayload | RiskStreamPayload | ConsoleEventStreamPayload | ArbitrageStreamPayload;
};

function buildEventId(channel: RealtimeChannel, timestamp: string, sequence: number) {
  return `${timestamp}_${channel}_${String(sequence).padStart(4, "0")}`;
}

function createSignalPayload(
  signalId: string,
  overrides: Partial<SignalStreamPayload>,
): SignalStreamPayload {
  const signal = signalFixtures.find((item) => item.id === signalId);

  if (!signal) {
    throw new Error(`Missing signal fixture for realtime payload: ${signalId}`);
  }

  const market = marketFixtures.find((item) => item.id === signal.market_id);

  return {
    signal_id: signal.id,
    market_id: signal.market_id,
    market_question: market?.question ?? signal.market_id,
    context_label: market
      ? `${market.category} / ${humanizeSnakeCase(market.tradability_status)}`
      : "Unknown / manual review",
    version: signal.version,
    lifecycle_state: signal.lifecycle_state,
    side: signal.side,
    fair_price: signal.fair_price,
    market_price: signal.market_price,
    edge: signal.edge,
    confidence: signal.confidence,
    requires_review: approvalFixtures.some(
      (approval) => approval.status === "pending" && approval.type === "signal" && approval.resource_id === signal.id,
    ),
    reason: signal.reason,
    risk_decision: signal.risk_decision,
    evidence_lines: [],
    updated_at: signal.updated_at,
    ...overrides,
  };
}

function createApprovalPayload(
  approvalId: string,
  overrides: Partial<RiskStreamPayload>,
): RiskStreamPayload {
  const approval = approvalFixtures.find((item) => item.id === approvalId);

  if (!approval) {
    throw new Error(`Missing approval fixture for realtime payload: ${approvalId}`);
  }

  return {
    resource_id: approval.id,
    version: approval.version,
    mode: riskStateFixture.mode,
    environment: riskStateFixture.environment,
    kill_switch: riskStateFixture.kill_switch,
    open_alerts: riskStateFixture.open_alerts,
    critical_alerts: riskAlertFixtures.filter((alert) => alert.severity === "critical").length,
    warning_alerts: riskAlertFixtures.filter((alert) => alert.severity === "warning").length,
    approval_count: approvalFixtures.filter((item) => item.status === "pending").length,
    approval_id: approval.id,
    approval_type: approval.type,
    approval_severity: approval.severity,
    approval_status: approval.status,
    approval_owner: approval.owner,
    approval_summary: approval.summary,
    approval_resource_id: approval.resource_id,
    approval_requires_step_up_auth: approval.requires_step_up_auth,
    created_at: approval.created_at,
    updated_at: approval.updated_at,
    ...overrides,
  };
}

const signalEvents: MockStreamEvent[] = [
  {
    id: buildEventId("signals", "2026-04-19T09:30:00Z", 1),
    type: "signal.updated",
    data: createSignalPayload("sig_2411", {
      version: 10,
      edge: "0.07",
      confidence: "0.91",
      reason: "BTC order flow remained supportive after ETF inflow data accelerated through the session.",
      updated_at: "2026-04-19T09:30:00Z",
    }),
  },
  {
    id: buildEventId("signals", "2026-04-19T09:30:05Z", 2),
    type: "signal.updated",
    data: createSignalPayload("sig_2412", {
      version: 10,
      lifecycle_state: "active",
      edge: "-0.07",
      confidence: "0.70",
      requires_review: true,
      reason: "Desk confirms the ETF delay thesis still dominates despite noisy policy commentary.",
      updated_at: "2026-04-19T09:30:05Z",
    }),
  },
  {
    id: buildEventId("signals", "2026-04-19T09:30:10Z", 3),
    type: "signal.created",
    data: {
      signal_id: "sig_2414",
      market_id: "mkt_122",
      market_question: "Will the Fed cut rates in June?",
      context_label: "Macro / observe only",
      version: 1,
      lifecycle_state: "new",
      side: "no",
      fair_price: "0.59",
      market_price: "0.63",
      edge: "-0.04",
      confidence: "0.73",
      requires_review: false,
      reason: "Fresh macro transcript analysis leans against an early cut despite current market pricing.",
      risk_decision: "Keep in desk view while macro bucket utilization remains near threshold.",
      evidence_lines: [
        "Supports no · strength 0.29 · novelty 71%",
        "Background · strength 0.14 · novelty 43%",
      ],
      updated_at: "2026-04-19T09:30:10Z",
    },
  },
  {
    id: buildEventId("signals", "2026-04-19T09:30:15Z", 4),
    type: "signal.invalidated",
    data: createSignalPayload("sig_2413", {
      version: 4,
      lifecycle_state: "invalidated",
      edge: "0.00",
      confidence: "0.21",
      requires_review: false,
      reason: "Momentum signal invalidated after source quality deteriorated across the social feed cluster.",
      updated_at: "2026-04-19T09:30:15Z",
    }),
  },
];

const riskEvents: MockStreamEvent[] = [
  {
    id: buildEventId("risk", "2026-04-19T09:30:03Z", 1),
    type: "risk.alerted",
    data: {
      resource_id: riskStateFixture.id,
      version: riskStateFixture.version + 1,
      mode: riskStateFixture.mode,
      environment: riskStateFixture.environment,
      kill_switch: riskStateFixture.kill_switch,
      daily_pnl: riskStateFixture.daily_pnl,
      gross_exposure: riskStateFixture.gross_exposure,
      net_exposure: riskStateFixture.net_exposure,
      daily_loss_limit: riskStateFixture.daily_loss_limit,
      daily_loss_used: "1295000.00",
      open_alerts: 4,
      critical_alerts: 2,
      warning_alerts: 2,
      approval_count: 3,
      alert_id: "alt_104",
      severity: "critical",
      reason: "Macro bucket utilization breached its configured ceiling after late-session repricing.",
      target: "Macro Bucket",
      status: "unresolved",
      created_at: "2026-04-19T09:30:03Z",
      updated_at: "2026-04-19T09:30:03Z",
    },
  },
  {
    id: buildEventId("risk", "2026-04-19T09:30:08Z", 2),
    type: "approval.created",
    data: {
      ...createApprovalPayload("apr_001", {
        resource_id: "apr_006",
        version: 1,
        approval_id: "apr_006",
        approval_type: "signal",
        approval_severity: "warning",
        approval_status: "pending",
        approval_owner: "Macro Desk",
        approval_summary: "June rate-cut short requires operator confirmation after transcript rerank.",
        approval_resource_id: "sig_2414",
        approval_requires_step_up_auth: true,
        approval_count: 4,
        created_at: "2026-04-19T09:30:08Z",
        updated_at: "2026-04-19T09:30:08Z",
      }),
    },
  },
  {
    id: buildEventId("risk", "2026-04-19T09:30:12Z", 3),
    type: "risk.mode_changed",
    data: {
      resource_id: riskStateFixture.id,
      version: riskStateFixture.version + 2,
      mode: "paper_trade",
      environment: riskStateFixture.environment,
      kill_switch: false,
      daily_pnl: "14780.00",
      gross_exposure: "0.58",
      net_exposure: "0.09",
      daily_loss_limit: riskStateFixture.daily_loss_limit,
      daily_loss_used: "1180000.00",
      open_alerts: 3,
      critical_alerts: 1,
      warning_alerts: 2,
      approval_count: 4,
      updated_at: "2026-04-19T09:30:12Z",
    },
  },
  {
    id: buildEventId("risk", "2026-04-19T09:30:16Z", 4),
    type: "approval.updated",
    data: createApprovalPayload("apr_002", {
      approval_status: "approved",
      approval_count: 3,
      updated_at: "2026-04-19T09:30:16Z",
    }),
  },
  {
    id: buildEventId("risk", "2026-04-19T09:30:20Z", 5),
    type: "risk.alerted",
    data: {
      resource_id: riskStateFixture.id,
      version: riskStateFixture.version + 3,
      mode: "paper_trade",
      environment: riskStateFixture.environment,
      kill_switch: false,
      daily_pnl: "14620.00",
      gross_exposure: "0.60",
      net_exposure: "0.10",
      daily_loss_limit: riskStateFixture.daily_loss_limit,
      daily_loss_used: "1205000.00",
      open_alerts: 3,
      critical_alerts: 1,
      warning_alerts: 2,
      approval_count: 3,
      alert_id: riskAlertFixtures[1].id,
      severity: riskAlertFixtures[1].severity,
      reason: "Stale market snapshot warning persisted for one macro feed partition.",
      target: riskAlertFixtures[1].target,
      status: "watching",
      created_at: "2026-04-19T09:30:20Z",
      updated_at: "2026-04-19T09:30:20Z",
    },
  },
];

const eventStreamEvents: MockStreamEvent[] = [
  {
    id: buildEventId("events", "2026-04-19T09:30:02Z", 1),
    type: "event.created",
    data: {
      event_id: "evt_9005",
      source: "sec_feed",
      summary: "SEC calendar update narrowed the review window for the staking ETF filing.",
      confidence: "0.82",
      created_at: "2026-04-19T09:30:02Z",
      version: 1,
    },
  },
  {
    id: buildEventId("events", "2026-04-19T09:30:09Z", 2),
    type: "event.created",
    data: {
      event_id: eventFixtures[0].id,
      source: eventFixtures[0].source,
      summary: "Desk note refreshed Reuters signal weight after legal review tightened the thesis.",
      confidence: "0.84",
      created_at: "2026-04-19T09:30:09Z",
      version: eventFixtures[0].version + 1,
    },
  },
  {
    id: buildEventId("events", "2026-04-19T09:30:18Z", 3),
    type: "event.created",
    data: {
      event_id: "evt_9006",
      source: "desk_model",
      summary: "Macro transcript reranking increased relevance on the June rate-cut market cluster.",
      confidence: "0.76",
      created_at: "2026-04-19T09:30:18Z",
      version: 1,
    },
  },
];

const arbitrageEvents: MockStreamEvent[] = [
  {
    id: "1",
    type: "arbitrage.scan.started",
    data: {
      sequence: 1,
      event_type: "arbitrage.scan.started",
      resource_type: "scan",
      resource_id: "scan_mock_live_1435",
      scan_id: "scan_mock_live_1435",
      started_at: "2026-04-16T14:34:45Z",
      market_count: 0,
      snapshot_count: 0,
      opportunity_count: 0,
      scanner_version: "arbitrage-radar-v0",
      metadata: { connector: "polymarket", mode: "opportunity_detection_only" },
      occurred_at: "2026-04-16T14:34:45Z",
      trace_id: "trc_mock_arb_live_1435",
    },
  },
  {
    id: "2",
    type: "arbitrage.opportunity.observed",
    data: {
      sequence: 2,
      event_type: "arbitrage.opportunity.observed",
      resource_type: "opportunity",
      resource_id: "arb_mock_live_mkt_121_sell_both",
      opportunity_id: "arb_mock_live_mkt_121_sell_both",
      scan_id: "scan_mock_live_1435",
      market_id: "mkt_121",
      opportunity_type: "binary_sell_both",
      status: "observed",
      gross_edge: "0.0310",
      price_sum: "1.0310",
      capacity: "470.00",
      yes_price: "0.445",
      no_price: "0.586",
      yes_size: "510.00",
      no_size: "470.00",
      observed_at: "2026-04-16T14:34:54Z",
      reason_codes: ["yes_bid_plus_no_bid_above_one"],
      analysis_payload: {
        formula: "yes_bid + no_bid - 1",
        yes_bid: "0.445",
        no_bid: "0.586",
        price_sum: "1.0310",
        gross_edge: "0.0310",
      },
      occurred_at: "2026-04-16T14:34:54Z",
      trace_id: "trc_mock_arb_live_1435",
    },
  },
  {
    id: "3",
    type: "arbitrage.validation.passed",
    data: {
      sequence: 3,
      event_type: "arbitrage.validation.passed",
      resource_type: "validation",
      resource_id: "arb_mock_live_mkt_121_sell_both",
      validation_id: "arb_val_mock_live_mkt_121_sell_both",
      opportunity_id: "arb_mock_live_mkt_121_sell_both",
      validation_status: "valid",
      gross_edge: "0.0310",
      net_edge: "0.0210",
      fee_estimate: "0.0050",
      slippage_buffer: "0.0050",
      validated_capacity: "470.00",
      book_age_ms: 280,
      reason_codes: ["net_edge_positive_after_buffers"],
      validation_payload: { source: "mock" },
      validated_at: "2026-04-16T14:34:54Z",
      occurred_at: "2026-04-16T14:34:54Z",
      trace_id: "trc_mock_arb_live_1435",
    },
  },
  {
    id: "4",
    type: "arbitrage.opportunity.expired",
    data: {
      sequence: 4,
      event_type: "arbitrage.opportunity.expired",
      resource_type: "opportunity",
      resource_id: arbitrageOpportunityFixtures[3].id,
      opportunity_id: arbitrageOpportunityFixtures[3].id,
      scan_id: arbitrageOpportunityFixtures[3].scan_id,
      market_id: arbitrageOpportunityFixtures[3].market_id,
      opportunity_type: arbitrageOpportunityFixtures[3].opportunity_type,
      status: "expired",
      gross_edge: arbitrageOpportunityFixtures[3].gross_edge,
      price_sum: arbitrageOpportunityFixtures[3].price_sum,
      capacity: arbitrageOpportunityFixtures[3].capacity,
      yes_price: arbitrageOpportunityFixtures[3].yes_price,
      no_price: arbitrageOpportunityFixtures[3].no_price,
      yes_size: arbitrageOpportunityFixtures[3].yes_size,
      no_size: arbitrageOpportunityFixtures[3].no_size,
      observed_at: arbitrageOpportunityFixtures[3].observed_at,
      reason_codes: arbitrageOpportunityFixtures[3].reason_codes,
      analysis_payload: arbitrageOpportunityFixtures[3].analysis_payload,
      occurred_at: "2026-04-16T14:35:00Z",
      trace_id: "trc_mock_arb_live_1435",
    },
  },
  {
    id: "5",
    type: "arbitrage.scan.completed",
    data: {
      sequence: 5,
      event_type: "arbitrage.scan.completed",
      resource_type: "scan",
      resource_id: "scan_mock_live_1435",
      scan_id: "scan_mock_live_1435",
      started_at: "2026-04-16T14:34:45Z",
      finished_at: "2026-04-16T14:34:59Z",
      market_count: arbitrageScanFixtures[0].market_count,
      snapshot_count: arbitrageScanFixtures[0].snapshot_count,
      opportunity_count: 1,
      scanner_version: "arbitrage-radar-v0",
      metadata: { connector: "polymarket", mode: "opportunity_detection_only" },
      occurred_at: "2026-04-16T14:34:59Z",
      trace_id: "trc_mock_arb_live_1435",
    },
  },
  {
    id: "6",
    type: "arbitrage.analysis.generated",
    data: {
      sequence: 6,
      event_type: "arbitrage.analysis.generated",
      resource_type: "analysis",
      resource_id: "arb_analysis_mock_live_1435",
      analysis_id: "arb_analysis_mock_live_1435",
      generated_at: "2026-04-16T14:35:02Z",
      lookback_hours: 24,
      opportunity_count: arbitrageAnalysisRunFixtures[0].opportunity_count + 1,
      market_count: arbitrageAnalysisRunFixtures[0].market_count,
      summary_payload: arbitrageAnalysisRunFixtures[0].summary_payload,
      occurred_at: "2026-04-16T14:35:02Z",
      trace_id: "trc_mock_arb_live_1435",
    },
  },
];

export function getMockStreamEvents(channel: RealtimeChannel): MockStreamEvent[] {
  if (channel === "signals") {
    return signalEvents;
  }

  if (channel === "risk") {
    return riskEvents;
  }

  if (channel === "arbitrage") {
    return arbitrageEvents;
  }

  return eventStreamEvents;
}
