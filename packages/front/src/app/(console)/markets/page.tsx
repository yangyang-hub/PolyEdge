"use client";

import { ClientDataBoundary } from "@/components/shared/client-data-boundary";
import { MarketsWorkbench } from "@/features/markets/components/markets-workbench";
import { getMarketsPageData } from "@/features/markets/loaders/markets-page-data";

export default function MarketsPage() {
  return (
    <ClientDataBoundary load={getMarketsPageData}>
      {(data) => <MarketsWorkbench data={data} />}
    </ClientDataBoundary>
  );
}
