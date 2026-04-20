"use client";

import { StatusPill } from "@/components/shared/status-pill";
import { useLiveStatus } from "@/hooks/use-live-status";

export function ConsoleStatusRail() {
  const { badges } = useLiveStatus();

  return (
    <div className="fixed inset-x-0 bottom-0 z-30 bg-sidebar/95 backdrop-blur md:left-16">
      <div className="flex h-8 flex-wrap items-center gap-2 px-4 text-[10px] md:px-6">
        {badges.map((badge) => (
          <StatusPill key={badge.label} tone={badge.tone}>
            {badge.label}
          </StatusPill>
        ))}
      </div>
    </div>
  );
}
