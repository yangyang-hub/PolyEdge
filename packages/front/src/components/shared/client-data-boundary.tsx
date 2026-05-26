"use client";

import { useEffect, useState, type ReactNode } from "react";

import { ConsoleLoadingSkeleton } from "@/components/shared/console-loading-skeleton";
import { StateBanner } from "@/components/shared/state-banner";
import { useI18n } from "@/lib/i18n/client";
import type { I18nRuntime } from "@/lib/i18n/runtime";

type ClientDataBoundaryProps<TData> = {
  load: (i18n: I18nRuntime) => Promise<TData>;
  children: (data: TData, i18n: I18nRuntime) => ReactNode;
};

type LoadState<TData> =
  | { status: "loading"; data: null; error: null }
  | { status: "ready"; data: TData; error: null }
  | { status: "error"; data: null; error: string };

export function ClientDataBoundary<TData>({
  load,
  children,
}: ClientDataBoundaryProps<TData>) {
  const i18n = useI18n();
  const [state, setState] = useState<LoadState<TData>>({
    status: "loading",
    data: null,
    error: null,
  });

  useEffect(() => {
    let cancelled = false;

    void load(i18n)
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
  }, [i18n, load]);

  if (state.status === "ready") {
    return children(state.data, i18n);
  }

  if (state.status === "error") {
    return (
      <StateBanner
        tone="warning"
        title={i18n.dictionary.routeStates.consoleErrorTitle}
        detail={state.error}
      />
    );
  }

  return <ConsoleLoadingSkeleton />;
}
