"use client";

import { ClientDataBoundary } from "@/components/shared/client-data-boundary";
import { CopyTradeWorkbench } from "@/features/copytrade/components/copytrade-workbench";
import { getCopyTradePageData } from "@/features/copytrade/loaders/copytrade-page-data";

export default function CopyTradingPage() {
  return (
    <ClientDataBoundary load={getCopyTradePageData}>
      {(data) => (
        <CopyTradeWorkbench
          initialSnapshot={data.snapshot}
          initialSmartMoneySnapshot={data.smartMoneySnapshot}
        />
      )}
    </ClientDataBoundary>
  );
}
