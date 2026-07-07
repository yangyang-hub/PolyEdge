"use client";

import { ClientDataBoundary } from "@/components/shared/client-data-boundary";
import { RewardsFairValueWorkbench } from "@/features/rewards/components/rewards-fair-value-workbench";
import { getRewardsFairValuePageData } from "@/features/rewards/loaders/rewards-fair-value-page-data";

export default function RewardsFairValuePage() {
  return (
    <ClientDataBoundary load={getRewardsFairValuePageData}>
      {(data) => <RewardsFairValueWorkbench initialSnapshot={data.snapshot} />}
    </ClientDataBoundary>
  );
}
