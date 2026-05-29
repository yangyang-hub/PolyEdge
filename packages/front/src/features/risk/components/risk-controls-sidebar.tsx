"use client";

import { Button } from "@/components/ui/button";
import { MeterBar } from "@/components/shared/meter-bar";
import { useI18n } from "@/lib/i18n/client";

import type { RiskPageData } from "../types";

export function RiskControlsSidebar({
  killSwitchAvailable,
  killSwitch,
  onTriggerKillSwitch,
  riskBuckets,
}: {
  killSwitchAvailable: boolean;
  killSwitch: boolean;
  onTriggerKillSwitch: () => void;
  riskBuckets: RiskPageData["riskBuckets"];
}) {
  const { dictionary } = useI18n();

  return (
    <div className="space-y-4">
      {killSwitchAvailable ? (
        <div className="rounded-lg bg-card/95 p-5 ring-1 ring-white/5">
          <p className="font-heading text-sm font-bold uppercase tracking-[0.18em] text-foreground">
            {dictionary.risk.globalControls}
          </p>
          <div className="mt-4 space-y-3">
            <div className="rounded-md bg-accent/45 p-4 text-sm text-muted-foreground">
              {dictionary.risk.globalControlsDetail}
            </div>
            <Button
              variant="outline"
              className="h-9 w-full rounded-sm border-white/10 bg-accent/40 text-foreground hover:bg-accent"
              onClick={onTriggerKillSwitch}
            >
              {killSwitch ? dictionary.risk.releaseKillSwitch : dictionary.risk.triggerKillSwitch}
            </Button>
          </div>
        </div>
      ) : null}

      <div className="rounded-lg bg-card/95 p-5 ring-1 ring-white/5">
        <p className="font-heading text-sm font-bold uppercase tracking-[0.18em] text-foreground">
          {dictionary.risk.riskBuckets}
        </p>
        <div className="mt-4 space-y-4">
          {riskBuckets.map((bucket, index) => (
            <div key={bucket.id} className="space-y-2">
              <div className="flex items-center justify-between gap-3">
                <p className="text-sm font-medium text-foreground">{bucket.name}</p>
                <span className="font-mono text-xs text-muted-foreground">{bucket.exposure}</span>
              </div>
              <MeterBar
                value={bucket.width}
                tone={index === 2 ? "danger" : index === 1 ? "warning" : "primary"}
                trackClassName="h-2 bg-background"
              />
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}
