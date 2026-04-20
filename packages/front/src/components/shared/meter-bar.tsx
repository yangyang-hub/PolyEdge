import { cn } from "@/lib/utils";

const toneMap = {
  primary: "bg-primary",
  success: "bg-secondary",
  warning: "bg-amber-300",
  danger: "bg-destructive",
  violet: "bg-violet-300",
  neutral: "bg-muted-foreground",
};

export function MeterBar({
  value,
  tone = "primary",
  className,
  trackClassName,
  barClassName,
}: {
  value: string;
  tone?: keyof typeof toneMap;
  className?: string;
  trackClassName?: string;
  barClassName?: string;
}) {
  return (
    <div className={cn("h-1.5 w-full overflow-hidden rounded-full bg-background/70", trackClassName)}>
      <div
        className={cn("h-full rounded-full transition-all duration-300", toneMap[tone], className, barClassName)}
        style={{ width: value }}
      />
    </div>
  );
}
