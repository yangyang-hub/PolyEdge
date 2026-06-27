"use client";

import { useState, useTransition } from "react";
import type {
  HighProbabilityBacktestRunDto,
  HighProbabilityBacktestTradeDto,
} from "@/lib/contracts/dto";
import { StatusPill } from "@/components/shared/status-pill";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import {
  formatOptionalFixed,
  formatOptionalProbability,
  formatProbability,
} from "@/features/high-probability/lib/high-probability-formatters";
import { readHighProbabilityBacktestTrades } from "@/lib/api/high-probability";
import { formatInteger, formatOptionalClock, toFiniteNumber } from "@/lib/formatters";
import { dictionary } from "@/lib/i18n/dictionaries";

export function HighProbabilityBacktestHistory({
  runs,
  trades,
}: {
  runs: HighProbabilityBacktestRunDto[];
  trades: HighProbabilityBacktestTradeDto[];
}) {
  const t = dictionary.highProbability;
  const [selectedRunId, setSelectedRunId] = useState(runs.at(0)?.id ?? null);
  const [selectedTrades, setSelectedTrades] = useState(trades);
  const [error, setError] = useState<string | null>(null);
  const [isPending, startTransition] = useTransition();
  const selectedRun = runs.find((run) => run.id === selectedRunId) ?? runs.at(0);

  function selectRun(run: HighProbabilityBacktestRunDto) {
    if (run.id === selectedRunId) {
      return;
    }
    setSelectedRunId(run.id);
    setError(null);
    startTransition(async () => {
      try {
        const response = await readHighProbabilityBacktestTrades(run.id);
        setSelectedTrades(response.data);
      } catch (caught) {
        setSelectedTrades([]);
        setError(caught instanceof Error ? caught.message : t.tradeLoadFailed);
      }
    });
  }

  return (
    <section className="grid gap-4 xl:grid-cols-[minmax(0,0.95fr)_minmax(0,1.05fr)]">
      <Card>
        <CardHeader>
          <CardTitle>{t.backtestRuns}</CardTitle>
          <CardDescription>{t.backtestRunsDescription}</CardDescription>
        </CardHeader>
        <CardContent>
          <Table className="min-w-[760px] table-fixed">
            <TableHeader>
              <TableRow>
                <TableHead>{t.run}</TableHead>
                <TableHead>{t.backtestTrades}</TableHead>
                <TableHead>{t.backtestWinRate}</TableHead>
                <TableHead>{t.backtestTotalPnl}</TableHead>
                <TableHead>{t.backtestRoi}</TableHead>
                <TableHead>{t.backtestDrawdown}</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {runs.length === 0 ? (
                <TableRow>
                  <TableCell colSpan={6} className="py-8 text-center text-sm text-muted-foreground">
                    {t.noBacktestRuns}
                  </TableCell>
                </TableRow>
              ) : (
                runs.map((run) => (
                  <TableRow key={run.id}>
                    <TableCell className="align-top">
                      <div className="space-y-1">
                        <Button
                          type="button"
                          variant={run.id === selectedRun?.id ? "default" : "outline"}
                          size="xs"
                          onClick={() => selectRun(run)}
                          disabled={isPending}
                        >
                          {run.id === selectedRun?.id ? t.selectedRun : `#${run.id}`}
                        </Button>
                        <p className="font-mono text-xs text-muted-foreground">
                          {formatOptionalClock(run.run_at)}
                        </p>
                      </div>
                    </TableCell>
                    <TableCell className="align-top font-mono">{formatInteger(run.report.trade_count)}</TableCell>
                    <TableCell className="align-top font-mono">
                      {formatOptionalProbability(run.report.win_rate)}
                    </TableCell>
                    <TableCell className="align-top font-mono">
                      {formatOptionalFixed(run.report.total_pnl)}
                    </TableCell>
                    <TableCell className="align-top font-mono">{formatOptionalProbability(run.report.roi)}</TableCell>
                    <TableCell className="align-top font-mono">
                      {formatOptionalFixed(run.report.max_drawdown)}
                    </TableCell>
                  </TableRow>
                ))
              )}
            </TableBody>
          </Table>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>{t.backtestTradesTable}</CardTitle>
          <CardDescription>
            {selectedRun == null
              ? t.backtestTradesDescription
              : `${t.selectedRun} ${formatOptionalClock(selectedRun.run_at)} · ${t.backtestTradesDescription}`}
          </CardDescription>
        </CardHeader>
        <CardContent>
          {error ? (
            <div className="mb-3 rounded-md border border-destructive/30 bg-destructive/5 px-3 py-2 text-xs text-destructive">
              {error}
            </div>
          ) : null}
          <Table className="min-w-[980px] table-fixed">
            <TableHeader>
              <TableRow>
                <TableHead>{t.outcome}</TableHead>
                <TableHead className="w-[180px]">{t.condition}</TableHead>
                <TableHead className="w-[190px]">{t.bucket}</TableHead>
                <TableHead>{t.price}</TableHead>
                <TableHead>{t.fairProbability}</TableHead>
                <TableHead>{t.netEdge}</TableHead>
                <TableHead>{t.tradePnl}</TableHead>
                <TableHead>{t.cumulativePnl}</TableHead>
                <TableHead>{t.drawdown}</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {selectedTrades.length === 0 ? (
                <TableRow>
                  <TableCell colSpan={9} className="py-8 text-center text-sm text-muted-foreground">
                    {selectedRun == null ? t.noBacktestRuns : t.noBacktestTrades}
                  </TableCell>
                </TableRow>
              ) : (
                selectedTrades.map((trade) => (
                  <TableRow key={trade.id}>
                    <TableCell className="align-top">
                      <StatusPill tone={trade.outcome === "win" ? "success" : "danger"}>
                        {trade.outcome === "win" ? t.win : t.loss}
                      </StatusPill>
                    </TableCell>
                    <TableCell className="whitespace-normal align-top font-mono text-xs">
                      {trade.condition_id}
                    </TableCell>
                    <TableCell className="whitespace-normal align-top">
                      <div className="space-y-1">
                        <StatusPill>{trade.bucket_key}</StatusPill>
                        <p className="font-mono text-xs text-muted-foreground">
                          {formatOptionalClock(trade.sampled_at)}
                        </p>
                      </div>
                    </TableCell>
                    <TableCell className="align-top font-mono">
                      {formatProbability(trade.executable_price)}
                    </TableCell>
                    <TableCell className="align-top font-mono">
                      {formatProbability(trade.fair_probability)}
                    </TableCell>
                    <TableCell className="align-top font-mono">
                      {formatOptionalProbability(trade.net_edge)}
                    </TableCell>
                    <TableCell className="align-top font-mono">
                      <span className={toFiniteNumber(trade.settlement_pnl) >= 0 ? "text-foreground" : "text-destructive"}>
                        {formatOptionalFixed(trade.settlement_pnl)}
                      </span>
                    </TableCell>
                    <TableCell className="align-top font-mono">
                      {formatOptionalFixed(trade.cumulative_pnl)}
                    </TableCell>
                    <TableCell className="align-top font-mono">{formatOptionalFixed(trade.drawdown)}</TableCell>
                  </TableRow>
                ))
              )}
            </TableBody>
          </Table>
        </CardContent>
      </Card>
    </section>
  );
}
