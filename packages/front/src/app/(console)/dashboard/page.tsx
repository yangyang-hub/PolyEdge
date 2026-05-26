"use client";

import { ClientDataBoundary } from "@/components/shared/client-data-boundary";
import { DashboardOverview } from "@/features/dashboard/components/dashboard-overview";
import { getDashboardPageData } from "@/features/dashboard/loaders/dashboard-page-data";

export default function DashboardPage() {
  return (
    <ClientDataBoundary load={getDashboardPageData}>
      {(data) => <DashboardOverview data={data} />}
    </ClientDataBoundary>
  );
}
