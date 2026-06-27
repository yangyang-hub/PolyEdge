"use client";

import type {
  HighProbabilityBacktestExitRuleReportDto,
  HighProbabilityBacktestReportDto,
  HighProbabilityBacktestRunDto,
  HighProbabilityBacktestTradeDto,
  HighProbabilityResearchReportDto,
  HighProbabilitySnapshotDto,
} from "@/lib/contracts/dto";
import { MetricCard } from "@/components/shared/metric-card";
import { PageHeader } from "@/components/shared/page-header";
import { StatusPill } from "@/components/shared/status-pill";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { HighProbabilityBacktestHistory } from "@/features/high-probability/components/high-probability-backtest-history";
import {
  bucketComputedAt,
  bucketDimensionLabel,
  bucketSampleHint,
  bucketTone,
  exitRuleLabel,
  formatCents,
  formatOptionalFixed,
  formatOptionalProbability,
  formatProbability,
  formatSeconds,
  reportNoteLabel,
} from "@/features/high-probability/lib/high-probability-formatters";
import {
  formatInteger,
  formatOptionalClock,
  formatUsdFixed,
  toFiniteNumber,
  type Tone,
} from "@/lib/formatters";
import { dictionary, translateEnum } from "@/lib/i18n/dictionaries";

export function HighProbabilityWorkbench({
  initialSnapshot,
  initialReport,
  initialBacktest,
  initialBacktestRuns,
  initialBacktestTrades,
}: {
  initialSnapshot: HighProbabilitySnapshotDto;
  initialReport: HighProbabilityResearchReportDto;
  initialBacktest: HighProbabilityBacktestReportDto;
  initialBacktestRuns: HighProbabilityBacktestRunDto[];
  initialBacktestTrades: HighProbabilityBacktestTradeDto[];
}) {
  const t = dictionary.highProbability;
  const { config, bucket_stats: bucketStats, observations } = initialSnapshot;
  const report = initialReport;
  const backtest = initialBacktest;
  const totalSamples = bucketStats.reduce((sum, bucket) => sum + bucket.sample_count, 0);
  const sortedBuckets = [...bucketStats].sort(
    (left, right) => toFiniteNumber(right.expected_pnl) - toFiniteNumber(left.expected_pnl),
  );

  return (
    <div className="space-y-6">
      <PageHeader
        eyebrow={t.eyebrow}
        title={t.title}
        description={t.description}
        actions={
          <>
            <StatusPill tone={config.enabled ? "success" : "neutral"}>
              {config.enabled ? dictionary.common.enabled : dictionary.common.disabled}
            </StatusPill>
            <StatusPill tone="primary">{modeLabel(config.mode)}</StatusPill>
            <StatusPill tone="violet">{config.model_version}</StatusPill>
          </>
        }
      />

      <section className="grid gap-4 md:grid-cols-2 xl:grid-cols-4">
        <MetricCard
          title={t.bucketCount}
          value={formatInteger(bucketStats.length)}
          hint={config.market_scope}
          accent="primary"
        />
        <MetricCard
          title={t.sampleCount}
          value={formatInteger(totalSamples)}
          hint={`${t.minSamples}: ${formatInteger(config.min_bucket_samples)}`}
          accent="success"
        />
        <MetricCard
          title={t.requiredEdge}
          value={formatProbability(config.min_required_edge)}
          hint={`${t.riskMargin}: ${formatProbability(config.default_risk_margin)}`}
          accent="violet"
        />
        <MetricCard
          title={t.observationCount}
          value={formatInteger(observations.length)}
          hint={`${t.maxTrade}: ${formatUsdFixed(config.max_single_trade_usd, 0)}`}
          accent={observations.length > 0 ? "success" : "primary"}
        />
      </section>

      <section className="grid gap-4 md:grid-cols-2 xl:grid-cols-4">
        <MetricCard
          title={t.reportSettledSamples}
          value={formatInteger(report.settled_samples)}
          hint={`${t.reportWinLoss}: ${formatInteger(report.win_samples)} / ${formatInteger(report.loss_samples)}`}
          accent="success"
        />
        <MetricCard
          title={t.reportQualifiedBuckets}
          value={formatInteger(report.qualified_bucket_count)}
          hint={`${t.reportPositiveBuckets}: ${formatInteger(report.positive_expected_pnl_bucket_count)}`}
          accent={report.qualified_bucket_count > 0 ? "success" : "primary"}
        />
        <MetricCard
          title={t.reportWeightedWinRate}
          value={formatOptionalProbability(report.weighted_win_rate)}
          hint={`${t.reportBreak70}: ${formatOptionalProbability(report.weighted_break_70_rate)}`}
          accent="violet"
        />
        <MetricCard
          title={t.reportWeightedExpectedPnl}
          value={formatOptionalFixed(report.weighted_expected_pnl)}
          hint={`${t.reportLimit}: ${formatInteger(report.sample_limit)}`}
          accent="primary"
        />
      </section>

      <Card>
        <CardHeader>
          <CardTitle>{t.report}</CardTitle>
          <CardDescription>{t.reportDescription}</CardDescription>
        </CardHeader>
        <CardContent className="grid gap-4 md:grid-cols-3">
          <ReportBucketSummary label={t.bestBucket} bucket={report.best_bucket} />
          <ReportBucketSummary label={t.worstBucket} bucket={report.worst_bucket} />
          <div className="space-y-2">
            <p className="text-xs font-semibold text-muted-foreground">{t.reportNotes}</p>
            <div className="flex flex-wrap gap-2">
              {report.notes.length === 0 ? (
                <StatusPill tone="success">{t.reportNoNotes}</StatusPill>
              ) : (
                report.notes.map((note) => (
                  <StatusPill key={note} tone="warning">
                    {reportNoteLabel(note)}
                  </StatusPill>
                ))
              )}
            </div>
          </div>
        </CardContent>
      </Card>

      <section className="grid gap-4 md:grid-cols-2 xl:grid-cols-4">
        <MetricCard
          title={t.backtestTrades}
          value={formatInteger(backtest.trade_count)}
          hint={`${t.backtestCandidates}: ${formatInteger(backtest.candidate_count)}`}
          accent={backtest.trade_count > 0 ? "success" : "primary"}
        />
        <MetricCard
          title={t.backtestWinRate}
          value={formatOptionalProbability(backtest.win_rate)}
          hint={`${t.reportWinLoss}: ${formatInteger(backtest.win_trades)} / ${formatInteger(backtest.loss_trades)}`}
          accent="violet"
        />
        <MetricCard
          title={t.backtestTotalPnl}
          value={formatOptionalFixed(backtest.total_pnl)}
          hint={`${t.backtestAveragePnl}: ${formatOptionalFixed(backtest.average_pnl)}`}
          accent={toFiniteNumber(backtest.total_pnl) >= 0 ? "success" : "danger"}
        />
        <MetricCard
          title={t.backtestRoi}
          value={formatOptionalProbability(backtest.roi)}
          hint={`${t.backtestDrawdown}: ${formatOptionalFixed(backtest.max_drawdown)}`}
          accent="primary"
        />
      </section>

      <Card>
        <CardHeader>
          <CardTitle>{t.backtest}</CardTitle>
          <CardDescription>{t.backtestDescription}</CardDescription>
        </CardHeader>
        <CardContent className="grid gap-4 md:grid-cols-3">
          <div className="space-y-3">
            <ConfigRow label={t.backtestTrainSamples} value={formatInteger(backtest.train_sample_count)} />
            <ConfigRow label={t.backtestTestSamples} value={formatInteger(backtest.test_sample_count)} />
            <ConfigRow
              label={t.backtestAverageEntry}
              value={formatOptionalProbability(backtest.average_entry_price)}
            />
          </div>
          <div className="space-y-3">
            <ConfigRow label={t.backtestSkippedNoBucket} value={formatInteger(backtest.skipped_no_bucket_count)} />
            <ConfigRow label={t.backtestSkippedNoEdge} value={formatInteger(backtest.skipped_no_edge_count)} />
            <ConfigRow
              label={t.backtestWindow}
              value={`${formatOptionalClock(backtest.train_start_at)} / ${formatOptionalClock(backtest.test_end_at)}`}
            />
          </div>
          <div className="space-y-2">
            <p className="text-xs font-semibold text-muted-foreground">{t.reportNotes}</p>
            <div className="flex flex-wrap gap-2">
              {backtest.notes.length === 0 ? (
                <StatusPill tone="success">{t.reportNoNotes}</StatusPill>
              ) : (
                backtest.notes.map((note) => (
                  <StatusPill key={note} tone="warning">
                    {reportNoteLabel(note)}
                  </StatusPill>
                ))
              )}
            </div>
          </div>
        </CardContent>
      </Card>

      <BacktestExitRuleTable rules={backtest.exit_rule_reports} />

      <HighProbabilityBacktestHistory runs={initialBacktestRuns} trades={initialBacktestTrades} />

      <section className="grid gap-4 xl:grid-cols-[1fr_20rem]">
        <Card>
          <CardHeader>
            <CardTitle>{t.bucketStats}</CardTitle>
            <CardDescription>{t.bucketStatsDescription}</CardDescription>
          </CardHeader>
          <CardContent>
            <Table className="min-w-[1040px] table-fixed">
              <TableHeader>
                <TableRow>
                  <TableHead className="w-[270px]">{t.bucket}</TableHead>
                  <TableHead>{t.samples}</TableHead>
                  <TableHead>{t.winRate}</TableHead>
                  <TableHead>{t.fairProbability}</TableHead>
                  <TableHead>{t.expectedPnl}</TableHead>
                  <TableHead>{t.drawdown}</TableHead>
                  <TableHead>{t.break70}</TableHead>
                  <TableHead>{t.maxEntry}</TableHead>
                  <TableHead>{t.holdTime}</TableHead>
                  <TableHead>{t.computed}</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {sortedBuckets.length === 0 ? (
                  <TableRow>
                    <TableCell colSpan={10} className="py-8 text-center text-sm text-muted-foreground">
                      {t.noBuckets}
                    </TableCell>
                  </TableRow>
                ) : (
                  sortedBuckets.map((bucket) => (
                    <TableRow key={bucket.bucket_key}>
                      <TableCell className="whitespace-normal align-top">
                        <div className="space-y-1">
                          <StatusPill tone={bucketTone(bucket)}>{bucket.bucket_key}</StatusPill>
                          <p className="text-xs leading-5 text-muted-foreground">
                            {bucketDimensionLabel(bucket)}
                          </p>
                        </div>
                      </TableCell>
                      <TableCell className="align-top font-mono">
                        {bucketSampleHint(bucket)}
                      </TableCell>
                      <TableCell className="align-top font-mono">{formatProbability(bucket.win_rate)}</TableCell>
                      <TableCell className="align-top font-mono">
                        {formatProbability(bucket.fair_probability)}
                      </TableCell>
                      <TableCell className="align-top font-mono">
                        {formatOptionalFixed(bucket.expected_pnl)}
                      </TableCell>
                      <TableCell className="align-top font-mono">
                        {formatCents(bucket.avg_max_drawdown_cents)}
                      </TableCell>
                      <TableCell className="align-top font-mono">
                        {formatOptionalProbability(bucket.break_70_rate)}
                      </TableCell>
                      <TableCell className="align-top font-mono">
                        {formatOptionalProbability(bucket.recommended_max_entry_price)}
                      </TableCell>
                      <TableCell className="align-top font-mono">{formatSeconds(bucket.avg_hold_seconds)}</TableCell>
                      <TableCell className="align-top font-mono text-xs text-muted-foreground">
                        {bucketComputedAt(bucket)}
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
            <CardTitle>{t.config}</CardTitle>
            <CardDescription>{t.configDescription}</CardDescription>
          </CardHeader>
          <CardContent className="space-y-3">
            <ConfigRow label={t.minConfidence} value={formatProbability(config.min_confidence)} />
            <ConfigRow label={t.feeBuffer} value={formatProbability(config.fee_buffer)} />
            <ConfigRow label={t.maxSpread} value={formatCents(config.max_spread_cents)} />
            <ConfigRow label={t.minDepth} value={formatUsdFixed(config.min_depth_usd, 0)} />
            <ConfigRow
              label={t.maxMarketExposure}
              value={formatUsdFixed(config.max_single_market_exposure_usd, 0)}
            />
            <ConfigRow
              label={t.dailyNotional}
              value={formatUsdFixed(config.max_daily_new_notional_usd, 0)}
            />
            <ConfigRow
              label={t.kellyMultiplier}
              value={formatProbability(config.conservative_kelly_multiplier)}
            />
            <div className="space-y-2 border-t border-border/70 pt-3">
              <p className="text-xs font-semibold text-muted-foreground">{t.excludedTags}</p>
              <div className="flex flex-wrap gap-2">
                {config.excluded_risk_tags.length === 0 ? (
                  <StatusPill>{dictionary.common.none}</StatusPill>
                ) : (
                  config.excluded_risk_tags.map((tag) => (
                    <StatusPill key={tag} tone="warning">
                      {tag}
                    </StatusPill>
                  ))
                )}
              </div>
            </div>
          </CardContent>
        </Card>
      </section>

      <Card>
        <CardHeader>
          <CardTitle>{t.observations}</CardTitle>
          <CardDescription>{t.observationsDescription}</CardDescription>
        </CardHeader>
        <CardContent>
          <Table className="min-w-[920px] table-fixed">
            <TableHeader>
              <TableRow>
                <TableHead>{t.decision}</TableHead>
                <TableHead className="w-[220px]">{t.condition}</TableHead>
                <TableHead>{t.price}</TableHead>
                <TableHead>{t.fairProbability}</TableHead>
                <TableHead>{t.netEdge}</TableHead>
                <TableHead>{t.size}</TableHead>
                <TableHead className="w-[260px]">{t.reason}</TableHead>
                <TableHead>{t.observed}</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {observations.length === 0 ? (
                <TableRow>
                  <TableCell colSpan={8} className="py-8 text-center text-sm text-muted-foreground">
                    {t.noObservations}
                  </TableCell>
                </TableRow>
              ) : (
                observations.map((observation) => (
                  <TableRow key={observation.id}>
                    <TableCell className="align-top">
                      <StatusPill tone={decisionTone(observation.decision)}>
                        {translateEnum(observation.decision)}
                      </StatusPill>
                    </TableCell>
                    <TableCell className="whitespace-normal align-top font-mono text-xs">
                      {observation.condition_id}
                    </TableCell>
                    <TableCell className="align-top font-mono">
                      {formatProbability(observation.executable_price)}
                    </TableCell>
                    <TableCell className="align-top font-mono">
                      {formatOptionalProbability(observation.fair_probability)}
                    </TableCell>
                    <TableCell className="align-top font-mono">
                      {formatOptionalProbability(observation.net_edge)}
                    </TableCell>
                    <TableCell className="align-top font-mono">
                      {observation.recommended_size_usd == null
                        ? dictionary.common.none
                        : formatUsdFixed(observation.recommended_size_usd)}
                    </TableCell>
                    <TableCell className="whitespace-normal align-top text-xs text-muted-foreground">
                      {observation.reasons.length > 0 ? observation.reasons.join(" / ") : dictionary.common.none}
                    </TableCell>
                    <TableCell className="align-top font-mono text-xs text-muted-foreground">
                      {formatOptionalClock(observation.observed_at)}
                    </TableCell>
                  </TableRow>
                ))
              )}
            </TableBody>
          </Table>
        </CardContent>
      </Card>
    </div>
  );
}

function BacktestExitRuleTable({ rules }: { rules: HighProbabilityBacktestExitRuleReportDto[] }) {
  const t = dictionary.highProbability;

  return (
    <Card>
      <CardHeader>
        <CardTitle>{t.backtestExitRules}</CardTitle>
        <CardDescription>{t.backtestExitRulesDescription}</CardDescription>
      </CardHeader>
      <CardContent>
        <Table className="min-w-[920px] table-fixed">
          <TableHeader>
            <TableRow>
              <TableHead className="w-[180px]">{t.exitRule}</TableHead>
              <TableHead>{t.backtestTrades}</TableHead>
              <TableHead>{t.backtestWinRate}</TableHead>
              <TableHead>{t.backtestTotalPnl}</TableHead>
              <TableHead>{t.backtestAveragePnl}</TableHead>
              <TableHead>{t.backtestRoi}</TableHead>
              <TableHead>{t.backtestDrawdown}</TableHead>
              <TableHead className="w-[260px]">{t.exitRuleNotes}</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {rules.length === 0 ? (
              <TableRow>
                <TableCell colSpan={8} className="py-8 text-center text-sm text-muted-foreground">
                  {t.noExitRules}
                </TableCell>
              </TableRow>
            ) : (
              rules.map((rule) => (
                <TableRow key={rule.rule_key}>
                  <TableCell className="align-top">
                    <StatusPill>{exitRuleLabel(rule.rule_key)}</StatusPill>
                  </TableCell>
                  <TableCell className="align-top font-mono">{formatInteger(rule.trade_count)}</TableCell>
                  <TableCell className="align-top font-mono">
                    {formatOptionalProbability(rule.win_rate)}
                  </TableCell>
                  <TableCell className="align-top font-mono">
                    <span className={toFiniteNumber(rule.total_pnl) >= 0 ? "text-foreground" : "text-destructive"}>
                      {formatOptionalFixed(rule.total_pnl)}
                    </span>
                  </TableCell>
                  <TableCell className="align-top font-mono">
                    {formatOptionalFixed(rule.average_pnl)}
                  </TableCell>
                  <TableCell className="align-top font-mono">{formatOptionalProbability(rule.roi)}</TableCell>
                  <TableCell className="align-top font-mono">
                    {formatOptionalFixed(rule.max_drawdown)}
                  </TableCell>
                  <TableCell className="whitespace-normal align-top text-xs text-muted-foreground">
                    {rule.notes.length > 0
                      ? rule.notes.map(reportNoteLabel).join(" / ")
                      : dictionary.common.none}
                  </TableCell>
                </TableRow>
              ))
            )}
          </TableBody>
        </Table>
      </CardContent>
    </Card>
  );
}

function ReportBucketSummary({
  label,
  bucket,
}: {
  label: string;
  bucket: HighProbabilityResearchReportDto["best_bucket"];
}) {
  return (
    <div className="space-y-2">
      <p className="text-xs font-semibold text-muted-foreground">{label}</p>
      {bucket == null ? (
        <StatusPill>{dictionary.common.none}</StatusPill>
      ) : (
        <div className="space-y-1">
          <StatusPill tone={bucketTone(bucket)}>{bucket.bucket_key}</StatusPill>
          <p className="text-xs leading-5 text-muted-foreground">{bucketDimensionLabel(bucket)}</p>
          <p className="font-mono text-xs text-muted-foreground">
            {formatOptionalFixed(bucket.expected_pnl)} / {formatProbability(bucket.win_rate)}
          </p>
        </div>
      )}
    </div>
  );
}

function ConfigRow({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex items-center justify-between gap-4 text-sm">
      <span className="text-muted-foreground">{label}</span>
      <span className="font-mono text-foreground">{value}</span>
    </div>
  );
}

function modeLabel(mode: HighProbabilitySnapshotDto["config"]["mode"]): string {
  return translateEnum(mode);
}

function decisionTone(decision: HighProbabilitySnapshotDto["observations"][number]["decision"]): Tone {
  if (decision === "allow") return "success";
  if (decision === "reject") return "danger";
  return "neutral";
}
