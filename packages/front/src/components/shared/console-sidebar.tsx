"use client";

import { usePathname } from "next/navigation";

import { consoleNavItems, isConsoleNavItemActive } from "@/components/shared/console-nav-items";
import { dictionary } from "@/lib/i18n/dictionaries";
import { cn } from "@/lib/utils";
import { useAuth } from "@/components/shared/auth-provider";

export function ConsoleSidebar() {
  const pathname = usePathname();
  const { user } = useAuth();

  return (
    <aside className="group fixed inset-y-0 left-0 z-40 hidden w-16 overflow-hidden border-r border-sidebar-border bg-sidebar transition-all duration-200 hover:w-52 md:flex md:flex-col">
      <div className="flex h-14 items-center gap-3 border-b border-sidebar-border px-3.5 whitespace-nowrap">
        <div className="flex size-7 shrink-0 items-center justify-center rounded-md bg-primary font-heading text-xs font-bold text-primary-foreground">
          P
        </div>
        <p className="font-heading text-base font-semibold tracking-tight text-foreground opacity-0 transition-opacity duration-150 group-hover:opacity-100">
          PolyEdge
        </p>
      </div>

      <nav className="flex-1 space-y-0.5 px-2 pt-3">
        {consoleNavItems.filter((item) => !item.roles || (user && item.roles.includes(user.role))).map(({ href, labelKey, icon: Icon }) => {
          const active = isConsoleNavItemActive(pathname, href);
          const label = dictionary.nav[labelKey];

          return (
            <a
              key={href}
              href={href}
              className={cn(
                "flex h-10 items-center gap-3 overflow-hidden rounded-md px-2.5 text-sm font-medium transition-colors",
                active
                  ? "bg-sidebar-accent text-sidebar-accent-foreground"
                  : "text-muted-foreground hover:bg-muted hover:text-foreground",
              )}
            >
              <Icon className="size-4 shrink-0" />
              <span className="min-w-max opacity-0 transition-opacity duration-150 group-hover:opacity-100">
                {label}
              </span>
            </a>
          );
        })}
      </nav>
    </aside>
  );
}
