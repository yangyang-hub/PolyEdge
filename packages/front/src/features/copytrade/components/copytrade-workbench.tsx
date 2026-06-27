"use client";

import { startTransition, useState } from "react";
import { Save, Search, UserPlus, X } from "lucide-react";
import { toast } from "sonner";

import { MetricCard } from "@/components/shared/metric-card";
import { OperationFeedbackBanner } from "@/components/shared/operation-feedback-banner";
import { PageHeader } from "@/components/shared/page-header";
import { PaginationBar } from "@/components/pagination-bar";
import { StatusPill } from "@/components/shared/status-pill";
import { TruncateText } from "@/components/shared/truncate-text";
import { SmartMoneyCandidatesPanel } from "@/features/copytrade/components/smart-money-candidates-panel";
import { SmartMoneyConfigPanel } from "@/features/copytrade/components/smart-money-config-panel";
import { SmartMoneySignalsPanel } from "@/features/copytrade/components/smart-money-signals-panel";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import type {
  CopyEventSeverity,
  CopyTradeConfigDto,
  CopyTradeSnapshotDto,
  SmartMoneySnapshotDto,
  TrackedWalletStatus,
} from "@/lib/contracts/dto";
import {
  formatOptionalClock,
  formatPercentFromRatio,
  formatUsdFixed,
  uppercaseEnum,
  type Tone,
} from "@/lib/formatters";
import { formatShortAddress } from "@/lib/format-address";
import { usePagination } from "@/hooks/use-pagination";
import { dictionary } from "@/lib/i18n/dictionaries";
import {
  addTrackedWalletAction,
  analyzeCopytradeWalletsAction,
  removeTrackedWalletAction,
  setCopytradeWalletStatusAction,
  updateCopyTradeConfigAction,
  type CopyTradeActionResult,
} from "@/lib/api/actions";

function walletStatusTone(status: TrackedWalletStatus): Tone {
  return status === "active" ? "success" : "warning";
}

function eventSeverityTone(severity: CopyEventSeverity): Tone {
  if (severity === "critical") {
    return "danger";
  }

  if (severity === "warning") {
    return "warning";
  }

  return "success";
}

export function CopyTradeWorkbench({
  initialSnapshot,
  initialSmartMoneySnapshot,
}: {
  initialSnapshot: CopyTradeSnapshotDto;
  initialSmartMoneySnapshot: SmartMoneySnapshotDto;
}) {
  const t = dictionary.copytrade;
  const [snapshot, setSnapshot] = useState(initialSnapshot);
  const [smartMoneySnapshot, setSmartMoneySnapshot] = useState(initialSmartMoneySnapshot);
  const [draft, setDraft] = useState<CopyTradeConfigDto>(initialSnapshot.config);
  const [feedback, setFeedback] = useState<CopyTradeActionResult | null>(null);
  const [pending, setPending] = useState(false);
  const [walletAddress, setWalletAddress] = useState("");
  const [walletLabelInput, setWalletLabelInput] = useState("");

  const tradesPagination = usePagination(snapshot.source_trades.length, 20);
  const eventsPagination = usePagination(snapshot.events.length, 20);

  function applyResult(result: CopyTradeActionResult) {
    setFeedback(result);
    if (result.ok) {
      toast.success(result.message);
    } else {
      toast.error(result.message);
    }
    if (result.snapshot) {
      setSnapshot(result.snapshot);
      setDraft(result.snapshot.config);
    }
  }

  function runAction(action: () => Promise<CopyTradeActionResult>) {
    setPending(true);
    startTransition(() => {
      void action()
        .then(applyResult)
        .finally(() => setPending(false));
    });
  }

  function addWallet() {
    const address = walletAddress;
    const label = walletLabelInput;
    runAction(() => addTrackedWalletAction({ address, label }));
    setWalletAddress("");
    setWalletLabelInput("");
  }

  return (
    <div className="space-y-6">
      <PageHeader eyebrow={t.eyebrow} title={t.title} description={t.description} />

      {feedback ? <OperationFeedbackBanner feedback={feedback} onDismiss={() => setFeedback(null)} /> : null}

      <div className="grid gap-4 md:grid-cols-2 xl:grid-cols-4">
        <MetricCard
          title={t.status}
          value={snapshot.status.enabled ? dictionary.common.enabled : dictionary.common.disabled}
          hint={snapshot.status.running ? dictionary.common.active : dictionary.common.idle}
          accent={snapshot.status.error ? "danger" : snapshot.status.enabled ? "success" : "primary"}
        />
        <MetricCard
          title={t.wallets}
          value={String(snapshot.status.wallets_tracked)}
          hint={`${snapshot.status.active_wallets} ${t.activeWallets}`}
          accent="violet"
        />
        <MetricCard
          title={t.sourceTrades}
          value={String(snapshot.status.source_trades_detected)}
          hint={formatOptionalClock(snapshot.status.last_scan_at)}
          accent="success"
        />
        <MetricCard
          title={t.mode}
          value={t.readOnlyTracking}
          hint={snapshot.status.error ?? t.noOrderExecution}
          accent={snapshot.status.error ? "danger" : "primary"}
        />
      </div>

      <SmartMoneyConfigPanel
        snapshot={smartMoneySnapshot}
        onSnapshotChange={setSmartMoneySnapshot}
      />

      <SmartMoneyCandidatesPanel
        snapshot={smartMoneySnapshot}
        onSnapshotChange={setSmartMoneySnapshot}
      />

      <SmartMoneySignalsPanel snapshot={smartMoneySnapshot} />

      <Card>
        <CardHeader className="flex flex-col gap-4 border-b border-border/70 xl:flex-row xl:items-center xl:justify-between">
          <div>
            <CardTitle className="font-heading text-base">{t.trackingControl}</CardTitle>
            <CardDescription>{t.trackingControlDescription}</CardDescription>
          </div>
          <div className="flex flex-wrap gap-2">
            <Button
              size="sm"
              variant="outline"
              disabled={pending}
              onClick={() => runAction(analyzeCopytradeWalletsAction)}
            >
              <Search className="size-4" /> {t.analyze}
            </Button>
            <Button size="sm" disabled={pending} onClick={() => runAction(() => updateCopyTradeConfigAction(draft))}>
              <Save className="size-4" /> {t.save}
            </Button>
          </div>
        </CardHeader>
        <CardContent>
          <label className="flex items-center gap-3 text-sm">
            <input
              type="checkbox"
              className="size-4 accent-primary"
              checked={draft.enabled}
              onChange={(event) => setDraft((current) => ({ ...current, enabled: event.target.checked }))}
            />
            {t.enabled}
          </label>
        </CardContent>
      </Card>

      <Card>
        <CardHeader className="flex flex-col gap-4 border-b border-border/70 xl:flex-row xl:items-center xl:justify-between">
          <div>
            <CardTitle className="font-heading text-base">
              {t.wallets} ({snapshot.wallets.length})
            </CardTitle>
            <CardDescription>{t.walletsDescription}</CardDescription>
          </div>
          <div className="flex flex-col gap-2 sm:flex-row">
            <Input
              placeholder={t.walletAddress}
              value={walletAddress}
              onChange={(event) => setWalletAddress(event.target.value)}
              className="w-full text-xs sm:w-64"
            />
            <Input
              placeholder={t.label}
              value={walletLabelInput}
              onChange={(event) => setWalletLabelInput(event.target.value)}
              className="w-full text-xs sm:w-36"
            />
            <Button size="sm" disabled={pending} onClick={addWallet}>
              <UserPlus className="size-4" /> {t.addWallet}
            </Button>
          </div>
        </CardHeader>
        <CardContent>
          {snapshot.wallets.length === 0 ? (
            <p className="py-8 text-center text-sm text-muted-foreground">{t.noWallets}</p>
          ) : (
            <div className="space-y-2">
              {snapshot.wallets.map((wallet) => (
                <div
                  key={wallet.address}
                  className="flex flex-wrap items-center gap-3 rounded-md border border-border/40 px-3 py-2 text-sm"
                >
                  <span className="font-mono text-xs">{formatShortAddress(wallet.address)}</span>
                  {wallet.label ? <span className="text-muted-foreground">{wallet.label}</span> : null}
                  <StatusPill tone={walletStatusTone(wallet.status)}>
                    {wallet.status === "active" ? dictionary.common.active : t.paused}
                  </StatusPill>
                  <span className="text-muted-foreground">
                    {t.trades}: {wallet.analysis.trades_window}
                  </span>
                  <span className="text-muted-foreground">
                    {t.volume}: {formatUsdFixed(wallet.analysis.volume_window_usd)}
                  </span>
                  <span className="text-muted-foreground">
                    {t.winRate}: {formatPercentFromRatio(wallet.analysis.win_rate, 1)}
                  </span>
                  <span className="text-muted-foreground">
                    {t.roi}: {formatPercentFromRatio(wallet.analysis.roi, 1)}
                  </span>
                  <div className="ml-auto flex gap-1">
                    <Button
                      size="sm"
                      variant="outline"
                      className="h-6 px-2 text-xs"
                      disabled={pending}
                      onClick={() =>
                        runAction(() =>
                          setCopytradeWalletStatusAction(
                            wallet.address,
                            wallet.status === "active" ? "paused" : "active",
                          ),
                        )
                      }
                    >
                      {wallet.status === "active" ? t.pause : t.resume}
                    </Button>
                    <Button
                      size="sm"
                      variant="destructive"
                      className="h-6 px-2 text-xs"
                      disabled={pending}
                      aria-label={t.remove}
                      onClick={() => runAction(() => removeTrackedWalletAction(wallet.address))}
                    >
                      <X className="size-3" />
                    </Button>
                  </div>
                </div>
              ))}
            </div>
          )}
        </CardContent>
      </Card>

      <div className="grid gap-4 xl:grid-cols-[1.25fr_0.75fr]">
        <Card>
          <CardHeader className="border-b border-border/70">
            <CardTitle className="font-heading text-base">{t.detectedTrades}</CardTitle>
            <CardDescription>{t.detectedTradesDescription}</CardDescription>
          </CardHeader>
          <CardContent className="max-h-96 overflow-auto">
            <table className="w-full text-xs">
              <thead className="sticky top-0 bg-card">
                <tr className="border-b border-border/60 text-left text-muted-foreground">
                  <th className="pb-2 pr-2">{t.sourceWallet}</th>
                  <th className="pb-2 pr-2">{t.market}</th>
                  <th className="pb-2 pr-2">{t.side}</th>
                  <th className="pb-2 pr-2">{t.price}</th>
                  <th className="pb-2 pr-2">{t.usdSize}</th>
                  <th className="pb-2 pr-2">{t.observed}</th>
                </tr>
              </thead>
              <tbody>
                {snapshot.source_trades.length === 0 ? (
                  <tr>
                    <td colSpan={6} className="py-8 text-center text-sm text-muted-foreground">
                      {t.noSourceTrades}
                    </td>
                  </tr>
                ) : (
                  snapshot.source_trades.slice(tradesPagination.start, tradesPagination.end).map((trade) => (
                    <tr key={trade.id} className="border-b border-border/20">
                      <td className="py-2 pr-2 font-mono">{formatShortAddress(trade.wallet_address)}</td>
                      <td className="max-w-72 py-2 pr-2">
                        <p className="truncate text-foreground" title={trade.title}>
                          {trade.title}
                        </p>
                        <p className="mt-1 text-muted-foreground">{trade.outcome}</p>
                      </td>
                      <td className="py-2 pr-2">{uppercaseEnum(trade.side)}</td>
                      <td className="py-2 pr-2">{trade.price}</td>
                      <td className="py-2 pr-2">{formatUsdFixed(trade.usd_size)}</td>
                      <td className="py-2 pr-2">{formatOptionalClock(trade.observed_at)}</td>
                    </tr>
                  ))
                )}
              </tbody>
            </table>
            <PaginationBar
              pagination={tradesPagination}
              totalItems={snapshot.source_trades.length}
              className="mt-3 flex items-center justify-between border-t border-border/70 pt-3"
            />
          </CardContent>
        </Card>

        <Card>
          <CardHeader className="border-b border-border/70">
            <CardTitle className="font-heading text-base">{t.riskEvents}</CardTitle>
            <CardDescription>{t.eventsDescription}</CardDescription>
          </CardHeader>
          <CardContent className="max-h-96 overflow-auto">
            <div className="space-y-2">
              {snapshot.events.length === 0 ? (
                <p className="py-8 text-center text-sm text-muted-foreground">{t.noEvents}</p>
              ) : (
                snapshot.events.slice(eventsPagination.start, eventsPagination.end).map((event) => (
                  <div key={event.id} className="rounded-sm border border-border/60 p-3 text-xs">
                    <div className="flex items-center justify-between gap-3">
                      <StatusPill tone={eventSeverityTone(event.severity)}>{event.severity}</StatusPill>
                      <span className="font-mono text-muted-foreground">{formatOptionalClock(event.created_at)}</span>
                    </div>
                    <p className="mt-2 font-medium text-foreground">{event.event_type}</p>
                    <TruncateText
                      text={event.message}
                      lines={2}
                      className="mt-1 block text-muted-foreground"
                    />
                  </div>
                ))
              )}
            </div>
            <PaginationBar
              pagination={eventsPagination}
              totalItems={snapshot.events.length}
              className="mt-3 flex items-center justify-between border-t border-border/70 pt-3"
            />
          </CardContent>
        </Card>
      </div>
    </div>
  );
}
