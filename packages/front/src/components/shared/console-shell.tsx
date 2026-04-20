import { ConsoleRealtimeProvider } from "@/components/shared/console-realtime-provider";
import { ConsoleSidebar } from "@/components/shared/console-sidebar";
import { ConsoleStatusRail } from "@/components/shared/console-status-rail";
import { ConsoleTopbar } from "@/components/shared/console-topbar";

export function ConsoleShell({ children }: { children: React.ReactNode }) {
  return (
    <div className="min-h-screen bg-background text-foreground">
      <ConsoleSidebar />
      <div className="md:pl-16">
        <ConsoleRealtimeProvider>
          <ConsoleTopbar />
          <main className="px-4 pb-12 pt-[4.5rem] md:px-6">{children}</main>
          <ConsoleStatusRail />
        </ConsoleRealtimeProvider>
      </div>
    </div>
  );
}
