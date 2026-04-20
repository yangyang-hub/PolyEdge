import Link from "next/link";

import { Button } from "@/components/ui/button";
import { RouteStateCard } from "@/components/shared/route-state-card";

export default function NotFound() {
  return (
    <RouteStateCard
      eyebrow="Console Not Found"
      title="The requested console resource does not exist"
      description="The current route resolved, but the specific resource or child segment could not be found."
      details={<p>Try returning to the dashboard or jump directly into the market workbench.</p>}
      actions={
        <>
          <Button asChild className="rounded-sm bg-primary text-primary-foreground hover:bg-primary/90">
            <Link href="/dashboard">Back to dashboard</Link>
          </Button>
          <Button asChild variant="outline" className="rounded-sm border-white/10 bg-accent/45 hover:bg-accent">
            <Link href="/markets">Open markets</Link>
          </Button>
        </>
      }
      className="max-w-3xl"
    />
  );
}
