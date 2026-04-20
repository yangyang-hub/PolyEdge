import { DashboardOverview } from "@/features/dashboard/components/dashboard-overview";
import { getDashboardPageData } from "@/features/dashboard/loaders/dashboard-page-data";

export default async function DashboardPage() {
  const data = await getDashboardPageData();

  return <DashboardOverview data={data} />;
}
