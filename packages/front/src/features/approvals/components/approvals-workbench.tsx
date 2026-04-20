"use client";

import { startTransition, useDeferredValue, useEffect, useState, useTransition } from "react";
import { Filter, ShieldAlert } from "lucide-react";
import { toast } from "sonner";

import { submitApprovalDecisionAction } from "@/server/actions/approval-actions";
import type { OperationActionResult } from "@/server/actions/action-result";
import { ActionDialog } from "@/components/shared/action-dialog";
import { useConsoleRealtimeChannel } from "@/components/shared/console-realtime-provider";
import { PageHeader } from "@/components/shared/page-header";
import { Button } from "@/components/ui/button";
import {
  Sheet,
  SheetContent,
  SheetDescription,
  SheetHeader,
  SheetTitle,
  SheetTrigger,
} from "@/components/ui/sheet";
import { MeterBar } from "@/components/shared/meter-bar";
import { OperationFeedbackBanner } from "@/components/shared/operation-feedback-banner";
import { StatusPill } from "@/components/shared/status-pill";
import { WorkbenchDetailPane, WorkbenchLayout } from "@/components/shared/workbench-layout";
import { WorkbenchSegmentedControl } from "@/components/shared/workbench-segmented-control";
import type { RiskStreamPayload } from "@/lib/contracts/realtime";
import { isKeyboardSelect } from "@/lib/keyboard";
import {
  approvalRiskPercent,
  approvalSeverityTone,
  formatClock,
  humanizeSnakeCase,
} from "@/lib/realtime-formatters";

type ApprovalTone = "neutral" | "primary" | "success" | "warning" | "danger" | "violet";
type ApprovalStatus = "pending" | "approved" | "rejected";

type ApprovalItem = {
  id: string;
  typeLabel: string;
  status: ApprovalStatus;
  severity: string;
  severityTone: ApprovalTone;
  owner: string;
  createdAt: string;
  summary: string;
  riskPercent: string;
  riskWidth: string;
  resourceId: string;
  version: number;
  requiresStepUpAuth: boolean;
  isSelected: boolean;
};

type SelectedApproval = {
  typeLabel: string;
  severity: string;
  severityLabel: string;
  severityTone: ApprovalTone;
  summary: string;
  resourceId: string;
  version: number;
  requiresStepUpAuth: boolean;
};

type ApprovalsWorkbenchProps = {
  pendingCount: number;
  completedCount: number;
  approvals: ApprovalItem[];
  selectedApproval: SelectedApproval;
};

type ApprovalTab = "pending" | "completed";
type ApprovalDecision = "approved" | "rejected" | null;

function buildApprovalItem(
  payload: RiskStreamPayload,
  current?: ApprovalItem,
): ApprovalItem | null {
  if (
    !payload.approval_id ||
    !payload.approval_type ||
    !payload.approval_status ||
    !payload.approval_severity ||
    !payload.approval_owner ||
    !payload.approval_summary ||
    !payload.approval_resource_id
  ) {
    return current ?? null;
  }

  const riskPercent = approvalRiskPercent(payload.approval_severity);

  return {
    id: payload.approval_id,
    typeLabel: humanizeSnakeCase(payload.approval_type),
    status: payload.approval_status,
    severity: payload.approval_severity,
    severityTone: approvalSeverityTone(payload.approval_severity),
    owner: payload.approval_owner,
    createdAt: payload.created_at ? formatClock(payload.created_at) : current?.createdAt ?? "--:--:--",
    summary: payload.approval_summary,
    riskPercent,
    riskWidth: riskPercent,
    resourceId: payload.approval_resource_id,
    version: payload.version,
    requiresStepUpAuth: payload.approval_requires_step_up_auth ?? current?.requiresStepUpAuth ?? false,
    isSelected: current?.isSelected ?? false,
  };
}

function upsertApprovalItem(items: ApprovalItem[], payload: RiskStreamPayload): ApprovalItem[] {
  const current = items.find((item) => item.id === payload.approval_id);
  const nextItem = buildApprovalItem(payload, current);

  if (!nextItem) {
    return items;
  }

  if (current) {
    return items.map((item) => (item.id === nextItem.id ? nextItem : item));
  }

  return [nextItem, ...items];
}

function ApprovalsDetailPanel({
  approval,
  onOpenDecision,
}: {
  approval: ApprovalItem | SelectedApproval;
  onOpenDecision?: (decision: Exclude<ApprovalDecision, null>) => void;
}) {
  const approvalStatus = "status" in approval ? approval.status : "pending";

  return (
    <div className="space-y-5">
      <div className="space-y-2">
        <div className="flex flex-wrap gap-2">
          <StatusPill tone={approval.severityTone}>{approval.typeLabel}</StatusPill>
          <StatusPill tone={approval.severityTone}>
            {"severityLabel" in approval ? approval.severityLabel : approval.severity}
          </StatusPill>
        </div>
        <p className="font-heading text-lg font-bold tracking-tight text-foreground">{approval.summary}</p>
      </div>

      <div className="rounded-md bg-popover/70 p-4">
        <div className="mb-3 flex items-center gap-2">
          <ShieldAlert className="size-4 text-destructive" />
          <p className="text-[10px] font-bold uppercase tracking-[0.18em] text-muted-foreground">
            Risk and Audit Context
          </p>
        </div>
        <div className="space-y-2 text-sm text-muted-foreground">
          <p>Target resource: {approval.resourceId}</p>
          <p>Expected version: {approval.version}</p>
          <p>Current queue status: {approvalStatus}</p>
          <p>
            {approval.requiresStepUpAuth
              ? "Step-up auth and operator note are required before backend acceptance."
              : "Standard audit logging applies."}
          </p>
        </div>
      </div>

      {"status" in approval && approval.status === "pending" && onOpenDecision ? (
        <div className="grid gap-3 sm:grid-cols-2">
          <Button
            className="rounded-sm bg-primary text-primary-foreground hover:bg-primary/90"
            onClick={() => onOpenDecision("approved")}
          >
            Approve
          </Button>
          <Button
            variant="outline"
            className="rounded-sm border-destructive/30 bg-destructive/5 text-destructive hover:bg-destructive/10"
            onClick={() => onOpenDecision("rejected")}
          >
            Reject
          </Button>
        </div>
      ) : (
        <div className="rounded-md bg-accent/45 p-4 text-sm text-muted-foreground">
          This queue item is already completed. Open another pending item to submit a new decision.
        </div>
      )}
    </div>
  );
}

export function ApprovalsWorkbench({
  pendingCount,
  completedCount,
  approvals,
  selectedApproval: initialSelectedApproval,
}: ApprovalsWorkbenchProps) {
  const { lastEvent: lastRiskEvent } = useConsoleRealtimeChannel("risk");
  const [approvalItems, setApprovalItems] = useState(approvals);
  const [tab, setTab] = useState<ApprovalTab>("pending");
  const [selectedId, setSelectedId] = useState<string>(
    approvals.find((approval) => approval.isSelected)?.id ?? approvals[0]?.id ?? "",
  );
  const [decision, setDecision] = useState<ApprovalDecision>(null);
  const [note, setNote] = useState("");
  const [stepUpCode, setStepUpCode] = useState("");
  const [dialogFeedback, setDialogFeedback] = useState<OperationActionResult | null>(null);
  const [lastOperation, setLastOperation] = useState<OperationActionResult | null>(null);
  const [fieldErrors, setFieldErrors] = useState<OperationActionResult["fieldErrors"]>({});
  const [isPending, startActionTransition] = useTransition();
  const deferredTab = useDeferredValue(tab);

  useEffect(() => {
    const streamEvent = lastRiskEvent;

    if (!streamEvent || !streamEvent.type.startsWith("approval.")) {
      return;
    }

    startTransition(() => {
      setApprovalItems((currentItems) => upsertApprovalItem(currentItems, streamEvent.data));
    });
  }, [lastRiskEvent]);

  const visibleApprovals = approvalItems.filter((approval) => {
    if (deferredTab === "pending") {
      return approval.status === "pending";
    }

    return approval.status !== "pending";
  });

  const selectedApproval =
    visibleApprovals.find((approval) => approval.id === selectedId) ??
    approvalItems.find((approval) => approval.id === selectedId) ??
    visibleApprovals[0] ??
    approvalItems[0];
  const computedPendingCount = approvalItems.length > 0
    ? approvalItems.filter((approval) => approval.status === "pending").length
    : pendingCount;
  const computedCompletedCount = approvalItems.length > 0
    ? approvalItems.filter((approval) => approval.status !== "pending").length
    : completedCount;
  const runtimeModeLabel = lastRiskEvent?.data.mode ? humanizeSnakeCase(lastRiskEvent.data.mode) : null;
  const killSwitchActive = lastRiskEvent?.data.kill_switch ?? false;

  const tabs: Array<{ key: ApprovalTab; label: string }> = [
    {
      key: "pending",
      label: `Pending (${computedPendingCount})`,
    },
    {
      key: "completed",
      label: `Completed (${computedCompletedCount})`,
    },
  ];
  const filters = ["Type: all", "Severity: high+", "Risk: critical"];

  function selectApproval(approvalId: string) {
    startTransition(() => {
      setSelectedId(approvalId);
    });
  }

  function openDecisionDialog(nextDecision: Exclude<ApprovalDecision, null>) {
    setDecision(nextDecision);
    setDialogFeedback(null);
    setFieldErrors({});
    setStepUpCode("");
    setNote(
      nextDecision === "approved"
        ? "Reviewed ambiguity notes and current exposure. Approving for manual execution with operator oversight."
        : "Rejecting this request because current risk posture and settlement ambiguity do not justify execution.",
    );
  }

  function closeDecisionDialog() {
    setDecision(null);
    setDialogFeedback(null);
    setFieldErrors({});
    setStepUpCode("");
  }

  function submitDecision() {
    if (!selectedApproval || !("status" in selectedApproval) || selectedApproval.status !== "pending" || !decision) {
      return;
    }

    startActionTransition(async () => {
      const result = await submitApprovalDecisionAction({
        approvalId: selectedApproval.id,
        resourceId: selectedApproval.resourceId,
        expectedVersion: selectedApproval.version,
        decision,
        requiresStepUpAuth: selectedApproval.requiresStepUpAuth,
        note,
        stepUpCode,
      });

      setDialogFeedback(result);
      setLastOperation(result);
      setFieldErrors(result.fieldErrors ?? {});

      if (result.ok) {
        setApprovalItems((currentItems) =>
          currentItems.map((approval) =>
            approval.id === selectedApproval.id
              ? {
                  ...approval,
                  status: decision,
                }
              : approval,
          ),
        );

        toast.success(result.message, {
          description: [result.requestId, result.traceId].filter(Boolean).join(" · "),
        });
        closeDecisionDialog();
        return;
      }

      toast.error(result.message, {
        description: [result.requestId, result.traceId].filter(Boolean).join(" · "),
      });
    });
  }

  return (
    <div className="space-y-4">
      <PageHeader
        eyebrow="Manual Controls"
        title="Approvals"
        description="High-risk actions are staged here with rationale, risk context and operator decision logging."
        className="border-none pb-0"
        actions={
          <>
            <StatusPill tone="violet">{computedPendingCount} pending</StatusPill>
            <StatusPill tone="primary">{computedCompletedCount} completed</StatusPill>
            {runtimeModeLabel ? (
              <StatusPill tone={killSwitchActive ? "danger" : "warning"}>{runtimeModeLabel}</StatusPill>
            ) : null}
          </>
        }
      />

      <WorkbenchLayout>
        <div className="overflow-hidden rounded-lg bg-card/95 ring-1 ring-white/5">
          <div className="space-y-5 p-5">
            {lastOperation ? <OperationFeedbackBanner feedback={lastOperation} /> : null}
            <div className="flex flex-col gap-4 md:flex-row md:items-center md:justify-between">
              <WorkbenchSegmentedControl items={tabs} value={tab} onChange={setTab} />

              <div className="flex flex-wrap items-center gap-2">
                <span className="text-[10px] font-bold uppercase tracking-[0.2em] text-muted-foreground">
                  Filter by
                </span>
                {filters.map((label) => (
                  <Button
                    key={label}
                    variant="outline"
                    size="sm"
                    className="rounded-sm border-white/10 bg-accent/40 text-foreground hover:bg-accent"
                  >
                    <Filter className="size-3.5" />
                    {label}
                  </Button>
                ))}
              </div>
            </div>

            {visibleApprovals.length > 0 ? (
              <div className="overflow-x-auto">
                <table className="w-full border-separate border-spacing-y-2 text-left">
                  <thead>
                    <tr className="text-[10px] font-bold uppercase tracking-[0.2em] text-muted-foreground">
                      <th className="pb-2 pl-4">Item Type</th>
                      <th className="pb-2">Severity</th>
                      <th className="pb-2">Ambiguity / Risk</th>
                      <th className="pb-2">Created By</th>
                      <th className="pb-2 pr-4 text-right">Time</th>
                      <th className="pb-2 pr-4 text-right xl:hidden">Review</th>
                    </tr>
                  </thead>
                  <tbody className="text-sm">
                    {visibleApprovals.map((approval) => (
                      <tr
                        key={approval.id}
                        tabIndex={0}
                        onClick={() => selectApproval(approval.id)}
                        onKeyDown={(event) => {
                          if (isKeyboardSelect(event)) {
                            event.preventDefault();
                            selectApproval(approval.id);
                          }
                        }}
                        className={
                          approval.id === selectedApproval?.id
                            ? "cursor-pointer bg-accent/55 shadow-[inset_0_0_0_1px_rgba(179,197,255,0.22),inset_2px_0_0_#0066ff]"
                            : "cursor-pointer transition-colors hover:bg-accent/35"
                        }
                      >
                        <td className="rounded-l-md py-4 pl-4">
                          <div className="flex items-center gap-3">
                            <div
                              className={
                                approval.severity === "critical"
                                  ? "size-2 rounded-full bg-destructive"
                                  : approval.severity === "warning"
                                    ? "size-2 rounded-full bg-secondary"
                                    : "size-2 rounded-full bg-primary"
                              }
                            />
                            <div>
                              <p className="font-bold uppercase tracking-wide text-foreground">
                                {approval.typeLabel}
                              </p>
                              <p className="mt-1 text-[11px] text-muted-foreground">{approval.summary}</p>
                            </div>
                          </div>
                        </td>
                        <td>
                          <StatusPill tone={approval.severityTone}>{approval.severity}</StatusPill>
                        </td>
                        <td>
                          <div className="w-24 space-y-1">
                            <MeterBar
                              value={approval.riskWidth}
                              tone={approval.severityTone}
                              trackClassName="h-1 bg-background"
                            />
                            <span className="font-mono text-[10px] text-muted-foreground">
                              {approval.riskPercent}
                            </span>
                          </div>
                        </td>
                        <td className="text-sm text-foreground">{approval.owner}</td>
                        <td className="pr-4 text-right font-mono text-xs text-muted-foreground">{approval.createdAt}</td>
                        <td className="pr-4 text-right xl:hidden">
                          <Sheet>
                            <SheetTrigger asChild>
                              <Button
                                variant="ghost"
                                size="sm"
                                className="rounded-sm text-primary hover:bg-primary/10"
                                onClick={() => selectApproval(approval.id)}
                              >
                                Review
                              </Button>
                            </SheetTrigger>
                            <SheetContent className="w-full max-w-none border-white/10 bg-card p-0 sm:max-w-md">
                              <SheetHeader className="border-b border-white/8 px-5 py-4">
                                <SheetTitle>Approval Detail</SheetTitle>
                                <SheetDescription>
                                  Operator note, risk context and audit requirements.
                                </SheetDescription>
                              </SheetHeader>
                              <div className="overflow-y-auto px-5 py-5">
                                <ApprovalsDetailPanel approval={approval} onOpenDecision={openDecisionDialog} />
                              </div>
                            </SheetContent>
                          </Sheet>
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            ) : (
              <div className="px-5 py-10 text-center">
                <p className="font-heading text-lg font-bold text-foreground">No approvals in this queue</p>
                <p className="mt-2 text-sm text-muted-foreground">
                  Current filters returned no approval items.
                </p>
              </div>
            )}
          </div>
        </div>

        <WorkbenchDetailPane desktopOnly>
          <ApprovalsDetailPanel
            approval={selectedApproval ?? initialSelectedApproval}
            onOpenDecision={openDecisionDialog}
          />
        </WorkbenchDetailPane>

        <ActionDialog
          open={decision !== null}
          onOpenChange={(open) => {
            if (!open) {
              closeDecisionDialog();
            }
          }}
          title={decision === "approved" ? "Approve queue item" : "Reject queue item"}
          description="This decision is submitted as a protected server action and returns audit identifiers for downstream tracing."
          confirmLabel={decision === "approved" ? "Queue approval" : "Queue rejection"}
          confirmVariant={decision === "approved" ? "default" : "destructive"}
          isPending={isPending}
          note={note}
          onNoteChange={setNote}
          noteError={fieldErrors?.note}
          stepUpCode={stepUpCode}
          onStepUpCodeChange={setStepUpCode}
          stepUpCodeError={fieldErrors?.stepUpCode}
          requiresStepUp={"requiresStepUpAuth" in (selectedApproval ?? {}) ? selectedApproval.requiresStepUpAuth : false}
          onSubmit={submitDecision}
          feedback={dialogFeedback}
          context={
            selectedApproval && "resourceId" in selectedApproval ? (
              <div className="space-y-1">
                <p>Resource: {selectedApproval.resourceId}</p>
                <p>Expected version: {selectedApproval.version}</p>
                <p>Severity: {selectedApproval.severity}</p>
              </div>
            ) : null
          }
        />
      </WorkbenchLayout>
    </div>
  );
}
