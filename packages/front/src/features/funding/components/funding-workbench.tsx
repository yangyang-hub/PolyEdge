"use client";

import {
  CheckCircle2,
  ExternalLink,
  Loader2,
  ShieldCheck,
} from "lucide-react";
import { useEffect, useMemo, useState, useTransition } from "react";

import { ActionDialog } from "@/components/shared/action-dialog";
import { PageHeader } from "@/components/shared/page-header";
import { StatusPill } from "@/components/shared/status-pill";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardAction,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { FundingSafetyCard, FundingStepsCard } from "@/features/funding/components/funding-info-cards";
import { FundingReviewCard } from "@/features/funding/components/funding-review-card";
import {
  clearFundingTransferIntent,
  createFundingTransferIntent,
  loadFundingTransferIntent,
  saveFundingTransferIntent,
  type FundingTransferIntent,
} from "@/features/funding/lib/funding-intent";
import {
  fundingAmountError,
  formatFundingTokenBalance,
  getFundingTokenNoteKey,
  getPolygonFundingToken,
  parseTokenAmountToUnits,
} from "@/features/funding/lib/polygon-funding";
import type { FundingSubmissionSnapshot } from "@/features/funding/types";
import { submitFundingTransferAction, type OperationActionResult } from "@/lib/api/actions";
import type { FundingStatusDto } from "@/lib/contracts/dto";
import { dictionary, formatMessage } from "@/lib/i18n/dictionaries";
import { cn } from "@/lib/utils";

type FundingWorkbenchProps = {
  initialStatus: FundingStatusDto;
};

const officialDepositUrl = "https://polymarket.com/profile";

type FundingConfirmState = {
  note: string;
  stepUpCode: string;
  attempted: boolean;
  intent: FundingTransferIntent;
};

export function FundingWorkbench({ initialStatus }: FundingWorkbenchProps) {
  const [selectedTokenId, setSelectedTokenId] = useState(initialStatus.tokens[0]?.id ?? "");
  const [amount, setAmount] = useState("");
  const [confirmed, setConfirmed] = useState(false);
  const [copyMessage, setCopyMessage] = useState<string | null>(null);
  const [fieldErrors, setFieldErrors] = useState<OperationActionResult["fieldErrors"]>({});
  const [confirm, setConfirm] = useState<FundingConfirmState | null>(null);
  const [dialogFeedback, setDialogFeedback] = useState<OperationActionResult | null>(null);
  const [activeIntent, setActiveIntent] = useState<FundingTransferIntent | null>(null);
  const [submission, setSubmission] = useState<FundingSubmissionSnapshot>({
    status: "idle",
    message: initialStatus.configuration_error ?? null,
    transfer: null,
  });
  const [isPending, startTransition] = useTransition();

  const selectedToken = useMemo(
    () => getPolygonFundingToken(initialStatus.tokens, selectedTokenId),
    [initialStatus.tokens, selectedTokenId],
  );
  const amountUnits = useMemo(
    () => (selectedToken ? parseTokenAmountToUnits(amount, selectedToken.decimals) : null),
    [amount, selectedToken],
  );
  const amountValidation = useMemo(
    () => selectedToken && amount.trim()
      ? fundingAmountError(amount, selectedToken, initialStatus.max_transfer_amount)
      : null,
    [amount, initialStatus.max_transfer_amount, selectedToken],
  );
  const amountErrorMessage = selectedToken && amountValidation
    ? amountValidation === "below_minimum"
      ? formatMessage(dictionary.funding.amountBelowMinimum, {
          minimum: selectedToken.min_transfer_amount,
          symbol: selectedToken.symbol,
        })
      : amountValidation === "above_maximum"
        ? formatMessage(dictionary.funding.amountAboveMaximum, {
            maximum: initialStatus.max_transfer_amount,
            symbol: selectedToken.symbol,
          })
        : amountValidation === "above_balance"
          ? dictionary.funding.amountAboveBalance
          : dictionary.funding.invalidAmount
    : null;
  const amountValid = amountUnits !== null && amountValidation === null;
  const canSubmit =
    initialStatus.enabled &&
    selectedToken !== null &&
    amountValid &&
    confirmed &&
    !isPending;

  useEffect(() => {
    const timeout = window.setTimeout(() => {
      const restored = loadFundingTransferIntent();
      if (!restored || !initialStatus.tokens.some((token) => token.id === restored.tokenId)) return;
      setSelectedTokenId(restored.tokenId);
      setAmount(restored.amount);
      setActiveIntent(restored);
    }, 0);
    return () => window.clearTimeout(timeout);
  }, [initialStatus.tokens]);

  function invalidateIntent() {
    clearFundingTransferIntent();
    setActiveIntent(null);
    setDialogFeedback(null);
  }

  function copyToClipboard(value: string, message: string) {
    if (!navigator.clipboard) {
      setCopyMessage(dictionary.funding.walletMessages.failed);
      return;
    }

    void navigator.clipboard
      .writeText(value)
      .then(() => setCopyMessage(message))
      .catch(() => setCopyMessage(dictionary.funding.walletMessages.failed));
  }

  function openTransferConfirmation() {
    if (!selectedToken || !canSubmit) {
      setSubmission((current) => ({
        ...current,
        status: "error",
        message: initialStatus.enabled ? dictionary.funding.confirmRequired : dictionary.funding.configUnavailable,
      }));
      return;
    }

    const intent = activeIntent?.tokenId === selectedToken.id && activeIntent.amount === amount.trim()
      ? activeIntent
      : createFundingTransferIntent(selectedToken.id, amount);
    saveFundingTransferIntent(intent);
    setActiveIntent(intent);
    setDialogFeedback(null);
    setConfirm({ note: "", stepUpCode: "", attempted: false, intent });
  }

  function submitTransfer() {
    if (!selectedToken || !confirm) return;
    const next = { ...confirm, attempted: true };
    setConfirm(next);
    if (!next.note.trim()) {
      window.requestAnimationFrame(() => document.getElementById("operation-note")?.focus());
      return;
    }
    if (!next.stepUpCode.trim()) {
      window.requestAnimationFrame(() => document.getElementById("step-up-code")?.focus());
      return;
    }

    setSubmission({ status: "submitting", message: null, transfer: null });
    setFieldErrors({});
    setDialogFeedback(null);

    startTransition(async () => {
      const result = await submitFundingTransferAction({
        tokenId: selectedToken.id,
        amount,
        confirmed: true,
        idempotencyKey: next.intent.idempotencyKey,
        operatorNote: next.note,
        stepUpCode: next.stepUpCode,
      });

      setFieldErrors(result.fieldErrors ?? {});
      setDialogFeedback(result);
      setSubmission({
        status: result.ok ? "submitted" : "error",
        message: result.message,
        transfer: result.transfer ?? null,
      });
      if (result.ok) {
        clearFundingTransferIntent();
        setActiveIntent(null);
        setConfirm(null);
      }
    });
  }

  const submitting = isPending || submission.status === "submitting";
  const tokenNote = selectedToken ? dictionary.funding.tokenNotes[getFundingTokenNoteKey(selectedToken)] : "";

  return (
    <div className="space-y-6">
      <PageHeader
        eyebrow={dictionary.funding.eyebrow}
        title={dictionary.funding.title}
        description={dictionary.funding.description}
        actions={
          <>
            <StatusPill tone="primary">{dictionary.funding.polygonOnly}</StatusPill>
            <StatusPill tone={initialStatus.enabled ? "success" : "warning"}>
              {initialStatus.enabled ? dictionary.funding.selfCustody : dictionary.funding.configUnavailable}
            </StatusPill>
          </>
        }
      />

      <section className="grid gap-4 xl:grid-cols-[1.15fr_0.85fr]">
        <Card>
          <CardHeader>
            <CardTitle>{dictionary.funding.formTitle}</CardTitle>
            <CardDescription>{dictionary.funding.formDescription}</CardDescription>
            <CardAction>
              <Button asChild size="sm" variant="outline">
                <a href={officialDepositUrl} rel="noreferrer" target="_blank">
                  <ExternalLink className="size-3.5" />
                  {dictionary.funding.openOfficialDeposit}
                </a>
              </Button>
            </CardAction>
          </CardHeader>
          <CardContent className="space-y-5">
            {!initialStatus.enabled ? (
              <div className="rounded-lg border border-destructive/35 bg-destructive/10 p-3 text-sm text-destructive">
                {initialStatus.configuration_error ?? dictionary.funding.walletMessages.notConfigured}
              </div>
            ) : null}
            {initialStatus.balance_error ? (
              <div className="rounded-lg border border-amber-300/30 bg-amber-400/10 p-3 text-sm text-amber-200">
                {dictionary.funding.balanceUnavailable}: {initialStatus.balance_error}
              </div>
            ) : null}

            <fieldset className="space-y-2">
              <legend className="text-xs font-medium text-muted-foreground">
                {dictionary.funding.token}
              </legend>
              <div className="grid gap-2 sm:grid-cols-2">
                {initialStatus.tokens.map((token) => {
                  const note = dictionary.funding.tokenNotes[getFundingTokenNoteKey(token)];
                  const balance = formatFundingTokenBalance(token.balance, token.symbol);

                  return (
                    <label
                      key={token.id}
                      className={cn(
                        "flex min-h-20 cursor-pointer flex-col items-start justify-between rounded-lg border p-3 text-left transition-colors has-[:focus-visible]:ring-2 has-[:focus-visible]:ring-ring has-[:focus-visible]:ring-offset-2",
                        selectedToken?.id === token.id
                          ? "border-primary/45 bg-primary/12 text-foreground"
                          : "border-border/80 bg-background/35 text-muted-foreground hover:border-primary/30 hover:text-foreground",
                      )}
                    >
                      <input
                        className="sr-only"
                        type="radio"
                        name="funding-token"
                        value={token.id}
                        checked={selectedToken?.id === token.id}
                        onChange={() => {
                          invalidateIntent();
                          setSelectedTokenId(token.id);
                        }}
                      />
                      <span className="flex w-full items-center justify-between gap-3">
                        <span className="font-heading text-base font-semibold text-foreground">{token.symbol}</span>
                        {selectedToken?.id === token.id ? <CheckCircle2 className="size-4 text-secondary" /> : null}
                      </span>
                      <span className="mt-2 text-xs leading-5">{note}</span>
                      <span className="mt-2 text-xs font-medium text-foreground">
                        {dictionary.funding.chainBalance}: {balance || dictionary.funding.balanceUnavailable}
                      </span>
                    </label>
                  );
                })}
              </div>
              {fieldErrors?.tokenId ? <p role="alert" className="text-xs text-destructive">{fieldErrors.tokenId}</p> : null}
            </fieldset>

            <div className="space-y-2">
              <label className="text-xs font-medium text-muted-foreground" htmlFor="funding-amount">
                {dictionary.funding.amount}
              </label>
              <Input
                id="funding-amount"
                inputMode="decimal"
                value={amount}
                placeholder={dictionary.funding.amountPlaceholder}
                aria-invalid={Boolean(amountErrorMessage || fieldErrors?.amount)}
                aria-describedby="funding-amount-help funding-amount-error"
                onChange={(event) => {
                  invalidateIntent();
                  setAmount(event.target.value);
                }}
              />
              <p id="funding-amount-help" className="text-xs text-muted-foreground">
                {selectedToken
                  ? formatMessage(dictionary.funding.amountLimits, {
                      minimum: selectedToken.min_transfer_amount,
                      maximum: initialStatus.max_transfer_amount,
                      symbol: selectedToken.symbol,
                    })
                  : dictionary.funding.selectTokenFirst}
              </p>
              <p id="funding-amount-error" role={amountErrorMessage || fieldErrors?.amount ? "alert" : undefined} className="text-xs text-destructive">
                {fieldErrors?.amount ?? amountErrorMessage ?? ""}
              </p>
            </div>

            <label className="flex items-start gap-3 rounded-lg border border-border/70 bg-background/35 p-3 text-sm text-muted-foreground">
              <input
                type="checkbox"
                checked={confirmed}
                onChange={(event) => setConfirmed(event.target.checked)}
                aria-describedby="funding-confirm-help"
                className="mt-1 size-4 accent-primary"
              />
              <span id="funding-confirm-help">{dictionary.funding.confirmLabel}</span>
            </label>
            {fieldErrors?.confirmed ? <p role="alert" className="text-xs text-destructive">{fieldErrors.confirmed}</p> : null}

            <Button disabled={!canSubmit} onClick={openTransferConfirmation} type="button">
              {submitting ? <Loader2 className="size-4 animate-spin" /> : <ShieldCheck className="size-4" />}
              {submitting ? dictionary.funding.sending : dictionary.funding.sendTransfer}
            </Button>

            {submission.message || copyMessage ? (
              <div aria-live="polite" role={submission.status === "error" ? "alert" : "status"} className="rounded-lg border border-border/70 bg-muted/35 p-3 text-sm text-muted-foreground">
                {submission.message ?? copyMessage}
              </div>
            ) : null}
          </CardContent>
        </Card>

        <div className="space-y-4">
          {selectedToken ? (
            <FundingReviewCard
              amount={amount}
              amountUnits={amountUnits}
              onCopy={copyToClipboard}
              selectedToken={selectedToken}
              status={initialStatus}
              submission={submission}
              tokenNote={tokenNote}
            />
          ) : null}
          <FundingSafetyCard />
        </div>
      </section>

      <FundingStepsCard />

      {confirm && selectedToken ? (
        <ActionDialog
          open
          onOpenChange={(open) => {
            if (!open && !submitting) setConfirm(null);
          }}
          title={dictionary.funding.confirmDialogTitle}
          description={dictionary.funding.confirmDialogDescription}
          confirmLabel={dictionary.funding.confirmDialogAction}
          confirmVariant="destructive"
          isPending={submitting}
          note={confirm.note}
          onNoteChange={(value) => setConfirm((current) => current ? { ...current, note: value } : current)}
          noteError={
            confirm.attempted && !confirm.note.trim()
              ? dictionary.funding.operatorNoteRequired
              : confirm.note.length > 500
                ? dictionary.funding.operatorNoteTooLong
                : fieldErrors?.note
          }
          stepUpCode={confirm.stepUpCode}
          onStepUpCodeChange={(value) => setConfirm((current) => current ? { ...current, stepUpCode: value } : current)}
          stepUpCodeError={
            confirm.attempted && !confirm.stepUpCode.trim()
              ? dictionary.funding.stepUpRequired
              : fieldErrors?.stepUpCode
          }
          requiresStepUp
          feedback={dialogFeedback}
          context={
            <div>
              <p className="font-medium text-foreground">{dictionary.funding.riskSummaryTitle}</p>
              <ul className="mt-2 list-disc space-y-1 pl-5">
                <li>{formatMessage(dictionary.funding.riskSummaryAmount, { amount, symbol: selectedToken.symbol })}</li>
                <li>{dictionary.funding.riskSummaryIrreversible}</li>
                <li>{dictionary.funding.riskSummaryRetry}</li>
              </ul>
            </div>
          }
          onSubmit={submitTransfer}
        />
      ) : null}
    </div>
  );
}
