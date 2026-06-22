import type { LucideIcon } from "lucide-react";
import {
  Activity,
  HandCoins,
  LayoutDashboard,
  Newspaper,
  Radar,
  Search,
  Settings,
  ShieldAlert,
  SquareChartGantt,
  Users,
  WalletCards,
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
  { href: "/radar", labelKey: "radar", icon: Radar },
  { href: "/rewards", labelKey: "rewards", icon: HandCoins },
  { href: "/copy-trading", labelKey: "copytrade", icon: Users },
  { href: "/wallet-analysis", labelKey: "walletAnalysis", icon: Search },
  { href: "/signals", labelKey: "signals", icon: Activity },
  { href: "/positions", labelKey: "positions", icon: WalletCards },
  { href: "/risk", labelKey: "risk", icon: ShieldAlert },
  { href: "/settings", labelKey: "settings", icon: Settings },
];

export function isConsoleNavItemActive(pathname: string, href: string): boolean {
  return pathname === href || (href !== "/dashboard" && pathname.startsWith(`${href}/`));
}
