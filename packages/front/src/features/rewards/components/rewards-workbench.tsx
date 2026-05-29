"use client";

import { startTransition, useMemo, useState } from "react";
import { Ban, Play, RotateCcw, Save } from "lucide-react";

import { MetricCard } from "@/components/shared/metric-card";
import { OperationFeedbackBanner } from "@/components/shared/operation-feedback-banner";
import { PageHeader } from "@/components/shared/page-header";
import { StatusPill } from "@/components/shared/status-pill";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import type {
  DecimalValue,
  ManagedRewardOrderDto,
  PostFillStrategy,
  RewardBotConfigDto,
  RewardBotSnapshotDto,
  RewardFillDto,
  RewardQuotePlanDto,
  RewardRiskEventDto,
} from "@/lib/contracts/dto";
import {
  approvalSeverityTone,
  formatFixed,
  formatOptionalClock,
  formatSignedFixed,
  formatUsdFixed,
  metricToneForPnl,
  toFiniteNumber,
} from "@/lib/formatters";
import { useI18n } from "@/lib/i18n/client";
import {
  cancelRewardBotOrdersAction,
  resetRewardBotAction,
  runRewardBotOnceAction,
  updateRewardBotConfigAction,
  type RewardBotActionResult,
} from "@/lib/api/actions";

type NumberConfigKey =
  | "max_markets"
  | "max_open_orders"
  | "per_market_usd"
  | "quote_size_usd"
  | "min_daily_reward"
  | "min_market_score"
  | "max_spread_cents"
  | "quote_edge_cents"
  | "safety_margin_cents"
  | "min_midpoint"
  | "max_midpoint"
  | "stale_book_ms"
  | "min_scoring_check_sec"
  | "max_position_usd"
  | "max_global_position_usd"
  | "exit_markup_cents"
  | "account_capital_usd"
  | "reward_competition_factor"
  | "single_sided_divisor_c"
  | "fill_rate_per_tick"
  | "max_fill_ratio"
  | "requote_drift_cents";

type EventCategory = "all" | "placements" | "cancels" | "fills" | "rewards";

function eventCategory(eventType: string): Exclude<EventCategory, "all"> | null {
  if (eventType === "reward_order_placed" || eventType === "reward_exit_placed") {
    return "placements";
  }
  if (eventType === "reward_order_cancelled") {
    return "cancels";
  }
  if (
    eventType === "reward_order_filled" ||
    eventType === "reward_exit_filled" ||
    eventType === "reward_position_flattened"
  ) {
    return "fills";
  }
  if (eventType === "reward_accrued") {
    return "rewards";
  }
  return null;
}

function rewardTone(status: ManagedRewardOrderDto["status"]) {
  if (status === "open" || status === "exit_pending") {
    return "success" as const;
  }
  if (status === "error") {
    return "danger" as const;
  }
  if (status === "cancelled") {
    return "neutral" as const;
  }
  return "warning" as const;
}

export function RewardsWorkbench({ initialSnapshot }: { initialSnapshot: RewardBotSnapshotDto }) {
  const { dictionary } = useI18n();
  const [snapshot, setSnapshot] = useState(initialSnapshot);
  const [draft, setDraft] = useState<RewardBotConfigDto>(initialSnapshot.config);
  const [feedback, setFeedback] = useState<RewardBotActionResult | null>(null);
  const [pending, setPending] = useState(false);

  const visiblePlans = useMemo(
    () => snapshot.quote_plans.slice(0, 30),
    [snapshot.quote_plans],
  );
  const visibleOrders = useMemo(() => snapshot.orders.slice(0, 50), [snapshot.orders]);
  const visibleEvents = useMemo(() => snapshot.events.slice(0, 60), [snapshot.events]);
  const visibleFills = useMemo(() => snapshot.fills.slice(0, 50), [snapshot.fills]);

  function applyResult(result: RewardBotActionResult) {
    setFeedback(result);
    if (result.snapshot) {
      setSnapshot(result.snapshot);
      setDraft(result.snapshot.config);
    }
  }

  function runAction(action: () => Promise<RewardBotActionResult>) {
    setPending(true);
    startTransition(() => {
      void action()
        .then(applyResult)
        .finally(() => setPending(false));
    });
  }

  function updateNumber(key: NumberConfigKey, value: string) {
    const nextValue = Number(value);
    setDraft((current) => ({
      ...current,
      [key]: Number.isFinite(nextValue) ? nextValue : 0,
    }));
  }

  const modeLabel = draft.mode === "live" ? dictionary.rewards.liveDisabled : dictionary.rewards.simulation;

  return (
    <div className="space-y-6">
      <PageHeader
        eyebrow={dictionary.rewards.eyebrow}
        title={dictionary.rewards.title}
        description={dictionary.rewards.description}
      />

      {feedback ? <OperationFeedbackBanner feedback={feedback} /> : null}

      <div className="grid gap-4 md:grid-cols-2 xl:grid-cols-5">
        <MetricCard
          title={dictionary.rewards.status}
          value={snapshot.status.enabled ? dictionary.common.enabled : dictionary.common.disabled}
          hint={snapshot.status.running ? dictionary.common.active : dictionary.common.idle}
          accent={snapshot.status.enabled ? "success" : "primary"}
        />
        <MetricCard
          title={dictionary.rewards.mode}
          value={modeLabel}
          hint={dictionary.rewards.lastRun}
          accent="primary"
        />
        <MetricCard
          title={dictionary.rewards.markets}
          value={String(snapshot.status.markets_tracked)}
          hint={formatOptionalClock(snapshot.status.last_scan_at)}
          accent="violet"
        />
        <MetricCard
          title={dictionary.rewards.eligible}
          value={String(snapshot.status.eligible_markets)}
          hint={formatOptionalClock(snapshot.status.last_run_at)}
          accent="success"
        />
        <MetricCard
          title={dictionary.rewards.openOrders}
          value={String(snapshot.status.open_orders)}
          hint={`${snapshot.status.positions} ${dictionary.rewards.positions}`}
          accent={snapshot.status.open_orders > 0 ? "success" : "primary"}
        />
      </div>

      <div className="grid gap-4 md:grid-cols-2 xl:grid-cols-5">
        <MetricCard
          title={dictionary.rewards.accountCapital}
          value={formatUsdFixed(snapshot.account.capital_usd)}
          hint={dictionary.rewards.account}
          accent="primary"
        />
        <MetricCard
          title={dictionary.rewards.available}
          value={formatUsdFixed(snapshot.account.available_usd)}
          hint={`${dictionary.rewards.reserved} ${formatUsdFixed(snapshot.account.reserved_usd)}`}
          accent="violet"
        />
        <MetricCard
          title={dictionary.rewards.reserved}
          value={formatUsdFixed(snapshot.account.reserved_usd)}
          hint={dictionary.rewards.openOrders}
          accent="primary"
        />
        <MetricCard
          title={dictionary.rewards.realizedPnl}
          value={formatUsdFixed(snapshot.account.realized_pnl)}
          hint={formatOptionalClock(snapshot.account.updated_at)}
          accent={metricToneForPnl(snapshot.account.realized_pnl)}
        />
        <MetricCard
          title={dictionary.rewards.rewardEarned}
          value={formatUsdFixed(snapshot.account.reward_earned_usd)}
          hint={dictionary.rewards.dailyReward}
          accent="success"
        />
      </div>

      <Card>
        <CardHeader className="flex flex-col gap-4 border-b border-border/70 xl:flex-row xl:items-center xl:justify-between">
          <CardTitle className="font-heading text-base">{dictionary.rewards.config}</CardTitle>
          <div className="flex flex-wrap gap-2">
            <Button
              type="button"
              size="sm"
              variant="outline"
              disabled={pending}
              onClick={() => runAction(runRewardBotOnceAction)}
            >
              <Play className="size-4" />
              {dictionary.rewards.run}
            </Button>
            <Button
              type="button"
              size="sm"
              variant="destructive"
              disabled={pending}
              onClick={() => runAction(cancelRewardBotOrdersAction)}
            >
              <Ban className="size-4" />
              {dictionary.rewards.cancelAll}
            </Button>
            <Button
              type="button"
              size="sm"
              variant="outline"
              disabled={pending}
              onClick={() => runAction(resetRewardBotAction)}
            >
              <RotateCcw className="size-4" />
              {dictionary.rewards.reset}
            </Button>
            <Button
              type="button"
              size="sm"
              disabled={pending}
              onClick={() => runAction(() => updateRewardBotConfigAction(draft))}
            >
              <Save className="size-4" />
              {dictionary.rewards.save}
            </Button>
          </div>
        </CardHeader>
        <CardContent className="space-y-5">
          <div className="grid gap-4 md:grid-cols-2 xl:grid-cols-4">
            <label className="space-y-1.5">
              <span className="text-xs font-medium text-muted-foreground">{dictionary.rewards.account}</span>
              <Input
                value={draft.account_id}
                onChange={(event) => setDraft((current) => ({ ...current, account_id: event.target.value }))}
              />
            </label>
            <label className="space-y-1.5">
              <span className="text-xs font-medium text-muted-foreground">{dictionary.rewards.mode}</span>
              <select
                className="h-8 w-full rounded-lg border border-input bg-background px-2.5 text-sm"
                value={draft.mode}
                onChange={(event) =>
                  setDraft((current) => ({
                    ...current,
                    mode: event.target.value === "live" ? "live" : "dry_run",
                  }))
                }
              >
                <option value="dry_run">{dictionary.rewards.simulation}</option>
                <option value="live">{dictionary.rewards.liveDisabled}</option>
              </select>
            </label>
            <label className="flex items-center gap-3 pt-6 text-sm">
              <input
                type="checkbox"
                className="size-4 accent-primary"
                checked={draft.enabled}
                onChange={(event) => setDraft((current) => ({ ...current, enabled: event.target.checked }))}
              />
              {dictionary.rewards.enabled}
            </label>
            <label className="flex items-center gap-3 pt-6 text-sm">
              <input
                type="checkbox"
                className="size-4 accent-primary"
                checked={draft.cancel_on_fill}
                onChange={(event) =>
                  setDraft((current) => ({ ...current, cancel_on_fill: event.target.checked }))
                }
              />
              {dictionary.rewards.cancelOnFill}
            </label>
          </div>

          <div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-4 xl:grid-cols-6">
            <NumberInput label={dictionary.rewards.maxMarkets} value={draft.max_markets} onChange={(value) => updateNumber("max_markets", value)} />
            <NumberInput label={dictionary.rewards.maxOpenOrders} value={draft.max_open_orders} onChange={(value) => updateNumber("max_open_orders", value)} />
            <NumberInput label={dictionary.rewards.perMarketUsd} value={draft.per_market_usd} suffix="$" onChange={(value) => updateNumber("per_market_usd", value)} />
            <NumberInput label={dictionary.rewards.quoteSizeUsd} value={draft.quote_size_usd} suffix="$" onChange={(value) => updateNumber("quote_size_usd", value)} />
            <NumberInput label={dictionary.rewards.minDailyReward} value={draft.min_daily_reward} suffix="$" onChange={(value) => updateNumber("min_daily_reward", value)} />
            <NumberInput label={dictionary.rewards.minMarketScore} value={draft.min_market_score} onChange={(value) => updateNumber("min_market_score", value)} />
            <NumberInput label={dictionary.rewards.maxSpreadCents} value={draft.max_spread_cents} suffix="c" onChange={(value) => updateNumber("max_spread_cents", value)} />
            <NumberInput label={dictionary.rewards.quoteEdgeCents} value={draft.quote_edge_cents} suffix="c" onChange={(value) => updateNumber("quote_edge_cents", value)} />
            <NumberInput label={dictionary.rewards.safetyMarginCents} value={draft.safety_margin_cents} suffix="c" onChange={(value) => updateNumber("safety_margin_cents", value)} />
            <NumberInput label={dictionary.rewards.minMidpoint} value={draft.min_midpoint} onChange={(value) => updateNumber("min_midpoint", value)} />
            <NumberInput label={dictionary.rewards.maxMidpoint} value={draft.max_midpoint} onChange={(value) => updateNumber("max_midpoint", value)} />
            <NumberInput label={dictionary.rewards.staleBookMs} value={draft.stale_book_ms} suffix="ms" onChange={(value) => updateNumber("stale_book_ms", value)} />
            <NumberInput label={dictionary.rewards.minScoringCheckSec} value={draft.min_scoring_check_sec} suffix="s" onChange={(value) => updateNumber("min_scoring_check_sec", value)} />
            <NumberInput label={dictionary.rewards.maxPositionUsd} value={draft.max_position_usd} suffix="$" onChange={(value) => updateNumber("max_position_usd", value)} />
            <NumberInput label={dictionary.rewards.maxGlobalPositionUsd} value={draft.max_global_position_usd} suffix="$" onChange={(value) => updateNumber("max_global_position_usd", value)} />
            <NumberInput label={dictionary.rewards.exitMarkupCents} value={draft.exit_markup_cents} suffix="c" onChange={(value) => updateNumber("exit_markup_cents", value)} />
            <NumberInput label={dictionary.rewards.accountCapital} value={draft.account_capital_usd} suffix="$" onChange={(value) => updateNumber("account_capital_usd", value)} />
            <NumberInput label={dictionary.rewards.competitionFactor} value={draft.reward_competition_factor} suffix="x" onChange={(value) => updateNumber("reward_competition_factor", value)} />
            <NumberInput label={dictionary.rewards.singleSidedC} value={draft.single_sided_divisor_c} onChange={(value) => updateNumber("single_sided_divisor_c", value)} />
            <NumberInput label={dictionary.rewards.fillRatePerTick} value={draft.fill_rate_per_tick} onChange={(value) => updateNumber("fill_rate_per_tick", value)} />
            <NumberInput label={dictionary.rewards.maxFillRatio} value={draft.max_fill_ratio} onChange={(value) => updateNumber("max_fill_ratio", value)} />
            <NumberInput label={dictionary.rewards.requoteDriftCents} value={draft.requote_drift_cents} suffix="c" onChange={(value) => updateNumber("requote_drift_cents", value)} />
            <label className="space-y-1.5">
              <span className="text-xs font-medium text-muted-foreground">{dictionary.rewards.postFillStrategy}</span>
              <select
                className="h-8 w-full rounded-lg border border-input bg-background px-2.5 text-sm"
                value={draft.post_fill_strategy}
                onChange={(event) =>
                  setDraft((current) => ({
                    ...current,
                    post_fill_strategy: event.target.value as PostFillStrategy,
                  }))
                }
              >
                <option value="exit_at_markup">{dictionary.rewards.strategyExitMarkup}</option>
                <option value="hold_and_requote">{dictionary.rewards.strategyHold}</option>
                <option value="flatten_immediately">{dictionary.rewards.strategyFlatten}</option>
              </select>
            </label>
          </div>
        </CardContent>
      </Card>

      <div className="grid gap-4 xl:grid-cols-[1.25fr_0.75fr]">
        <Card>
          <CardHeader className="border-b border-border/70">
            <CardTitle className="font-heading text-base">{dictionary.rewards.quotePlans}</CardTitle>
          </CardHeader>
          <CardContent>
            <QuotePlansTable plans={visiblePlans} />
          </CardContent>
        </Card>

        <Card>
          <CardHeader className="border-b border-border/70">
            <CardTitle className="font-heading text-base">{dictionary.rewards.managedOrders}</CardTitle>
          </CardHeader>
          <CardContent>
            <OrdersTable orders={visibleOrders} />
          </CardContent>
        </Card>
      </div>

      <Card>
        <CardHeader className="border-b border-border/70">
          <CardTitle className="font-heading text-base">{dictionary.rewards.riskEvents}</CardTitle>
        </CardHeader>
        <CardContent>
          <EventsPanel events={visibleEvents} fills={visibleFills} />
        </CardContent>
      </Card>
    </div>
  );
}

function EventsPanel({
  events,
  fills,
}: {
  events: RewardRiskEventDto[];
  fills: RewardFillDto[];
}) {
  const { dictionary } = useI18n();
  const [category, setCategory] = useState<EventCategory>("all");

  const filteredEvents = useMemo(
    () =>
      category === "all"
        ? events
        : events.filter((event) => eventCategory(event.event_type) === category),
    [events, category],
  );

  return (
    <Tabs
      value={category}
      onValueChange={(value) => setCategory(value as EventCategory)}
      className="gap-4"
    >
      <TabsList>
        <TabsTrigger value="all">{dictionary.rewards.eventsAll}</TabsTrigger>
        <TabsTrigger value="placements">{dictionary.rewards.eventsPlacements}</TabsTrigger>
        <TabsTrigger value="cancels">{dictionary.rewards.eventsCancels}</TabsTrigger>
        <TabsTrigger value="fills">{dictionary.rewards.eventsFills}</TabsTrigger>
        <TabsTrigger value="rewards">{dictionary.rewards.eventsRewards}</TabsTrigger>
      </TabsList>
      <TabsContent value="all">
        <EventsTable events={filteredEvents} />
      </TabsContent>
      <TabsContent value="placements">
        <EventsTable events={filteredEvents} />
      </TabsContent>
      <TabsContent value="cancels">
        <EventsTable events={filteredEvents} />
      </TabsContent>
      <TabsContent value="fills">
        <FillsTable fills={fills} />
      </TabsContent>
      <TabsContent value="rewards">
        <EventsTable events={filteredEvents} />
      </TabsContent>
    </Tabs>
  );
}

function FillsTable({ fills }: { fills: RewardFillDto[] }) {
  const { dictionary } = useI18n();

  if (fills.length === 0) {
    return <p className="py-6 text-center text-sm text-muted-foreground">{dictionary.rewards.none}</p>;
  }

  return (
    <Table>
      <TableHeader>
        <TableRow>
          <TableHead>{dictionary.rewards.outcome}</TableHead>
          <TableHead>{dictionary.rewards.state}</TableHead>
          <TableHead>{dictionary.rewards.role}</TableHead>
          <TableHead>{dictionary.rewards.price}</TableHead>
          <TableHead>{dictionary.rewards.size}</TableHead>
          <TableHead>{dictionary.rewards.pnl}</TableHead>
          <TableHead>{dictionary.rewards.time}</TableHead>
        </TableRow>
      </TableHeader>
      <TableBody>
        {fills.map((fill) => (
          <TableRow key={fill.id}>
            <TableCell>{fill.outcome}</TableCell>
            <TableCell>
              <StatusPill tone={fill.side === "buy" ? "success" : "warning"}>{fill.side}</StatusPill>
            </TableCell>
            <TableCell className="font-mono text-xs">{fill.role}</TableCell>
            <TableCell className="font-mono">{formatFixed(fill.price, 2)}</TableCell>
            <TableCell className="font-mono">{formatFixed(fill.size, 2)}</TableCell>
            <TableCell className="font-mono">{formatSignedFixed(fill.realized_pnl, 2)}</TableCell>
            <TableCell className="font-mono text-xs text-muted-foreground">
              {formatOptionalClock(fill.created_at)}
            </TableCell>
          </TableRow>
        ))}
      </TableBody>
    </Table>
  );
}

function NumberInput({
  label,
  value,
  suffix,
  onChange,
}: {
  label: string;
  value: DecimalValue;
  suffix?: string;
  onChange: (value: string) => void;
}) {
  return (
    <label className="space-y-1.5">
      <span className="text-xs font-medium text-muted-foreground">{label}</span>
      <div className="flex">
        <Input
          type="number"
          className="rounded-r-none font-mono"
          value={String(toFiniteNumber(value))}
          onChange={(event) => onChange(event.target.value)}
        />
        {suffix ? (
          <span className="flex h-8 min-w-8 items-center justify-center rounded-r-lg border border-l-0 border-input px-2 text-xs text-muted-foreground">
            {suffix}
          </span>
        ) : null}
      </div>
    </label>
  );
}

function QuotePlansTable({ plans }: { plans: RewardQuotePlanDto[] }) {
  const { dictionary } = useI18n();

  return (
    <Table>
      <TableHeader>
        <TableRow>
          <TableHead>{dictionary.rewards.market}</TableHead>
          <TableHead>{dictionary.rewards.score}</TableHead>
          <TableHead>{dictionary.rewards.dailyReward}</TableHead>
          <TableHead>{dictionary.rewards.midpoint}</TableHead>
          <TableHead>{dictionary.rewards.quotes}</TableHead>
        </TableRow>
      </TableHeader>
      <TableBody>
        {plans.map((plan) => (
          <TableRow key={plan.condition_id}>
            <TableCell className="max-w-[360px]">
              <div className="space-y-1">
                <p className="truncate font-medium">{plan.question}</p>
                <p className="text-xs text-muted-foreground">{plan.reason}</p>
              </div>
            </TableCell>
            <TableCell>
              <StatusPill tone={plan.eligible ? "success" : "neutral"}>
                {formatFixed(plan.score, 1)}
              </StatusPill>
            </TableCell>
            <TableCell className="font-mono">{formatUsdFixed(plan.total_daily_rate)}</TableCell>
            <TableCell className="font-mono">{plan.midpoint == null ? "n/a" : formatFixed(plan.midpoint, 3)}</TableCell>
            <TableCell className="font-mono text-xs">
              {plan.legs.length === 0
                ? dictionary.rewards.none
                : plan.legs.map((leg) => `${leg.outcome} ${formatFixed(leg.size, 2)}@${formatFixed(leg.price, 2)}`).join(" / ")}
            </TableCell>
          </TableRow>
        ))}
      </TableBody>
    </Table>
  );
}

function OrdersTable({ orders }: { orders: ManagedRewardOrderDto[] }) {
  const { dictionary } = useI18n();

  return (
    <Table>
      <TableHeader>
        <TableRow>
          <TableHead>{dictionary.rewards.state}</TableHead>
          <TableHead>{dictionary.rewards.outcome}</TableHead>
          <TableHead>{dictionary.rewards.price}</TableHead>
          <TableHead>{dictionary.rewards.size}</TableHead>
          <TableHead>{dictionary.rewards.scoring}</TableHead>
        </TableRow>
      </TableHeader>
      <TableBody>
        {orders.map((order) => (
          <TableRow key={order.id}>
            <TableCell>
              <StatusPill tone={rewardTone(order.status)}>{order.status}</StatusPill>
            </TableCell>
            <TableCell>{order.outcome}</TableCell>
            <TableCell className="font-mono">{formatFixed(order.price, 2)}</TableCell>
            <TableCell className="font-mono">{formatFixed(order.size, 2)}</TableCell>
            <TableCell>{order.scoring ? dictionary.common.active : dictionary.common.idle}</TableCell>
          </TableRow>
        ))}
      </TableBody>
    </Table>
  );
}

function EventsTable({ events }: { events: RewardRiskEventDto[] }) {
  const { dictionary } = useI18n();

  return (
    <Table>
      <TableHeader>
        <TableRow>
          <TableHead>{dictionary.rewards.severity}</TableHead>
          <TableHead>{dictionary.rewards.type}</TableHead>
          <TableHead>{dictionary.rewards.message}</TableHead>
          <TableHead>{dictionary.common.published}</TableHead>
        </TableRow>
      </TableHeader>
      <TableBody>
        {events.map((event) => (
          <TableRow key={event.id}>
            <TableCell>
              <StatusPill tone={approvalSeverityTone(event.severity)}>{event.severity}</StatusPill>
            </TableCell>
            <TableCell className="font-mono text-xs">{event.event_type}</TableCell>
            <TableCell className="max-w-[520px] truncate">{event.message}</TableCell>
            <TableCell className="font-mono text-xs text-muted-foreground">{formatOptionalClock(event.created_at)}</TableCell>
          </TableRow>
        ))}
      </TableBody>
    </Table>
  );
}
