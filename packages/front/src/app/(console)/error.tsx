"use client";

import { useEffect } from "react";
import Link from "next/link";

import { Button } from "@/components/ui/button";
import { RouteStateCard } from "@/components/shared/route-state-card";

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
      eyebrow="Console Error"
      title="The console segment failed to render"
      description="A runtime error interrupted the current route. Retry the segment render or go back to the dashboard shell."
      details={
        <div className="space-y-2">
          <p>Message: {error.message}</p>
          {error.digest ? <p>Digest: {error.digest}</p> : null}
        </div>
      }
      actions={
        <>
          <Button
            onClick={() => unstable_retry()}
            className="rounded-sm bg-primary text-primary-foreground hover:bg-primary/90"
          >
            Retry segment
          </Button>
          <Button asChild variant="outline" className="rounded-sm border-white/10 bg-accent/45 hover:bg-accent">
            <Link href="/dashboard">Return to dashboard</Link>
          </Button>
        </>
      }
      className="max-w-3xl"
    />
  );
}
