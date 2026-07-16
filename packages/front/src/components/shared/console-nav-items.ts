import type { LucideIcon } from "lucide-react";
import {
  LayoutDashboard,
  WalletCards,
  PlaySquare,
  Settings,
  SquareChartGantt,
} from "lucide-react";

import type { Dictionary } from "@/lib/i18n/dictionaries";

export type ConsoleNavItem = {
  href: string;
  labelKey: keyof Dictionary["nav"];
  icon: LucideIcon;
};

export const consoleNavItems: ConsoleNavItem[] = [
  { href: "/dashboard", labelKey: "dashboard", icon: LayoutDashboard },
  { href: "/strategies", labelKey: "strategies", icon: SquareChartGantt },
  { href: "/wallets", labelKey: "wallets", icon: WalletCards },
  { href: "/operations", labelKey: "operations", icon: PlaySquare },
  { href: "/settings", labelKey: "settings", icon: Settings },
];

export function isConsoleNavItemActive(pathname: string, href: string): boolean {
  return pathname === href || (href !== "/dashboard" && pathname.startsWith(`${href}/`));
}
