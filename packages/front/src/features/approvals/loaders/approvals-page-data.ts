import "server-only";

import { listApprovals } from "@/server/api/system";
import { localizeGeneratedCopy } from "@/lib/i18n/generated-copy";
import { getServerI18n } from "@/lib/i18n/server";
import { selectFirstMatchingItem } from "@/server/loaders/console-loader-utils";
import { approvalSeverityTone, formatClock } from "@/lib/server/console-formatters";

export async function getApprovalsPageData() {
  const [{ data: approvals }, i18n] = await Promise.all([listApprovals(), getServerI18n()]);
  const { locale, dictionary, enumLabel } = i18n;
  const selectedApproval = approvals.length > 0
    ? selectFirstMatchingItem(
        approvals,
        [(approval) => approval.severity === "critical"],
        dictionary.routeStates.approvalsDataRequired,
      )
    : null;
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
      typeLabel: enumLabel(approval.type),
      status: approval.status,
      severity: approval.severity,
      severityLabel: enumLabel(approval.severity),
      severityTone: approvalSeverityTone(approval.severity),
      owner: localizeGeneratedCopy(locale, dictionary, approval.owner),
      createdAt: formatClock(approval.created_at),
      summary: localizeGeneratedCopy(locale, dictionary, approval.summary),
      riskPercent: riskMap[approval.severity],
      riskWidth: riskMap[approval.severity],
      resourceId: approval.resource_id,
      version: approval.version,
      requiresStepUpAuth: approval.requires_step_up_auth,
      isSelected: approval.id === selectedApproval?.id,
    })),
    selectedApproval: selectedApproval
      ? {
          typeLabel: enumLabel(selectedApproval.type),
          severity: selectedApproval.severity,
          severityLabel: enumLabel(selectedApproval.severity),
          severityTone: approvalSeverityTone(selectedApproval.severity),
          summary: localizeGeneratedCopy(locale, dictionary, selectedApproval.summary),
          resourceId: selectedApproval.resource_id,
          version: selectedApproval.version,
          requiresStepUpAuth: selectedApproval.requires_step_up_auth,
        }
      : null,
  };
}
