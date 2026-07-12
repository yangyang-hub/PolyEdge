"use client";

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { RefreshCcw } from "lucide-react";

import { PaginationBar } from "@/components/pagination-bar";
import { MeterBar } from "@/components/shared/meter-bar";
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
import type {
  RewardListPageDto,
  RewardStrategyActionDto,
  RewardStrategyDecisionDto,
  RewardStrategyRunDto,
} from "@/lib/contracts/dto";
import {
  listAllRewardStrategyActions,
  listAllRewardStrategyDecisions,
  listRewardStrategyRuns,
} from "@/lib/api/rewards";
import { formatFixed, formatUsdFixed, toFiniteNumber } from "@/lib/formatters";
import type { PaginationState } from "@/hooks/use-pagination";
import { dictionary, translateEnum } from "@/lib/i18n/dictionaries";

const RUNS_PAGE_SIZE = 20;
const DETAIL_PAGE_SIZE = 20;

type AnalyticsCount = { key: string; count: number };

type StatusTone = "neutral" | "primary" | "success" | "warning" | "danger" | "violet";

function statusTone(status: string): StatusTone {
  if (status === "completed" || status === "succeeded") return "success";
  if (status === "failed") return "danger";
  if (status === "running" || status === "executing") return "warning";
  return "neutral";
}

function formatTime(value?: string | null) {
  if (!value) return "—";
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  return date.toLocaleString();
}

function metricValue(run: RewardStrategyRunDto, key: string) {
  const metrics = run.metrics;
  if (metrics == null || typeof metrics !== "object" || Array.isArray(metrics)) return "—";
  const value = (metrics as Record<string, unknown>)[key];
  if (value == null) return "—";
  return String(value);
}

function countBy<T>(items: T[], keyFor: (item: T) => string | null | undefined): AnalyticsCount[] {
  const counts = new Map<string, number>();
  for (const item of items) {
    const key = keyFor(item);
    if (!key) continue;
    counts.set(key, (counts.get(key) ?? 0) + 1);
  }
  return [...counts.entries()]
    .map(([key, count]) => ({ key, count }))
    .sort((left, right) => right.count - left.count || left.key.localeCompare(right.key));
}

function DistributionList({
  items,
  labelFor = translateEnum,
}: {
  items: AnalyticsCount[];
  labelFor?: (key: string) => string;
}) {
  const maximum = Math.max(0, ...items.map((item) => item.count));
  if (items.length === 0) {
    return <p className="py-4 text-center text-sm text-muted-foreground">{dictionary.rewards.analyticsNoData}</p>;
  }
  return (
    <div className="space-y-3">
      {items.map((item) => (
        <div key={item.key} className="space-y-1.5">
          <div className="flex items-center justify-between gap-3 text-xs">
            <span className="min-w-0 truncate text-muted-foreground" title={labelFor(item.key)}>
              {labelFor(item.key)}
            </span>
            <span className="font-mono text-foreground">{item.count}</span>
          </div>
          <MeterBar
            value={`${maximum > 0 ? Math.max(4, (item.count / maximum) * 100) : 0}%`}
            tone="primary"
          />
        </div>
      ))}
    </div>
  );
}

export function RewardsRunLedgerPanel() {
  const [runs, setRuns] = useState<RewardStrategyRunDto[]>([]);
  const [runsPage, setRunsPage] = useState<RewardListPageDto | null>(null);
  const [runPage, setRunPage] = useState(1);
  const [requestedRunPage, setRequestedRunPage] = useState(1);
  const [selectedRunId, setSelectedRunId] = useState<number | null>(null);
  const [decisions, setDecisions] = useState<RewardStrategyDecisionDto[]>([]);
  const [actions, setActions] = useState<RewardStrategyActionDto[]>([]);
  const [loading, setLoading] = useState(false);
  const [detailLoading, setDetailLoading] = useState(false);
  const [detailReloadToken, setDetailReloadToken] = useState(0);
  const [error, setError] = useState<string | null>(null);
  const runsRequestSequence = useRef(0);
  const detailRequestSequence = useRef(0);

  const selectedRun = useMemo(
    () => runs.find((run) => run.run_id === selectedRunId) ?? null,
    [runs, selectedRunId],
  );
  const analytics = useMemo(() => {
    const eligible = decisions.filter((decision) => decision.eligible).length;
    const fairValueAssessed = decisions.filter((decision) => decision.fair_value_passed != null);
    const fairValuePassed = fairValueAssessed.filter((decision) => decision.fair_value_passed).length;
    const selectionTotal = decisions.reduce(
      (total, decision) => total + toFiniteNumber(decision.selection_score),
      0,
    );
    const succeededActions = actions.filter((action) => action.status === "succeeded").length;
    const blockers = countBy(
      decisions.flatMap((decision) =>
        decision.eligible
          ? []
          : decision.blocker_codes.length > 0
            ? decision.blocker_codes
            : [decision.reason_code],
      ),
      (code) => code,
    );
    const providerActions = [
      ...countBy(decisions, (decision) => decision.ai_action).map((item) => ({
        ...item,
        key: `ai:${item.key}`,
      })),
      ...countBy(decisions, (decision) => decision.info_risk_action).map((item) => ({
        ...item,
        key: `info:${item.key}`,
      })),
    ].sort((left, right) => right.count - left.count || left.key.localeCompare(right.key));
    return {
      eligible,
      fairValueAssessed: fairValueAssessed.length,
      fairValuePassed,
      averageSelection: decisions.length > 0 ? selectionTotal / decisions.length : 0,
      succeededActions,
      blockers,
      providerActions,
      actionTypes: countBy(actions, (action) => action.action_type),
      actionStatuses: countBy(actions, (action) => action.status),
    };
  }, [actions, decisions]);

  const blockerLabels = dictionary.rewards.analyticsBlockerLabels as Readonly<Record<string, string>>;
  const providerLabel = useCallback((key: string) => {
    const [provider, action] = key.split(":", 2);
    const providerName = provider === "ai" ? dictionary.rewards.aiAdvisory : dictionary.rewards.infoRisk;
    return `${providerName} · ${translateEnum(action ?? key)}`;
  }, []);

  const loadRuns = useCallback(async (page: number) => {
    const requestSequence = ++runsRequestSequence.current;
    setLoading(true);
    setError(null);
    try {
      const response = await listRewardStrategyRuns({ page, page_size: RUNS_PAGE_SIZE });
      if (requestSequence !== runsRequestSequence.current) return;
      setRuns(response.data.items);
      setRunsPage(response.data.page);
      setRunPage(response.data.page.page);
      setDecisions([]);
      setActions([]);
      if (response.data.items.length === 0) {
        detailRequestSequence.current += 1;
        setSelectedRunId(null);
        setDetailLoading(false);
      } else {
        setDetailLoading(true);
        setSelectedRunId((current) =>
          response.data.items.some((run) => run.run_id === current)
            ? current
            : response.data.items[0]?.run_id ?? null,
        );
        setDetailReloadToken((current) => current + 1);
      }
    } catch (err) {
      if (requestSequence !== runsRequestSequence.current) return;
      setError(err instanceof Error ? err.message : dictionary.rewards.runLedgerLoadFailed);
    } finally {
      if (requestSequence === runsRequestSequence.current) setLoading(false);
    }
  }, []);

  const handleRunPageChange = useCallback((page: number) => {
    setRequestedRunPage(page);
  }, []);
  const runsPagination: PaginationState | null = runsPage
    ? {
        page: runsPage.page,
        totalPages: runsPage.total_pages,
        start: 0,
        end: 0,
        setPage: handleRunPageChange,
        goPrevious: () => handleRunPageChange(Math.max(1, runsPage.page - 1)),
        goNext: () => handleRunPageChange(Math.min(runsPage.total_pages, runsPage.page + 1)),
        reset: () => handleRunPageChange(1),
        hasPrevious: runsPage.page > 1,
        hasNext: runsPage.page < runsPage.total_pages,
      }
    : null;

  useEffect(() => {
    const timeout = window.setTimeout(() => {
      void loadRuns(requestedRunPage);
    }, 0);
    return () => window.clearTimeout(timeout);
  }, [loadRuns, requestedRunPage]);

  useEffect(() => {
    if (selectedRunId == null) {
      detailRequestSequence.current += 1;
      return;
    }
    const requestSequence = ++detailRequestSequence.current;
    void Promise.all([
      listAllRewardStrategyDecisions(selectedRunId),
      listAllRewardStrategyActions(selectedRunId),
    ])
      .then(([decisionItems, actionItems]) => {
        if (requestSequence !== detailRequestSequence.current) return;
        setError(null);
        setDecisions(decisionItems);
        setActions(actionItems);
      })
      .catch((err) => {
        if (requestSequence !== detailRequestSequence.current) return;
        setError(err instanceof Error ? err.message : dictionary.rewards.runLedgerLoadFailed);
      })
      .finally(() => {
        if (requestSequence === detailRequestSequence.current) setDetailLoading(false);
      });
  }, [detailReloadToken, selectedRunId]);

  return (
    <div className="grid gap-4 xl:grid-cols-[360px_minmax(0,1fr)]" aria-busy={loading || detailLoading}>
      <Card>
        <CardHeader className="flex flex-row items-center justify-between border-b border-border/70">
          <CardTitle>{dictionary.rewards.strategyRuns}</CardTitle>
          <Button
            type="button"
            variant="outline"
            size="icon"
            aria-label={dictionary.rewards.refreshRuns}
            onClick={() => void loadRuns(runPage)}
            disabled={loading}
          >
            <RefreshCcw className="size-4" aria-hidden="true" />
          </Button>
        </CardHeader>
        <CardContent className="space-y-3">
          {error ? <p className="text-sm text-destructive">{error}</p> : null}
          <div className="space-y-2">
            {runs.length === 0 ? (
              <p className="py-6 text-center text-sm text-muted-foreground">{dictionary.rewards.none}</p>
            ) : (
              runs.map((run) => (
                <button
                  key={run.run_id}
                  type="button"
                  onClick={() => {
                    if (run.run_id === selectedRunId) return;
                    detailRequestSequence.current += 1;
                    setDecisions([]);
                    setActions([]);
                    setDetailLoading(true);
                    setSelectedRunId(run.run_id);
                  }}
                  className="flex w-full flex-col gap-2 rounded-md border border-border/70 px-3 py-2 text-left transition-colors hover:bg-muted/60 data-[active=true]:border-primary data-[active=true]:bg-muted"
                  data-active={run.run_id === selectedRunId}
                >
                  <div className="flex items-center justify-between gap-2">
                    <span className="font-mono text-xs text-muted-foreground">#{run.run_id}</span>
                    <StatusPill tone={statusTone(run.status)}>{translateEnum(run.status)}</StatusPill>
                  </div>
                  <div className="flex items-center justify-between gap-2 text-xs">
                    <span>{translateEnum(run.trigger_type)}</span>
                    <span className="font-mono text-muted-foreground">{formatTime(run.started_at)}</span>
                  </div>
                </button>
              ))
            )}
          </div>
          {runsPage && runsPagination ? (
            <PaginationBar
              pagination={runsPagination}
              totalItems={runsPage.total_items}
            />
          ) : null}
        </CardContent>
      </Card>

      <div className="space-y-4">
        <Card>
          <CardHeader className="border-b border-border/70">
            <CardTitle>{dictionary.rewards.runSummary}</CardTitle>
          </CardHeader>
          <CardContent>
            {selectedRun == null ? (
              <p className="py-6 text-center text-sm text-muted-foreground">{dictionary.rewards.none}</p>
            ) : (
              <div className="grid gap-3 text-sm md:grid-cols-4">
                <div>
                  <div className="text-xs text-muted-foreground">{dictionary.rewards.status}</div>
                  <StatusPill tone={statusTone(selectedRun.status)}>{translateEnum(selectedRun.status)}</StatusPill>
                </div>
                <div>
                  <div className="text-xs text-muted-foreground">{dictionary.rewards.candidatePlans}</div>
                  <div className="font-mono">{metricValue(selectedRun, "plans_built")}</div>
                </div>
                <div>
                  <div className="text-xs text-muted-foreground">{dictionary.rewards.openOrders}</div>
                  <div className="font-mono">{metricValue(selectedRun, "placed_orders")}</div>
                </div>
                <div>
                  <div className="text-xs text-muted-foreground">{dictionary.rewards.time}</div>
                  <div className="font-mono text-xs">{formatTime(selectedRun.completed_at ?? selectedRun.started_at)}</div>
                </div>
              </div>
            )}
          </CardContent>
        </Card>

        <Card>
          <CardHeader className="border-b border-border/70">
            <CardTitle>{dictionary.rewards.runAnalytics}</CardTitle>
          </CardHeader>
          <CardContent className="space-y-5">
            <div className="grid gap-3 sm:grid-cols-2 xl:grid-cols-4">
              <div className="rounded-md border border-border/70 p-3">
                <div className="text-xs text-muted-foreground">{dictionary.rewards.analyticsDecisionCoverage}</div>
                <div className="mt-1 font-mono text-xl font-semibold">
                  {analytics.eligible}/{decisions.length}
                </div>
                <div className="text-xs text-muted-foreground">{dictionary.rewards.analyticsEligibleHint}</div>
              </div>
              <div className="rounded-md border border-border/70 p-3">
                <div className="text-xs text-muted-foreground">{dictionary.rewards.analyticsAverageSelection}</div>
                <div className="mt-1 font-mono text-xl font-semibold">{formatFixed(analytics.averageSelection, 2)}</div>
                <div className="text-xs text-muted-foreground">{dictionary.rewards.analyticsSelectionHint}</div>
              </div>
              <div className="rounded-md border border-border/70 p-3">
                <div className="text-xs text-muted-foreground">{dictionary.rewards.analyticsFairValuePass}</div>
                <div className="mt-1 font-mono text-xl font-semibold">
                  {analytics.fairValuePassed}/{analytics.fairValueAssessed}
                </div>
                <div className="text-xs text-muted-foreground">{dictionary.rewards.analyticsFairValueHint}</div>
              </div>
              <div className="rounded-md border border-border/70 p-3">
                <div className="text-xs text-muted-foreground">{dictionary.rewards.analyticsActionSuccess}</div>
                <div className="mt-1 font-mono text-xl font-semibold">
                  {analytics.succeededActions}/{actions.length}
                </div>
                <div className="text-xs text-muted-foreground">{dictionary.rewards.analyticsActionHint}</div>
              </div>
            </div>

            <div className="grid gap-4 lg:grid-cols-2">
              <div className="rounded-md border border-border/70 p-4">
                <h3 className="mb-4 text-sm font-medium">{dictionary.rewards.analyticsBlockers}</h3>
                <DistributionList items={analytics.blockers} labelFor={(key) => blockerLabels[key] ?? translateEnum(key)} />
              </div>
              <div className="rounded-md border border-border/70 p-4">
                <h3 className="mb-4 text-sm font-medium">{dictionary.rewards.analyticsProviderActions}</h3>
                <DistributionList items={analytics.providerActions} labelFor={providerLabel} />
              </div>
              <div className="rounded-md border border-border/70 p-4">
                <h3 className="mb-4 text-sm font-medium">{dictionary.rewards.analyticsActionTypes}</h3>
                <DistributionList items={analytics.actionTypes} />
              </div>
              <div className="rounded-md border border-border/70 p-4">
                <h3 className="mb-4 text-sm font-medium">{dictionary.rewards.analyticsActionStatuses}</h3>
                <DistributionList items={analytics.actionStatuses} />
              </div>
            </div>
          </CardContent>
        </Card>

        <Card>
          <CardHeader className="border-b border-border/70">
            <CardTitle>{dictionary.rewards.runDecisions}</CardTitle>
          </CardHeader>
          <CardContent>
            <Table className="min-w-[980px] table-fixed">
              <TableHeader>
                <TableRow>
                  <TableHead className="w-[42%]">{dictionary.rewards.market}</TableHead>
                  <TableHead>{dictionary.rewards.state}</TableHead>
                  <TableHead>{dictionary.rewards.selectionScore}</TableHead>
                  <TableHead>{dictionary.rewards.notional}</TableHead>
                  <TableHead className="w-[28%]">{dictionary.rewards.reason}</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {decisions.length === 0 ? (
                  <TableRow>
                    <TableCell colSpan={5} className="py-6 text-center text-sm text-muted-foreground">
                      {dictionary.rewards.none}
                    </TableCell>
                  </TableRow>
                ) : (
                  decisions.slice(0, DETAIL_PAGE_SIZE).map((decision) => (
                    <TableRow key={`${decision.run_id}:${decision.condition_id}:${decision.strategy_profile}`}>
                      <TableCell className="align-top font-mono text-xs">{decision.condition_id}</TableCell>
                      <TableCell className="align-top">
                        <StatusPill tone={decision.eligible ? "success" : "neutral"}>
                          {translateEnum(decision.quote_readiness)}
                        </StatusPill>
                      </TableCell>
                      <TableCell className="align-top font-mono">
                        {formatFixed(decision.selection_score, 2)}
                      </TableCell>
                      <TableCell className="align-top font-mono">
                        {formatUsdFixed(decision.planned_buy_notional_usd)}
                      </TableCell>
                      <TableCell className="align-top text-xs text-muted-foreground">
                        <TruncateText text={decision.reason} lines={2} />
                      </TableCell>
                    </TableRow>
                  ))
                )}
              </TableBody>
            </Table>
          </CardContent>
        </Card>

        <Card>
          <CardHeader className="border-b border-border/70">
            <CardTitle>{dictionary.rewards.runActions}</CardTitle>
          </CardHeader>
          <CardContent>
            <Table className="min-w-[980px] table-fixed">
              <TableHeader>
                <TableRow>
                  <TableHead>{dictionary.rewards.type}</TableHead>
                  <TableHead>{dictionary.rewards.state}</TableHead>
                  <TableHead>{dictionary.rewards.outcome}</TableHead>
                  <TableHead>{dictionary.rewards.time}</TableHead>
                  <TableHead className="w-[36%]">{dictionary.rewards.reason}</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {actions.length === 0 ? (
                  <TableRow>
                    <TableCell colSpan={5} className="py-6 text-center text-sm text-muted-foreground">
                      {dictionary.rewards.none}
                    </TableCell>
                  </TableRow>
                ) : (
                  actions.slice(0, DETAIL_PAGE_SIZE).map((action) => (
                    <TableRow key={action.action_id}>
                      <TableCell className="align-top text-xs">{translateEnum(action.action_type)}</TableCell>
                      <TableCell className="align-top">
                        <StatusPill tone={statusTone(action.status)}>{translateEnum(action.status)}</StatusPill>
                      </TableCell>
                      <TableCell className="align-top font-mono text-xs">
                        {action.managed_order_id ?? action.external_order_id ?? "—"}
                      </TableCell>
                      <TableCell className="align-top font-mono text-xs text-muted-foreground">
                        {formatTime(action.created_at)}
                      </TableCell>
                      <TableCell className="align-top text-xs text-muted-foreground">
                        <TruncateText text={action.reason} lines={2} />
                      </TableCell>
                    </TableRow>
                  ))
                )}
              </TableBody>
            </Table>
          </CardContent>
        </Card>
      </div>
    </div>
  );
}
