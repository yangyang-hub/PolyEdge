"use client";

import Link from "next/link";
import { usePathname } from "next/navigation";
import { Bell, Power, Search } from "lucide-react";

import { useConsoleRealtimeChannel } from "@/components/shared/console-realtime-provider";
import { LanguageSwitcher } from "@/components/shared/language-switcher";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { StatusPill } from "@/components/shared/status-pill";
import { useI18n } from "@/lib/i18n/client";
import { cn } from "@/lib/utils";

const topNavLinks = [
  { href: "/dashboard", labelKey: "dashboard" },
  { href: "/signals", labelKey: "signals" },
  { href: "/approvals", labelKey: "approvals" },
  { href: "/replay", labelKey: "replay" },
] as const;

export function ConsoleTopbar() {
  const pathname = usePathname();
  const { lastEvent } = useConsoleRealtimeChannel("risk");
  const { dictionary, enumLabel } = useI18n();
  const modeLabel = lastEvent?.data.mode ? enumLabel(lastEvent.data.mode) : dictionary.topbar.runtimeSync;
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

          <div className="relative hidden w-full max-w-sm md:block">
            <Search className="pointer-events-none absolute left-3 top-1/2 size-4 -translate-y-1/2 text-muted-foreground" />
            <Input
              className="h-8 rounded-sm border-transparent bg-card/90 pl-10 text-xs text-foreground shadow-none ring-1 ring-white/5 placeholder:text-muted-foreground/70 focus-visible:border-transparent focus-visible:ring-2 focus-visible:ring-primary/20"
              placeholder={dictionary.topbar.searchPlaceholder}
            />
          </div>

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
          <Button
            size="icon"
            variant="ghost"
            className="text-muted-foreground hover:bg-accent hover:text-foreground"
          >
            <Bell className="size-4" />
          </Button>
          <div className="hidden items-center gap-3 border-l border-white/8 pl-4 md:flex">
            <div className="text-right">
              <p className="text-xs font-semibold uppercase tracking-wide text-foreground">{dictionary.topbar.userName}</p>
              <p className="font-mono text-[10px] uppercase tracking-[0.24em] text-muted-foreground">
                {dictionary.topbar.userRole}
              </p>
            </div>
            <div className="flex size-8 items-center justify-center rounded-full border border-primary/30 bg-card text-primary">
              JT
            </div>
          </div>
        </div>
      </div>
    </header>
  );
}
