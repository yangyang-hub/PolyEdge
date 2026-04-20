import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { cn } from "@/lib/utils";

const accentMap = {
  primary: "border-primary/50 text-primary",
  success: "border-secondary/50 text-secondary",
  danger: "border-destructive/50 text-destructive",
  violet: "border-violet-400/50 text-violet-200",
};

export function MetricCard({
  title,
  value,
  hint,
  accent,
}: {
  title: string;
  value: string;
  hint: string;
  accent: keyof typeof accentMap;
}) {
  return (
    <Card className={cn("border-l-4 bg-card", accentMap[accent])}>
      <CardHeader className="pb-2">
        <CardTitle className="text-[11px] font-semibold uppercase tracking-[0.24em] text-muted-foreground">
          {title}
        </CardTitle>
      </CardHeader>
      <CardContent className="flex items-end justify-between gap-3">
        <span className="font-mono text-2xl font-semibold text-foreground">{value}</span>
        <span className="text-xs text-muted-foreground">{hint}</span>
      </CardContent>
    </Card>
  );
}
