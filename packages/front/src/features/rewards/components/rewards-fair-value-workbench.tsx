"use client";

import { useMemo, useState } from "react";
import { RefreshCw } from "lucide-react";

import { PageHeader } from "@/components/shared/page-header";
import { StatusPill } from "@/components/shared/status-pill";
import { TruncateText } from "@/components/shared/truncate-text";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import type { RewardBotSnapshotDto, RewardQuotePlanDto } from "@/lib/contracts/dto";
import { readRewardBotSnapshot } from "@/lib/api/rewards";
import { formatFixed, formatInteger, toFiniteNumber } from "@/lib/formatters";

type FairValueStats = {
  total: number;
  passed: number;
  blocked: number;
  avgConfidence: number;
};

export function RewardsFairValueWorkbench({
  initialSnapshot,
}: {
  initialSnapshot: RewardBotSnapshotDto;
}) {
  const [snapshot, setSnapshot] = useState(initialSnapshot);
  const [refreshing, setRefreshing] = useState(false);
  const plans = useMemo(() => snapshot.quote_plans ?? [], [snapshot.quote_plans]);
  const stats = useMemo(() => fairValueStats(plans), [plans]);

  async function refresh() {
    setRefreshing(true);
    try {
      const response = await readRewardBotSnapshot({
        plans_page: 1,
        plans_page_size: 100,
        plans_sort_by: "score",
        plans_sort_order: "desc",
        orders_page: 1,
        orders_page_size: 5,
      });
      setSnapshot(response.data);
    } finally {
      setRefreshing(false);
    }
  }

  return (
    <div className="space-y-5">
      <PageHeader
        eyebrow="Market maker"
        title="Fair value"
        description="实时做市估值、edge 与 rewards rebate 约束。"
        actions={
          <Button type="button" variant="outline" size="sm" onClick={refresh} disabled={refreshing}>
            <RefreshCw className="size-4" />
            Refresh
          </Button>
        }
      />

      <section className="grid gap-3 md:grid-cols-4">
        <FairValueStat title="Tracked" value={formatInteger(stats.total)} />
        <FairValueStat title="Passed" value={formatInteger(stats.passed)} tone="success" />
        <FairValueStat title="Blocked" value={formatInteger(stats.blocked)} tone="danger" />
        <FairValueStat title="Avg confidence" value={`${stats.avgConfidence.toFixed(0)}%`} />
      </section>

      <FairValueTable plans={plans} />
    </div>
  );
}

function FairValueStat({
  title,
  value,
  tone = "neutral",
}: {
  title: string;
  value: string;
  tone?: "neutral" | "success" | "danger";
}) {
  return (
    <Card>
      <CardHeader className="pb-2">
        <CardTitle className="text-xs font-medium text-muted-foreground">{title}</CardTitle>
      </CardHeader>
      <CardContent>
        <StatusPill tone={tone}>{value}</StatusPill>
      </CardContent>
    </Card>
  );
}

function FairValueTable({ plans }: { plans: RewardQuotePlanDto[] }) {
  return (
    <div className="overflow-x-auto rounded-lg border border-border/70">
      <Table className="min-w-[1280px] table-fixed">
        <TableHeader>
          <TableRow>
            <TableHead className="w-[30%]">Market</TableHead>
            <TableHead>Fair YES</TableHead>
            <TableHead>Market mid</TableHead>
            <TableHead>Confidence</TableHead>
            <TableHead>Uncertainty</TableHead>
            <TableHead className="w-[25%]">Edges</TableHead>
            <TableHead className="w-[18%]">Decision</TableHead>
          </TableRow>
        </TableHeader>
        <TableBody>
          {plans.length === 0 ? (
            <TableRow>
              <TableCell colSpan={7} className="py-8 text-center text-sm text-muted-foreground">
                No quote plans.
              </TableCell>
            </TableRow>
          ) : (
            plans.map((plan) => {
              const fair = plan.fair_value;
              return (
                <TableRow key={`${plan.condition_id}:${plan.strategy_profile ?? "standard"}`}>
                  <TableCell className="align-top">
                    <div className="space-y-1">
                      <TruncateText text={plan.question} lines={2} className="font-medium" />
                      <div className="flex gap-2">
                        <StatusPill tone={plan.eligible ? "success" : "neutral"}>
                          {plan.eligible ? "eligible" : "blocked"}
                        </StatusPill>
                        <StatusPill tone="neutral">{plan.strategy_profile ?? "standard"}</StatusPill>
                      </div>
                    </div>
                  </TableCell>
                  <TableCell className="align-top font-mono">
                    {fair ? formatFixed(fair.estimate.fair_yes, 4) : "n/a"}
                  </TableCell>
                  <TableCell className="align-top font-mono">
                    {fair?.estimate.market_midpoint_yes == null
                      ? "n/a"
                      : formatFixed(fair.estimate.market_midpoint_yes, 4)}
                  </TableCell>
                  <TableCell className="align-top">
                    <StatusPill tone={fair?.passed ? "success" : "warning"}>
                      {fair ? `${(toFiniteNumber(fair.estimate.confidence) * 100).toFixed(0)}%` : "n/a"}
                    </StatusPill>
                  </TableCell>
                  <TableCell className="align-top font-mono">
                    {fair ? `${formatFixed(fair.estimate.uncertainty_cents, 2)}c` : "n/a"}
                  </TableCell>
                  <TableCell className="align-top">
                    <div className="space-y-1 font-mono text-xs">
                      {fair?.edges.length
                        ? fair.edges.map((edge) => (
                            <div key={`${edge.token_id}:${edge.outcome}`} className="flex gap-2">
                              <StatusPill tone={edge.passed ? "success" : "danger"}>
                                {edge.outcome}
                              </StatusPill>
                              <span>
                                raw {formatFixed(edge.raw_edge_cents, 2)}c / eff{" "}
                                {formatFixed(edge.effective_edge_cents, 2)}c
                              </span>
                            </div>
                          ))
                        : "n/a"}
                    </div>
                  </TableCell>
                  <TableCell className="align-top">
                    <div className="space-y-1">
                      <StatusPill tone={fair?.passed ? "success" : "danger"}>
                        {fair?.passed ? "pass" : "blocked"}
                      </StatusPill>
                      <TruncateText
                        text={fair?.reason ?? plan.reason}
                        lines={2}
                        className="text-xs text-muted-foreground"
                      />
                    </div>
                  </TableCell>
                </TableRow>
              );
            })
          )}
        </TableBody>
      </Table>
    </div>
  );
}

function fairValueStats(plans: RewardQuotePlanDto[]): FairValueStats {
  const fairPlans = plans.filter((plan) => plan.fair_value);
  const passed = fairPlans.filter((plan) => plan.fair_value?.passed).length;
  const confidenceSum = fairPlans.reduce(
    (sum, plan) => sum + toFiniteNumber(plan.fair_value?.estimate.confidence),
    0,
  );
  return {
    total: fairPlans.length,
    passed,
    blocked: fairPlans.length - passed,
    avgConfidence: fairPlans.length === 0 ? 0 : (confidenceSum / fairPlans.length) * 100,
  };
}
