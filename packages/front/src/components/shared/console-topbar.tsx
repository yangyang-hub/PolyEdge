"use client";

import Link from "next/link";
import { usePathname } from "next/navigation";
import { Power, ToggleLeft, ToggleRight } from "lucide-react";
import { startTransition, useEffect, useState, useTransition } from "react";
import { toast } from "sonner";

import { ActionDialog } from "@/components/shared/action-dialog";
import { useConsoleRealtimeChannel } from "@/components/shared/console-realtime-provider";
import { LanguageSwitcher } from "@/components/shared/language-switcher";
import { Button } from "@/components/ui/button";
import { StatusPill } from "@/components/shared/status-pill";
import type { RuntimeMode } from "@/lib/contracts/dto";
import { useI18n } from "@/lib/i18n/client";
import { normalizeOptionalRuntimeMode } from "@/lib/runtime-mode";
import { requestModeSwitchAction } from "@/lib/api/actions";
import type { OperationActionResult } from "@/lib/api/actions";
import { cn } from "@/lib/utils";

const topNavLinks = [
  { href: "/dashboard", labelKey: "dashboard" },
  { href: "/signals", labelKey: "signals" },
  { href: "/replay", labelKey: "replay" },
] as const;

function nextGlobalRuntimeMode(mode: RuntimeMode | null): RuntimeMode {
  return mode === "live_auto" ? "paper_trade" : "live_auto";
}

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
  const { lastEvent } = useConsoleRealtimeChannel("risk");
  const { dictionary, enumLabel } = useI18n();
  const [runtimeMode, setRuntimeMode] = useState<RuntimeMode | null>(initialMode);
  const [environment, setEnvironment] = useState<string | null>(initialEnvironment);
  const [killSwitch, setKillSwitch] = useState(initialKillSwitch ?? false);
  const [dialogOpen, setDialogOpen] = useState(false);
  const [note, setNote] = useState("");
  const [stepUpCode, setStepUpCode] = useState("");
  const [fieldErrors, setFieldErrors] = useState<OperationActionResult["fieldErrors"]>({});
  const [dialogFeedback, setDialogFeedback] = useState<OperationActionResult | null>(null);
  const [isPending, startActionTransition] = useTransition();
  const targetMode = nextGlobalRuntimeMode(runtimeMode);
  const targetModeLabel = targetMode === "live_auto"
    ? dictionary.topbar.runtimeLive
    : dictionary.topbar.runtimeSimulation;
  const switchLabel = targetMode === "live_auto"
    ? dictionary.topbar.switchToLive
    : dictionary.topbar.switchToSimulation;
  const modeLabel = runtimeMode ? enumLabel(runtimeMode) : dictionary.topbar.runtimeSync;
  const environmentLabel = environment ?? dictionary.topbar.streamSync;
  const warningCount = lastEvent?.data.warning_alerts;
  const criticalCount = lastEvent?.data.critical_alerts;
  const killSwitchActive = killSwitch;
  const killSwitchAvailable =
    runtimeMode === "live_auto" || runtimeMode === "kill_switch_locked" || killSwitchActive;

  useEffect(() => {
    const nextMode = normalizeOptionalRuntimeMode(lastEvent?.data.mode);
    const nextEnvironment = lastEvent?.data.environment;
    const nextKillSwitch = lastEvent?.data.kill_switch;

    if (!nextMode && !nextEnvironment && typeof nextKillSwitch !== "boolean") {
      return;
    }

    startTransition(() => {
      if (nextMode) {
        setRuntimeMode(nextMode);
      }

      if (nextEnvironment) {
        setEnvironment(nextEnvironment);
      }

      if (typeof nextKillSwitch === "boolean") {
        setKillSwitch(nextKillSwitch);
      }
    });
  }, [lastEvent]);

  function openRuntimeDialog() {
    setFieldErrors({});
    setDialogFeedback(null);
    setStepUpCode("");
    setNote(
      targetMode === "live_auto"
        ? dictionary.topbar.runtimeSwitchLiveNote
        : dictionary.topbar.runtimeSwitchSimulationNote,
    );
    setDialogOpen(true);
  }

  function closeRuntimeDialog() {
    setDialogOpen(false);
    setFieldErrors({});
    setDialogFeedback(null);
    setStepUpCode("");
  }

  function submitRuntimeSwitch() {
    if (!runtimeMode) {
      return;
    }

    startActionTransition(async () => {
      const result = await requestModeSwitchAction({
        currentMode: runtimeMode,
        targetMode,
        note,
        stepUpCode,
      });

      setFieldErrors(result.fieldErrors ?? {});
      setDialogFeedback(result);

      if (result.ok) {
        setRuntimeMode(targetMode);
        toast.success(result.message, {
          description: [result.requestId, result.traceId].filter(Boolean).join(" · "),
        });
        closeRuntimeDialog();
        return;
      }

      toast.error(result.message, {
        description: [result.requestId, result.traceId].filter(Boolean).join(" · "),
      });
    });
  }

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
            size="sm"
            variant="outline"
            className={
              targetMode === "live_auto"
                ? "rounded-sm border-primary/35 bg-primary/10 text-primary hover:bg-primary/15"
                : "rounded-sm border-white/10 bg-accent/40 text-foreground hover:bg-accent"
            }
            disabled={!runtimeMode || killSwitchActive || isPending}
            onClick={openRuntimeDialog}
          >
            {targetMode === "live_auto" ? <ToggleRight className="size-4" /> : <ToggleLeft className="size-4" />}
            {switchLabel}
          </Button>
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

      <ActionDialog
        open={dialogOpen}
        onOpenChange={(open) => {
          if (!open) {
            closeRuntimeDialog();
          }
        }}
        title={dictionary.topbar.runtimeSwitchTitle}
        description={dictionary.topbar.runtimeSwitchDescription}
        confirmLabel={dictionary.topbar.queueRuntimeSwitch}
        isPending={isPending}
        note={note}
        onNoteChange={setNote}
        noteError={fieldErrors?.note}
        stepUpCode={stepUpCode}
        onStepUpCodeChange={setStepUpCode}
        stepUpCodeError={fieldErrors?.stepUpCode}
        requiresStepUp
        onSubmit={submitRuntimeSwitch}
        feedback={dialogFeedback}
        context={
          <div className="space-y-1">
            <p>{dictionary.risk.currentMode}: {modeLabel}</p>
            <p>{dictionary.risk.targetMode}: {targetModeLabel}</p>
            <p>{dictionary.risk.environment}: {environmentLabel}</p>
            {killSwitchActive ? <p>{dictionary.topbar.runtimeSwitchDisabled}</p> : null}
          </div>
        }
      />
    </header>
  );
}
