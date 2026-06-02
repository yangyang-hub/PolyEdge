"use client";

import { StatusPill } from "@/components/shared/status-pill";
import { dictionary } from "@/lib/i18n/dictionaries";
import type { RadarOpportunityItem } from "@/features/radar/types";

export function OpportunityDetail({ opportunity }: { opportunity: RadarOpportunityItem | null }) {

  if (!opportunity) {
    return (
      <div className="rounded-md bg-popover/70 p-4">
        <p className="font-heading text-lg font-bold text-foreground">{dictionary.radar.noSelectionTitle}</p>
        <p className="mt-2 text-sm text-muted-foreground">{dictionary.radar.noSelectionDetail}</p>
      </div>
    );
  }

  return (
    <div className="space-y-4">
      <div className="space-y-2">
        <p className="font-heading text-lg font-bold tracking-tight text-foreground">
          {opportunity.marketQuestion}
        </p>
        <div className="flex flex-wrap gap-2">
          <StatusPill tone={opportunity.typeTone}>{opportunity.typeLabel}</StatusPill>
          <StatusPill tone={opportunity.statusTone}>{opportunity.statusLabel}</StatusPill>
          <StatusPill tone={opportunity.validationTone}>{opportunity.validationLabel}</StatusPill>
          <StatusPill tone={opportunity.candidateTone}>{opportunity.candidateLabel}</StatusPill>
          <StatusPill tone="primary">{opportunity.observedClock}</StatusPill>
        </div>
      </div>

      <div className="grid grid-cols-2 gap-3">
        <div className="rounded-md bg-accent/45 p-3">
          <p className="text-[10px] font-bold uppercase tracking-[0.18em] text-muted-foreground">{dictionary.radar.grossEdge}</p>
          <p className="mt-2 font-mono text-lg text-secondary">{opportunity.grossEdge}</p>
        </div>
        <div className="rounded-md bg-accent/45 p-3">
          <p className="text-[10px] font-bold uppercase tracking-[0.18em] text-muted-foreground">{dictionary.radar.priceSum}</p>
          <p className="mt-2 font-mono text-lg text-foreground">{opportunity.priceSum}</p>
        </div>
        <div className="rounded-md bg-accent/45 p-3">
          <p className="text-[10px] font-bold uppercase tracking-[0.18em] text-muted-foreground">{dictionary.radar.yesPrice}</p>
          <p className="mt-2 font-mono text-lg text-primary">{opportunity.yesPrice}</p>
        </div>
        <div className="rounded-md bg-accent/45 p-3">
          <p className="text-[10px] font-bold uppercase tracking-[0.18em] text-muted-foreground">{dictionary.radar.noPrice}</p>
          <p className="mt-2 font-mono text-lg text-primary">{opportunity.noPrice}</p>
        </div>
        <div className="rounded-md bg-accent/45 p-3">
          <p className="text-[10px] font-bold uppercase tracking-[0.18em] text-muted-foreground">{dictionary.radar.yesSize}</p>
          <p className="mt-2 font-mono text-lg text-foreground">{opportunity.yesSize}</p>
        </div>
        <div className="rounded-md bg-accent/45 p-3">
          <p className="text-[10px] font-bold uppercase tracking-[0.18em] text-muted-foreground">{dictionary.radar.noSize}</p>
          <p className="mt-2 font-mono text-lg text-foreground">{opportunity.noSize}</p>
        </div>
      </div>

      <div className="rounded-md bg-popover/70 p-4">
        <p className="text-[10px] font-bold uppercase tracking-[0.18em] text-muted-foreground">
          {dictionary.radar.validation}
        </p>
        <div className="mt-3 grid grid-cols-2 gap-3">
          <div>
            <p className="text-[10px] uppercase text-muted-foreground">{dictionary.radar.netEdge}</p>
            <p className="mt-1 font-mono text-sm text-foreground">{opportunity.netEdge}</p>
          </div>
          <div>
            <p className="text-[10px] uppercase text-muted-foreground">{dictionary.radar.capacity}</p>
            <p className="mt-1 font-mono text-sm text-foreground">{opportunity.validatedCapacity}</p>
          </div>
          <div>
            <p className="text-[10px] uppercase text-muted-foreground">{dictionary.radar.feeBuffer}</p>
            <p className="mt-1 font-mono text-sm text-foreground">{opportunity.feeEstimate}</p>
          </div>
          <div>
            <p className="text-[10px] uppercase text-muted-foreground">{dictionary.radar.bookAge}</p>
            <p className="mt-1 font-mono text-sm text-foreground">{opportunity.bookAge}</p>
          </div>
        </div>
        {opportunity.validationReasonCodes.length > 0 ? (
          <div className="mt-3 flex flex-wrap gap-2">
            {opportunity.validationReasonCodes.map((reason) => (
              <StatusPill key={reason} tone={opportunity.validationTone}>
                {reason}
              </StatusPill>
            ))}
          </div>
        ) : null}
      </div>

      <div className="rounded-md bg-popover/70 p-4">
        <p className="text-[10px] font-bold uppercase tracking-[0.18em] text-muted-foreground">
          {dictionary.radar.candidatePreview}
        </p>
        <div className="mt-3 flex items-center justify-between gap-3">
          <StatusPill tone={opportunity.candidateTone}>{opportunity.candidateLabel}</StatusPill>
          <p className="text-right text-xs text-muted-foreground">{opportunity.candidateReason}</p>
        </div>
      </div>

      <div className="rounded-md bg-popover/70 p-4">
        <p className="text-[10px] font-bold uppercase tracking-[0.18em] text-muted-foreground">
          {dictionary.radar.detectionFormula}
        </p>
        <p className="mt-3 font-mono text-sm text-foreground">{opportunity.formula}</p>
      </div>

      <div className="rounded-md bg-popover/70 p-4">
        <p className="text-[10px] font-bold uppercase tracking-[0.18em] text-muted-foreground">
          {dictionary.radar.reasonCodes}
        </p>
        <div className="mt-3 flex flex-wrap gap-2">
          {opportunity.reasonCodes.map((reason) => (
            <StatusPill key={reason} tone="neutral">
              {reason}
            </StatusPill>
          ))}
        </div>
      </div>
    </div>
  );
}
