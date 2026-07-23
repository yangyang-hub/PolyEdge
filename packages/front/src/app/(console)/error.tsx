"use client";

import { useEffect } from "react";

import { Button } from "@/components/ui/button";
import { RouteStateCard } from "@/components/shared/route-state-card";
import { dictionary } from "@/lib/i18n/dictionaries";

export default function Error({
  error,
  unstable_retry,
}: {
  error: Error & { digest?: string };
  unstable_retry: () => void;
}) {
  useEffect(() => {
    console.error(error);
  }, [error]);

  return (
    <RouteStateCard
      eyebrow={dictionary.routeStates.consoleErrorEyebrow}
      title={dictionary.routeStates.consoleErrorTitle}
      description={dictionary.routeStates.consoleErrorDescription}
      details={
        <div className="space-y-2">
          <p>{dictionary.routeStates.message}: {error.message}</p>
          {error.digest ? <p>{dictionary.routeStates.digest}: {error.digest}</p> : null}
        </div>
      }
      actions={
        <>
          <Button
            onClick={() => unstable_retry()}
            className="rounded-sm bg-primary text-primary-foreground hover:bg-primary/90"
          >
            {dictionary.routeStates.retrySegment}
          </Button>
          <Button asChild variant="outline" className="rounded-sm border-white/10 bg-accent/45 hover:bg-accent">
            <a href="/dashboard">{dictionary.routeStates.returnDashboard}</a>
          </Button>
        </>
      }
      className="max-w-3xl"
    />
  );
}
