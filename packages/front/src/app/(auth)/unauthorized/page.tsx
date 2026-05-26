"use client";

import Link from "next/link";
import { useEffect, useState } from "react";

import {
  normalizeConsoleRole,
  sanitizeNextPath,
  type ConsoleRole,
} from "@/lib/console-auth";
import { Button } from "@/components/ui/button";
import { RouteStateCard } from "@/components/shared/route-state-card";
import { useI18n } from "@/lib/i18n/client";

type UnauthorizedState = {
  nextPath: string;
  requiredRole: ConsoleRole | null;
  currentRole: ConsoleRole | null;
};

export default function UnauthorizedPage() {
  const { dictionary } = useI18n();
  const [state, setState] = useState<UnauthorizedState>({
    nextPath: "/dashboard",
    requiredRole: null,
    currentRole: null,
  });

  useEffect(() => {
    const timeoutId = window.setTimeout(() => {
      const searchParams = new URLSearchParams(window.location.search);

      setState({
        nextPath: sanitizeNextPath(searchParams.get("next")),
        requiredRole: normalizeConsoleRole(searchParams.get("required")),
        currentRole: normalizeConsoleRole(searchParams.get("current")),
      });
    }, 0);

    return () => window.clearTimeout(timeoutId);
  }, []);

  return (
    <RouteStateCard
      eyebrow={dictionary.auth.accessBoundary}
      title={dictionary.auth.unauthorizedTitle}
      description={dictionary.auth.unauthorizedDescription}
      details={
        <div className="space-y-3">
          <p>{dictionary.auth.requestedRoute}: {state.nextPath}</p>
          <p>{dictionary.auth.requiredRole}: {state.requiredRole ? dictionary.roles[state.requiredRole] : dictionary.common.unknown}</p>
          <p>{dictionary.auth.currentRole}: {state.currentRole ? dictionary.roles[state.currentRole] : dictionary.auth.noSession}</p>
        </div>
      }
      actions={
        <>
          <Button asChild className="rounded-sm bg-primary text-primary-foreground hover:bg-primary/90">
            <Link href={`/login?next=${encodeURIComponent(state.nextPath)}`}>{dictionary.auth.switchRole}</Link>
          </Button>
          <Button asChild variant="outline" className="rounded-sm border-white/10 bg-accent/45 hover:bg-accent">
            <Link href="/dashboard">{dictionary.auth.goToDashboard}</Link>
          </Button>
        </>
      }
      className="w-full max-w-3xl"
    />
  );
}
