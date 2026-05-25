"use client";

import Link from "next/link";
import { usePathname } from "next/navigation";
import { Power } from "lucide-react";

import { useConsoleRealtimeChannel } from "@/components/shared/console-realtime-provider";
import { LanguageSwitcher } from "@/components/shared/language-switcher";
import { Button } from "@/components/ui/button";
import { StatusPill } from "@/components/shared/status-pill";
import { useI18n } from "@/lib/i18n/client";
import { normalizeOptionalRuntimeMode } from "@/lib/runtime-mode";
import { cn } from "@/lib/utils";

const topNavLinks = [
  { href: "/dashboard", labelKey: "dashboard" },
  { href: "/signals", labelKey: "signals" },
  { href: "/replay", labelKey: "replay" },
] as const;

export function ConsoleTopbar() {
  const pathname = usePathname();
  const { lastEvent } = useConsoleRealtimeChannel("risk");
  const { dictionary, enumLabel } = useI18n();
  const runtimeMode = normalizeOptionalRuntimeMode(lastEvent?.data.mode);
  const modeLabel = runtimeMode ? enumLabel(runtimeMode) : dictionary.topbar.runtimeSync;
  const environmentLabel = lastEvent?.data.environment ?? dictionary.topbar.streamSync;
  const warningCount = lastEvent?.data.warning_alerts;
  const criticalCount = lastEvent?.data.critical_alerts;
  const killSwitchActive = lastEvent?.data.kill_switch ?? false;

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
          <StatusPill tone={warningCount && warningCount > 0 ? "warning" : "neutral"}>
            {warningCount !== undefined ? `${warningCount} ${dictionary.common.warnings}` : dictionary.topbar.riskSync}
          </StatusPill>
          <StatusPill tone="neutral" className="hidden md:inline-flex">
            {criticalCount !== undefined ? `${criticalCount} ${dictionary.common.critical}` : dictionary.topbar.alertsSync}
          </StatusPill>
          <LanguageSwitcher />
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
        </div>
      </div>
    </header>
  );
}
