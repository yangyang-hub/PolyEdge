"use client";

import Link from "next/link";
import { useEffect } from "react";

export default function Home() {
  useEffect(() => {
    window.location.replace("/dashboard");
  }, []);

  return (
    <main className="flex min-h-screen items-center justify-center bg-background text-foreground">
      <Link className="text-sm text-primary underline-offset-4 hover:underline" href="/dashboard">
        Open dashboard
      </Link>
    </main>
  );
}
