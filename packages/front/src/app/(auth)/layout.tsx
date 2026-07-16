import type { ReactNode } from "react";

import { dictionary } from "@/lib/i18n/dictionaries";

export default function AuthLayout({ children }: { children: ReactNode }) {
  return (
    <main className="relative min-h-screen overflow-hidden bg-background px-6 py-10">
      <div className="pointer-events-none absolute inset-0">
        <div className="absolute inset-x-0 top-0 h-72 bg-[radial-gradient(ellipse_at_top,_rgb(46_92_255_/_0.12),_transparent_60%)]" />
        <div className="absolute inset-0 bg-[linear-gradient(to_bottom,_transparent,_var(--background)_70%)]" />
      </div>
      <div className="relative mx-auto flex min-h-[calc(100vh-5rem)] max-w-6xl flex-col items-center justify-center gap-8">
        <div className="flex items-center gap-3">
          <span className="flex size-9 items-center justify-center rounded-lg bg-primary text-sm font-bold text-primary-foreground">
            P
          </span>
          <div>
            <p className="text-lg font-semibold tracking-tight text-foreground">{dictionary.meta.title}</p>
            <p className="max-w-xs text-xs text-muted-foreground">{dictionary.meta.description}</p>
          </div>
        </div>
        {children}
      </div>
    </main>
  );
}
