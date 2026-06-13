"use client";

import Link from "next/link";
import { usePathname } from "next/navigation";
import { Power } from "lucide-react";
import { useState } from "react";

import { Button } from "@/components/ui/button";
import { StatusPill } from "@/components/shared/status-pill";
import type { RuntimeMode } from "@/lib/contracts/dto";
import { dictionary, translateEnum } from "@/lib/i18n/dictionaries";
import { cn } from "@/lib/utils";

const topNavLinks = [
  { href: "/dashboard", labelKey: "dashboard" },
  { href: "/signals", labelKey: "signals" },
  { href: "/rewards", labelKey: "rewards" },
] as const;

export function ConsoleTopbar({
  initialEnvironment,
  initialKillSwitch,
  initialMode,
}: {
  initialEnvironment: string | null;
  initialKillSwitch: boolean | null;
  initialMode: RuntimeMode | null;
}) {
  const pathname = usePathname();
  const [runtimeMode] = useState<RuntimeMode | null>(initialMode);
  const [environment] = useState<string | null>(initialEnvironment);
  const [killSwitch] = useState(initialKillSwitch ?? false);
  const modeLabel = runtimeMode ? translateEnum(runtimeMode) : dictionary.topbar.runtimeSync;
  const environmentLabel = environment ?? dictionary.topbar.streamSync;
  const killSwitchActive = killSwitch;
  const killSwitchAvailable =
    runtimeMode === "live_auto" || runtimeMode === "kill_switch_locked" || killSwitchActive;

  return (
    <header className="fixed inset-x-0 top-0 z-30 bg-background/95 backdrop-blur md:left-16">
      <div className="flex h-14 items-center justify-between gap-6 px-4 md:px-6">
        <div className="flex min-w-0 flex-1 items-center gap-6">
          <p className="hidden font-heading text-lg font-black tracking-tight text-primary md:block">
            {dictionary.topbar.title}
          </p>

          <nav className="hidden items-center gap-4 xl:flex">
            {topNavLinks.map((item) => (
              <Link
                key={item.href}
                href={item.href}
                className={cn(
                  "flex h-14 items-center border-b-2 text-sm font-medium transition-colors",
                  pathname === item.href || pathname.startsWith(`${item.href}/`)
                    ? "border-primary text-primary"
                    : "border-transparent text-muted-foreground hover:text-foreground",
                )}
              >
                {dictionary.nav[item.labelKey]}
              </Link>
            ))}
          </nav>
        </div>

        <div className="flex items-center gap-2 md:gap-3">
          <div className="hidden items-center gap-2 xl:flex">
            <StatusPill tone={killSwitchActive ? "danger" : "warning"}>{modeLabel}</StatusPill>
            <StatusPill tone="primary">{environmentLabel}</StatusPill>
          </div>
          {killSwitchAvailable ? (
            <Button
              asChild
              size="sm"
              className={
                killSwitchActive
                  ? "rounded-sm bg-destructive text-destructive-foreground shadow-[0_0_18px_rgba(255,180,171,0.24)] hover:bg-destructive/90"
                  : "rounded-sm bg-destructive/85 text-destructive-foreground shadow-[0_0_18px_rgba(255,180,171,0.12)] hover:bg-destructive hover:shadow-[0_0_18px_rgba(255,180,171,0.32)]"
              }
            >
              <Link href="/risk">
                <Power className="size-4" />
                {killSwitchActive ? dictionary.topbar.killSwitchActive : dictionary.topbar.killSwitch}
              </Link>
            </Button>
          ) : null}
        </div>
      </div>
    </header>
  );
}
