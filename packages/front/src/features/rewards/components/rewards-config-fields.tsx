"use client";

import type { ReactNode } from "react";
import { Info } from "lucide-react";

import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";

export const selectClassName =
  "h-8 w-full rounded-lg border border-input bg-background px-2.5 text-sm";

export function ConfigSection({
  title,
  description,
  children,
}: {
  title: string;
  description: string;
  children: ReactNode;
}) {
  return (
    <section className="grid gap-4 xl:grid-cols-[220px_1fr]">
      <div className="space-y-1">
        <h3 className="font-heading text-sm font-medium">{title}</h3>
        <p className="max-w-sm text-xs leading-5 text-muted-foreground">{description}</p>
      </div>
      <div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-3 2xl:grid-cols-4">
        {children}
      </div>
    </section>
  );
}

export function ToggleField({
  label,
  hint,
  checked,
  onChange,
}: {
  label: string;
  hint?: string;
  checked: boolean;
  onChange: (checked: boolean) => void;
}) {
  return (
    <label className="flex min-h-16 items-center gap-3 rounded-lg border border-border/70 bg-muted/20 px-3 py-2 text-sm">
      <input
        type="checkbox"
        className="size-4 accent-primary"
        checked={checked}
        onChange={(event) => onChange(event.target.checked)}
      />
      <span className="flex items-center gap-1">
        {label}
        {hint ? <Hint content={hint} /> : null}
      </span>
    </label>
  );
}

export function Hint({ content }: { content: string }) {
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <button
          type="button"
          className="inline-flex size-6 shrink-0 items-center justify-center rounded-full text-muted-foreground/70 hover:bg-muted hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
          aria-label={content}
          onClick={(event) => {
            event.preventDefault();
            event.stopPropagation();
          }}
        >
          <Info className="size-3.5" aria-hidden="true" />
        </button>
      </TooltipTrigger>
      <TooltipContent side="top" className="max-w-xs text-wrap">
        {content}
      </TooltipContent>
    </Tooltip>
  );
}
