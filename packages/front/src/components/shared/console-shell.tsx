"use client";

import { useEffect, useState, type ReactNode } from "react";

import { ConsoleRealtimeProvider } from "@/components/shared/console-realtime-provider";
import { ConsoleSidebar } from "@/components/shared/console-sidebar";
import { ConsoleStatusRail } from "@/components/shared/console-status-rail";
import { ConsoleTopbar } from "@/components/shared/console-topbar";
import type { RuntimeMode } from "@/lib/contracts/dto";
import { normalizeRuntimeMode } from "@/lib/runtime-mode";
import { readRiskState } from "@/lib/api/risk";

async function getShellRuntimeState(): Promise<{
  mode: RuntimeMode;
  environment: string;
  killSwitch: boolean;
} | null> {
  try {
    const { data } = await readRiskState();

    return {
      mode: normalizeRuntimeMode(data.mode),
      environment: data.environment,
      killSwitch: data.kill_switch,
    };
  } catch {
    return null;
  }
}

export function ConsoleShell({ children }: { children: ReactNode }) {
  const [runtimeState, setRuntimeState] = useState<{
    mode: RuntimeMode;
    environment: string;
    killSwitch: boolean;
  } | null>(null);

  useEffect(() => {
    let cancelled = false;

    void getShellRuntimeState().then((nextState) => {
      if (!cancelled) {
        setRuntimeState(nextState);
      }
    });

    return () => {
      cancelled = true;
    };
  }, []);

  return (
    <div className="min-h-screen bg-background text-foreground">
      <ConsoleSidebar />
      <div className="md:pl-16">
        <ConsoleRealtimeProvider>
          <ConsoleTopbar
            initialEnvironment={runtimeState?.environment ?? null}
            initialKillSwitch={runtimeState?.killSwitch ?? null}
            initialMode={runtimeState?.mode ?? null}
          />
          <main className="px-4 pb-12 pt-[4.5rem] md:px-6">{children}</main>
          <ConsoleStatusRail />
        </ConsoleRealtimeProvider>
      </div>
    </div>
  );
}
