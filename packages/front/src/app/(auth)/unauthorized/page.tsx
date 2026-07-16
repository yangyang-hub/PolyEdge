"use client";

import Link from "next/link";
import { useEffect, useState } from "react";

import { sanitizeNextPath } from "@/lib/console-auth";
import type { UserRole } from "@/lib/contracts/dto";
import { Button } from "@/components/ui/button";
import { RouteStateCard } from "@/components/shared/route-state-card";
import { dictionary } from "@/lib/i18n/dictionaries";

type UnauthorizedState = {
  nextPath: string;
  requiredRole: UserRole | null;
  currentRole: UserRole | null;
};

const USER_ROLES: UserRole[] = ["admin", "market_editor", "read_only"];
const parseRole = (value: string | null): UserRole | null =>
  USER_ROLES.find((role) => role === value) ?? null;

export default function UnauthorizedPage() {
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
        requiredRole: parseRole(searchParams.get("required")),
        currentRole: parseRole(searchParams.get("current")),
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
