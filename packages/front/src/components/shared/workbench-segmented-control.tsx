"use client";

import { cn } from "@/lib/utils";

type SegmentedItem<TValue extends string> = {
  key: TValue;
  label: string;
};

type WorkbenchSegmentedControlProps<TValue extends string> = {
  items: Array<SegmentedItem<TValue>>;
  value: TValue;
  onChange: (value: TValue) => void;
  className?: string;
};

export function WorkbenchSegmentedControl<TValue extends string>({
  items,
  value,
  onChange,
  className,
}: WorkbenchSegmentedControlProps<TValue>) {
  return (
    <div className={cn("flex rounded-md bg-accent/70 p-1", className)}>
      {items.map((item) => (
        <button
          key={item.key}
          type="button"
          onClick={() => onChange(item.key)}
          className={
            value === item.key
              ? "rounded-sm bg-card px-3 py-1 text-[10px] font-bold uppercase tracking-[0.18em] text-foreground"
              : "px-3 py-1 text-[10px] font-bold uppercase tracking-[0.18em] text-muted-foreground transition-colors hover:text-foreground"
          }
        >
          {item.label}
        </button>
      ))}
    </div>
  );
}
