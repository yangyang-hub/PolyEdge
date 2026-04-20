import type { OperationActionResult } from "@/server/actions/action-result";

import { StatusPill } from "@/components/shared/status-pill";
import { cn } from "@/lib/utils";

export function OperationFeedbackBanner({
  feedback,
  className,
}: {
  feedback: OperationActionResult;
  className?: string;
}) {
  return (
    <div
      className={cn(
        "rounded-lg border p-4",
        feedback.ok
          ? "border-secondary/20 bg-secondary/8"
          : "border-destructive/20 bg-destructive/8",
        className,
      )}
    >
      <div className="flex flex-wrap items-center gap-2">
        <StatusPill tone={feedback.ok ? "success" : "danger"}>
          {feedback.ok ? "operation queued" : "operation failed"}
        </StatusPill>
        {feedback.status ? <StatusPill tone="primary">{feedback.status}</StatusPill> : null}
      </div>
      <p className="mt-3 text-sm text-foreground">{feedback.message}</p>
      {(feedback.requestId || feedback.traceId || feedback.operationId) ? (
        <div className="mt-3 grid gap-2 text-[11px] text-muted-foreground md:grid-cols-3">
          {feedback.requestId ? <p>request_id: {feedback.requestId}</p> : null}
          {feedback.traceId ? <p>trace_id: {feedback.traceId}</p> : null}
          {feedback.operationId ? <p>operation_id: {feedback.operationId}</p> : null}
        </div>
      ) : null}
    </div>
  );
}
