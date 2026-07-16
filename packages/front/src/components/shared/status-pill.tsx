import { Badge } from "@/components/ui/badge";
import { cn } from "@/lib/utils";

const toneMap = {
  neutral: "bg-muted text-muted-foreground border-border",
  primary: "bg-primary/10 text-primary border-primary/20",
  success: "bg-secondary/10 text-secondary border-secondary/25",
  warning: "bg-amber-500/10 text-amber-700 border-amber-500/25 dark:text-amber-300",
  danger: "bg-destructive/10 text-destructive border-destructive/25",
  violet: "bg-violet-500/10 text-violet-700 border-violet-500/25 dark:text-violet-300",
};

export function StatusPill({
  children,
  tone = "neutral",
  className,
}: {
  children: React.ReactNode;
  tone?: keyof typeof toneMap;
  className?: string;
}) {
  return (
    <Badge
      variant="outline"
      className={cn(
        "rounded-md border px-2 py-0.5 text-[10px] font-semibold tracking-wide uppercase",
        toneMap[tone],
        className,
      )}
    >
      {children}
    </Badge>
  );
}
