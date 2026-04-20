import type { ReactNode } from "react";

import { cn } from "@/lib/utils";

type WorkbenchLayoutProps = {
  children: ReactNode;
  columnsClassName?: string;
  className?: string;
};

type WorkbenchDetailPaneProps = {
  children: ReactNode;
  className?: string;
  desktopOnly?: boolean;
};

export function WorkbenchLayout({
  children,
  columnsClassName = "xl:grid-cols-[1.45fr_0.95fr]",
  className,
}: WorkbenchLayoutProps) {
  return <section className={cn("grid gap-4", columnsClassName, className)}>{children}</section>;
}

export function WorkbenchDetailPane({
  children,
  className,
  desktopOnly = false,
}: WorkbenchDetailPaneProps) {
  return (
    <aside
      className={cn(
        "rounded-lg bg-card/95 p-5 ring-1 ring-white/5",
        desktopOnly && "hidden xl:block",
        className,
      )}
    >
      {children}
    </aside>
  );
}
