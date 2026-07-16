"use client";

import { usePathname } from "next/navigation";
import Link from "next/link";
import { Menu } from "lucide-react";
import { useState } from "react";

import { Button } from "@/components/ui/button";
import {
  Sheet,
  SheetContent,
  SheetHeader,
  SheetTitle,
  SheetTrigger,
} from "@/components/ui/sheet";
import { consoleNavItems, isConsoleNavItemActive } from "@/components/shared/console-nav-items";
import { dictionary } from "@/lib/i18n/dictionaries";
import { cn } from "@/lib/utils";
import { useAuth } from "@/components/shared/auth-provider";

export function ConsoleTopbar() {
  const pathname = usePathname();
  const [mobileMenuOpen, setMobileMenuOpen] = useState(false);
  const { user, signOut } = useAuth();

  return (
    <header className="fixed inset-x-0 top-0 z-30 border-b border-border bg-card/90 backdrop-blur-md md:left-16">
      <div className="mx-auto flex h-14 w-full max-w-[1400px] items-center justify-between gap-6 px-4 md:px-6">
        <div className="flex min-w-0 flex-1 items-center gap-4">
          <Sheet open={mobileMenuOpen} onOpenChange={setMobileMenuOpen}>
            <SheetTrigger asChild>
              <Button
                aria-label={dictionary.topbar.openNavigation}
                className="md:hidden"
                size="icon-sm"
                variant="ghost"
              >
                <Menu className="size-4" />
              </Button>
            </SheetTrigger>
            <SheetContent className="w-[18rem] max-w-[calc(100vw-2rem)] gap-0 p-0" side="left">
              <SheetHeader className="border-b border-border px-4 py-4">
                <SheetTitle className="flex items-center gap-3">
                  <span className="flex size-7 shrink-0 items-center justify-center rounded-md bg-primary text-xs font-bold text-primary-foreground">
                    P
                  </span>
                  <span className="font-heading text-lg font-semibold text-foreground">PolyEdge</span>
                </SheetTitle>
              </SheetHeader>
              <nav className="flex flex-col gap-0.5 px-2 py-3">
                {consoleNavItems.filter((item) => !item.roles || (user && item.roles.includes(user.role))).map(({ href, labelKey, icon: Icon }) => {
                  const active = isConsoleNavItemActive(pathname, href);
                  const label = dictionary.nav[labelKey];

                  return (
                    <Link
                      key={href}
                      href={href}
                      onClick={() => setMobileMenuOpen(false)}
                      className={cn(
                        "flex h-10 items-center gap-3 rounded-md px-3 text-sm font-medium transition-colors",
                        active
                          ? "bg-sidebar-accent text-sidebar-accent-foreground"
                          : "text-muted-foreground hover:bg-muted hover:text-foreground",
                      )}
                    >
                      <Icon className="size-4 shrink-0" />
                      <span>{label}</span>
                    </Link>
                  );
                })}
              </nav>
            </SheetContent>
          </Sheet>
          <p className="truncate text-sm font-semibold tracking-tight text-foreground md:text-base">
            {dictionary.topbar.title}
          </p>
        </div>
        <div className="flex items-center gap-2 text-sm">
          <span className="hidden rounded-md bg-muted px-2.5 py-1 text-xs font-medium text-muted-foreground sm:inline">
            {user?.display_name}
          </span>
          <Button size="sm" variant="outline" onClick={() => void signOut()}>
            退出
          </Button>
        </div>
      </div>
    </header>
  );
}
