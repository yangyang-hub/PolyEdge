"use client";

import { ClientDataBoundary } from "@/components/shared/client-data-boundary";
import { getReplayPageData } from "@/features/replay/loaders/replay-page-data";
import { ReplayWorkbench } from "@/features/replay/components/replay-workbench";

export default function ReplayPage() {
  return (
    <ClientDataBoundary load={getReplayPageData}>
      {(data) => <ReplayWorkbench data={data} />}
    </ClientDataBoundary>
  );
}
