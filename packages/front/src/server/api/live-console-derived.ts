import "server-only";

import { cache } from "react";

import type { ApiMeta, ApiResponse } from "@/lib/contracts/api";
import type {
  ApprovalDto,
  MarketDto,
  PositionDto,
  RiskAlertDto,
  RiskBucketDto,
  RiskStateDto,
  SignalDto,
} from "@/lib/contracts/dto";
import { riskStateFixture } from "@/lib/server/polyedge-mock-data";
import { createResponse, fetchContract } from "@/server/api/base";
import { listMarkets } from "@/server/api/markets";
import { listPositions } from "@/server/api/positions";
import { listSignals } from "@/server/api/signals";

type RawRiskStateData = {
  mode: RiskStateDto["mode"];
  kill_switch: boolean;
  daily_pnl: string;
  gross_exposure: string;
  net_exposure: string;
  open_alerts: number;
  updated_at: string;
  version: number;
};

type RawSystemModeData = {
  mode: RiskStateDto["mode"];
  environment: RiskStateDto["environment"];
  updated_at: string;
  version: number;
};

type LiveConsoleDerivations = {
  meta: ApiMeta;
  approvals: ApprovalDto[];
  riskAlerts: RiskAlertDto[];
  riskBuckets: RiskBucketDto[];
  riskState: RiskStateDto;
};

const DEFAULT_BUCKET_LIMIT = 0.2;
const APPROVAL_ELIGIBLE_STATES = new Set<SignalDto["lifecycle_state"]>(["new", "active", "weakened"]);
const MANUAL_REVIEW_STATUSES = new Set<MarketDto["tradability_status"]>(["manual_review", "observe_only", "blocked"]);

function parseNumber(value: string): number {
  const parsed = Number.parseFloat(value);
  return Number.isFinite(parsed) ? parsed : 0;
}

function formatRatio(value: number): string {
  return value.toFixed(2);
}

function slugify(value: string): string {
  return value
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "_")
    .replace(/^_+|_+$/g, "");
}

function categoryLimit(category: string): number {
  switch (category.toLowerCase()) {
    case "crypto":
      return 0.35;
    case "regulation":
      return 0.25;
    case "macro":
      return 0.18;
    default:
      return DEFAULT_BUCKET_LIMIT;
  }
}

function approvalSeverity(signal: SignalDto, market: MarketDto | undefined): ApprovalDto["severity"] {
  if (market?.tradability_status === "blocked" || market?.ambiguity_level === "high") {
    return "critical";
  }

  if (market?.tradability_status && MANUAL_REVIEW_STATUSES.has(market.tradability_status)) {
    return "warning";
  }

  if (parseNumber(signal.confidence) < 0.65) {
    return "warning";
  }

  return "info";
}

function signalNeedsApproval(signal: SignalDto, market: MarketDto | undefined, mode: RiskStateDto["mode"]): boolean {
  if (signal.approved_at || signal.rejected_at) {
    return false;
  }

  if (!APPROVAL_ELIGIBLE_STATES.has(signal.lifecycle_state)) {
    return false;
  }

  const reviewText = `${signal.reason} ${signal.risk_decision}`.toLowerCase();

  if (
    reviewText.includes("manual review") ||
    reviewText.includes("manual confirmation") ||
    reviewText.includes("operator review") ||
    reviewText.includes("operator confirmation")
  ) {
    return true;
  }

  if (market?.tradability_status && MANUAL_REVIEW_STATUSES.has(market.tradability_status)) {
    return true;
  }

  return signal.lifecycle_state === "new" && mode === "manual_confirm";
}

function deriveApprovals(
  signals: SignalDto[],
  marketsById: Map<string, MarketDto>,
  mode: RiskStateDto["mode"],
): ApprovalDto[] {
  return signals
    .flatMap((signal) => {
      const market = marketsById.get(signal.market_id);
      const severity = approvalSeverity(signal, market);
      const pending = signalNeedsApproval(signal, market, mode);
      const status: ApprovalDto["status"] | null = signal.approved_at
        ? "approved"
        : signal.rejected_at
          ? "rejected"
          : pending
            ? "pending"
            : null;

      if (!status) {
        return [];
      }

      const occurredAt = signal.approved_at ?? signal.rejected_at ?? signal.updated_at;
      const marketQuestion = market?.question ?? signal.market_id;
      const summary = status === "pending"
        ? `${marketQuestion} requires manual confirmation. ${signal.risk_decision}`
        : status === "approved"
          ? `${marketQuestion} was approved for operator-driven execution.`
          : `${marketQuestion} was rejected and returned to manual monitoring.`;

      return [{
        id: `apr_signal_${signal.id}`,
        type: "signal",
        severity,
        owner: status === "pending"
          ? "Risk Engine"
          : signal.approved_by_user_id ?? signal.rejected_by_user_id ?? "Operator Desk",
        resource_id: signal.id,
        summary,
        status,
        requires_step_up_auth: status === "pending",
        created_at: occurredAt,
        updated_at: signal.updated_at,
        version: signal.version,
      }] satisfies ApprovalDto[];
    })
    .sort((left, right) => {
      const statusRank = { pending: 0, approved: 1, rejected: 2 } as const;
      const rankDelta = statusRank[left.status] - statusRank[right.status];

      if (rankDelta !== 0) {
        return rankDelta;
      }

      return right.updated_at.localeCompare(left.updated_at);
    });
}

function deriveRiskBuckets(
  positions: PositionDto[],
  marketsById: Map<string, MarketDto>,
): RiskBucketDto[] {
  const grouped = new Map<
    string,
    {
      exposureDollars: number;
      updatedAt: string;
      version: number;
    }
  >();

  for (const position of positions) {
    const market = marketsById.get(position.market_id);
    const bucketName = market?.category ?? "Uncategorized";
    const exposureDollars = Math.abs(parseNumber(position.quantity) * parseNumber(position.mark_price));
    const current = grouped.get(bucketName);

    if (current) {
      current.exposureDollars += exposureDollars;
      current.updatedAt = current.updatedAt > position.updated_at ? current.updatedAt : position.updated_at;
      current.version = Math.max(current.version, position.version);
      continue;
    }

    grouped.set(bucketName, {
      exposureDollars,
      updatedAt: position.updated_at,
      version: position.version,
    });
  }

  const totalExposure = [...grouped.values()].reduce((sum, bucket) => sum + bucket.exposureDollars, 0);

  return [...grouped.entries()]
    .map(([name, bucket]) => {
      const exposure = totalExposure > 0 ? bucket.exposureDollars / totalExposure : 0;
      const limit = categoryLimit(name);
      const utilization = limit > 0 ? exposure / limit : 0;
      const status: RiskBucketDto["status"] = utilization >= 1 ? "breach" : utilization >= 0.85 ? "watch" : "healthy";

      return {
        id: `bucket_${slugify(name)}`,
        name,
        exposure: formatRatio(exposure),
        limit: formatRatio(limit),
        utilization: formatRatio(utilization),
        status,
        updated_at: bucket.updatedAt,
        version: bucket.version,
      };
    })
    .sort((left, right) => Number.parseFloat(right.exposure) - Number.parseFloat(left.exposure));
}

function deriveRiskAlerts(
  riskState: RawRiskStateData,
  approvals: ApprovalDto[],
  buckets: RiskBucketDto[],
): RiskAlertDto[] {
  const alerts: RiskAlertDto[] = [];
  const dailyLossLimit = parseNumber(riskStateFixture.daily_loss_limit);
  const dailyLossUsed = Math.abs(Math.min(parseNumber(riskState.daily_pnl), 0));
  const dailyLossUsage = dailyLossLimit > 0 ? dailyLossUsed / dailyLossLimit : 0;
  const pendingApprovals = approvals.filter((approval) => approval.status === "pending");

  if (riskState.kill_switch) {
    alerts.push({
      id: "alt_kill_switch_active",
      severity: "critical",
      reason: "Kill switch is active. Execution remains halted until a protected release is approved.",
      target: "System Runtime",
      status: "unresolved",
      created_at: riskState.updated_at,
      updated_at: riskState.updated_at,
      version: riskState.version,
    });
  }

  if (dailyLossUsage >= 0.8) {
    alerts.push({
      id: "alt_daily_loss_usage",
      severity: dailyLossUsage >= 0.9 ? "critical" : "warning",
      reason: `Daily loss usage reached ${(dailyLossUsage * 100).toFixed(0)}% of the configured budget.`,
      target: "Global Risk",
      status: dailyLossUsage >= 0.9 ? "unresolved" : "watching",
      created_at: riskState.updated_at,
      updated_at: riskState.updated_at,
      version: riskState.version,
    });
  }

  for (const bucket of buckets) {
    if (bucket.status === "healthy") {
      continue;
    }

    alerts.push({
      id: `alt_bucket_${bucket.id}`,
      severity: bucket.status === "breach" ? "critical" : "warning",
      reason:
        bucket.status === "breach"
          ? `${bucket.name} exposure exceeded its configured concentration limit.`
          : `${bucket.name} exposure is approaching its configured concentration limit.`,
      target: `${bucket.name} Bucket`,
      status: bucket.status === "breach" ? "unresolved" : "watching",
      created_at: bucket.updated_at,
      updated_at: bucket.updated_at,
      version: bucket.version,
    });
  }

  if (pendingApprovals.length > 0) {
    alerts.push({
      id: "alt_pending_signal_approvals",
      severity: pendingApprovals.length >= 3 ? "critical" : "warning",
      reason: `${pendingApprovals.length} signal approval item${pendingApprovals.length === 1 ? "" : "s"} await operator review.`,
      target: "Approval Queue",
      status: "watching",
      created_at: pendingApprovals[0].created_at,
      updated_at: pendingApprovals[0].updated_at,
      version: Math.max(...pendingApprovals.map((approval) => approval.version)),
    });
  }

  return alerts.sort((left, right) => {
    const severityRank = { critical: 0, warning: 1 } as const;
    const rankDelta = severityRank[left.severity] - severityRank[right.severity];

    if (rankDelta !== 0) {
      return rankDelta;
    }

    return right.updated_at.localeCompare(left.updated_at);
  });
}

const readLiveConsoleDerivations = cache(async (): Promise<LiveConsoleDerivations> => {
  const rawRiskStateFallback = createResponse<RawRiskStateData>("risk_state", {
    mode: riskStateFixture.mode,
    kill_switch: riskStateFixture.kill_switch,
    daily_pnl: riskStateFixture.daily_pnl,
    gross_exposure: riskStateFixture.gross_exposure,
    net_exposure: riskStateFixture.net_exposure,
    open_alerts: riskStateFixture.open_alerts,
    updated_at: riskStateFixture.updated_at,
    version: riskStateFixture.version,
  });
  const rawSystemModeFallback = createResponse<RawSystemModeData>("system_mode", {
    mode: riskStateFixture.mode,
    environment: riskStateFixture.environment,
    updated_at: riskStateFixture.updated_at,
    version: riskStateFixture.version,
  });

  const [{ data: markets, meta }, { data: signals }, { data: positions }, riskStateResponse, systemModeResponse] =
    await Promise.all([
      listMarkets(),
      listSignals(),
      listPositions(),
      fetchContract<ApiResponse<RawRiskStateData>>("/api/v1/risk/state", rawRiskStateFallback),
      fetchContract<ApiResponse<RawSystemModeData>>("/api/v1/system/mode", rawSystemModeFallback),
    ]);

  const marketsById = new Map(markets.map((market) => [market.id, market]));
  const approvals = deriveApprovals(signals, marketsById, systemModeResponse.data.mode);
  const riskBuckets = deriveRiskBuckets(positions, marketsById);
  const riskAlerts = deriveRiskAlerts(riskStateResponse.data, approvals, riskBuckets);
  const openAlerts = riskAlerts.filter((alert) => alert.status !== "contained").length;
  const dailyLossUsed = Math.abs(Math.min(parseNumber(riskStateResponse.data.daily_pnl), 0));

  return {
    meta,
    approvals,
    riskAlerts,
    riskBuckets,
    riskState: {
      id: "risk_state_global",
      mode: riskStateResponse.data.mode,
      environment: systemModeResponse.data.environment,
      kill_switch: riskStateResponse.data.kill_switch,
      daily_pnl: riskStateResponse.data.daily_pnl,
      gross_exposure: riskStateResponse.data.gross_exposure,
      net_exposure: riskStateResponse.data.net_exposure,
      open_alerts: openAlerts,
      daily_loss_limit: riskStateFixture.daily_loss_limit,
      daily_loss_used: dailyLossUsed.toFixed(2),
      updated_at: riskStateResponse.data.updated_at,
      version: riskStateResponse.data.version,
    },
  };
});

export async function readDerivedLiveRiskState(): Promise<{ data: RiskStateDto; meta: ApiMeta }> {
  const snapshot = await readLiveConsoleDerivations();
  return {
    data: snapshot.riskState,
    meta: snapshot.meta,
  };
}

export async function readDerivedLiveApprovals(): Promise<{ data: ApprovalDto[]; meta: ApiMeta }> {
  const snapshot = await readLiveConsoleDerivations();
  return {
    data: snapshot.approvals,
    meta: snapshot.meta,
  };
}

export async function readDerivedLiveRiskAlerts(): Promise<{ data: RiskAlertDto[]; meta: ApiMeta }> {
  const snapshot = await readLiveConsoleDerivations();
  return {
    data: snapshot.riskAlerts,
    meta: snapshot.meta,
  };
}

export async function readDerivedLiveRiskBuckets(): Promise<{ data: RiskBucketDto[]; meta: ApiMeta }> {
  const snapshot = await readLiveConsoleDerivations();
  return {
    data: snapshot.riskBuckets,
    meta: snapshot.meta,
  };
}
