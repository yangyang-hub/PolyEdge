import { AlertTriangle } from "lucide-react";

import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { dictionary } from "@/lib/i18n/dictionaries";

export function FundingSafetyCard() {
  return (
    <Card>
      <CardHeader>
        <CardTitle className="flex items-center gap-2">
          <AlertTriangle className="size-4 text-destructive" />
          {dictionary.funding.safetyTitle}
        </CardTitle>
        <CardDescription>{dictionary.funding.safetyDescription}</CardDescription>
      </CardHeader>
      <CardContent>
        <ul className="space-y-2 text-sm text-muted-foreground">
          {Object.values(dictionary.funding.safetyItems).map((item) => (
            <li key={item} className="flex gap-2">
              <span className="mt-2 size-1.5 shrink-0 rounded-full bg-secondary" />
              <span>{item}</span>
            </li>
          ))}
        </ul>
      </CardContent>
    </Card>
  );
}

export function FundingStepsCard() {
  return (
    <Card>
      <CardHeader>
        <CardTitle>{dictionary.funding.stepsTitle}</CardTitle>
        <CardDescription>{dictionary.funding.stepsDescription}</CardDescription>
      </CardHeader>
      <CardContent>
        <ol className="grid gap-3 md:grid-cols-4">
          {Object.values(dictionary.funding.steps).map((step, index) => (
            <li key={step} className="rounded-lg border border-border/70 bg-background/35 p-3 text-sm text-muted-foreground">
              <span className="mb-3 flex size-7 items-center justify-center rounded-sm bg-primary/15 font-mono text-xs text-primary">
                {index + 1}
              </span>
              {step}
            </li>
          ))}
        </ol>
      </CardContent>
    </Card>
  );
}
