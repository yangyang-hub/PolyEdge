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
import { dictionary, formatMessage, translateEnum } from "@/lib/i18n/dictionaries";

type FairValueStats = {
  total: number;
  passed: number;
  blocked: number;
  notEvaluated: number;
  avgConfidence: number;
};

export function RewardsFairValueWorkbench({
  initialSnapshot,
}: {
  initialSnapshot: RewardBotSnapshotDto;
}) {
  const [snapshot, setSnapshot] = useState(initialSnapshot);
  const [refreshing, setRefreshing] = useState(false);
  const [refreshError, setRefreshError] = useState<string | null>(null);
  const plans = useMemo(() => snapshot.quote_plans ?? [], [snapshot.quote_plans]);
  const stats = useMemo(() => fairValueStats(plans), [plans]);

  async function refresh() {
    setRefreshing(true);
    setRefreshError(null);
    try {
      const response = await readRewardBotSnapshot({
        plans_page: 1,
        plans_page_size: 100,
        plans_sort_by: "selection_score",
        plans_sort_order: "desc",
        orders_page: 1,
        orders_page_size: 5,
      });
      setSnapshot(response.data);
    } catch (error) {
      setRefreshError(
        error instanceof Error ? error.message : dictionary.rewards.fairValueRefreshFailed,
      );
    } finally {
      setRefreshing(false);
    }
  }

  return (
    <div className="space-y-5">
      <PageHeader
        eyebrow={dictionary.rewards.eyebrow}
        title={dictionary.rewards.fairValuePageTitle}
        description={dictionary.rewards.fairValuePageDescription}
        actions={
          <Button type="button" variant="outline" size="sm" onClick={refresh} disabled={refreshing} aria-busy={refreshing}>
            <RefreshCw className={refreshing ? "size-4 animate-spin" : "size-4"} aria-hidden="true" />
            {refreshing ? dictionary.rewards.refreshing : dictionary.rewards.refresh}
          </Button>
        }
      />

      {refreshError ? (
        <div role="alert" className="rounded-lg border border-destructive/30 bg-destructive/10 p-4 text-sm text-destructive">
          <p className="font-medium">{dictionary.rewards.fairValueRefreshFailed}</p>
          <p className="mt-1 break-words text-xs">{refreshError}</p>
        </div>
      ) : null}

      <p className="text-sm text-muted-foreground">
        {formatMessage(dictionary.rewards.fairValueCurrentPageNotice, {
          loaded: plans.length,
          total: snapshot.plans_page?.total_items ?? snapshot.status.plans_total,
        })}
      </p>

      <section className="grid gap-3 md:grid-cols-2 xl:grid-cols-5">
        <FairValueStat title={dictionary.rewards.fairValueAssessedCurrentPage} value={formatInteger(stats.total)} />
        <FairValueStat title={dictionary.rewards.fairValuePassedCurrentPage} value={formatInteger(stats.passed)} tone="success" />
        <FairValueStat title={dictionary.rewards.fairValueBlockedCurrentPage} value={formatInteger(stats.blocked)} tone="danger" />
        <FairValueStat title={dictionary.rewards.fairValueNotEvaluated} value={formatInteger(stats.notEvaluated)} />
        <FairValueStat title={dictionary.rewards.fairValueAverageConfidenceCurrentPage} value={`${formatFixed(stats.avgConfidence, 0)}%`} />
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
    <div className="overflow-x-auto rounded-lg border border-border/70 [content-visibility:auto]">
      <Table className="min-w-[1280px] table-fixed">
        <TableHeader>
          <TableRow>
            <TableHead className="w-[30%]">{dictionary.rewards.market}</TableHead>
            <TableHead>{dictionary.rewards.fairValueYes}</TableHead>
            <TableHead>{dictionary.rewards.fairValueMarketMidpoint}</TableHead>
            <TableHead>{dictionary.rewards.fairValueConfidence}</TableHead>
            <TableHead>{dictionary.rewards.fairValueUncertainty}</TableHead>
            <TableHead className="w-[25%]">{dictionary.rewards.fairValueEdges}</TableHead>
            <TableHead className="w-[18%]">{dictionary.rewards.fairValueDecision}</TableHead>
          </TableRow>
        </TableHeader>
        <TableBody>
          {plans.length === 0 ? (
            <TableRow>
              <TableCell colSpan={7} className="py-8 text-center text-sm text-muted-foreground">
                {dictionary.rewards.fairValueNoPlans}
              </TableCell>
            </TableRow>
          ) : (
            plans.map((plan) => {
              const fair = plan.fair_value;
              const evaluated = fair?.assessment_status !== "not_evaluated";
              return (
                <TableRow key={`${plan.condition_id}:${plan.strategy_profile ?? "standard"}`}>
                  <TableCell className="align-top">
                    <div className="space-y-1">
                      <TruncateText text={plan.question} lines={2} className="font-medium" />
                      <div className="flex gap-2">
                        <StatusPill tone={plan.eligible ? "success" : "neutral"}>
                          {plan.eligible ? dictionary.rewards.eligible : dictionary.rewards.blocked}
                        </StatusPill>
                        <StatusPill tone="neutral">{plan.strategy_profile ?? "standard"}</StatusPill>
                      </div>
                    </div>
                  </TableCell>
                  <TableCell className="align-top font-mono">
                    {fair ? formatFixed(fair.estimate.fair_yes, 4) : dictionary.rewards.notAvailable}
                  </TableCell>
                  <TableCell className="align-top font-mono">
                    {fair?.estimate.market_midpoint_yes == null
                      ? dictionary.rewards.notAvailable
                      : formatFixed(fair.estimate.market_midpoint_yes, 4)}
                  </TableCell>
                  <TableCell className="align-top">
                    <StatusPill tone={fair?.passed ? "success" : evaluated ? "warning" : "neutral"}>
                      {fair ? `${(toFiniteNumber(fair.estimate.confidence) * 100).toFixed(0)}%` : dictionary.rewards.notAvailable}
                    </StatusPill>
                  </TableCell>
                  <TableCell className="align-top font-mono">
                    {fair ? `${formatFixed(fair.estimate.uncertainty_cents, 2)}c` : dictionary.rewards.notAvailable}
                  </TableCell>
                  <TableCell className="align-top">
                    <div className="space-y-1 font-mono text-xs">
                      {fair?.edges.length
                        ? fair.edges.map((edge) => (
                            <div key={`${edge.token_id}:${edge.outcome}`} className="flex gap-2">
                              <StatusPill tone={edge.passed ? "success" : "danger"}>
                                {translateEnum(edge.outcome)}
                              </StatusPill>
                              <span>
                                raw {formatFixed(edge.raw_edge_cents, 2)}c / eff{" "}
                                {formatFixed(edge.effective_edge_cents, 2)}c / reward-adj{" "}
                                {formatFixed(edge.reward_adjusted_edge_cents, 2)}c
                              </span>
                            </div>
                          ))
                        : dictionary.rewards.notAvailable}
                    </div>
                  </TableCell>
                  <TableCell className="align-top">
                    <div className="space-y-1">
                      <StatusPill tone={fair?.passed ? "success" : evaluated ? "danger" : "warning"}>
                        {fair?.passed
                          ? dictionary.rewards.fairValuePass
                          : evaluated
                            ? dictionary.rewards.blocked
                            : dictionary.rewards.fairValueNotEvaluated}
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
  let total = 0;
  let passed = 0;
  let notEvaluated = 0;
  let confidenceSum = 0;
  for (const plan of plans) {
    const fair = plan.fair_value;
    if (!fair) continue;
    if (fair.assessment_status === "not_evaluated") {
      notEvaluated += 1;
      continue;
    }
    total += 1;
    if (fair.passed) passed += 1;
    confidenceSum += toFiniteNumber(fair.estimate.confidence);
  }
  return {
    total,
    passed,
    blocked: total - passed,
    notEvaluated,
    avgConfidence: total === 0 ? 0 : (confidenceSum / total) * 100,
  };
}
