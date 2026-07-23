import { Button } from "@/components/ui/button";
import { RouteStateCard } from "@/components/shared/route-state-card";
import { dictionary } from "@/lib/i18n/dictionaries";

export default function NotFound() {
  return (
    <RouteStateCard
      eyebrow={dictionary.routeStates.consoleNotFoundEyebrow}
      title={dictionary.routeStates.consoleNotFoundTitle}
      description={dictionary.routeStates.consoleNotFoundDescription}
      details={<p>{dictionary.routeStates.consoleNotFoundDetails}</p>}
      actions={
        <>
          <Button asChild className="rounded-sm bg-primary text-primary-foreground hover:bg-primary/90">
            <a href="/dashboard">{dictionary.common.backToDashboard}</a>
          </Button>
          <Button asChild variant="outline" className="rounded-sm border-white/10 bg-accent/45 hover:bg-accent">
            <a href="/strategies">{dictionary.routeStates.openStrategies}</a>
          </Button>
        </>
      }
      className="max-w-3xl"
    />
  );
}
