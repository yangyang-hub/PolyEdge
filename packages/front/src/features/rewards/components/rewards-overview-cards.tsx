"use client";

import { Ban, CheckCircle2, Loader2, Play, RotateCcw, Save } from "lucide-react";

import { MeterBar } from "@/components/shared/meter-bar";
import { StatusPill } from "@/components/shared/status-pill";
import { TruncateText } from "@/components/shared/truncate-text";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardAction,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import type { RewardBotSnapshotDto } from "@/lib/contracts/dto";
import {
  formatFixed,
  formatOptionalClock,
  formatUsdFixed,
  toFiniteNumber,
} from "@/lib/formatters";
import { dictionary, formatMessage } from "@/lib/i18n/dictionaries";

import { eventCategory } from "../lib/rewards-helpers";
import type { EventCategory } from "../types";

export type RewardEventCounts = Record<
  Extract<EventCategory, "placements" | "cancels" | "fills" | "rewards">,
  number
>;

export function ModeStatusPanel({
  snapshot,
  eventCounts,
}: {
  snapshot: RewardBotSnapshotDto;
  eventCounts: RewardEventCounts;
}) {
  const readyQuotePlans = readyQuotePlanCount(snapshot);
  const readyQuoteRatio = ratio(readyQuotePlans, snapshot.status.plans_total);
  const availableRatio = ratio(
    snapshot.account.available_usd,
    snapshot.config.account_capital_usd,
  );

  return (
    <Card>
      <CardHeader className="border-b border-border/70">
        <CardTitle>{dictionary.rewards.modeSummary}</CardTitle>
        <CardDescription>
          {dictionary.rewards.liveModeSummary}
        </CardDescription>
        <CardAction className="flex flex-wrap justify-end gap-2">
          <StatusPill tone="warning">
            {dictionary.rewards.modeLive}
          </StatusPill>
          <StatusPill tone={snapshot.status.enabled ? "success" : "neutral"}>
            {snapshot.status.enabled ? dictionary.common.enabled : dictionary.common.disabled}
          </StatusPill>
          <StatusPill tone={snapshot.status.running ? "success" : "neutral"}>
            {snapshot.status.running ? dictionary.common.active : dictionary.common.idle}
          </StatusPill>
        </CardAction>
      </CardHeader>
      <CardContent className="grid gap-5 lg:grid-cols-[1fr_1fr]">
        <div className="space-y-4">
          <ProgressLine
            label={dictionary.rewards.liveQuoteReadiness}
            value={formatRatio(readyQuoteRatio)}
            meter={readyQuoteRatio}
            tone={readyQuoteRatio > 0 ? "success" : "neutral"}
          />
          <ProgressLine
            label={dictionary.rewards.availableCapital}
            value={formatRatio(availableRatio)}
            meter={availableRatio}
            tone={availableRatio > 0.25 ? "success" : "warning"}
          />
          <div className="rounded-lg border border-border/70 bg-muted/20 p-3 text-xs leading-5 text-muted-foreground">
            {dictionary.rewards.liveExecutorNotice}
          </div>
          <ProviderUsageBlock usage={snapshot.llm_usage ?? []} />
        </div>

        <dl className="grid grid-cols-1 gap-3 text-sm sm:grid-cols-2">
          <StatusDatum label={dictionary.rewards.account} value={snapshot.account.account_id} />
          {snapshot.account.wallet_address ? (
            <StatusDatum label={dictionary.rewards.walletAddress} value={snapshot.account.wallet_address} />
          ) : null}
          <StatusDatum label={dictionary.rewards.walletBalance} value={formatUsdFixed(snapshot.account.available_usd)} />
          <StatusDatum label={dictionary.rewards.tick} value={String(snapshot.account.tick_index)} />
          <StatusDatum
            label={dictionary.rewards.lastScan}
            value={formatOptionalClock(snapshot.status.last_scan_at)}
          />
          <StatusDatum
            label={dictionary.rewards.lastRun}
            value={formatOptionalClock(snapshot.status.last_run_at)}
          />
          <StatusDatum
            label={dictionary.rewards.eventsTriggered}
            value={`${eventCounts.placements}/${eventCounts.fills}/${eventCounts.cancels}`}
          />
          <StatusDatum
            label={dictionary.rewards.rewardEarned}
            value={formatUsdFixed(snapshot.account.reward_earned_usd)}
          />
          {snapshot.status.error ? (
            <div className="rounded-lg border border-destructive/30 bg-destructive/10 p-3 text-xs text-destructive sm:col-span-2">
              <TruncateText text={snapshot.status.error} lines={3} />
            </div>
          ) : null}
        </dl>
      </CardContent>
    </Card>
  );
}

export function CommandPanel({
  pending,
  isDirty,
  onRun,
  onCancel,
  onReset,
  onSave,
}: {
  pending: boolean;
  isDirty: boolean;
  onRun: () => void;
  onCancel: () => void;
  onReset: () => void;
  onSave: () => void;
}) {
  return (
    <Card>
      <CardHeader className="border-b border-border/70">
        <CardTitle>{dictionary.rewards.commandCenter}</CardTitle>
        <CardDescription>
          {dictionary.rewards.liveActionNote}
        </CardDescription>
      </CardHeader>
      <CardContent className="flex flex-1 flex-col gap-4">
        <div className="grid gap-2 sm:grid-cols-2">
          <Button type="button" size="lg" disabled={pending} aria-busy={pending} onClick={onRun}>
            {pending ? <Loader2 className="size-4 animate-spin" aria-hidden="true" /> : <Play className="size-4" aria-hidden="true" />}
            {dictionary.rewards.run}
          </Button>
          <Button type="button" size="lg" variant="outline" disabled={!isDirty || pending} aria-busy={pending} onClick={onSave}>
            {pending ? <Loader2 className="size-4 animate-spin" aria-hidden="true" /> : <Save className="size-4" aria-hidden="true" />}
            {dictionary.rewards.save}
          </Button>
          <Button type="button" size="lg" variant="destructive" disabled={pending} aria-busy={pending} onClick={onCancel}>
            {pending ? <Loader2 className="size-4 animate-spin" aria-hidden="true" /> : <Ban className="size-4" aria-hidden="true" />}
            {dictionary.rewards.cancelAll}
          </Button>
          <Button type="button" size="lg" variant="outline" disabled={pending} aria-busy={pending} onClick={onReset}>
            {pending ? <Loader2 className="size-4 animate-spin" aria-hidden="true" /> : <RotateCcw className="size-4" aria-hidden="true" />}
            {dictionary.rewards.reset}
          </Button>
        </div>
        <div className="mt-auto flex items-start gap-2 rounded-lg border border-border/70 bg-muted/20 p-3 text-xs leading-5 text-muted-foreground">
          <CheckCircle2 className="mt-0.5 size-4 shrink-0 text-secondary" aria-hidden="true" />
          <span>{dictionary.rewards.commandCenterHint}</span>
        </div>
      </CardContent>
    </Card>
  );
}

export function SummaryStrip({
  snapshot,
  eventCounts,
}: {
  snapshot: RewardBotSnapshotDto;
  eventCounts: RewardEventCounts;
}) {
  const eligiblePlans = eligiblePlanCount(snapshot);
  const readyQuotePlans = readyQuotePlanCount(snapshot);
  const waitingOrderbookMarkets = snapshot.status.waiting_orderbook_markets ?? 0;
  const providerPendingMarkets = snapshot.status.provider_pending_markets ?? 0;
  const aiPendingMarkets = snapshot.status.blocker_counts?.ai_pending ?? 0;
  const infoRiskPendingMarkets = snapshot.status.blocker_counts?.info_risk_pending ?? 0;
  const blockedPlans = blockedPlanCount(snapshot);
  const fundingBlocked = capitalRiskBlockerCount(snapshot);
  const liveValidationBlocked = blockerCount(snapshot, "live_validation");
  const providerBlocked = providerRiskBlockerCount(snapshot);

  return (
    <Card size="sm">
      <CardContent className="grid gap-3 sm:grid-cols-2 lg:grid-cols-4 2xl:grid-cols-10">
        <SummaryMetric
          label={dictionary.rewards.liveReadyPlans}
          value={String(readyQuotePlans)}
          hint={`${eligiblePlans} ${dictionary.rewards.finalEligiblePlans} / ${snapshot.status.plans_total} ${dictionary.rewards.candidatePlans}`}
        />
        <SummaryMetric
          label={dictionary.rewards.finalEligiblePlans}
          value={String(eligiblePlans)}
          hint={`${waitingOrderbookMarkets} ${dictionary.rewards.waitingOrderbook} / ${providerPendingMarkets} ${dictionary.rewards.providerPending}`}
        />
        <SummaryMetric
          label={dictionary.rewards.blocked}
          value={String(blockedPlans)}
          hint={`${providerBlocked} ${dictionary.rewards.providerRiskBlocked} / ${providerPendingMarkets} ${dictionary.rewards.providerPending}`}
        />
        <SummaryMetric
          label={dictionary.rewards.blockerFunding}
          value={String(fundingBlocked)}
          hint={dictionary.rewards.fundingBlockerHint}
        />
        <SummaryMetric
          label={dictionary.rewards.blockerLiveValidation}
          value={String(liveValidationBlocked)}
          hint={dictionary.rewards.liveValidationBlockerHint}
        />
        <SummaryMetric
          label={dictionary.rewards.providerRiskBlocked}
          value={String(providerBlocked)}
          hint={formatMessage(dictionary.rewards.providerPendingGraceHint, {
            aiPending: aiPendingMarkets,
            infoPending: infoRiskPendingMarkets,
            aiGrace: snapshot.config.ai_advisory_provider_pending_grace_sec,
            infoGrace: snapshot.config.info_risk_provider_pending_grace_sec,
          })}
        />
        <SummaryMetric
          label={dictionary.rewards.openOrders}
          value={String(snapshot.status.open_orders)}
          hint={`${snapshot.status.positions} ${dictionary.rewards.positions}`}
        />
        <SummaryMetric
          label={dictionary.rewards.accountCapital}
          value={formatUsdFixed(snapshot.config.account_capital_usd)}
          hint={dictionary.rewards.accountSummary}
        />
        <SummaryMetric
          label={dictionary.rewards.walletBalance}
          value={formatUsdFixed(snapshot.account.available_usd)}
          hint={dictionary.rewards.walletBalanceHint}
        />
        <SummaryMetric
          label={dictionary.rewards.eventFlow}
          value={`${eventCounts.placements}/${eventCounts.fills}/${eventCounts.cancels}`}
          hint={`${dictionary.rewards.eventsPlacements} / ${dictionary.rewards.eventsFills} / ${dictionary.rewards.eventsCancels}`}
        />
      </CardContent>
    </Card>
  );
}

export function countRewardEvents(snapshot: RewardBotSnapshotDto): RewardEventCounts {
  const counts: RewardEventCounts = {
    placements: 0,
    cancels: 0,
    fills: snapshot.fills.length,
    rewards: 0,
  };

  for (const event of snapshot.events) {
    const category = eventCategory(event.event_type);
    if (category && category !== "fills") {
      counts[category] += 1;
    }
  }

  return counts;
}

function SummaryMetric({ label, value, hint }: { label: string; value: string; hint: string }) {
  return (
    <div className="min-w-0 rounded-lg border border-border/70 bg-background/30 p-3">
      <p className="break-words text-[11px] font-semibold uppercase leading-4 text-muted-foreground">{label}</p>
      <p className="mt-2 break-words font-mono text-xl font-semibold leading-tight text-foreground">{value}</p>
      <p className="mt-1 break-words text-xs leading-4 text-muted-foreground">{hint}</p>
    </div>
  );
}

function ProviderUsageBlock({
  usage,
}: {
  usage: NonNullable<RewardBotSnapshotDto["llm_usage"]>;
}) {
  const rows = usage.slice(0, 7);
  const today = usage.find((item) => item.day === utcToday());

  return (
    <div className="rounded-lg border border-border/70 bg-background/30 p-3">
      <div className="flex items-start justify-between gap-3">
        <div>
          <p className="text-xs font-semibold text-foreground">{dictionary.rewards.llmUsage}</p>
          <p className="mt-1 text-xs leading-4 text-muted-foreground">
            {dictionary.rewards.llmUsageToday}: {today?.total_calls ?? 0}
          </p>
        </div>
        <span className="shrink-0 rounded-md border border-border/70 px-2 py-1 font-mono text-xs text-muted-foreground">
          {dictionary.rewards.llmUsageCalls}
        </span>
      </div>
      <p className="mt-2 text-xs leading-4 text-muted-foreground">
        {dictionary.rewards.llmUsageDescription}
      </p>
      {rows.length > 0 ? (
        <div className="mt-3 grid grid-cols-[1.2fr_repeat(4,minmax(0,1fr))] gap-x-2 gap-y-1 text-xs">
          <span className="text-muted-foreground">{dictionary.rewards.llmUsageDay}</span>
          <span className="text-right text-muted-foreground">{dictionary.rewards.llmUsageTotal}</span>
          <span className="text-right text-muted-foreground">{dictionary.rewards.llmUsageAi}</span>
          <span className="text-right text-muted-foreground">{dictionary.rewards.llmUsageInfoRisk}</span>
          <span className="text-right text-muted-foreground">{dictionary.rewards.llmUsageFailed}</span>
          {rows.map((item) => (
            <ProviderUsageRow key={item.day} item={item} />
          ))}
        </div>
      ) : (
        <p className="mt-3 text-xs leading-4 text-muted-foreground">
          {dictionary.rewards.llmUsageEmpty}
        </p>
      )}
    </div>
  );
}

function ProviderUsageRow({
  item,
}: {
  item: NonNullable<RewardBotSnapshotDto["llm_usage"]>[number];
}) {
  return (
    <>
      <span className="font-mono text-muted-foreground">{shortDay(item.day)}</span>
      <span className="text-right font-mono text-foreground">{item.total_calls}</span>
      <span className="text-right font-mono text-foreground">{item.ai_advisory_calls}</span>
      <span className="text-right font-mono text-foreground">{item.info_risk_calls}</span>
      <span className="text-right font-mono text-foreground">{item.failed_calls}</span>
    </>
  );
}

function ProgressLine({
  label,
  value,
  meter,
  tone,
}: {
  label: string;
  value: string;
  meter: number;
  tone: "success" | "warning" | "neutral";
}) {
  return (
    <div className="space-y-2">
      <div className="flex items-center justify-between gap-3 text-xs">
        <span className="font-medium text-muted-foreground">{label}</span>
        <span className="font-mono text-foreground">{value}</span>
      </div>
      <MeterBar value={`${Math.round(clamp01(meter) * 100)}%`} tone={tone} />
    </div>
  );
}

function StatusDatum({ label, value }: { label: string; value: string }) {
  return (
    <div className="min-w-0 rounded-lg border border-border/70 bg-background/30 p-3">
      <dt className="break-words text-[11px] font-semibold uppercase leading-4 text-muted-foreground">{label}</dt>
      <dd className="mt-1 break-all font-mono text-sm leading-snug text-foreground">{value}</dd>
    </div>
  );
}

function ratio(numerator: number | string, denominator: number | string) {
  const nextDenominator = toFiniteNumber(denominator);
  if (nextDenominator <= 0) {
    return 0;
  }
  return toFiniteNumber(numerator) / nextDenominator;
}

function readyQuotePlanCount(snapshot: RewardBotSnapshotDto) {
  return snapshot.status.ready_quote_markets ?? snapshot.status.eligible_markets;
}

function eligiblePlanCount(snapshot: RewardBotSnapshotDto) {
  return snapshot.status.eligible_markets;
}

function blockedPlanCount(snapshot: RewardBotSnapshotDto) {
  return Math.max(0, snapshot.status.plans_total - eligiblePlanCount(snapshot));
}

function blockerCount(
  snapshot: RewardBotSnapshotDto,
  key: "funding" | "live_validation",
) {
  return snapshot.status.blocker_counts?.[key] ?? 0;
}

function capitalRiskBlockerCount(snapshot: RewardBotSnapshotDto) {
  const blockers = snapshot.status.blocker_counts;
  if (!blockers) return 0;
  return (
    (blockers.funding ?? 0) +
    (blockers.maker_budget ?? 0) +
    (blockers.inventory_headroom ?? 0)
  );
}

function providerRiskBlockerCount(snapshot: RewardBotSnapshotDto) {
  const blockers = snapshot.status.blocker_counts;
  if (!blockers) return 0;
  return (
    (blockers.ai_stop_new ?? 0) +
    (blockers.provider_size ?? 0) +
    (blockers.info_risk ?? 0)
  );
}

function clamp01(value: number) {
  return Math.max(0, Math.min(1, value));
}

function formatRatio(value: number) {
  return `${formatFixed(clamp01(value) * 100, 0)}%`;
}

function utcToday() {
  return new Date().toISOString().slice(0, 10);
}

function shortDay(day: string) {
  const parts = day.split("-");
  if (parts.length === 3) {
    return `${parts[1]}-${parts[2]}`;
  }
  return day;
}
