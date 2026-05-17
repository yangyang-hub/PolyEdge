"use client";

import { startTransition, useState } from "react";
import { ArrowRight, ShieldCheck, UserRound } from "lucide-react";
import { useRouter } from "next/navigation";

import {
  CONSOLE_ROLE_COOKIE,
  CONSOLE_ROLES,
  CONSOLE_USER_COOKIE,
  type ConsoleAuthMode,
  type ConsoleRole,
} from "@/lib/console-auth";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { RouteStateCard } from "@/components/shared/route-state-card";
import { useI18n } from "@/lib/i18n/client";

type MockSessionLoginPanelProps = {
  authMode: ConsoleAuthMode;
  nextPath: string;
  requiredRole: ConsoleRole;
};

export function MockSessionLoginPanel({
  authMode,
  nextPath,
  requiredRole,
}: MockSessionLoginPanelProps) {
  const router = useRouter();
  const [selectedRole, setSelectedRole] = useState<ConsoleRole>(requiredRole);
  const { dictionary } = useI18n();
  const [displayName, setDisplayName] = useState(dictionary.auth.defaultDisplayName);

  function createMockSession() {
    document.cookie = `${CONSOLE_ROLE_COOKIE}=${selectedRole}; path=/; max-age=28800; samesite=lax`;
    document.cookie = `${CONSOLE_USER_COOKIE}=${encodeURIComponent(displayName)}; path=/; max-age=28800; samesite=lax`;

    startTransition(() => {
      router.push(nextPath);
    });
  }

  if (authMode === "off") {
    return (
      <RouteStateCard
        eyebrow={dictionary.auth.consoleAuth}
        title={dictionary.auth.authDisabledTitle}
        description={dictionary.auth.authDisabledDescription}
        details={<p>{dictionary.auth.authDisabledDetails}</p>}
        actions={
          <Button
            onClick={() => startTransition(() => router.push(nextPath))}
            className="rounded-sm bg-primary text-primary-foreground hover:bg-primary/90"
          >
            {dictionary.auth.continueToConsole}
            <ArrowRight className="size-4" />
          </Button>
        }
      />
    );
  }

  return (
    <RouteStateCard
      eyebrow={dictionary.auth.mockSession}
      title={dictionary.auth.startSessionTitle}
      description={dictionary.auth.startSessionDescription}
      details={
        <div className="space-y-3">
          <p>{dictionary.auth.requestedRoute}: {nextPath}</p>
          <p>{dictionary.auth.minimumRole}: {dictionary.roles[requiredRole]}</p>
        </div>
      }
      actions={
        <div className="grid w-full gap-4 lg:grid-cols-[1.45fr_0.95fr]">
          <div className="space-y-3">
            {CONSOLE_ROLES.map((role) => (
              <button
                key={role}
                type="button"
                onClick={() => setSelectedRole(role)}
                className={
                  selectedRole === role
                    ? "w-full rounded-xl border border-primary/40 bg-primary/10 p-4 text-left shadow-[inset_0_0_0_1px_rgba(179,197,255,0.16)]"
                    : "w-full rounded-xl border border-border/70 bg-accent/35 p-4 text-left transition-colors hover:bg-accent/55"
                }
              >
                <div className="flex items-center gap-3">
                  <div className="flex size-9 items-center justify-center rounded-full bg-card text-primary">
                    <ShieldCheck className="size-4" />
                  </div>
                  <div>
                    <p className="font-semibold text-foreground">{dictionary.roles[role]}</p>
                    <p className="mt-1 text-sm text-muted-foreground">{dictionary.auth.roleCopy[role]}</p>
                  </div>
                </div>
              </button>
            ))}
          </div>

          <div className="rounded-xl border border-border/70 bg-accent/35 p-4">
            <p className="font-mono text-[10px] font-bold uppercase tracking-[0.2em] text-muted-foreground">
              {dictionary.auth.sessionProfile}
            </p>
            <div className="mt-4 space-y-4">
              <div className="space-y-2">
                <label className="text-sm font-medium text-foreground" htmlFor="display-name">
                  {dictionary.auth.displayName}
                </label>
                <div className="relative">
                  <UserRound className="pointer-events-none absolute left-3 top-1/2 size-4 -translate-y-1/2 text-muted-foreground" />
                  <Input
                    id="display-name"
                    value={displayName}
                    onChange={(event) => setDisplayName(event.target.value)}
                    className="h-10 rounded-sm border-white/10 bg-card/90 pl-10"
                  />
                </div>
              </div>

              <div className="rounded-lg bg-card/80 p-3 text-sm text-muted-foreground">
                {dictionary.auth.sessionRole}: {dictionary.roles[selectedRole]}
              </div>

              <Button
                onClick={createMockSession}
                className="h-10 w-full rounded-sm bg-primary text-primary-foreground hover:bg-primary/90"
              >
                {dictionary.auth.continueWithMock}
                <ArrowRight className="size-4" />
              </Button>
            </div>
          </div>
        </div>
      }
      className="w-full max-w-5xl"
    />
  );
}
