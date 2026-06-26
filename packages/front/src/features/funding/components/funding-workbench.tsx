"use client";

import {
  CheckCircle2,
  ExternalLink,
  Loader2,
  ShieldCheck,
} from "lucide-react";
import { useMemo, useState, useTransition } from "react";

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
  getFundingTokenNoteKey,
  getPolygonFundingToken,
  parseTokenAmountToUnits,
} from "@/features/funding/lib/polygon-funding";
import type { FundingSubmissionSnapshot } from "@/features/funding/types";
import { submitFundingTransferAction, type OperationActionResult } from "@/lib/api/actions";
import type { FundingStatusDto } from "@/lib/contracts/dto";
import { dictionary } from "@/lib/i18n/dictionaries";
import { cn } from "@/lib/utils";

type FundingWorkbenchProps = {
  initialStatus: FundingStatusDto;
};

const officialDepositUrl = "https://polymarket.com/profile";

export function FundingWorkbench({ initialStatus }: FundingWorkbenchProps) {
  const [selectedTokenId, setSelectedTokenId] = useState(initialStatus.tokens[0]?.id ?? "");
  const [amount, setAmount] = useState("");
  const [stepUpCode, setStepUpCode] = useState("");
  const [confirmed, setConfirmed] = useState(false);
  const [copyMessage, setCopyMessage] = useState<string | null>(null);
  const [fieldErrors, setFieldErrors] = useState<OperationActionResult["fieldErrors"]>({});
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
  const amountValid = amount.length === 0 || amountUnits !== null;
  const canSubmit =
    initialStatus.enabled &&
    selectedToken !== null &&
    amountUnits !== null &&
    confirmed &&
    stepUpCode.trim().length >= 6 &&
    !isPending;

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

  function submitTransfer() {
    if (!selectedToken || !canSubmit) {
      setSubmission((current) => ({
        ...current,
        status: "error",
        message: initialStatus.enabled ? dictionary.funding.confirmRequired : dictionary.funding.configUnavailable,
      }));
      return;
    }

    setSubmission({ status: "submitting", message: null, transfer: null });
    setFieldErrors({});

    startTransition(async () => {
      const result = await submitFundingTransferAction({
        tokenId: selectedToken.id,
        amount,
        confirmed,
        stepUpCode,
      });

      setFieldErrors(result.fieldErrors ?? {});
      setSubmission({
        status: result.ok ? "submitted" : "error",
        message: result.message,
        transfer: result.transfer ?? null,
      });
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

            <div className="space-y-2">
              <label className="text-xs font-medium text-muted-foreground" htmlFor="funding-token">
                {dictionary.funding.token}
              </label>
              <div id="funding-token" className="grid gap-2 sm:grid-cols-2" role="radiogroup">
                {initialStatus.tokens.map((token) => {
                  const note = dictionary.funding.tokenNotes[getFundingTokenNoteKey(token)];

                  return (
                    <button
                      key={token.id}
                      type="button"
                      role="radio"
                      aria-checked={selectedToken?.id === token.id}
                      className={cn(
                        "flex min-h-20 flex-col items-start justify-between rounded-lg border p-3 text-left transition-colors",
                        selectedToken?.id === token.id
                          ? "border-primary/45 bg-primary/12 text-foreground"
                          : "border-border/80 bg-background/35 text-muted-foreground hover:border-primary/30 hover:text-foreground",
                      )}
                      onClick={() => setSelectedTokenId(token.id)}
                    >
                      <span className="flex w-full items-center justify-between gap-3">
                        <span className="font-heading text-base font-semibold text-foreground">{token.symbol}</span>
                        {selectedToken?.id === token.id ? <CheckCircle2 className="size-4 text-secondary" /> : null}
                      </span>
                      <span className="mt-2 text-xs leading-5">{note}</span>
                    </button>
                  );
                })}
              </div>
              {fieldErrors?.tokenId ? <p className="text-xs text-destructive">{fieldErrors.tokenId}</p> : null}
            </div>

            <div className="grid gap-4 md:grid-cols-2">
              <div className="space-y-2">
                <label className="text-xs font-medium text-muted-foreground" htmlFor="funding-amount">
                  {dictionary.funding.amount}
                </label>
                <Input
                  id="funding-amount"
                  inputMode="decimal"
                  value={amount}
                  placeholder={dictionary.funding.amountPlaceholder}
                  aria-invalid={!amountValid || Boolean(fieldErrors?.amount)}
                  onChange={(event) => setAmount(event.target.value)}
                />
                {!amountValid ? <p className="text-xs text-destructive">{dictionary.funding.invalidAmount}</p> : null}
                {fieldErrors?.amount ? <p className="text-xs text-destructive">{fieldErrors.amount}</p> : null}
              </div>

              <div className="space-y-2">
                <label className="text-xs font-medium text-muted-foreground" htmlFor="funding-step-up-code">
                  {dictionary.funding.stepUpCode}
                </label>
                <Input
                  id="funding-step-up-code"
                  value={stepUpCode}
                  placeholder={dictionary.funding.stepUpCodePlaceholder}
                  type="password"
                  aria-invalid={Boolean(fieldErrors?.stepUpCode)}
                  onChange={(event) => setStepUpCode(event.target.value)}
                />
                {fieldErrors?.stepUpCode ? (
                  <p className="text-xs text-destructive">{fieldErrors.stepUpCode}</p>
                ) : null}
              </div>
            </div>

            <label className="flex items-start gap-3 rounded-lg border border-border/70 bg-background/35 p-3 text-sm text-muted-foreground">
              <input
                type="checkbox"
                checked={confirmed}
                onChange={(event) => setConfirmed(event.target.checked)}
                className="mt-1 size-4 accent-primary"
              />
              <span>{dictionary.funding.confirmLabel}</span>
            </label>
            {fieldErrors?.confirmed ? <p className="text-xs text-destructive">{fieldErrors.confirmed}</p> : null}

            <Button disabled={!canSubmit} onClick={submitTransfer} type="button">
              {submitting ? <Loader2 className="size-4 animate-spin" /> : <ShieldCheck className="size-4" />}
              {submitting ? dictionary.funding.sending : dictionary.funding.sendTransfer}
            </Button>

            {submission.message || copyMessage ? (
              <div className="rounded-lg border border-border/70 bg-muted/35 p-3 text-sm text-muted-foreground">
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
    </div>
  );
}
