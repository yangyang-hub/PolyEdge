import { SignalsWorkbench } from "@/features/signals/components/signals-workbench";
import { getSignalsPageData } from "@/features/signals/loaders/signals-page-data";

export default async function SignalsPage() {
  const data = await getSignalsPageData();

  return <SignalsWorkbench {...data} />;
}
