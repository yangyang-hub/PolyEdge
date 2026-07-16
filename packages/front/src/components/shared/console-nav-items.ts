import type { LucideIcon } from "lucide-react";
import {
  LayoutDashboard,
  WalletCards,
  PlaySquare,
  Settings,
  SquareChartGantt,
  Users,
  Landmark,
  GitFork,
} from "lucide-react";

import type { Dictionary } from "@/lib/i18n/dictionaries";

export type ConsoleNavItem = {
  href: string;
  labelKey: keyof Dictionary["nav"];
  icon: LucideIcon;
  roles?: Array<"admin" | "market_editor" | "read_only">;
};

export const consoleNavItems: ConsoleNavItem[] = [
  { href: "/dashboard", labelKey: "dashboard", icon: LayoutDashboard },
  { href: "/strategies", labelKey: "strategies", icon: SquareChartGantt },
  { href: "/following", labelKey: "following", icon: GitFork },
  { href: "/wallets", labelKey: "wallets", icon: WalletCards },
  { href: "/operations", labelKey: "operations", icon: PlaySquare },
  { href: "/settings", labelKey: "settings", icon: Settings, roles: ["admin"] },
  { href: "/admin/users", labelKey: "users", icon: Users, roles: ["admin"] },
  { href: "/admin/finance", labelKey: "finance", icon: Landmark, roles: ["admin"] },
];

export function isConsoleNavItemActive(pathname: string, href: string): boolean {
  return pathname === href || (href !== "/dashboard" && pathname.startsWith(`${href}/`));
}
