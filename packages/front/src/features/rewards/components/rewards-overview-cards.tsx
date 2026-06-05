"use client";

import { Ban, CheckCircle2, Play, RotateCcw, Save } from "lucide-react";

import { MeterBar } from "@/components/shared/meter-bar";
import { StatusPill } from "@/components/shared/status-pill";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardAction,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import type { RewardBotConfigDto, RewardBotSnapshotDto } from "@/lib/contracts/dto";
import {
  formatFixed,
  formatOptionalClock,
  formatUsdFixed,
  toFiniteNumber,
} from "@/lib/formatters";
import { dictionary } from "@/lib/i18n/dictionaries";

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
  const eligibleRatio = ratio(snapshot.status.eligible_markets, snapshot.status.plans_total);
  const availableRatio = ratio(snapshot.account.available_usd, snapshot.account.capital_usd);

  return (
    <Card>
      <CardHeader className="border-b border-border/70">
        <CardTitle>{dictionary.rewards.modeSummary}</CardTitle>
        <CardDescription>
          {dictionary.rewards.liveModeSummary}
        </CardDescription>
        <CardAction className="flex gap-2">
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
            label={dictionary.rewards.marketReadiness}
            value={formatRatio(eligibleRatio)}
            meter={eligibleRatio}
            tone={eligibleRatio > 0 ? "success" : "neutral"}
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
        </div>

        <dl className="grid grid-cols-2 gap-3 text-sm">
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
            <div className="col-span-2 rounded-lg border border-destructive/30 bg-destructive/10 p-3 text-xs text-destructive">
              {snapshot.status.error}
            </div>
          ) : null}
        </dl>
      </CardContent>
    </Card>
  );
}

export function CommandPanel({
  config,
  pending,
  onRun,
  onCancel,
  onReset,
  onSave,
}: {
  config: RewardBotConfigDto;
  pending: boolean;
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
      <CardContent className="space-y-4">
        <div className="grid gap-2 sm:grid-cols-2">
          <Button type="button" size="lg" disabled={pending} onClick={onRun}>
            <Play className="size-4" />
            {dictionary.rewards.run}
          </Button>
          <Button type="button" size="lg" variant="outline" disabled={pending} onClick={onSave}>
            <Save className="size-4" />
            {dictionary.rewards.save}
          </Button>
          <Button type="button" size="lg" variant="destructive" disabled={pending} onClick={onCancel}>
            <Ban className="size-4" />
            {dictionary.rewards.cancelAll}
          </Button>
          <Button type="button" size="lg" variant="outline" disabled={pending} onClick={onReset}>
            <RotateCcw className="size-4" />
            {dictionary.rewards.reset}
          </Button>
        </div>
        <div className="flex items-start gap-2 rounded-lg border border-border/70 bg-muted/20 p-3 text-xs leading-5 text-muted-foreground">
          <CheckCircle2 className="mt-0.5 size-4 shrink-0 text-secondary" />
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

  return (
    <Card size="sm">
      <CardContent className="grid gap-3 sm:grid-cols-2 lg:grid-cols-4 2xl:grid-cols-8">
        <SummaryMetric
          label={dictionary.rewards.quotableMarkets}
          value={String(snapshot.status.eligible_markets)}
          hint={`${snapshot.status.plans_total} ${dictionary.rewards.quotePlans} / ${snapshot.status.markets_tracked} ${dictionary.rewards.totalMarkets}`}
        />
        <SummaryMetric
          label={dictionary.rewards.openOrders}
          value={String(snapshot.status.open_orders)}
          hint={`${snapshot.status.positions} ${dictionary.rewards.positions}`}
        />
        <SummaryMetric
          label={dictionary.rewards.accountCapital}
          value={formatUsdFixed(snapshot.account.capital_usd)}
          hint={dictionary.rewards.accountSummary}
        />
        <SummaryMetric
          label={dictionary.rewards.walletBalance}
          value={formatUsdFixed(snapshot.account.available_usd)}
          hint={dictionary.rewards.walletBalanceHint}
        />
        <SummaryMetric
          label={dictionary.rewards.realizedPnl}
          value={formatUsdFixed(snapshot.account.realized_pnl)}
          hint={formatOptionalClock(snapshot.account.updated_at)}
        />
        <SummaryMetric
          label={dictionary.rewards.eventsPlacements}
          value={String(eventCounts.placements)}
          hint={dictionary.rewards.eventsTriggered}
        />
        <SummaryMetric
          label={dictionary.rewards.eventsFills}
          value={String(eventCounts.fills)}
          hint={dictionary.rewards.eventsTriggered}
        />
        <SummaryMetric
          label={dictionary.rewards.eventsCancels}
          value={String(eventCounts.cancels)}
          hint={dictionary.rewards.eventsTriggered}
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
      <p className="truncate text-[11px] font-semibold uppercase text-muted-foreground">{label}</p>
      <p className="mt-2 truncate font-mono text-xl font-semibold text-foreground">{value}</p>
      <p className="mt-1 truncate text-xs text-muted-foreground">{hint}</p>
    </div>
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
    <div className="rounded-lg border border-border/70 bg-background/30 p-3">
      <dt className="truncate text-[11px] font-semibold uppercase text-muted-foreground">{label}</dt>
      <dd className="mt-1 truncate font-mono text-sm text-foreground">{value}</dd>
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

function clamp01(value: number) {
  return Math.max(0, Math.min(1, value));
}

function formatRatio(value: number) {
  return `${formatFixed(clamp01(value) * 100, 0)}%`;
}
