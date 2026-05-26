"use client";

import { ClientDataBoundary } from "@/components/shared/client-data-boundary";
import { getPositionsPageData } from "@/features/positions/loaders/positions-page-data";
import { PositionsWorkbench } from "@/features/positions/components/positions-workbench";

export default function PositionsPage() {
  return (
    <ClientDataBoundary load={getPositionsPageData}>
      {(data) => <PositionsWorkbench data={data} />}
    </ClientDataBoundary>
  );
}
