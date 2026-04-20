import { ApprovalsWorkbench } from "@/features/approvals/components/approvals-workbench";
import { getApprovalsPageData } from "@/features/approvals/loaders/approvals-page-data";

export default async function ApprovalsPage() {
  const data = await getApprovalsPageData();

  return <ApprovalsWorkbench {...data} />;
}
