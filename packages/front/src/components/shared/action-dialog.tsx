"use client";

import type { ReactNode } from "react";

import type { OperationActionResult } from "@/server/actions/action-result";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import { OperationFeedbackBanner } from "@/components/shared/operation-feedback-banner";

type ActionDialogProps = {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  title: string;
  description: string;
  confirmLabel: string;
  confirmVariant?: "default" | "destructive";
  isPending: boolean;
  note: string;
  onNoteChange: (value: string) => void;
  noteError?: string;
  stepUpCode: string;
  onStepUpCodeChange: (value: string) => void;
  stepUpCodeError?: string;
  requiresStepUp: boolean;
  onSubmit: () => void;
  feedback?: OperationActionResult | null;
  context?: ReactNode;
  children?: ReactNode;
};

export function ActionDialog({
  open,
  onOpenChange,
  title,
  description,
  confirmLabel,
  confirmVariant = "default",
  isPending,
  note,
  onNoteChange,
  noteError,
  stepUpCode,
  onStepUpCodeChange,
  stepUpCodeError,
  requiresStepUp,
  onSubmit,
  feedback,
  context,
  children,
}: ActionDialogProps) {
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-lg border-white/10 bg-card p-0">
        <DialogHeader className="border-b border-white/8 px-5 py-4">
          <DialogTitle>{title}</DialogTitle>
          <DialogDescription>{description}</DialogDescription>
        </DialogHeader>

        <div className="space-y-4 px-5 py-5">
          {feedback ? <OperationFeedbackBanner feedback={feedback} /> : null}
          {context ? <div className="rounded-md bg-accent/45 p-4 text-sm text-muted-foreground">{context}</div> : null}
          {children}
          <div className="space-y-2">
            <label className="text-sm font-medium text-foreground" htmlFor="operation-note">
              Operator note
            </label>
            <Textarea
              id="operation-note"
              value={note}
              onChange={(event) => onNoteChange(event.target.value)}
              className="min-h-28 rounded-sm border-white/10 bg-accent/45 text-foreground"
              placeholder="Describe why this action is warranted, what context was reviewed and what should happen next."
            />
            {noteError ? <p className="text-xs text-destructive">{noteError}</p> : null}
          </div>

          {requiresStepUp ? (
            <div className="space-y-2">
              <label className="text-sm font-medium text-foreground" htmlFor="step-up-code">
                Step-up code
              </label>
              <Input
                id="step-up-code"
                value={stepUpCode}
                onChange={(event) => onStepUpCodeChange(event.target.value)}
                className="h-10 rounded-sm border-white/10 bg-accent/45"
                placeholder="Enter step-up confirmation code"
              />
              {stepUpCodeError ? <p className="text-xs text-destructive">{stepUpCodeError}</p> : null}
            </div>
          ) : null}
        </div>

        <DialogFooter className="border-white/8 bg-card/90">
          <Button
            variant="outline"
            className="rounded-sm border-white/10 bg-accent/45 hover:bg-accent"
            onClick={() => onOpenChange(false)}
          >
            Cancel
          </Button>
          <Button
            variant={confirmVariant}
            className="rounded-sm"
            onClick={onSubmit}
            disabled={isPending}
          >
            {isPending ? "Submitting..." : confirmLabel}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
