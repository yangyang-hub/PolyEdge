import Link from "next/link";

import {
  normalizeConsoleRole,
  sanitizeNextPath,
} from "@/lib/console-auth";
import { Button } from "@/components/ui/button";
import { RouteStateCard } from "@/components/shared/route-state-card";
import { getServerI18n } from "@/lib/i18n/server";

type UnauthorizedPageProps = {
  searchParams: Promise<{
    next?: string | string[];
    required?: string | string[];
    current?: string | string[];
  }>;
};

export default async function UnauthorizedPage({ searchParams }: UnauthorizedPageProps) {
  const [resolvedSearchParams, { dictionary }] = await Promise.all([searchParams, getServerI18n()]);
  const nextPath = sanitizeNextPath(resolvedSearchParams.next);
  const requiredRole = normalizeConsoleRole(
    Array.isArray(resolvedSearchParams.required)
      ? resolvedSearchParams.required[0]
      : resolvedSearchParams.required,
  );
  const currentRole = normalizeConsoleRole(
    Array.isArray(resolvedSearchParams.current)
      ? resolvedSearchParams.current[0]
      : resolvedSearchParams.current,
  );

  return (
    <RouteStateCard
      eyebrow={dictionary.auth.accessBoundary}
      title={dictionary.auth.unauthorizedTitle}
      description={dictionary.auth.unauthorizedDescription}
      details={
        <div className="space-y-3">
          <p>{dictionary.auth.requestedRoute}: {nextPath}</p>
          <p>{dictionary.auth.requiredRole}: {requiredRole ? dictionary.roles[requiredRole] : dictionary.common.unknown}</p>
          <p>{dictionary.auth.currentRole}: {currentRole ? dictionary.roles[currentRole] : dictionary.auth.noSession}</p>
        </div>
      }
      actions={
        <>
          <Button asChild className="rounded-sm bg-primary text-primary-foreground hover:bg-primary/90">
            <Link href={`/login?next=${encodeURIComponent(nextPath)}`}>{dictionary.auth.switchRole}</Link>
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
