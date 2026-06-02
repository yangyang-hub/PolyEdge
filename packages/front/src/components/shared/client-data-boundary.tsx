"use client";

import { useEffect, useState, type ReactNode } from "react";

import { ConsoleLoadingSkeleton } from "@/components/shared/console-loading-skeleton";
import { StateBanner } from "@/components/shared/state-banner";
import { dictionary } from "@/lib/i18n/dictionaries";

type ClientDataBoundaryProps<TData> = {
  load: () => Promise<TData>;
  children: (data: TData) => ReactNode;
};

type LoadState<TData> =
  | { status: "loading"; data: null; error: null }
  | { status: "ready"; data: TData; error: null }
  | { status: "error"; data: null; error: string };

export function ClientDataBoundary<TData>({
  load,
  children,
}: ClientDataBoundaryProps<TData>) {
  const [state, setState] = useState<LoadState<TData>>({
    status: "loading",
    data: null,
    error: null,
  });

  useEffect(() => {
    let cancelled = false;

    void load()
      .then((data) => {
        if (!cancelled) {
          setState({ status: "ready", data, error: null });
        }
      })
      .catch((error: unknown) => {
        if (!cancelled) {
          setState({
            status: "error",
            data: null,
            error: error instanceof Error ? error.message : "Unable to load live console data.",
          });
        }
      });

    return () => {
      cancelled = true;
    };
  }, [load]);

  if (state.status === "ready") {
    return children(state.data);
  }

  if (state.status === "error") {
    return (
      <StateBanner
        tone="warning"
        title={dictionary.routeStates.consoleErrorTitle}
        detail={state.error}
      />
    );
  }

  return <ConsoleLoadingSkeleton />;
}
