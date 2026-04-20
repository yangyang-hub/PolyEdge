import { Badge } from "@/components/ui/badge";
import { cn } from "@/lib/utils";

const toneMap = {
  neutral: "bg-muted text-muted-foreground border-border",
  primary: "bg-primary/16 text-primary border-primary/20",
  success: "bg-secondary/16 text-secondary border-secondary/20",
  warning: "bg-amber-400/16 text-amber-200 border-amber-300/20",
  danger: "bg-destructive/16 text-destructive border-destructive/20",
  violet: "bg-violet-400/16 text-violet-200 border-violet-300/20",
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
        "rounded-sm border px-2 py-0.5 text-[10px] font-semibold tracking-wide uppercase",
        toneMap[tone],
        className,
      )}
    >
      {children}
    </Badge>
  );
}
