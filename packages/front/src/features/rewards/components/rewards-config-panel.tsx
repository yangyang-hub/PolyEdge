"use client";

import type { Dispatch, ReactNode, SetStateAction } from "react";
import { Info } from "lucide-react";

import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Separator } from "@/components/ui/separator";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import type {
  PostFillStrategy,
  RewardBotConfigDto,
  RewardExecutionMode,
} from "@/lib/contracts/dto";
import { useI18n } from "@/lib/i18n/client";

import type { NumberConfigKey } from "../types";
import { NumberInput } from "./number-input";

type RewardsConfigPanelProps = {
  draft: RewardBotConfigDto;
  setDraft: Dispatch<SetStateAction<RewardBotConfigDto>>;
  updateNumber: (key: NumberConfigKey, value: string) => void;
};

const selectClassName = "h-8 w-full rounded-lg border border-input bg-background px-2.5 text-sm";

export function RewardsConfigPanel({
  draft,
  setDraft,
  updateNumber,
}: RewardsConfigPanelProps) {
  const { dictionary } = useI18n();
  const h = dictionary.rewards.configHints;

  return (
    <Card>
      <CardHeader className="border-b border-border/70">
        <CardTitle>{dictionary.rewards.config}</CardTitle>
        <CardDescription>{dictionary.rewards.configDescription}</CardDescription>
      </CardHeader>
      <CardContent className="space-y-6">
        <ConfigSection
          title={dictionary.rewards.configExecution}
          description={dictionary.rewards.configExecutionDescription}
        >
          <label className="space-y-1.5">
            <span className="text-xs font-medium text-muted-foreground">
              {dictionary.rewards.account}
            </span>
            <Input
              value={draft.account_id}
              onChange={(event) =>
                setDraft((current) => ({ ...current, account_id: event.target.value }))
              }
            />
          </label>

          <label className="space-y-1.5">
            <span className="flex items-center gap-1 text-xs font-medium text-muted-foreground">
              {dictionary.rewards.executionMode}
              <Hint content={h.executionMode} />
            </span>
            <select
              className={selectClassName}
              value={draft.execution_mode}
              onChange={(event) =>
                setDraft((current) => ({
                  ...current,
                  execution_mode: event.target.value as RewardExecutionMode,
                }))
              }
            >
              <option value="validation">{dictionary.rewards.modeValidation}</option>
              <option value="live">{dictionary.rewards.modeLive}</option>
            </select>
          </label>

          <ToggleField
            label={dictionary.rewards.enabled}
            checked={draft.enabled}
            onChange={(checked) => setDraft((current) => ({ ...current, enabled: checked }))}
          />

          <ToggleField
            label={dictionary.rewards.cancelOnFill}
            hint={h.cancelOnFill}
            checked={draft.cancel_on_fill}
            onChange={(checked) =>
              setDraft((current) => ({ ...current, cancel_on_fill: checked }))
            }
          />

          <label className="space-y-1.5">
            <span className="flex items-center gap-1 text-xs font-medium text-muted-foreground">
              {dictionary.rewards.postFillStrategy}
              <Hint content={h.postFillStrategy} />
            </span>
            <select
              className={selectClassName}
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
        </ConfigSection>

        <Separator />

        <ConfigSection
          title={dictionary.rewards.configMarketSelection}
          description={dictionary.rewards.configMarketSelectionDescription}
        >
          <NumberInput
            label={dictionary.rewards.maxMarkets}
            value={draft.max_markets}
            hint={h.maxMarkets}
            onChange={(value) => updateNumber("max_markets", value)}
          />
          <NumberInput
            label={dictionary.rewards.minDailyReward}
            value={draft.min_daily_reward}
            suffix="$"
            hint={h.minDailyReward}
            onChange={(value) => updateNumber("min_daily_reward", value)}
          />
          <NumberInput
            label={dictionary.rewards.minMarketScore}
            value={draft.min_market_score}
            hint={h.minMarketScore}
            onChange={(value) => updateNumber("min_market_score", value)}
          />
          <NumberInput
            label={dictionary.rewards.minMidpoint}
            value={draft.min_midpoint}
            hint={h.minMidpoint}
            onChange={(value) => updateNumber("min_midpoint", value)}
          />
          <NumberInput
            label={dictionary.rewards.maxMidpoint}
            value={draft.max_midpoint}
            hint={h.maxMidpoint}
            onChange={(value) => updateNumber("max_midpoint", value)}
          />
          <NumberInput
            label={dictionary.rewards.staleBookMs}
            value={draft.stale_book_ms}
            suffix="ms"
            hint={h.staleBookMs}
            onChange={(value) => updateNumber("stale_book_ms", value)}
          />
          <NumberInput
            label={dictionary.rewards.minScoringCheckSec}
            value={draft.min_scoring_check_sec}
            suffix="s"
            hint={h.minScoringCheckSec}
            onChange={(value) => updateNumber("min_scoring_check_sec", value)}
          />
        </ConfigSection>

        <Separator />

        <ConfigSection
          title={dictionary.rewards.configQuoteConstruction}
          description={dictionary.rewards.configQuoteConstructionDescription}
        >
          <NumberInput
            label={dictionary.rewards.maxOpenOrders}
            value={draft.max_open_orders}
            hint={h.maxOpenOrders}
            onChange={(value) => updateNumber("max_open_orders", value)}
          />
          <NumberInput
            label={dictionary.rewards.perMarketUsd}
            value={draft.per_market_usd}
            suffix="$"
            hint={h.perMarketUsd}
            onChange={(value) => updateNumber("per_market_usd", value)}
          />
          <NumberInput
            label={dictionary.rewards.quoteSizeUsd}
            value={draft.quote_size_usd}
            suffix="$"
            hint={h.quoteSizeUsd}
            onChange={(value) => updateNumber("quote_size_usd", value)}
          />
          <NumberInput
            label={dictionary.rewards.maxSpreadCents}
            value={draft.max_spread_cents}
            suffix="c"
            hint={h.maxSpreadCents}
            onChange={(value) => updateNumber("max_spread_cents", value)}
          />
          <NumberInput
            label={dictionary.rewards.quoteEdgeCents}
            value={draft.quote_edge_cents}
            suffix="c"
            hint={h.quoteEdgeCents}
            onChange={(value) => updateNumber("quote_edge_cents", value)}
          />
          <NumberInput
            label={dictionary.rewards.safetyMarginCents}
            value={draft.safety_margin_cents}
            suffix="c"
            hint={h.safetyMarginCents}
            onChange={(value) => updateNumber("safety_margin_cents", value)}
          />
          <NumberInput
            label={dictionary.rewards.requoteDriftCents}
            value={draft.requote_drift_cents}
            suffix="c"
            hint={h.requoteDriftCents}
            onChange={(value) => updateNumber("requote_drift_cents", value)}
          />
        </ConfigSection>

        <Separator />

        <ConfigSection
          title={dictionary.rewards.configInventoryValidation}
          description={dictionary.rewards.configInventoryValidationDescription}
        >
          <NumberInput
            label={dictionary.rewards.maxPositionUsd}
            value={draft.max_position_usd}
            suffix="$"
            hint={h.maxPositionUsd}
            onChange={(value) => updateNumber("max_position_usd", value)}
          />
          <NumberInput
            label={dictionary.rewards.maxGlobalPositionUsd}
            value={draft.max_global_position_usd}
            suffix="$"
            hint={h.maxGlobalPositionUsd}
            onChange={(value) => updateNumber("max_global_position_usd", value)}
          />
          <NumberInput
            label={dictionary.rewards.exitMarkupCents}
            value={draft.exit_markup_cents}
            suffix="c"
            hint={h.exitMarkupCents}
            onChange={(value) => updateNumber("exit_markup_cents", value)}
          />
          <NumberInput
            label={dictionary.rewards.accountCapital}
            value={draft.account_capital_usd}
            suffix="$"
            hint={h.accountCapital}
            onChange={(value) => updateNumber("account_capital_usd", value)}
          />
          <NumberInput
            label={dictionary.rewards.competitionFactor}
            value={draft.reward_competition_factor}
            suffix="x"
            hint={h.competitionFactor}
            onChange={(value) => updateNumber("reward_competition_factor", value)}
          />
          <NumberInput
            label={dictionary.rewards.singleSidedC}
            value={draft.single_sided_divisor_c}
            hint={h.singleSidedC}
            onChange={(value) => updateNumber("single_sided_divisor_c", value)}
          />
          <NumberInput
            label={dictionary.rewards.fillRatePerTick}
            value={draft.fill_rate_per_tick}
            hint={h.fillRatePerTick}
            onChange={(value) => updateNumber("fill_rate_per_tick", value)}
          />
          <NumberInput
            label={dictionary.rewards.maxFillRatio}
            value={draft.max_fill_ratio}
            hint={h.maxFillRatio}
            onChange={(value) => updateNumber("max_fill_ratio", value)}
          />
        </ConfigSection>
      </CardContent>
    </Card>
  );
}

function ConfigSection({
  title,
  description,
  children,
}: {
  title: string;
  description: string;
  children: ReactNode;
}) {
  return (
    <section className="grid gap-4 xl:grid-cols-[220px_1fr]">
      <div className="space-y-1">
        <h3 className="font-heading text-sm font-medium">{title}</h3>
        <p className="max-w-sm text-xs leading-5 text-muted-foreground">{description}</p>
      </div>
      <div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-3 2xl:grid-cols-4">
        {children}
      </div>
    </section>
  );
}

function ToggleField({
  label,
  hint,
  checked,
  onChange,
}: {
  label: string;
  hint?: string;
  checked: boolean;
  onChange: (checked: boolean) => void;
}) {
  return (
    <label className="flex min-h-16 items-center gap-3 rounded-lg border border-border/70 bg-muted/20 px-3 py-2 text-sm">
      <input
        type="checkbox"
        className="size-4 accent-primary"
        checked={checked}
        onChange={(event) => onChange(event.target.checked)}
      />
      <span className="flex items-center gap-1">
        {label}
        {hint ? <Hint content={hint} /> : null}
      </span>
    </label>
  );
}

function Hint({ content }: { content: string }) {
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <Info className="size-3 cursor-help text-muted-foreground/60" />
      </TooltipTrigger>
      <TooltipContent side="top" className="max-w-xs text-wrap">
        {content}
      </TooltipContent>
    </Tooltip>
  );
}
