"use client";

import { ClientDataBoundary } from "@/components/shared/client-data-boundary";
import { RewardsWorkbench } from "@/features/rewards/components/rewards-workbench";
import { getRewardsPageData } from "@/features/rewards/loaders/rewards-page-data";

export default function RewardsPage() {
  return (
    <ClientDataBoundary load={getRewardsPageData}>
      {(data) => <RewardsWorkbench initialSnapshot={data.snapshot} />}
    </ClientDataBoundary>
  );
}
