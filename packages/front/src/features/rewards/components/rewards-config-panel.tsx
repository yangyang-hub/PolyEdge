"use client";

import type { Dispatch, SetStateAction } from "react";

import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Separator } from "@/components/ui/separator";
import type { PostFillStrategy, RewardBotConfigDto } from "@/lib/contracts/dto";
import { dictionary } from "@/lib/i18n/dictionaries";

import type { NumberConfigKey } from "../types";
import { NumberInput } from "./number-input";
import {
  AiAdvisoryConfig,
  BookSelectionConfig,
} from "./rewards-advanced-config";
import { ConfigSection, Hint, ToggleField, selectClassName } from "./rewards-config-fields";
import { OpportunityConfig } from "./rewards-opportunity-config";

type RewardsConfigPanelProps = {
  draft: RewardBotConfigDto;
  setDraft: Dispatch<SetStateAction<RewardBotConfigDto>>;
  updateNumber: (key: NumberConfigKey, value: string) => void;
};

export function RewardsConfigPanel({
  draft,
  setDraft,
  updateNumber,
}: RewardsConfigPanelProps) {
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
            label={dictionary.rewards.minMarketLiquidityUsd}
            value={draft.min_market_liquidity_usd}
            suffix="$"
            hint={h.minMarketLiquidityUsd}
            onChange={(value) => updateNumber("min_market_liquidity_usd", value)}
          />
          <NumberInput
            label={dictionary.rewards.minMarketVolume24hUsd}
            value={draft.min_market_volume_24h_usd}
            suffix="$"
            hint={h.minMarketVolume24hUsd}
            onChange={(value) => updateNumber("min_market_volume_24h_usd", value)}
          />
          <NumberInput
            label={dictionary.rewards.minHoursToEnd}
            value={draft.min_hours_to_end}
            suffix="h"
            hint={h.minHoursToEnd}
            onChange={(value) => updateNumber("min_hours_to_end", value)}
          />
          <NumberInput
            label={dictionary.rewards.maxMarketSpreadCents}
            value={draft.max_market_spread_cents}
            suffix="c"
            hint={h.maxMarketSpreadCents}
            onChange={(value) => updateNumber("max_market_spread_cents", value)}
          />
          <NumberInput
            label={dictionary.rewards.maxMarketDataAgeMinutes}
            value={draft.max_market_data_age_minutes}
            suffix="min"
            hint={h.maxMarketDataAgeMinutes}
            onChange={(value) => updateNumber("max_market_data_age_minutes", value)}
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

        <OpportunityConfig
          draft={draft}
          setDraft={setDraft}
          updateNumber={updateNumber}
        />

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
            label={dictionary.rewards.maxSpreadCents}
            value={draft.max_spread_cents}
            suffix="c"
            hint={h.maxSpreadCents}
            onChange={(value) => updateNumber("max_spread_cents", value)}
          />
          <label className="space-y-1.5">
            <span className="flex items-center gap-1 text-xs font-medium text-muted-foreground">
              {dictionary.rewards.quoteBidRank}
              <Hint content={h.quoteBidRank} />
            </span>
            <select
              className={selectClassName}
              value={draft.quote_bid_rank}
              onChange={(event) =>
                setDraft((current) => ({
                  ...current,
                  quote_bid_rank: Number(event.target.value),
                }))
              }
            >
              <option value={1}>{dictionary.rewards.bidRank1}</option>
              <option value={2}>{dictionary.rewards.bidRank2}</option>
              <option value={3}>{dictionary.rewards.bidRank3}</option>
            </select>
          </label>
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
          <NumberInput
            label={dictionary.rewards.requoteDriftConfirmSec}
            value={draft.requote_drift_confirm_sec}
            suffix="s"
            hint={h.requoteDriftConfirmSec}
            onChange={(value) => updateNumber("requote_drift_confirm_sec", value)}
          />
          <NumberInput
            label={dictionary.rewards.requoteDriftCooldownSec}
            value={draft.requote_drift_cooldown_sec}
            suffix="s"
            hint={h.requoteDriftCooldownSec}
            onChange={(value) => updateNumber("requote_drift_cooldown_sec", value)}
          />
          <NumberInput
            label={dictionary.rewards.requoteDriftMaxCancelsPerCycle}
            value={draft.requote_drift_max_cancels_per_cycle}
            hint={h.requoteDriftMaxCancelsPerCycle}
            onChange={(value) => updateNumber("requote_drift_max_cancels_per_cycle", value)}
          />
        </ConfigSection>

        <Separator />

        <FairValueConfig draft={draft} setDraft={setDraft} updateNumber={updateNumber} />

        <Separator />

        <ConfigSection
          title={dictionary.rewards.configBalancedMerge}
          description={dictionary.rewards.configBalancedMergeDescription}
        >
          <ToggleField
            label={dictionary.rewards.balancedMergeEnabled}
            hint={h.balancedMergeEnabled}
            checked={draft.balanced_merge_enabled}
            onChange={(checked) =>
              setDraft((current) => ({ ...current, balanced_merge_enabled: checked }))
            }
          />
          <ToggleField
            label={dictionary.rewards.balancedMergeAutoExecute}
            hint={h.balancedMergeAutoExecute}
            checked={draft.balanced_merge_auto_execute_enabled}
            onChange={(checked) =>
              setDraft((current) => ({
                ...current,
                balanced_merge_auto_execute_enabled: checked,
              }))
            }
          />
          <NumberInput
            label={dictionary.rewards.balancedMergeMaxMarkets}
            value={draft.balanced_merge_max_markets}
            hint={h.balancedMergeMaxMarkets}
            onChange={(value) => updateNumber("balanced_merge_max_markets", value)}
          />
          <NumberInput
            label={dictionary.rewards.balancedMergeMaxOpenOrders}
            value={draft.balanced_merge_max_open_orders}
            hint={h.balancedMergeMaxOpenOrders}
            onChange={(value) => updateNumber("balanced_merge_max_open_orders", value)}
          />
          <NumberInput
            label={dictionary.rewards.balancedMergeMinEdgeCents}
            value={draft.balanced_merge_min_edge_cents}
            suffix="c"
            hint={h.balancedMergeMinEdgeCents}
            onChange={(value) => updateNumber("balanced_merge_min_edge_cents", value)}
          />
          <NumberInput
            label={dictionary.rewards.balancedMergeMinMarketScore}
            value={draft.balanced_merge_min_market_score}
            hint={h.balancedMergeMinMarketScore}
            onChange={(value) => updateNumber("balanced_merge_min_market_score", value)}
          />
          <NumberInput
            label={dictionary.rewards.balancedMergeMinLiquidity}
            value={draft.balanced_merge_min_market_liquidity_usd}
            suffix="$"
            hint={h.balancedMergeMinLiquidity}
            onChange={(value) => updateNumber("balanced_merge_min_market_liquidity_usd", value)}
          />
          <NumberInput
            label={dictionary.rewards.balancedMergeMinVolume24h}
            value={draft.balanced_merge_min_market_volume_24h_usd}
            suffix="$"
            hint={h.balancedMergeMinVolume24h}
            onChange={(value) => updateNumber("balanced_merge_min_market_volume_24h_usd", value)}
          />
          <NumberInput
            label={dictionary.rewards.balancedMergeMaxMarketSpread}
            value={draft.balanced_merge_max_market_spread_cents}
            suffix="c"
            hint={h.balancedMergeMaxMarketSpread}
            onChange={(value) => updateNumber("balanced_merge_max_market_spread_cents", value)}
          />
          <label className="space-y-1.5">
            <span className="flex items-center gap-1 text-xs font-medium text-muted-foreground">
              {dictionary.rewards.balancedMergeQuoteBidRank}
              <Hint content={h.balancedMergeQuoteBidRank} />
            </span>
            <select
              className={selectClassName}
              value={draft.balanced_merge_quote_bid_rank}
              onChange={(event) =>
                setDraft((current) => ({
                  ...current,
                  balanced_merge_quote_bid_rank: Number(event.target.value),
                }))
              }
            >
              <option value={1}>{dictionary.rewards.bidRank1}</option>
              <option value={2}>{dictionary.rewards.bidRank2}</option>
              <option value={3}>{dictionary.rewards.bidRank3}</option>
            </select>
          </label>
          <NumberInput
            label={dictionary.rewards.balancedMergeMaxUnpairedPosition}
            value={draft.balanced_merge_max_unpaired_position_usd}
            suffix="$"
            hint={h.balancedMergeMaxUnpairedPosition}
            onChange={(value) => updateNumber("balanced_merge_max_unpaired_position_usd", value)}
          />
        </ConfigSection>

        <Separator />

        <BookSelectionConfig draft={draft} setDraft={setDraft} updateNumber={updateNumber} />

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
        </ConfigSection>

        <Separator />

        <AiAdvisoryConfig draft={draft} setDraft={setDraft} updateNumber={updateNumber} />
      </CardContent>
    </Card>
  );
}

function FairValueConfig({
  draft,
  setDraft,
  updateNumber,
}: RewardsConfigPanelProps) {
  const h = dictionary.rewards.configHints;

  return (
    <ConfigSection
      title={dictionary.rewards.configFairValue}
      description={dictionary.rewards.configFairValueDescription}
    >
      <ToggleField
        label={dictionary.rewards.fairValueEnabled}
        hint={h.fairValueEnabled}
        checked={draft.fair_value_enabled}
        onChange={(checked) =>
          setDraft((current) => ({ ...current, fair_value_enabled: checked }))
        }
      />
      <ToggleField
        label={dictionary.rewards.fairValueRecordHistory}
        hint={h.fairValueRecordHistory}
        checked={draft.fair_value_record_history_enabled}
        onChange={(checked) =>
          setDraft((current) => ({
            ...current,
            fair_value_record_history_enabled: checked,
          }))
        }
      />
      <NumberInput
        label={dictionary.rewards.fairValueMinConfidence}
        value={draft.fair_value_min_confidence}
        hint={h.fairValueMinConfidence}
        onChange={(value) => updateNumber("fair_value_min_confidence", value)}
      />
      <NumberInput
        label={dictionary.rewards.fairValueMinRawEdgeCents}
        value={draft.fair_value_min_raw_edge_cents}
        suffix="c"
        hint={h.fairValueMinRawEdgeCents}
        onChange={(value) => updateNumber("fair_value_min_raw_edge_cents", value)}
      />
      <NumberInput
        label={dictionary.rewards.fairValueMinEffectiveEdgeCents}
        value={draft.fair_value_min_effective_edge_cents}
        suffix="c"
        hint={h.fairValueMinEffectiveEdgeCents}
        onChange={(value) => updateNumber("fair_value_min_effective_edge_cents", value)}
      />
      <NumberInput
        label={dictionary.rewards.fairValueUncertaintyBufferCents}
        value={draft.fair_value_uncertainty_buffer_cents}
        suffix="c"
        hint={h.fairValueUncertaintyBufferCents}
        onChange={(value) => updateNumber("fair_value_uncertainty_buffer_cents", value)}
      />
      <NumberInput
        label={dictionary.rewards.fairValueRebateHaircut}
        value={draft.fair_value_rebate_haircut}
        hint={h.fairValueRebateHaircut}
        onChange={(value) => updateNumber("fair_value_rebate_haircut", value)}
      />
      <NumberInput
        label={dictionary.rewards.fairValueMaxRewardRebateCents}
        value={draft.fair_value_max_reward_rebate_cents}
        suffix="c"
        hint={h.fairValueMaxRewardRebateCents}
        onChange={(value) => updateNumber("fair_value_max_reward_rebate_cents", value)}
      />
      <NumberInput
        label={dictionary.rewards.fairValueMaxMidpointDeviationCents}
        value={draft.fair_value_max_midpoint_deviation_cents}
        suffix="c"
        hint={h.fairValueMaxMidpointDeviationCents}
        onChange={(value) => updateNumber("fair_value_max_midpoint_deviation_cents", value)}
      />
      <NumberInput
        label={dictionary.rewards.fairValueHistoryWindowSec}
        value={draft.fair_value_history_window_sec}
        suffix="s"
        hint={h.fairValueHistoryWindowSec}
        onChange={(value) => updateNumber("fair_value_history_window_sec", value)}
      />
      <NumberInput
        label={dictionary.rewards.fairValueMinHistorySamples}
        value={draft.fair_value_min_history_samples}
        hint={h.fairValueMinHistorySamples}
        onChange={(value) => updateNumber("fair_value_min_history_samples", value)}
      />
    </ConfigSection>
  );
}
