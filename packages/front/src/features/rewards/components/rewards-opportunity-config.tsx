"use client";

import type { Dispatch, SetStateAction } from "react";

import type { RewardBotConfigDto } from "@/lib/contracts/dto";
import { dictionary } from "@/lib/i18n/dictionaries";

import type { NumberConfigKey } from "../types";
import { NumberInput } from "./number-input";
import { ConfigSection, ToggleField } from "./rewards-config-fields";

type OpportunityConfigProps = {
  draft: RewardBotConfigDto;
  setDraft: Dispatch<SetStateAction<RewardBotConfigDto>>;
  updateNumber: (key: NumberConfigKey, value: string) => void;
};

export function OpportunityConfig({
  draft,
  setDraft,
  updateNumber,
}: OpportunityConfigProps) {
  const h = dictionary.rewards.configHints;

  return (
    <ConfigSection
      title={dictionary.rewards.configOpportunity}
      description={dictionary.rewards.configOpportunityDescription}
    >
      <ToggleField
        label={dictionary.rewards.opportunityMetricsEnabled}
        hint={h.opportunityMetricsEnabled}
        checked={draft.opportunity_metrics_enabled}
        onChange={(checked) =>
          setDraft((current) => ({
            ...current,
            opportunity_metrics_enabled: checked,
          }))
        }
      />
      <NumberInput
        label={dictionary.rewards.opportunityProbeNotionalUsd}
        value={draft.opportunity_probe_notional_usd}
        suffix="$"
        hint={h.opportunityProbeNotionalUsd}
        onChange={(value) => updateNumber("opportunity_probe_notional_usd", value)}
      />
      <NumberInput
        label={dictionary.rewards.opportunityMinRewardPer100UsdDay}
        value={draft.opportunity_min_reward_per_100_usd_day}
        suffix="$"
        hint={h.opportunityMinRewardPer100UsdDay}
        onChange={(value) => updateNumber("opportunity_min_reward_per_100_usd_day", value)}
      />
      <NumberInput
        label={dictionary.rewards.opportunityMaxCompetitionMultiple}
        value={draft.opportunity_max_competition_multiple}
        hint={h.opportunityMaxCompetitionMultiple}
        onChange={(value) => updateNumber("opportunity_max_competition_multiple", value)}
      />
      <ToggleField
        label={dictionary.rewards.opportunityCompetitionHardGateEnabled}
        hint={h.opportunityCompetitionHardGateEnabled}
        checked={draft.opportunity_competition_hard_gate_enabled}
        onChange={(checked) =>
          setDraft((current) => ({
            ...current,
            opportunity_competition_hard_gate_enabled: checked,
          }))
        }
      />
      <NumberInput
        label={dictionary.rewards.opportunityCompetitionHardGateMultiple}
        value={draft.opportunity_competition_hard_gate_multiple}
        hint={h.opportunityCompetitionHardGateMultiple}
        onChange={(value) => updateNumber("opportunity_competition_hard_gate_multiple", value)}
      />
      <NumberInput
        label={dictionary.rewards.opportunityMaxAccountAllocationBps}
        value={draft.opportunity_max_account_allocation_bps}
        suffix="bps"
        hint={h.opportunityMaxAccountAllocationBps}
        onChange={(value) => updateNumber("opportunity_max_account_allocation_bps", value)}
      />
      <NumberInput
        label={dictionary.rewards.opportunityMaxMarketAllocationBps}
        value={draft.opportunity_max_market_allocation_bps}
        suffix="bps"
        hint={h.opportunityMaxMarketAllocationBps}
        onChange={(value) => updateNumber("opportunity_max_market_allocation_bps", value)}
      />
      <NumberInput
        label={dictionary.rewards.opportunityMinExitDepthUsd}
        value={draft.opportunity_min_exit_depth_usd}
        suffix="$"
        hint={h.opportunityMinExitDepthUsd}
        onChange={(value) => updateNumber("opportunity_min_exit_depth_usd", value)}
      />
      <NumberInput
        label={dictionary.rewards.opportunityMinExitDepthMultiple}
        value={draft.opportunity_min_exit_depth_multiple}
        hint={h.opportunityMinExitDepthMultiple}
        onChange={(value) => updateNumber("opportunity_min_exit_depth_multiple", value)}
      />
      <NumberInput
        label={dictionary.rewards.opportunityMaxEntryExitSlippageCents}
        value={draft.opportunity_max_entry_exit_slippage_cents}
        suffix="c"
        hint={h.opportunityMaxEntryExitSlippageCents}
        onChange={(value) => updateNumber("opportunity_max_entry_exit_slippage_cents", value)}
      />
      <NumberInput
        label={dictionary.rewards.opportunityMaxBadFillRecoveryDays}
        value={draft.opportunity_max_bad_fill_recovery_days}
        suffix="d"
        hint={h.opportunityMaxBadFillRecoveryDays}
        onChange={(value) => updateNumber("opportunity_max_bad_fill_recovery_days", value)}
      />
      <NumberInput
        label={dictionary.rewards.opportunityObservationWindowSec}
        value={draft.opportunity_observation_window_sec}
        suffix="s"
        hint={h.opportunityObservationWindowSec}
        onChange={(value) => updateNumber("opportunity_observation_window_sec", value)}
      />
      <NumberInput
        label={dictionary.rewards.opportunityMinBookSamples}
        value={draft.opportunity_min_book_samples}
        hint={h.opportunityMinBookSamples}
        onChange={(value) => updateNumber("opportunity_min_book_samples", value)}
      />
      <NumberInput
        label={dictionary.rewards.opportunityMaxMidpointRangeCents}
        value={draft.opportunity_max_midpoint_range_cents}
        suffix="c"
        hint={h.opportunityMaxMidpointRangeCents}
        onChange={(value) => updateNumber("opportunity_max_midpoint_range_cents", value)}
      />
      <NumberInput
        label={dictionary.rewards.opportunityMaxTopOfBookFlipCount}
        value={draft.opportunity_max_top_of_book_flip_count}
        hint={h.opportunityMaxTopOfBookFlipCount}
        onChange={(value) => updateNumber("opportunity_max_top_of_book_flip_count", value)}
      />
      <NumberInput
        label={dictionary.rewards.opportunityRewardWeight}
        value={draft.opportunity_reward_weight}
        hint={h.opportunityRewardWeight}
        onChange={(value) => updateNumber("opportunity_reward_weight", value)}
      />
      <NumberInput
        label={dictionary.rewards.opportunityCompetitionWeight}
        value={draft.opportunity_competition_weight}
        hint={h.opportunityCompetitionWeight}
        onChange={(value) => updateNumber("opportunity_competition_weight", value)}
      />
      <NumberInput
        label={dictionary.rewards.opportunityExitWeight}
        value={draft.opportunity_exit_weight}
        hint={h.opportunityExitWeight}
        onChange={(value) => updateNumber("opportunity_exit_weight", value)}
      />
      <NumberInput
        label={dictionary.rewards.opportunityStabilityWeight}
        value={draft.opportunity_stability_weight}
        hint={h.opportunityStabilityWeight}
        onChange={(value) => updateNumber("opportunity_stability_weight", value)}
      />
    </ConfigSection>
  );
}
