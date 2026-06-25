"use client";

import type { Dispatch, SetStateAction } from "react";

import type {
  RewardBotConfigDto,
  RewardInfoRiskLevel,
  RewardLowCompetitionMode,
} from "@/lib/contracts/dto";
import { dictionary } from "@/lib/i18n/dictionaries";

import type { NumberConfigKey } from "../types";
import { NumberInput } from "./number-input";
import { ConfigSection, Hint, selectClassName } from "./rewards-config-fields";

type LowCompetitionConfigProps = {
  draft: RewardBotConfigDto;
  setDraft: Dispatch<SetStateAction<RewardBotConfigDto>>;
  updateNumber: (key: NumberConfigKey, value: string) => void;
};

export function LowCompetitionConfig({
  draft,
  setDraft,
  updateNumber,
}: LowCompetitionConfigProps) {
  const h = dictionary.rewards.configHints;

  return (
    <ConfigSection
      title={dictionary.rewards.configLowCompetition}
      description={dictionary.rewards.configLowCompetitionDescription}
    >
      <label className="space-y-1.5">
        <span className="flex items-center gap-1 text-xs font-medium text-muted-foreground">
          {dictionary.rewards.lowCompetitionMode}
          <Hint content={h.lowCompetitionMode} />
        </span>
        <select
          className={selectClassName}
          value={draft.low_competition_mode}
          onChange={(event) =>
            setDraft((current) => ({
              ...current,
              low_competition_mode: event.target.value as RewardLowCompetitionMode,
            }))
          }
        >
          <option value="off">{dictionary.rewards.lowCompetitionOff}</option>
          <option value="observe">{dictionary.rewards.selectionObserve}</option>
          <option value="enforce">{dictionary.rewards.selectionEnforce}</option>
        </select>
      </label>

      <NumberInput
        label={dictionary.rewards.lowCompetitionMaxMarkets}
        value={draft.low_competition_max_markets}
        hint={h.lowCompetitionMaxMarkets}
        onChange={(value) => updateNumber("low_competition_max_markets", value)}
      />
      <NumberInput
        label={dictionary.rewards.lowCompetitionMaxOpenOrders}
        value={draft.low_competition_max_open_orders}
        hint={h.lowCompetitionMaxOpenOrders}
        onChange={(value) => updateNumber("low_competition_max_open_orders", value)}
      />
      <NumberInput
        label={dictionary.rewards.lowCompetitionMaxPositionUsd}
        value={draft.low_competition_max_position_usd}
        suffix="$"
        hint={h.lowCompetitionMaxPositionUsd}
        onChange={(value) => updateNumber("low_competition_max_position_usd", value)}
      />
      <NumberInput
        label={dictionary.rewards.lowCompetitionProbeNotionalUsd}
        value={draft.low_competition_probe_notional_usd}
        suffix="$"
        hint={h.lowCompetitionProbeNotionalUsd}
        onChange={(value) => updateNumber("low_competition_probe_notional_usd", value)}
      />
      <NumberInput
        label={dictionary.rewards.lowCompetitionMinCompetitionShareBps}
        value={draft.low_competition_min_competition_share_bps}
        suffix="bps"
        hint={h.lowCompetitionMinCompetitionShareBps}
        onChange={(value) => updateNumber("low_competition_min_competition_share_bps", value)}
      />
      <NumberInput
        label={dictionary.rewards.lowCompetitionMaxCompetitionMultiple}
        value={draft.low_competition_max_competition_multiple}
        hint={h.lowCompetitionMaxCompetitionMultiple}
        onChange={(value) => updateNumber("low_competition_max_competition_multiple", value)}
      />
      <NumberInput
        label={dictionary.rewards.lowCompetitionCandidateMaxCompetitionMultiple}
        value={draft.low_competition_candidate_max_competition_multiple}
        hint={h.lowCompetitionCandidateMaxCompetitionMultiple}
        onChange={(value) =>
          updateNumber("low_competition_candidate_max_competition_multiple", value)
        }
      />
      <NumberInput
        label={dictionary.rewards.lowCompetitionMaxAccountAllocationBps}
        value={draft.low_competition_max_account_allocation_bps}
        suffix="bps"
        hint={h.lowCompetitionMaxAccountAllocationBps}
        onChange={(value) => updateNumber("low_competition_max_account_allocation_bps", value)}
      />
      <NumberInput
        label={dictionary.rewards.lowCompetitionMaxMarketAllocationBps}
        value={draft.low_competition_max_market_allocation_bps}
        suffix="bps"
        hint={h.lowCompetitionMaxMarketAllocationBps}
        onChange={(value) => updateNumber("low_competition_max_market_allocation_bps", value)}
      />
      <NumberInput
        label={dictionary.rewards.lowCompetitionMaxCompetitionUsd}
        value={draft.low_competition_max_competition_usd}
        suffix="$"
        hint={h.lowCompetitionMaxCompetitionUsd}
        onChange={(value) => updateNumber("low_competition_max_competition_usd", value)}
      />
      <NumberInput
        label={dictionary.rewards.lowCompetitionMinRewardPer100UsdDay}
        value={draft.low_competition_min_reward_per_100_usd_day}
        suffix="$"
        hint={h.lowCompetitionMinRewardPer100UsdDay}
        onChange={(value) => updateNumber("low_competition_min_reward_per_100_usd_day", value)}
      />
      <NumberInput
        label={dictionary.rewards.lowCompetitionMinExitDepthUsd}
        value={draft.low_competition_min_exit_depth_usd}
        suffix="$"
        hint={h.lowCompetitionMinExitDepthUsd}
        onChange={(value) => updateNumber("low_competition_min_exit_depth_usd", value)}
      />
      <NumberInput
        label={dictionary.rewards.lowCompetitionMinExitDepthMultiple}
        value={draft.low_competition_min_exit_depth_multiple}
        hint={h.lowCompetitionMinExitDepthMultiple}
        onChange={(value) => updateNumber("low_competition_min_exit_depth_multiple", value)}
      />
      <NumberInput
        label={dictionary.rewards.lowCompetitionMaxEntryExitSlippageCents}
        value={draft.low_competition_max_entry_exit_slippage_cents}
        suffix="c"
        hint={h.lowCompetitionMaxEntryExitSlippageCents}
        onChange={(value) =>
          updateNumber("low_competition_max_entry_exit_slippage_cents", value)
        }
      />
      <NumberInput
        label={dictionary.rewards.lowCompetitionMaxBadFillRecoveryDays}
        value={draft.low_competition_max_bad_fill_recovery_days}
        suffix="d"
        hint={h.lowCompetitionMaxBadFillRecoveryDays}
        onChange={(value) => updateNumber("low_competition_max_bad_fill_recovery_days", value)}
      />
      <NumberInput
        label={dictionary.rewards.lowCompetitionMaxMidpointRangeCents}
        value={draft.low_competition_max_midpoint_range_cents}
        suffix="c"
        hint={h.lowCompetitionMaxMidpointRangeCents}
        onChange={(value) => updateNumber("low_competition_max_midpoint_range_cents", value)}
      />
      <NumberInput
        label={dictionary.rewards.lowCompetitionMaxTopOfBookFlipCount}
        value={draft.low_competition_max_top_of_book_flip_count}
        hint={h.lowCompetitionMaxTopOfBookFlipCount}
        onChange={(value) => updateNumber("low_competition_max_top_of_book_flip_count", value)}
      />
      <NumberInput
        label={dictionary.rewards.lowCompetitionObservationWindowSec}
        value={draft.low_competition_observation_window_sec}
        suffix="s"
        hint={h.lowCompetitionObservationWindowSec}
        onChange={(value) => updateNumber("low_competition_observation_window_sec", value)}
      />
      <NumberInput
        label={dictionary.rewards.lowCompetitionMinBookSamples}
        value={draft.low_competition_min_book_samples}
        hint={h.lowCompetitionMinBookSamples}
        onChange={(value) => updateNumber("low_competition_min_book_samples", value)}
      />
      <label className="space-y-1.5">
        <span className="flex items-center gap-1 text-xs font-medium text-muted-foreground">
          {dictionary.rewards.lowCompetitionQuoteBidRank}
          <Hint content={h.lowCompetitionQuoteBidRank} />
        </span>
        <select
          className={selectClassName}
          value={draft.low_competition_quote_bid_rank}
          onChange={(event) =>
            updateNumber("low_competition_quote_bid_rank", event.target.value)
          }
        >
          <option value={1}>{dictionary.rewards.bidRank1}</option>
          <option value={2}>{dictionary.rewards.bidRank2}</option>
          <option value={3}>{dictionary.rewards.bidRank3}</option>
        </select>
      </label>
      <NumberInput
        label={dictionary.rewards.lowCompetitionSafetyMarginCents}
        value={draft.low_competition_safety_margin_cents}
        suffix="c"
        hint={h.lowCompetitionSafetyMarginCents}
        onChange={(value) => updateNumber("low_competition_safety_margin_cents", value)}
      />
      <NumberInput
        label={dictionary.rewards.lowCompetitionMaxSpreadCents}
        value={draft.low_competition_max_spread_cents}
        suffix="c"
        hint={h.lowCompetitionMaxSpreadCents}
        onChange={(value) => updateNumber("low_competition_max_spread_cents", value)}
      />
      <NumberInput
        label={dictionary.rewards.lowCompetitionMaxMarketSpreadCents}
        value={draft.low_competition_max_market_spread_cents}
        suffix="c"
        hint={h.lowCompetitionMaxMarketSpreadCents}
        onChange={(value) => updateNumber("low_competition_max_market_spread_cents", value)}
      />
      <NumberInput
        label={dictionary.rewards.lowCompetitionMinMarketScore}
        value={draft.low_competition_min_market_score}
        hint={h.lowCompetitionMinMarketScore}
        onChange={(value) => updateNumber("low_competition_min_market_score", value)}
      />
      <label className="flex items-start gap-2 rounded-lg border border-border/70 bg-muted/20 p-3">
        <input
          type="checkbox"
          className="mt-0.5 size-4 rounded border-border"
          checked={draft.low_competition_require_ai_allow}
          onChange={(event) =>
            setDraft((current) => ({
              ...current,
              low_competition_require_ai_allow: event.target.checked,
            }))
          }
        />
        <span className="space-y-1 text-xs text-muted-foreground">
          <span className="block font-medium text-foreground">
            {dictionary.rewards.lowCompetitionRequireAiAllow}
          </span>
          <span>{h.lowCompetitionRequireAiAllow}</span>
        </span>
      </label>
      <label className="space-y-1.5">
        <span className="flex items-center gap-1 text-xs font-medium text-muted-foreground">
          {dictionary.rewards.lowCompetitionInfoRiskAvoidLevel}
          <Hint content={h.lowCompetitionInfoRiskAvoidLevel} />
        </span>
        <select
          className={selectClassName}
          value={draft.low_competition_info_risk_avoid_level}
          onChange={(event) =>
            setDraft((current) => ({
              ...current,
              low_competition_info_risk_avoid_level: event.target.value as RewardInfoRiskLevel,
            }))
          }
        >
          <option value="low">{dictionary.rewards.infoRiskLow}</option>
          <option value="medium">{dictionary.rewards.infoRiskMedium}</option>
          <option value="high">{dictionary.rewards.infoRiskHigh}</option>
          <option value="critical">{dictionary.rewards.infoRiskCritical}</option>
          <option value="unknown">{dictionary.rewards.infoRiskUnknown}</option>
        </select>
      </label>
      <NumberInput
        label={dictionary.rewards.lowCompetitionCancelConfirmSec}
        value={draft.low_competition_cancel_confirm_sec}
        suffix="s"
        hint={h.lowCompetitionCancelConfirmSec}
        onChange={(value) => updateNumber("low_competition_cancel_confirm_sec", value)}
      />
      <NumberInput
        label={dictionary.rewards.lowCompetitionCancelShareThresholdRatioBps}
        value={draft.low_competition_cancel_share_threshold_ratio_bps}
        suffix="bps"
        hint={h.lowCompetitionCancelShareThresholdRatioBps}
        onChange={(value) =>
          updateNumber("low_competition_cancel_share_threshold_ratio_bps", value)
        }
      />
      <NumberInput
        label={dictionary.rewards.lowCompetitionCancelCompetitionMultipleFactor}
        value={draft.low_competition_cancel_competition_multiple_factor}
        hint={h.lowCompetitionCancelCompetitionMultipleFactor}
        onChange={(value) =>
          updateNumber("low_competition_cancel_competition_multiple_factor", value)
        }
      />
      <NumberInput
        label={dictionary.rewards.lowCompetitionCancelMaxExitSlippageCents}
        value={draft.low_competition_cancel_max_exit_slippage_cents}
        suffix="c"
        hint={h.lowCompetitionCancelMaxExitSlippageCents}
        onChange={(value) =>
          updateNumber("low_competition_cancel_max_exit_slippage_cents", value)
        }
      />
      <NumberInput
        label={dictionary.rewards.lowCompetitionCancelMinExitDepthUsd}
        value={draft.low_competition_cancel_min_exit_depth_usd}
        suffix="$"
        hint={h.lowCompetitionCancelMinExitDepthUsd}
        onChange={(value) => updateNumber("low_competition_cancel_min_exit_depth_usd", value)}
      />
      <NumberInput
        label={dictionary.rewards.lowCompetitionCancelExitDepthMultiple}
        value={draft.low_competition_cancel_exit_depth_multiple}
        hint={h.lowCompetitionCancelExitDepthMultiple}
        onChange={(value) => updateNumber("low_competition_cancel_exit_depth_multiple", value)}
      />
      <NumberInput
        label={dictionary.rewards.lowCompetitionCancelMidpointRangeFloorCents}
        value={draft.low_competition_cancel_midpoint_range_floor_cents}
        suffix="c"
        hint={h.lowCompetitionCancelMidpointRangeFloorCents}
        onChange={(value) =>
          updateNumber("low_competition_cancel_midpoint_range_floor_cents", value)
        }
      />
      <NumberInput
        label={dictionary.rewards.lowCompetitionGlobalOpenOrderShareBps}
        value={draft.low_competition_global_open_order_share_bps}
        suffix="bps"
        hint={h.lowCompetitionGlobalOpenOrderShareBps}
        onChange={(value) => updateNumber("low_competition_global_open_order_share_bps", value)}
      />
    </ConfigSection>
  );
}
