"use client";

import type { Dispatch, SetStateAction } from "react";

import type {
  RewardBotConfigDto,
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
        label={dictionary.rewards.lowCompetitionMaxMidpointRangeCents}
        value={draft.low_competition_max_midpoint_range_cents}
        suffix="c"
        hint={h.lowCompetitionMaxMidpointRangeCents}
        onChange={(value) => updateNumber("low_competition_max_midpoint_range_cents", value)}
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
    </ConfigSection>
  );
}
