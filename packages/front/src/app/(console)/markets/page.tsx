import { MarketsWorkbench } from "@/features/markets/components/markets-workbench";
import { getMarketsPageData } from "@/features/markets/loaders/markets-page-data";

export default async function MarketsPage() {
  const data = await getMarketsPageData();

  return <MarketsWorkbench data={data} />;
}
