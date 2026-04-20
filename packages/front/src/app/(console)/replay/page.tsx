import { getReplayPageData } from "@/features/replay/loaders/replay-page-data";
import { ReplayWorkbench } from "@/features/replay/components/replay-workbench";

export default async function ReplayPage() {
  const data = await getReplayPageData();

  return <ReplayWorkbench data={data} />;
}
