import { Button } from "@/components/ui/button";
import { RouteStateCard } from "@/components/shared/route-state-card";
import { dictionary } from "@/lib/i18n/dictionaries";

export default function NotFound() {
  return (
    <div className="flex min-h-screen items-center justify-center px-6 py-12">
      <RouteStateCard
        eyebrow={dictionary.routeStates.notFoundEyebrow}
        title={dictionary.routeStates.notFoundTitle}
        description={dictionary.routeStates.notFoundDescription}
        details={<p>{dictionary.routeStates.notFoundDetails}</p>}
        actions={
          <>
            <Button asChild className="rounded-sm bg-primary text-primary-foreground hover:bg-primary/90">
              <a href="/dashboard">{dictionary.routeStates.openDashboard}</a>
            </Button>
            <Button asChild variant="outline" className="rounded-sm border-white/10 bg-accent/45 hover:bg-accent">
              <a href="/login">{dictionary.routeStates.openLogin}</a>
            </Button>
          </>
        }
        className="w-full max-w-3xl"
      />
    </div>
  );
}
