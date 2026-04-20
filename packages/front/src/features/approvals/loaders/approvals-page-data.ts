import "server-only";

import { listApprovals } from "@/server/api/system";
import { selectFirstMatchingItem } from "@/server/loaders/console-loader-utils";
import { approvalSeverityTone, formatClock, humanizeSnakeCase } from "@/lib/server/console-formatters";

export async function getApprovalsPageData() {
  const { data: approvals } = await listApprovals();
  const selectedApproval = selectFirstMatchingItem(
    approvals,
    [(approval) => approval.severity === "critical"],
    "Approvals page requires at least one approval fixture or API result.",
  );
  const riskMap = {
    critical: "98%",
    warning: "32%",
    info: "08%",
  } as const;

  return {
    pendingCount: approvals.filter((approval) => approval.status === "pending").length,
    completedCount: approvals.filter((approval) => approval.status !== "pending").length,
    approvals: approvals.map((approval) => ({
      id: approval.id,
      typeLabel: humanizeSnakeCase(approval.type),
      status: approval.status,
      severity: approval.severity,
      severityTone: approvalSeverityTone(approval.severity),
      owner: approval.owner,
      createdAt: formatClock(approval.created_at),
      summary: approval.summary,
      riskPercent: riskMap[approval.severity],
      riskWidth: riskMap[approval.severity],
      resourceId: approval.resource_id,
      version: approval.version,
      requiresStepUpAuth: approval.requires_step_up_auth,
      isSelected: approval.id === selectedApproval.id,
    })),
    selectedApproval: {
      typeLabel: humanizeSnakeCase(selectedApproval.type),
      severity: selectedApproval.severity,
      severityLabel: selectedApproval.severity,
      severityTone: approvalSeverityTone(selectedApproval.severity),
      summary: selectedApproval.summary,
      resourceId: selectedApproval.resource_id,
      version: selectedApproval.version,
      requiresStepUpAuth: selectedApproval.requires_step_up_auth,
    },
  };
}
