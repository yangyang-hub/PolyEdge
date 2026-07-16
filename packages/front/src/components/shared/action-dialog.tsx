"use client";

import type { ReactNode } from "react";

import type { OperationActionResult } from "@/lib/api/actions";
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
import { OperationFeedbackBanner } from "@/components/shared/operation-feedback-banner";
import { dictionary } from "@/lib/i18n/dictionaries";

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
  onSubmit: () => void;
  confirmDisabled?: boolean;
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
  onSubmit,
  confirmDisabled = false,
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
              {dictionary.actionDialog.operatorNote}
            </label>
            <Input
              id="operation-note"
              name="operator-note"
              autoComplete="off"
              maxLength={500}
              value={note}
              onChange={(event) => onNoteChange(event.target.value)}
              aria-invalid={Boolean(noteError)}
              aria-describedby={noteError ? "operation-note-error" : undefined}
              className="h-10 rounded-sm border-white/10 bg-accent/45 text-foreground"
              placeholder={dictionary.actionDialog.notePlaceholder}
            />
            {noteError ? <p id="operation-note-error" className="text-xs text-destructive">{noteError}</p> : null}
          </div>

        </div>

        <DialogFooter className="border-white/8 bg-card/90">
          <Button
            variant="outline"
            className="rounded-sm border-white/10 bg-accent/45 hover:bg-accent"
            onClick={() => onOpenChange(false)}
          >
            {dictionary.common.cancel}
          </Button>
          <Button
            variant={confirmVariant}
            className="rounded-sm"
            onClick={onSubmit}
            disabled={isPending || confirmDisabled}
          >
            {isPending ? dictionary.common.submitting : confirmLabel}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
