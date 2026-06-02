"use client";

import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import type { RewardBotConfigDto } from "@/lib/contracts/dto";
import { dictionary } from "@/lib/i18n/dictionaries";

import type { NumberConfigKey } from "../types";
import { NumberInput } from "./number-input";

interface Props {
  draft: RewardBotConfigDto;
  updateNumber: (key: NumberConfigKey, value: string) => void;
}

export function RiskControlConfig({ draft, updateNumber }: Props) {
  const h = dictionary.rewards.configHints;

  return (
    <Card>
      <CardHeader className="border-b border-border/70">
        <CardTitle className="font-heading text-base">
          {dictionary.rewards.riskControl}
        </CardTitle>
      </CardHeader>
      <CardContent className="space-y-5">
        {/* Group 1: Depth & position checks */}
        <div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-6">
          <NumberInput
            label={dictionary.rewards.minDepthUsd}
            value={draft.min_depth_usd}
            suffix="$"
            hint={h.minDepthUsd}
            onChange={(v) => updateNumber("min_depth_usd", v)}
          />
          <NumberInput
            label={dictionary.rewards.cancelBidRank}
            value={draft.cancel_bid_rank}
            hint={h.cancelBidRank}
            onChange={(v) => updateNumber("cancel_bid_rank", v)}
          />
          <NumberInput
            label={dictionary.rewards.depthDropPct}
            value={draft.depth_drop_pct}
            suffix="%"
            hint={h.depthDropPct}
            onChange={(v) => updateNumber("depth_drop_pct", v)}
          />
          <NumberInput
            label={dictionary.rewards.depthDropWindow}
            value={draft.depth_drop_window_sec}
            suffix="s"
            hint={h.depthDropWindow}
            onChange={(v) => updateNumber("depth_drop_window_sec", v)}
          />
          <NumberInput
            label={dictionary.rewards.fillVelocityUsd}
            value={draft.fill_velocity_usd}
            suffix="$"
            hint={h.fillVelocityUsd}
            onChange={(v) => updateNumber("fill_velocity_usd", v)}
          />
          <NumberInput
            label={dictionary.rewards.fillVelocityWindow}
            value={draft.fill_velocity_window_sec}
            suffix="s"
            hint={h.fillVelocityWindow}
            onChange={(v) => updateNumber("fill_velocity_window_sec", v)}
          />
        </div>

        {/* Group 2: Mass cancel & requote */}
        <div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-6">
          <NumberInput
            label={dictionary.rewards.massCancelPct}
            value={draft.mass_cancel_pct}
            suffix="%"
            hint={h.massCancelPct}
            onChange={(v) => updateNumber("mass_cancel_pct", v)}
          />
          <NumberInput
            label={dictionary.rewards.massCancelWindow}
            value={draft.mass_cancel_window_sec}
            suffix="s"
            hint={h.massCancelWindow}
            onChange={(v) => updateNumber("mass_cancel_window_sec", v)}
          />
          <NumberInput
            label={dictionary.rewards.requoteInterval}
            value={draft.requote_interval_sec}
            suffix="s"
            hint={h.requoteInterval}
            onChange={(v) => updateNumber("requote_interval_sec", v)}
          />
          <NumberInput
            label={dictionary.rewards.requoteJitter}
            value={draft.requote_jitter_sec}
            suffix="s"
            hint={h.requoteJitter}
            onChange={(v) => updateNumber("requote_jitter_sec", v)}
          />
          <NumberInput
            label={dictionary.rewards.reconcileInterval}
            value={draft.reconcile_interval_sec}
            suffix="s"
            hint={h.reconcileInterval}
            onChange={(v) => updateNumber("reconcile_interval_sec", v)}
          />
        </div>
      </CardContent>
    </Card>
  );
}
