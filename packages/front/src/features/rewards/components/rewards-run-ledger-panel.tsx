"use client";

import { useCallback, useEffect, useMemo, useState } from "react";
import { RefreshCcw } from "lucide-react";

import { PaginationBar } from "@/components/pagination-bar";
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
  listRewardStrategyActions,
  listRewardStrategyDecisions,
  listRewardStrategyRuns,
} from "@/lib/api/rewards";
import { formatFixed, formatUsdFixed } from "@/lib/formatters";
import type { PaginationState } from "@/hooks/use-pagination";
import { dictionary } from "@/lib/i18n/dictionaries";

const RUNS_PAGE_SIZE = 20;
const DETAIL_PAGE_SIZE = 20;

type StatusTone = "neutral" | "primary" | "success" | "warning" | "danger" | "violet";

function statusTone(status: string): StatusTone {
  if (status === "completed" || status === "succeeded") return "success";
  if (status === "failed") return "danger";
  if (status === "running" || status === "executing") return "warning";
  return "neutral";
}

function runPagination(page: RewardListPageDto, onPageChange: (page: number) => void): PaginationState {
  return {
    page: page.page,
    totalPages: page.total_pages,
    start: 0,
    end: 0,
    setPage: onPageChange,
    goPrevious: () => onPageChange(Math.max(1, page.page - 1)),
    goNext: () => onPageChange(Math.min(page.total_pages, page.page + 1)),
    reset: () => onPageChange(1),
    hasPrevious: page.page > 1,
    hasNext: page.page < page.total_pages,
  };
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

export function RewardsRunLedgerPanel() {
  const [runs, setRuns] = useState<RewardStrategyRunDto[]>([]);
  const [runsPage, setRunsPage] = useState<RewardListPageDto | null>(null);
  const [runPage, setRunPage] = useState(1);
  const [selectedRunId, setSelectedRunId] = useState<number | null>(null);
  const [decisions, setDecisions] = useState<RewardStrategyDecisionDto[]>([]);
  const [actions, setActions] = useState<RewardStrategyActionDto[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const selectedRun = useMemo(
    () => runs.find((run) => run.run_id === selectedRunId) ?? null,
    [runs, selectedRunId],
  );

  const loadRuns = useCallback(async (page: number) => {
    setLoading(true);
    setError(null);
    try {
      const response = await listRewardStrategyRuns({ page, page_size: RUNS_PAGE_SIZE });
      setRuns(response.data.items);
      setRunsPage(response.data.page);
      setRunPage(response.data.page.page);
      if (response.data.items.length === 0) {
        setSelectedRunId(null);
        setDecisions([]);
        setActions([]);
      } else {
        setSelectedRunId((current) => current ?? response.data.items[0]?.run_id ?? null);
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : dictionary.rewards.runLedgerLoadFailed);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    const timeout = window.setTimeout(() => {
      void loadRuns(1);
    }, 0);
    return () => window.clearTimeout(timeout);
  }, [loadRuns]);

  useEffect(() => {
    if (selectedRunId == null) {
      return;
    }
    let active = true;
    void Promise.all([
      listRewardStrategyDecisions(selectedRunId, { page: 1, page_size: DETAIL_PAGE_SIZE }),
      listRewardStrategyActions(selectedRunId, { page: 1, page_size: DETAIL_PAGE_SIZE }),
    ])
      .then(([decisionResponse, actionResponse]) => {
        if (!active) return;
        setError(null);
        setDecisions(decisionResponse.data.items);
        setActions(actionResponse.data.items);
      })
      .catch((err) => {
        if (!active) return;
        setError(err instanceof Error ? err.message : dictionary.rewards.runLedgerLoadFailed);
      });
    return () => {
      active = false;
    };
  }, [selectedRunId]);

  return (
    <div className="grid gap-4 xl:grid-cols-[360px_minmax(0,1fr)]">
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
            <RefreshCcw className="size-4" />
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
                  onClick={() => setSelectedRunId(run.run_id)}
                  className="flex w-full flex-col gap-2 rounded-md border border-border/70 px-3 py-2 text-left transition-colors hover:bg-muted/60 data-[active=true]:border-primary data-[active=true]:bg-muted"
                  data-active={run.run_id === selectedRunId}
                >
                  <div className="flex items-center justify-between gap-2">
                    <span className="font-mono text-xs text-muted-foreground">#{run.run_id}</span>
                    <StatusPill tone={statusTone(run.status)}>{run.status}</StatusPill>
                  </div>
                  <div className="flex items-center justify-between gap-2 text-xs">
                    <span>{run.trigger_type}</span>
                    <span className="font-mono text-muted-foreground">{formatTime(run.started_at)}</span>
                  </div>
                </button>
              ))
            )}
          </div>
          {runsPage ? (
            <PaginationBar
              pagination={runPagination(runsPage, (page) => void loadRuns(page))}
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
                  <StatusPill tone={statusTone(selectedRun.status)}>{selectedRun.status}</StatusPill>
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
                  decisions.map((decision) => (
                    <TableRow key={`${decision.run_id}:${decision.condition_id}:${decision.strategy_profile}`}>
                      <TableCell className="align-top font-mono text-xs">{decision.condition_id}</TableCell>
                      <TableCell className="align-top">
                        <StatusPill tone={decision.eligible ? "success" : "neutral"}>
                          {decision.quote_readiness}
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
                  actions.map((action) => (
                    <TableRow key={action.action_id}>
                      <TableCell className="align-top font-mono text-xs">{action.action_type}</TableCell>
                      <TableCell className="align-top">
                        <StatusPill tone={statusTone(action.status)}>{action.status}</StatusPill>
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
