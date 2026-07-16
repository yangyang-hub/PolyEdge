import type { ReactNode } from "react";

import { cn } from "@/lib/utils";

type RouteStateCardProps = {
  eyebrow: string;
  title: string;
  description: string;
  details?: ReactNode;
  actions?: ReactNode;
  className?: string;
};

export function RouteStateCard({
  eyebrow,
  title,
  description,
  details,
  actions,
  className,
}: RouteStateCardProps) {
  return (
    <section
      className={cn(
        "relative overflow-hidden rounded-xl border border-border bg-card p-6 shadow-sm",
        className,
      )}
    >
      <div className="absolute inset-x-0 top-0 h-0.5 bg-primary" />
      <div className="space-y-4">
        <div className="space-y-2">
          <p className="text-xs font-medium uppercase tracking-[0.08em] text-muted-foreground">
            {eyebrow}
          </p>
          <h1 className="font-heading text-2xl font-semibold tracking-tight text-foreground">{title}</h1>
          <p className="max-w-2xl text-sm leading-6 text-muted-foreground">{description}</p>
        </div>
        {details ? <div className="rounded-lg bg-muted p-4 text-sm text-muted-foreground">{details}</div> : null}
        {actions ? <div className="flex flex-wrap gap-3">{actions}</div> : null}
      </div>
    </section>
  );
}
