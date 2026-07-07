import type { LucideIcon } from "lucide-react";
import {
  CircleDollarSign,
  HandCoins,
  LayoutDashboard,
  Newspaper,
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
  { href: "/markets", labelKey: "markets", icon: SquareChartGantt },
  { href: "/events", labelKey: "events", icon: Newspaper },
  { href: "/rewards", labelKey: "rewards", icon: HandCoins },
  { href: "/funding", labelKey: "funding", icon: CircleDollarSign },
  { href: "/settings", labelKey: "settings", icon: Settings },
];

export function isConsoleNavItemActive(pathname: string, href: string): boolean {
  return pathname === href || (href !== "/dashboard" && pathname.startsWith(`${href}/`));
}
