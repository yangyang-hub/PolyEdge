"use client";

import { ClientDataBoundary } from "@/components/shared/client-data-boundary";
import { SignalsWorkbench } from "@/features/signals/components/signals-workbench";
import { getSignalsPageData } from "@/features/signals/loaders/signals-page-data";

export default function SignalsPage() {
  return (
    <ClientDataBoundary load={getSignalsPageData}>
      {(data) => <SignalsWorkbench {...data} />}
    </ClientDataBoundary>
  );
}
