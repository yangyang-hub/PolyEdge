import Link from "next/link";

import { Button } from "@/components/ui/button";
import { RouteStateCard } from "@/components/shared/route-state-card";
import { DEFAULT_LOCALE } from "@/lib/i18n/locales";
import { createI18nRuntime } from "@/lib/i18n/runtime";

export default function NotFound() {
  const { dictionary } = createI18nRuntime(DEFAULT_LOCALE);

  return (
    <RouteStateCard
      eyebrow={dictionary.routeStates.consoleNotFoundEyebrow}
      title={dictionary.routeStates.consoleNotFoundTitle}
      description={dictionary.routeStates.consoleNotFoundDescription}
      details={<p>{dictionary.routeStates.consoleNotFoundDetails}</p>}
      actions={
        <>
          <Button asChild className="rounded-sm bg-primary text-primary-foreground hover:bg-primary/90">
            <Link href="/dashboard">{dictionary.common.backToDashboard}</Link>
          </Button>
          <Button asChild variant="outline" className="rounded-sm border-white/10 bg-accent/45 hover:bg-accent">
            <Link href="/markets">{dictionary.routeStates.openMarkets}</Link>
          </Button>
        </>
      }
      className="max-w-3xl"
    />
  );
}
