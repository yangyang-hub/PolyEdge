"use client";

import { ActionDialog } from "@/components/shared/action-dialog";
import { useI18n } from "@/lib/i18n/client";
import type { OperationActionResult } from "@/lib/api/actions";

import type { RiskDialog, RiskPageData } from "../types";

export function RiskActionDialogs({
  activeDialog,
  controls,
  note,
  onNoteChange,
  stepUpCode,
  onStepUpCodeChange,
  fieldErrors,
  isPending,
  dialogFeedback,
  onClose,
  onSubmitRelease,
  onSubmitKillSwitch,
}: {
  activeDialog: RiskDialog;
  controls: RiskPageData["controls"];
  note: string;
  onNoteChange: (value: string) => void;
  stepUpCode: string;
  onStepUpCodeChange: (value: string) => void;
  fieldErrors: OperationActionResult["fieldErrors"];
  isPending: boolean;
  dialogFeedback: OperationActionResult | null;
  onClose: () => void;
  onSubmitRelease: () => void;
  onSubmitKillSwitch: () => void;
}) {
  const { dictionary } = useI18n();

  return (
    <>
      <ActionDialog
        open={activeDialog === "release"}
        onOpenChange={(open) => {
          if (!open) {
            onClose();
          }
        }}
        title={dictionary.risk.releaseTitle}
        description={dictionary.risk.releaseDescription}
        confirmLabel={dictionary.risk.queueRelease}
        isPending={isPending}
        note={note}
        onNoteChange={onNoteChange}
        noteError={fieldErrors?.note}
        stepUpCode={stepUpCode}
        onStepUpCodeChange={onStepUpCodeChange}
        stepUpCodeError={fieldErrors?.stepUpCode}
        requiresStepUp
        onSubmit={onSubmitRelease}
        feedback={dialogFeedback}
        context={
          <div className="space-y-1">
            <p>{dictionary.risk.killSwitch}: {controls.killSwitch ? dictionary.common.active : dictionary.common.armed}</p>
          </div>
        }
      />

      <ActionDialog
        open={activeDialog === "kill_switch"}
        onOpenChange={(open) => {
          if (!open) {
            onClose();
          }
        }}
        title={controls.killSwitch ? dictionary.risk.releaseKillSwitch : dictionary.risk.triggerKillSwitch}
        description={dictionary.risk.killSwitchDescription}
        confirmLabel={controls.killSwitch ? dictionary.risk.queueRelease : dictionary.risk.queueKillSwitch}
        confirmVariant={controls.killSwitch ? "default" : "destructive"}
        isPending={isPending}
        note={note}
        onNoteChange={onNoteChange}
        noteError={fieldErrors?.note}
        stepUpCode={stepUpCode}
        onStepUpCodeChange={onStepUpCodeChange}
        stepUpCodeError={fieldErrors?.stepUpCode}
        requiresStepUp
        onSubmit={onSubmitKillSwitch}
        feedback={dialogFeedback}
        context={
          <div className="space-y-1">
            <p>{dictionary.risk.currentMode}: {controls.modeLabel}</p>
            <p>{dictionary.risk.killSwitchStatus}: {controls.killSwitch ? dictionary.common.active : dictionary.common.armed}</p>
          </div>
        }
      />
    </>
  );
}
