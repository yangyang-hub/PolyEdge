"use client";

import { startTransition, useMemo, useState } from "react";
import { Ban, Info, Play, RotateCcw, Save } from "lucide-react";

import { MetricCard } from "@/components/shared/metric-card";
import { OperationFeedbackBanner } from "@/components/shared/operation-feedback-banner";
import { PageHeader } from "@/components/shared/page-header";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import type {
  PostFillStrategy,
  RewardBotConfigDto,
  RewardBotSnapshotDto,
} from "@/lib/contracts/dto";
import {
  formatOptionalClock,
  formatUsdFixed,
  metricToneForPnl,
} from "@/lib/formatters";
import { useI18n } from "@/lib/i18n/client";
import {
  cancelRewardBotOrdersAction,
  resetRewardBotAction,
  runRewardBotOnceAction,
  updateRewardBotConfigAction,
  type RewardBotActionResult,
} from "@/lib/api/actions";

import type { NumberConfigKey } from "../types";
import { NumberInput } from "./number-input";
import { EventsPanel } from "./rewards-events-panel";
import { OrdersTable, QuotePlansTable } from "./rewards-tables";

export function RewardsWorkbench({ initialSnapshot }: { initialSnapshot: RewardBotSnapshotDto }) {
  const { dictionary } = useI18n();
  const [snapshot, setSnapshot] = useState(initialSnapshot);
  const [draft, setDraft] = useState<RewardBotConfigDto>(initialSnapshot.config);
  const [feedback, setFeedback] = useState<RewardBotActionResult | null>(null);
  const [pending, setPending] = useState(false);

  const visiblePlans = snapshot.quote_plans;
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
              <Tooltip>
                <TooltipTrigger asChild>
                  <Info className="size-3 cursor-help text-muted-foreground/60" />
                </TooltipTrigger>
                <TooltipContent side="top" className="max-w-xs text-wrap">
                  {dictionary.rewards.configHints.cancelOnFill}
                </TooltipContent>
              </Tooltip>
            </label>
          </div>

          <div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-4 xl:grid-cols-6">
            <NumberInput label={dictionary.rewards.maxMarkets} value={draft.max_markets} hint={dictionary.rewards.configHints.maxMarkets} onChange={(value) => updateNumber("max_markets", value)} />
            <NumberInput label={dictionary.rewards.maxOpenOrders} value={draft.max_open_orders} hint={dictionary.rewards.configHints.maxOpenOrders} onChange={(value) => updateNumber("max_open_orders", value)} />
            <NumberInput label={dictionary.rewards.perMarketUsd} value={draft.per_market_usd} suffix="$" hint={dictionary.rewards.configHints.perMarketUsd} onChange={(value) => updateNumber("per_market_usd", value)} />
            <NumberInput label={dictionary.rewards.quoteSizeUsd} value={draft.quote_size_usd} suffix="$" hint={dictionary.rewards.configHints.quoteSizeUsd} onChange={(value) => updateNumber("quote_size_usd", value)} />
            <NumberInput label={dictionary.rewards.minDailyReward} value={draft.min_daily_reward} suffix="$" hint={dictionary.rewards.configHints.minDailyReward} onChange={(value) => updateNumber("min_daily_reward", value)} />
            <NumberInput label={dictionary.rewards.minMarketScore} value={draft.min_market_score} hint={dictionary.rewards.configHints.minMarketScore} onChange={(value) => updateNumber("min_market_score", value)} />
            <NumberInput label={dictionary.rewards.maxSpreadCents} value={draft.max_spread_cents} suffix="c" hint={dictionary.rewards.configHints.maxSpreadCents} onChange={(value) => updateNumber("max_spread_cents", value)} />
            <NumberInput label={dictionary.rewards.quoteEdgeCents} value={draft.quote_edge_cents} suffix="c" hint={dictionary.rewards.configHints.quoteEdgeCents} onChange={(value) => updateNumber("quote_edge_cents", value)} />
            <NumberInput label={dictionary.rewards.safetyMarginCents} value={draft.safety_margin_cents} suffix="c" hint={dictionary.rewards.configHints.safetyMarginCents} onChange={(value) => updateNumber("safety_margin_cents", value)} />
            <NumberInput label={dictionary.rewards.minMidpoint} value={draft.min_midpoint} hint={dictionary.rewards.configHints.minMidpoint} onChange={(value) => updateNumber("min_midpoint", value)} />
            <NumberInput label={dictionary.rewards.maxMidpoint} value={draft.max_midpoint} hint={dictionary.rewards.configHints.maxMidpoint} onChange={(value) => updateNumber("max_midpoint", value)} />
            <NumberInput label={dictionary.rewards.staleBookMs} value={draft.stale_book_ms} suffix="ms" hint={dictionary.rewards.configHints.staleBookMs} onChange={(value) => updateNumber("stale_book_ms", value)} />
            <NumberInput label={dictionary.rewards.minScoringCheckSec} value={draft.min_scoring_check_sec} suffix="s" hint={dictionary.rewards.configHints.minScoringCheckSec} onChange={(value) => updateNumber("min_scoring_check_sec", value)} />
            <NumberInput label={dictionary.rewards.maxPositionUsd} value={draft.max_position_usd} suffix="$" hint={dictionary.rewards.configHints.maxPositionUsd} onChange={(value) => updateNumber("max_position_usd", value)} />
            <NumberInput label={dictionary.rewards.maxGlobalPositionUsd} value={draft.max_global_position_usd} suffix="$" hint={dictionary.rewards.configHints.maxGlobalPositionUsd} onChange={(value) => updateNumber("max_global_position_usd", value)} />
            <NumberInput label={dictionary.rewards.exitMarkupCents} value={draft.exit_markup_cents} suffix="c" hint={dictionary.rewards.configHints.exitMarkupCents} onChange={(value) => updateNumber("exit_markup_cents", value)} />
            <NumberInput label={dictionary.rewards.accountCapital} value={draft.account_capital_usd} suffix="$" hint={dictionary.rewards.configHints.accountCapital} onChange={(value) => updateNumber("account_capital_usd", value)} />
            <NumberInput label={dictionary.rewards.competitionFactor} value={draft.reward_competition_factor} suffix="x" hint={dictionary.rewards.configHints.competitionFactor} onChange={(value) => updateNumber("reward_competition_factor", value)} />
            <NumberInput label={dictionary.rewards.singleSidedC} value={draft.single_sided_divisor_c} hint={dictionary.rewards.configHints.singleSidedC} onChange={(value) => updateNumber("single_sided_divisor_c", value)} />
            <NumberInput label={dictionary.rewards.fillRatePerTick} value={draft.fill_rate_per_tick} hint={dictionary.rewards.configHints.fillRatePerTick} onChange={(value) => updateNumber("fill_rate_per_tick", value)} />
            <NumberInput label={dictionary.rewards.maxFillRatio} value={draft.max_fill_ratio} hint={dictionary.rewards.configHints.maxFillRatio} onChange={(value) => updateNumber("max_fill_ratio", value)} />
            <NumberInput label={dictionary.rewards.requoteDriftCents} value={draft.requote_drift_cents} suffix="c" hint={dictionary.rewards.configHints.requoteDriftCents} onChange={(value) => updateNumber("requote_drift_cents", value)} />
            <label className="space-y-1.5">
              <span className="flex items-center gap-1 text-xs font-medium text-muted-foreground">
                {dictionary.rewards.postFillStrategy}
                <Tooltip>
                  <TooltipTrigger asChild>
                    <Info className="size-3 cursor-help text-muted-foreground/60" />
                  </TooltipTrigger>
                  <TooltipContent side="top" className="max-w-xs text-wrap">
                    {dictionary.rewards.configHints.postFillStrategy}
                  </TooltipContent>
                </Tooltip>
              </span>
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
