"use client";

import { ClientDataBoundary } from "@/components/shared/client-data-boundary";
import { RiskControlCenter } from "@/features/risk/components/risk-control-center";
import { getRiskPageData } from "@/features/risk/loaders/risk-page-data";

export default function RiskPage() {
  return (
    <ClientDataBoundary load={getRiskPageData}>
      {(data) => <RiskControlCenter data={data} />}
    </ClientDataBoundary>
  );
}
