import Link from "next/link";

import {
  CONSOLE_ROLE_LABELS,
  normalizeConsoleRole,
  sanitizeNextPath,
} from "@/lib/console-auth";
import { Button } from "@/components/ui/button";
import { RouteStateCard } from "@/components/shared/route-state-card";

type UnauthorizedPageProps = {
  searchParams: Promise<{
    next?: string | string[];
    required?: string | string[];
    current?: string | string[];
  }>;
};

export default async function UnauthorizedPage({ searchParams }: UnauthorizedPageProps) {
  const resolvedSearchParams = await searchParams;
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
      eyebrow="Access Boundary"
      title="Current session does not satisfy the route policy"
      description="The console guard allowed the request to reach the auth shell, but the active role is below the minimum requirement for the destination route."
      details={
        <div className="space-y-3">
          <p>Requested route: {nextPath}</p>
          <p>Required role: {requiredRole ? CONSOLE_ROLE_LABELS[requiredRole] : "Unknown"}</p>
          <p>Current role: {currentRole ? CONSOLE_ROLE_LABELS[currentRole] : "No session"}</p>
        </div>
      }
      actions={
        <>
          <Button asChild className="rounded-sm bg-primary text-primary-foreground hover:bg-primary/90">
            <Link href={`/login?next=${encodeURIComponent(nextPath)}`}>Switch role</Link>
          </Button>
          <Button asChild variant="outline" className="rounded-sm border-white/10 bg-accent/45 hover:bg-accent">
            <Link href="/dashboard">Go to dashboard</Link>
          </Button>
        </>
      }
      className="w-full max-w-3xl"
    />
  );
}
