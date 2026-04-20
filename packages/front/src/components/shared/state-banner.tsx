import { AlertTriangle, CheckCircle2, Info } from "lucide-react";

import { cn } from "@/lib/utils";

const toneMap = {
  info: {
    icon: Info,
    wrap: "bg-accent/45 text-foreground ring-white/6",
    iconClass: "text-primary",
  },
  success: {
    icon: CheckCircle2,
    wrap: "bg-secondary/8 text-foreground ring-secondary/10",
    iconClass: "text-secondary",
  },
  warning: {
    icon: AlertTriangle,
    wrap: "bg-destructive/6 text-foreground ring-destructive/10",
    iconClass: "text-destructive",
  },
};

export function StateBanner({
  tone,
  title,
  detail,
  className,
}: {
  tone: keyof typeof toneMap;
  title: string;
  detail: string;
  className?: string;
}) {
  const config = toneMap[tone];
  const Icon = config.icon;

  return (
    <div className={cn("flex items-start gap-3 rounded-lg p-4 ring-1", config.wrap, className)}>
      <Icon className={cn("mt-0.5 size-4 shrink-0", config.iconClass)} />
      <div className="space-y-1">
        <p className="text-sm font-semibold text-foreground">{title}</p>
        <p className="text-sm text-muted-foreground">{detail}</p>
      </div>
    </div>
  );
}
