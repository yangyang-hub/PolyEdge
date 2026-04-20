import Link from "next/link";

import { Button } from "@/components/ui/button";
import { RouteStateCard } from "@/components/shared/route-state-card";

export default function NotFound() {
  return (
    <div className="flex min-h-screen items-center justify-center px-6 py-12">
      <RouteStateCard
        eyebrow="404"
        title="This route is not mapped"
        description="The requested URL does not match any page in the PolyEdge console."
        details={<p>Use the primary console entrypoint or return to the login shell if route protection is enabled.</p>}
        actions={
          <>
            <Button asChild className="rounded-sm bg-primary text-primary-foreground hover:bg-primary/90">
              <Link href="/dashboard">Open dashboard</Link>
            </Button>
            <Button asChild variant="outline" className="rounded-sm border-white/10 bg-accent/45 hover:bg-accent">
              <Link href="/login">Open login</Link>
            </Button>
          </>
        }
        className="w-full max-w-3xl"
      />
    </div>
  );
}
