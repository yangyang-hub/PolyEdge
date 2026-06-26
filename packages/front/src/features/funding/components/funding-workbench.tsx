"use client";

import {
  CheckCircle2,
  ExternalLink,
  Loader2,
  RadioTower,
  ShieldCheck,
  Wallet,
} from "lucide-react";
import { useMemo, useState } from "react";

import { PageHeader } from "@/components/shared/page-header";
import { StatusPill } from "@/components/shared/status-pill";
import { Button } from "@/components/ui/button";
import { Card, CardAction, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { FundingSafetyCard, FundingStepsCard } from "@/features/funding/components/funding-info-cards";
import { FundingReviewCard } from "@/features/funding/components/funding-review-card";
import {
  buildErc20TransferData,
  getPolygonFundingToken,
  isEvmAddress,
  parseTokenAmountToUnits,
  polygonChain,
  polygonFundingTokens,
  type PolygonFundingTokenId,
} from "@/features/funding/lib/polygon-funding";
import type { WalletSnapshot } from "@/features/funding/types";
import { dictionary } from "@/lib/i18n/dictionaries";
import { cn } from "@/lib/utils";

type EthereumProvider = {
  request<TResponse = unknown>(args: { method: string; params?: unknown[] }): Promise<TResponse>;
};

type EthereumError = {
  code?: number;
  message?: string;
};

const officialDepositUrl = "https://polymarket.com/profile";

function getEthereumProvider(): EthereumProvider | null {
  if (typeof window === "undefined") {
    return null;
  }

  return (window as Window & { ethereum?: EthereumProvider }).ethereum ?? null;
}

function getWalletErrorMessage(error: unknown): string {
  const walletError = error as EthereumError;

  if (walletError?.code === 4001) {
    return dictionary.funding.walletMessages.rejected;
  }

  return walletError?.message ?? dictionary.funding.walletMessages.failed;
}

async function ensurePolygonNetwork(provider: EthereumProvider): Promise<void> {
  try {
    await provider.request({
      method: "wallet_switchEthereumChain",
      params: [{ chainId: polygonChain.chainIdHex }],
    });
  } catch (error) {
    const walletError = error as EthereumError;

    if (walletError?.code !== 4902) {
      throw error;
    }

    await provider.request({
      method: "wallet_addEthereumChain",
      params: [
        {
          chainId: polygonChain.chainIdHex,
          chainName: polygonChain.chainName,
          nativeCurrency: polygonChain.nativeCurrency,
          rpcUrls: polygonChain.rpcUrls,
          blockExplorerUrls: polygonChain.blockExplorerUrls,
        },
      ],
    });
  }
}

export function FundingWorkbench() {
  const [selectedTokenId, setSelectedTokenId] = useState<PolygonFundingTokenId>(polygonFundingTokens[0].id);
  const [recipient, setRecipient] = useState("");
  const [amount, setAmount] = useState("");
  const [confirmed, setConfirmed] = useState(false);
  const [copyMessage, setCopyMessage] = useState<string | null>(null);
  const [wallet, setWallet] = useState<WalletSnapshot>({
    account: null,
    txHash: null,
    status: "idle",
    message: null,
  });

  const selectedToken = getPolygonFundingToken(selectedTokenId);
  const recipientValid = recipient.length === 0 || isEvmAddress(recipient);
  const amountUnits = useMemo(
    () => parseTokenAmountToUnits(amount, selectedToken.decimals),
    [amount, selectedToken.decimals],
  );
  const amountValid = amount.length === 0 || amountUnits !== null;
  const canSubmit =
    isEvmAddress(recipient) &&
    amountUnits !== null &&
    confirmed &&
    wallet.status !== "connecting" &&
    wallet.status !== "switching" &&
    wallet.status !== "submitting";

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

  async function connectWallet() {
    const provider = getEthereumProvider();

    if (!provider) {
      setWallet((current) => ({
        ...current,
        status: "error",
        message: dictionary.funding.walletMessages.missingProvider,
      }));
      return;
    }

    setWallet((current) => ({ ...current, status: "connecting", message: null }));

    try {
      const accounts = await provider.request<string[]>({ method: "eth_requestAccounts" });
      setWallet((current) => ({
        ...current,
        account: accounts[0] ?? null,
        status: "idle",
        message: dictionary.funding.walletMessages.connected,
      }));
    } catch (error) {
      setWallet((current) => ({ ...current, status: "error", message: getWalletErrorMessage(error) }));
    }
  }

  async function switchNetwork() {
    const provider = getEthereumProvider();

    if (!provider) {
      setWallet((current) => ({
        ...current,
        status: "error",
        message: dictionary.funding.walletMessages.missingProvider,
      }));
      return;
    }

    setWallet((current) => ({ ...current, status: "switching", message: null }));

    try {
      await ensurePolygonNetwork(provider);
      setWallet((current) => ({
        ...current,
        status: "idle",
        message: dictionary.funding.walletMessages.switched,
      }));
    } catch (error) {
      setWallet((current) => ({ ...current, status: "error", message: getWalletErrorMessage(error) }));
    }
  }

  async function submitTransfer() {
    const provider = getEthereumProvider();

    if (!provider) {
      setWallet((current) => ({
        ...current,
        status: "error",
        message: dictionary.funding.walletMessages.missingProvider,
      }));
      return;
    }

    const units = parseTokenAmountToUnits(amount, selectedToken.decimals);

    if (!isEvmAddress(recipient) || units === null || !confirmed) {
      setWallet((current) => ({ ...current, status: "error", message: dictionary.funding.confirmRequired }));
      return;
    }

    setWallet((current) => ({ ...current, status: "submitting", message: null, txHash: null }));

    try {
      await ensurePolygonNetwork(provider);

      const accounts = wallet.account
        ? [wallet.account]
        : await provider.request<string[]>({ method: "eth_requestAccounts" });
      const account = accounts[0];

      if (!account) {
        throw new Error(dictionary.funding.walletMessages.missingProvider);
      }

      const txHash = await provider.request<string>({
        method: "eth_sendTransaction",
        params: [
          {
            from: account,
            to: selectedToken.address,
            value: "0x0",
            data: buildErc20TransferData(recipient, units),
          },
        ],
      });

      setWallet({
        account,
        txHash,
        status: "submitted",
        message: dictionary.funding.walletMessages.submitted,
      });
    } catch (error) {
      setWallet((current) => ({ ...current, status: "error", message: getWalletErrorMessage(error) }));
    }
  }

  const submitting = wallet.status === "submitting";
  const tokenNote = dictionary.funding.tokenNotes[selectedToken.noteKey];

  return (
    <div className="space-y-6">
      <PageHeader
        eyebrow={dictionary.funding.eyebrow}
        title={dictionary.funding.title}
        description={dictionary.funding.description}
        actions={
          <>
            <StatusPill tone="primary">{dictionary.funding.polygonOnly}</StatusPill>
            <StatusPill tone="success">{dictionary.funding.selfCustody}</StatusPill>
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
            <div className="space-y-2">
              <label className="text-xs font-medium text-muted-foreground" htmlFor="funding-token">
                {dictionary.funding.token}
              </label>
              <div id="funding-token" className="grid gap-2 sm:grid-cols-2" role="radiogroup">
                {polygonFundingTokens.map((token) => (
                  <button
                    key={token.id}
                    type="button"
                    role="radio"
                    aria-checked={selectedToken.id === token.id}
                    className={cn(
                      "flex min-h-20 flex-col items-start justify-between rounded-lg border p-3 text-left transition-colors",
                      selectedToken.id === token.id
                        ? "border-primary/45 bg-primary/12 text-foreground"
                        : "border-border/80 bg-background/35 text-muted-foreground hover:border-primary/30 hover:text-foreground",
                    )}
                    onClick={() => setSelectedTokenId(token.id)}
                  >
                    <span className="flex w-full items-center justify-between gap-3">
                      <span className="font-heading text-base font-semibold text-foreground">{token.symbol}</span>
                      {selectedToken.id === token.id ? <CheckCircle2 className="size-4 text-secondary" /> : null}
                    </span>
                    <span className="mt-2 text-xs leading-5">{dictionary.funding.tokenNotes[token.noteKey]}</span>
                  </button>
                ))}
              </div>
            </div>

            <div className="grid gap-4 md:grid-cols-[1.4fr_0.6fr]">
              <div className="space-y-2">
                <label className="text-xs font-medium text-muted-foreground" htmlFor="funding-recipient">
                  {dictionary.funding.recipient}
                </label>
                <Input
                  id="funding-recipient"
                  value={recipient}
                  placeholder={dictionary.funding.recipientPlaceholder}
                  aria-invalid={!recipientValid}
                  onChange={(event) => setRecipient(event.target.value)}
                />
                {!recipientValid ? (
                  <p className="text-xs text-destructive">{dictionary.funding.invalidRecipient}</p>
                ) : null}
              </div>

              <div className="space-y-2">
                <label className="text-xs font-medium text-muted-foreground" htmlFor="funding-amount">
                  {dictionary.funding.amount}
                </label>
                <Input
                  id="funding-amount"
                  inputMode="decimal"
                  value={amount}
                  placeholder={dictionary.funding.amountPlaceholder}
                  aria-invalid={!amountValid}
                  onChange={(event) => setAmount(event.target.value)}
                />
                {!amountValid ? <p className="text-xs text-destructive">{dictionary.funding.invalidAmount}</p> : null}
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

            <div className="flex flex-wrap items-center gap-2">
              <Button onClick={connectWallet} type="button" variant="outline">
                <Wallet className="size-4" />
                {dictionary.funding.connectWallet}
              </Button>
              <Button onClick={switchNetwork} type="button" variant="outline">
                <RadioTower className="size-4" />
                {dictionary.funding.switchNetwork}
              </Button>
              <Button disabled={!canSubmit} onClick={submitTransfer} type="button">
                {submitting ? <Loader2 className="size-4 animate-spin" /> : <ShieldCheck className="size-4" />}
                {submitting ? dictionary.funding.sending : dictionary.funding.sendTransfer}
              </Button>
            </div>

            {wallet.message || copyMessage ? (
              <div className="rounded-lg border border-border/70 bg-muted/35 p-3 text-sm text-muted-foreground">
                {wallet.message ?? copyMessage}
              </div>
            ) : null}
          </CardContent>
        </Card>

        <div className="space-y-4">
          <FundingReviewCard
            amount={amount}
            amountUnits={amountUnits}
            onCopy={copyToClipboard}
            recipient={recipient}
            selectedToken={selectedToken}
            tokenNote={tokenNote}
            wallet={wallet}
          />
          <FundingSafetyCard />
        </div>
      </section>

      <FundingStepsCard />
    </div>
  );
}
