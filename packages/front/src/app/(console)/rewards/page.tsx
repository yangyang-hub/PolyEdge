import { RewardsWorkbench } from "@/features/rewards/components/rewards-workbench";
import { getRewardsPageData } from "@/features/rewards/loaders/rewards-page-data";

export default async function RewardsPage() {
  const data = await getRewardsPageData();

  return <RewardsWorkbench initialSnapshot={data.snapshot} />;
}
