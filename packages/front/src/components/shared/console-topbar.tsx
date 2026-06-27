"use client";

import { usePathname } from "next/navigation";
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

export function ConsoleTopbar() {
  const pathname = usePathname();
  const [mobileMenuOpen, setMobileMenuOpen] = useState(false);

  return (
    <header className="fixed inset-x-0 top-0 z-30 bg-background/95 backdrop-blur md:left-16">
      <div className="flex h-14 items-center justify-between gap-6 px-4 md:px-6">
        <div className="flex min-w-0 flex-1 items-center gap-6">
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
              <SheetHeader className="border-b border-border/70 px-4 py-4">
                <SheetTitle className="flex items-center gap-3">
                  <span className="flex size-7 shrink-0 items-center justify-center rounded-sm bg-primary/15 font-heading text-sm font-black text-primary">
                    P
                  </span>
                  <span className="font-heading text-lg font-extrabold text-primary">PolyEdge</span>
                </SheetTitle>
              </SheetHeader>
              <nav className="flex flex-col gap-1 px-2 py-3">
                {consoleNavItems.map(({ href, labelKey, icon: Icon }) => {
                  const active = isConsoleNavItemActive(pathname, href);
                  const label = dictionary.nav[labelKey];

                  return (
                    <a
                      key={href}
                      href={href}
                      onClick={() => setMobileMenuOpen(false)}
                      className={cn(
                        "flex h-11 items-center gap-3 rounded-sm border-l-2 px-3 text-sm font-medium transition-colors",
                        active
                          ? "border-sidebar-primary bg-sidebar-accent text-sidebar-accent-foreground"
                          : "border-transparent text-muted-foreground hover:bg-accent hover:text-foreground",
                      )}
                    >
                      <Icon className="size-4 shrink-0" />
                      <span>{label}</span>
                    </a>
                  );
                })}
              </nav>
            </SheetContent>
          </Sheet>
          <p className="hidden font-heading text-lg font-black tracking-tight text-primary md:block">
            {dictionary.topbar.title}
          </p>
        </div>
      </div>
    </header>
  );
}
