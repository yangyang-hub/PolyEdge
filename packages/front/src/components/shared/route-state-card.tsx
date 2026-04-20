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
        "relative overflow-hidden rounded-2xl border border-border/70 bg-card/95 p-6 shadow-[0_24px_80px_rgba(0,0,0,0.28)]",
        className,
      )}
    >
      <div className="absolute inset-x-0 top-0 h-px bg-gradient-to-r from-transparent via-primary/50 to-transparent" />
      <div className="space-y-4">
        <div className="space-y-2">
          <p className="font-mono text-[10px] font-bold uppercase tracking-[0.24em] text-muted-foreground">
            {eyebrow}
          </p>
          <h1 className="font-heading text-2xl font-black tracking-tight text-foreground">{title}</h1>
          <p className="max-w-2xl text-sm leading-6 text-muted-foreground">{description}</p>
        </div>
        {details ? <div className="rounded-xl bg-accent/45 p-4 text-sm text-muted-foreground">{details}</div> : null}
        {actions ? <div className="flex flex-wrap gap-3">{actions}</div> : null}
      </div>
    </section>
  );
}
