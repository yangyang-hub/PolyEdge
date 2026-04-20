import { RiskControlCenter } from "@/features/risk/components/risk-control-center";
import { getRiskPageData } from "@/features/risk/loaders/risk-page-data";

export default async function RiskPage() {
  const data = await getRiskPageData();

  return <RiskControlCenter data={data} />;
}
