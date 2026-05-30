"use client";

import { Info } from "lucide-react";

import { Input } from "@/components/ui/input";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import type { DecimalValue } from "@/lib/contracts/dto";
import { toFiniteNumber } from "@/lib/formatters";

export function NumberInput({
  label,
  value,
  suffix,
  hint,
  onChange,
}: {
  label: string;
  value: DecimalValue;
  suffix?: string;
  hint?: string;
  onChange: (value: string) => void;
}) {
  return (
    <label className="space-y-1.5">
      <span className="flex items-center gap-1 text-xs font-medium text-muted-foreground">
        {label}
        {hint ? (
          <Tooltip>
            <TooltipTrigger asChild>
              <Info className="size-3 cursor-help text-muted-foreground/60" />
            </TooltipTrigger>
            <TooltipContent side="top" className="max-w-xs text-wrap">
              {hint}
            </TooltipContent>
          </Tooltip>
        ) : null}
      </span>
      <div className="flex">
        <Input
          type="number"
          className="rounded-r-none font-mono"
          value={String(toFiniteNumber(value))}
          onChange={(event) => onChange(event.target.value)}
        />
        {suffix ? (
          <span className="flex h-8 min-w-8 items-center justify-center rounded-r-lg border border-l-0 border-input px-2 text-xs text-muted-foreground">
            {suffix}
          </span>
        ) : null}
      </div>
    </label>
  );
}
