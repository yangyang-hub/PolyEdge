import { ArbitrageRadarWorkbench } from "@/features/radar/components/arbitrage-radar-workbench";
import { getRadarPageData } from "@/features/radar/loaders/radar-page-data";

export default async function RadarPage() {
  const data = await getRadarPageData();

  return <ArbitrageRadarWorkbench data={data} />;
}
