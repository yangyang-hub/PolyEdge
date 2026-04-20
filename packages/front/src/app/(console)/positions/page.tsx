import { getPositionsPageData } from "@/features/positions/loaders/positions-page-data";
import { PositionsWorkbench } from "@/features/positions/components/positions-workbench";

export default async function PositionsPage() {
  const data = await getPositionsPageData();

  return <PositionsWorkbench data={data} />;
}
