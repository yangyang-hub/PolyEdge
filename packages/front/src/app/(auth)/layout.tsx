import type { ReactNode } from "react";

export default function AuthLayout({ children }: { children: ReactNode }) {
  return (
    <main className="relative min-h-screen overflow-hidden bg-background px-6 py-10">
      <div className="pointer-events-none absolute inset-0">
        <div className="absolute left-[-10rem] top-[-10rem] size-[28rem] rounded-full bg-primary/10 blur-3xl" />
        <div className="absolute bottom-[-12rem] right-[-6rem] size-[24rem] rounded-full bg-secondary/10 blur-3xl" />
      </div>
      <div className="relative mx-auto flex min-h-[calc(100vh-5rem)] max-w-6xl items-center justify-center">
        {children}
      </div>
    </main>
  );
}
