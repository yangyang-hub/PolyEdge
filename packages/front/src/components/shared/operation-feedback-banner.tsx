"use client";

import { useEffect } from "react";
import { X } from "lucide-react";

import type { OperationActionResult } from "@/lib/api/actions";
import { StatusPill } from "@/components/shared/status-pill";
import { dictionary } from "@/lib/i18n/dictionaries";
import { cn } from "@/lib/utils";

export function OperationFeedbackBanner({
  feedback,
  className,
  onDismiss,
}: {
  feedback: OperationActionResult;
  className?: string;
  /** 传入后显示关闭按钮并在 8 秒后自动清除（用于页面级顶部 banner；对话框内的 banner 不传）。 */
  onDismiss?: () => void;
}) {
  useEffect(() => {
    if (!onDismiss) return;
    const timer = window.setTimeout(onDismiss, 8000);
    return () => window.clearTimeout(timer);
  }, [onDismiss, feedback]);

  return (
    <div
      role="status"
      aria-live="polite"
      className={cn(
        "relative rounded-lg border p-4",
        feedback.ok
          ? "border-secondary/20 bg-secondary/8"
          : "border-destructive/20 bg-destructive/8",
        className,
      )}
    >
      {onDismiss ? (
        <button
          type="button"
          onClick={onDismiss}
          aria-label={dictionary.common.close}
          className="absolute right-3 top-3 rounded text-muted-foreground transition-colors hover:text-foreground"
        >
          <X className="size-4" />
        </button>
      ) : null}
      <div className="flex flex-wrap items-center gap-2 pr-6">
        <StatusPill tone={feedback.ok ? "success" : "danger"}>
          {feedback.ok ? dictionary.feedback.operationQueued : dictionary.feedback.operationFailed}
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
