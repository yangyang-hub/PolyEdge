"use client";

import Link from "next/link";
import { usePathname } from "next/navigation";
import type { LucideIcon } from "lucide-react";
import {
  CheckSquare,
  History,
  LayoutDashboard,
  Newspaper,
  Plus,
  Radar,
  Settings,
  ShieldAlert,
  SquareChartGantt,
  WalletCards,
} from "lucide-react";

import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";

type NavItem = {
  href: string;
  label: string;
  icon: LucideIcon;
};

const navItems: NavItem[] = [
  { href: "/dashboard", label: "Dashboard", icon: LayoutDashboard },
  { href: "/markets", label: "Markets", icon: SquareChartGantt },
  { href: "/events", label: "Events", icon: Newspaper },
  { href: "/signals", label: "Signals", icon: Radar },
  { href: "/positions", label: "Positions", icon: WalletCards },
  { href: "/risk", label: "Risk", icon: ShieldAlert },
  { href: "/approvals", label: "Approvals", icon: CheckSquare },
  { href: "/replay", label: "Replay", icon: History },
  { href: "/settings", label: "Settings", icon: Settings },
];

export function ConsoleSidebar() {
  const pathname = usePathname();

  return (
    <aside className="group fixed inset-y-0 left-0 z-40 hidden w-16 overflow-hidden bg-sidebar transition-all duration-300 hover:w-64 md:flex md:flex-col">
      <div className="flex h-14 items-center gap-4 px-4 whitespace-nowrap">
        <div className="flex size-7 shrink-0 items-center justify-center rounded-sm bg-primary/15 font-heading text-sm font-black text-primary">
          P
        </div>
        <p className="font-heading text-xl font-extrabold tracking-tight text-primary opacity-0 transition-opacity duration-200 group-hover:opacity-100">
          PolyEdge
        </p>
      </div>

      <nav className="flex-1 space-y-1 px-2 pt-4">
        {navItems.map(({ href, label, icon: Icon }) => {
          const active = pathname === href || (href !== "/dashboard" && pathname.startsWith(`${href}/`));

          return (
            <Link
              key={href}
              href={href}
              className={cn(
                "flex h-11 items-center gap-4 overflow-hidden rounded-sm border-l-2 px-3 text-sm font-medium transition-colors",
                active
                  ? "border-sidebar-primary bg-sidebar-accent text-sidebar-accent-foreground"
                  : "border-transparent text-muted-foreground hover:bg-accent hover:text-foreground",
              )}
            >
              <Icon className="size-4 shrink-0" />
              <span className="min-w-max opacity-0 transition-opacity duration-200 group-hover:opacity-100">
                {label}
              </span>
            </Link>
          );
        })}
      </nav>

      <div className="p-2 pb-6">
        <Button className="h-10 w-full justify-start gap-3 overflow-hidden rounded-sm bg-primary text-primary-foreground shadow-[0_0_20px_rgba(0,102,255,0.18)] hover:bg-primary/90">
          <Plus className="size-4 shrink-0" />
          <span className="min-w-max opacity-0 transition-opacity duration-200 group-hover:opacity-100">
            New Order
          </span>
        </Button>
      </div>
    </aside>
  );
}
