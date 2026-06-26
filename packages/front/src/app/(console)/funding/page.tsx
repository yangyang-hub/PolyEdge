"use client";

import { ClientDataBoundary } from "@/components/shared/client-data-boundary";
import { FundingWorkbench } from "@/features/funding/components/funding-workbench";
import { getFundingPageData } from "@/features/funding/loaders/funding-page-data";

export default function FundingPage() {
  return (
    <ClientDataBoundary load={getFundingPageData}>
      {(data) => <FundingWorkbench initialStatus={data.status} />}
    </ClientDataBoundary>
  );
}
