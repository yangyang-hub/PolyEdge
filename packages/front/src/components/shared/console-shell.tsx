"use client";

import type { ReactNode } from "react";

import { ConsoleSidebar } from "@/components/shared/console-sidebar";
import { ConsoleTopbar } from "@/components/shared/console-topbar";

export function ConsoleShell({ children }: { children: ReactNode }) {
  return (
    <div className="min-h-screen bg-background text-foreground">
      <ConsoleSidebar />
      <div className="md:pl-16">
        <ConsoleTopbar />
        <main className="mx-auto w-full max-w-[1400px] px-4 pb-12 pt-[4.5rem] md:px-6">
          {children}
        </main>
      </div>
    </div>
  );
}
