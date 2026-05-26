"use client";

import { ClientDataBoundary } from "@/components/shared/client-data-boundary";
import { ArbitrageRadarWorkbench } from "@/features/radar/components/arbitrage-radar-workbench";
import { getRadarPageData } from "@/features/radar/loaders/radar-page-data";

export default function RadarPage() {
  return (
    <ClientDataBoundary load={getRadarPageData}>
      {(data) => <ArbitrageRadarWorkbench data={data} />}
    </ClientDataBoundary>
  );
}
