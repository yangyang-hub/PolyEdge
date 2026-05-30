"use client";

import { startTransition, useState } from "react";
import { Ban, Play, RotateCcw, Save, Search, UserPlus, X } from "lucide-react";

import { MetricCard } from "@/components/shared/metric-card";
import { OperationFeedbackBanner } from "@/components/shared/operation-feedback-banner";
import { PageHeader } from "@/components/shared/page-header";
import { PaginationBar } from "@/components/pagination-bar";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import type {
  CopySizingMode,
  CopyTradeConfigDto,
  CopyTradeSnapshotDto,
} from "@/lib/contracts/dto";
import { formatOptionalClock, formatUsdFixed, metricToneForPnl } from "@/lib/formatters";
import { usePagination } from "@/hooks/use-pagination";
import { useI18n } from "@/lib/i18n/client";
import {
  addTrackedWalletAction,
  analyzeCopytradeWalletsAction,
  cancelCopyTradeOrdersAction,
  removeTrackedWalletAction,
  resetCopyTradeAction,
  runCopyTradeOnceAction,
  setCopytradeWalletStatusAction,
  updateCopyTradeConfigAction,
  type CopyTradeActionResult,
} from "@/lib/api/actions";

export function CopyTradeWorkbench({
  initialSnapshot,
}: {
  initialSnapshot: CopyTradeSnapshotDto;
}) {
  const { dictionary } = useI18n();
  const t = dictionary.copytrade;
  const [snapshot, setSnapshot] = useState(initialSnapshot);
  const [draft, setDraft] = useState<CopyTradeConfigDto>(initialSnapshot.config);
  const [feedback, setFeedback] = useState<CopyTradeActionResult | null>(null);
  const [pending, setPending] = useState(false);
  const [walletAddress, setWalletAddress] = useState("");
  const [walletLabel, setWalletLabel] = useState("");

  const tradesPagination = usePagination(snapshot.source_trades.length, 20);
  const ordersPagination = usePagination(snapshot.orders.length, 20);
  const eventsPagination = usePagination(snapshot.events.length, 20);

  function applyResult(result: CopyTradeActionResult) {
    setFeedback(result);
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

  function updateNumber(key: keyof CopyTradeConfigDto, value: string) {
    const nextValue = Number(value);
    setDraft((current) => ({
      ...current,
      [key]: Number.isFinite(nextValue) ? nextValue : 0,
    }));
  }

  const modeLabel = draft.mode === "live" ? t.liveDisabled : t.simulation;

  return (
    <div className="space-y-6">
      <PageHeader eyebrow={t.eyebrow} title={t.title} description={t.description} />

      {feedback ? <OperationFeedbackBanner feedback={feedback} /> : null}

      {/* ── Metric cards ─────────────────────────────────────────── */}
      <div className="grid gap-4 md:grid-cols-2 xl:grid-cols-5">
        <MetricCard
          title={t.status}
          value={snapshot.status.enabled ? dictionary.common.enabled : dictionary.common.disabled}
          hint={snapshot.status.running ? dictionary.common.active : dictionary.common.idle}
          accent={snapshot.status.enabled ? "success" : "primary"}
        />
        <MetricCard title={t.mode} value={modeLabel} hint={t.lastRun} accent="primary" />
        <MetricCard
          title={t.wallets}
          value={String(snapshot.status.wallets_tracked)}
          hint={`${snapshot.status.active_wallets} ${t.activeWallets}`}
          accent="violet"
        />
        <MetricCard
          title={t.openOrders}
          value={String(snapshot.status.open_orders)}
          hint={`${snapshot.status.positions} ${t.positions}`}
          accent={snapshot.status.open_orders > 0 ? "success" : "primary"}
        />
        <MetricCard
          title={t.sourceTrades}
          value={String(snapshot.status.source_trades_detected)}
          hint={formatOptionalClock(snapshot.status.last_scan_at)}
          accent="success"
        />
      </div>

      <div className="grid gap-4 md:grid-cols-2 xl:grid-cols-4">
        <MetricCard
          title={t.accountCapital}
          value={formatUsdFixed(snapshot.account.capital_usd)}
          hint={t.account}
          accent="primary"
        />
        <MetricCard
          title={t.available}
          value={formatUsdFixed(snapshot.account.available_usd)}
          hint={`${t.reserved} ${formatUsdFixed(snapshot.account.reserved_usd)}`}
          accent="violet"
        />
        <MetricCard
          title={t.realizedPnl}
          value={formatUsdFixed(snapshot.account.realized_pnl)}
          hint={formatOptionalClock(snapshot.account.updated_at)}
          accent={metricToneForPnl(snapshot.account.realized_pnl)}
        />
        <MetricCard
          title={t.positions}
          value={String(snapshot.positions.length)}
          hint={`${snapshot.wallets.length} ${t.wallets}`}
          accent="primary"
        />
      </div>

      {/* ── Config card ───────────────────────────────────────────── */}
      <Card>
        <CardHeader className="flex flex-col gap-4 border-b border-border/70 xl:flex-row xl:items-center xl:justify-between">
          <CardTitle className="font-heading text-base">{t.config}</CardTitle>
          <div className="flex flex-wrap gap-2">
            <Button size="sm" variant="outline" disabled={pending} onClick={() => runAction(runCopyTradeOnceAction)}>
              <Play className="size-4" /> {t.run}
            </Button>
            <Button size="sm" variant="outline" disabled={pending} onClick={() => runAction(analyzeCopytradeWalletsAction)}>
              <Search className="size-4" /> {t.analyze}
            </Button>
            <Button size="sm" variant="destructive" disabled={pending} onClick={() => runAction(cancelCopyTradeOrdersAction)}>
              <Ban className="size-4" /> {t.cancelAll}
            </Button>
            <Button size="sm" variant="outline" disabled={pending} onClick={() => runAction(resetCopyTradeAction)}>
              <RotateCcw className="size-4" /> {t.reset}
            </Button>
            <Button size="sm" disabled={pending} onClick={() => runAction(() => updateCopyTradeConfigAction(draft))}>
              <Save className="size-4" /> {t.save}
            </Button>
          </div>
        </CardHeader>
        <CardContent className="space-y-5">
          {/* Top row: basic toggles */}
          <div className="grid gap-4 md:grid-cols-2 xl:grid-cols-4">
            <label className="space-y-1.5">
              <span className="text-xs font-medium text-muted-foreground">{t.account}</span>
              <Input value={draft.account_id} onChange={(e) => setDraft((c) => ({ ...c, account_id: e.target.value }))} />
            </label>
            <label className="space-y-1.5">
              <span className="text-xs font-medium text-muted-foreground">{t.mode}</span>
              <select
                className="h-8 w-full rounded-lg border border-input bg-background px-2.5 text-sm"
                value={draft.mode}
                onChange={(e) => setDraft((c) => ({ ...c, mode: e.target.value === "live" ? "live" : "paper" }))}
              >
                <option value="paper">{t.simulation}</option>
                <option value="live">{t.liveDisabled}</option>
              </select>
            </label>
            <label className="flex items-center gap-3 pt-6 text-sm">
              <input type="checkbox" className="size-4 accent-primary" checked={draft.enabled} onChange={(e) => setDraft((c) => ({ ...c, enabled: e.target.checked }))} />
              {t.enabled}
            </label>
            <label className="flex items-center gap-3 pt-6 text-sm">
              <input type="checkbox" className="size-4 accent-primary" checked={draft.copy_sells} onChange={(e) => setDraft((c) => ({ ...c, copy_sells: e.target.checked }))} />
              {t.copySells}
            </label>
          </div>

          {/* Sizing + numeric params */}
          <div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-4 xl:grid-cols-6">
            <label className="space-y-1.5">
              <span className="text-xs font-medium text-muted-foreground">{t.sizingMode}</span>
              <select
                className="h-8 w-full rounded-lg border border-input bg-background px-2.5 text-sm"
                value={draft.sizing_mode}
                onChange={(e) => setDraft((c) => ({ ...c, sizing_mode: e.target.value as CopySizingMode }))}
              >
                <option value="fixed_usd">{t.fixedUsd}</option>
                <option value="proportional_to_source">{t.proportional}</option>
                <option value="capital_ratio">{t.capitalRatio}</option>
                <option value="mirror_portfolio_weight">{t.mirrorWeight}</option>
              </select>
            </label>
            <NumberField label={t.accountCapital} value={draft.account_capital_usd} suffix="$" onChange={(v) => updateNumber("account_capital_usd", v)} />
            <NumberField label={t.fixedUsdPerTrade} value={draft.fixed_usd_per_trade} suffix="$" onChange={(v) => updateNumber("fixed_usd_per_trade", v)} />
            <NumberField label={t.proportionalFactor} value={draft.proportional_factor} onChange={(v) => updateNumber("proportional_factor", v)} />
            <NumberField label={t.capitalRatioField} value={draft.capital_ratio} onChange={(v) => updateNumber("capital_ratio", v)} />
            <NumberField label={t.minSourceTradeUsd} value={draft.min_source_trade_usd} suffix="$" onChange={(v) => updateNumber("min_source_trade_usd", v)} />
            <NumberField label={t.maxPrice} value={draft.max_price} onChange={(v) => updateNumber("max_price", v)} />
            <NumberField label={t.minPrice} value={draft.min_price} onChange={(v) => updateNumber("min_price", v)} />
            <NumberField label={t.maxPositionPerMarket} value={draft.max_position_per_market_usd} suffix="$" onChange={(v) => updateNumber("max_position_per_market_usd", v)} />
            <NumberField label={t.perWalletMaxExposure} value={draft.per_wallet_max_exposure_usd} suffix="$" onChange={(v) => updateNumber("per_wallet_max_exposure_usd", v)} />
            <NumberField label={t.maxTotalExposure} value={draft.max_total_exposure_usd} suffix="$" onChange={(v) => updateNumber("max_total_exposure_usd", v)} />
            <NumberField label={t.maxOpenCopyOrders} value={draft.max_open_copy_orders} onChange={(v) => updateNumber("max_open_copy_orders", v)} />
            <NumberField label={t.dailyLossLimit} value={draft.daily_loss_limit_usd} suffix="$" onChange={(v) => updateNumber("daily_loss_limit_usd", v)} />
            <NumberField label={t.cooldownSecs} value={draft.cooldown_secs} suffix="s" onChange={(v) => updateNumber("cooldown_secs", v)} />
            <NumberField label={t.maxSlippageCents} value={draft.max_slippage_cents} suffix="¢" onChange={(v) => updateNumber("max_slippage_cents", v)} />
          </div>
        </CardContent>
      </Card>

      {/* ── Wallets panel ─────────────────────────────────────────── */}
      <Card>
        <CardHeader className="flex flex-col gap-4 border-b border-border/70 xl:flex-row xl:items-center xl:justify-between">
          <CardTitle className="font-heading text-base">{t.wallets} ({snapshot.wallets.length})</CardTitle>
          <div className="flex gap-2">
            <Input
              placeholder={t.walletAddress}
              value={walletAddress}
              onChange={(e) => setWalletAddress(e.target.value)}
              className="w-64 text-xs"
            />
            <Input
              placeholder={t.label}
              value={walletLabel}
              onChange={(e) => setWalletLabel(e.target.value)}
              className="w-32 text-xs"
            />
            <Button
              size="sm"
              disabled={pending}
              onClick={() => {
                runAction(() => addTrackedWalletAction({ address: walletAddress, label: walletLabel }));
                setWalletAddress("");
                setWalletLabel("");
              }}
            >
              <UserPlus className="size-4" /> {t.addWallet}
            </Button>
          </div>
        </CardHeader>
        <CardContent>
          {snapshot.wallets.length === 0 ? (
            <p className="py-8 text-center text-sm text-muted-foreground">{t.addWallet}</p>
          ) : (
            <div className="space-y-2">
              {snapshot.wallets.map((wallet) => (
                <div
                  key={wallet.address}
                  className="flex flex-wrap items-center gap-3 rounded-md border border-border/40 px-3 py-2 text-sm"
                >
                  <span className="font-mono text-xs">{wallet.address.slice(0, 6)}…{wallet.address.slice(-4)}</span>
                  {wallet.label && <span className="text-muted-foreground">{wallet.label}</span>}
                  <span className={`rounded px-1.5 py-0.5 text-xs font-medium ${wallet.status === "active" ? "bg-green-500/15 text-green-400" : "bg-yellow-500/15 text-yellow-400"}`}>
                    {wallet.status}
                  </span>
                  <span className="text-muted-foreground">{t.trades}: {wallet.analysis.trades_window}</span>
                  <span className="text-muted-foreground">{t.winRate}: {(Number(wallet.analysis.win_rate) * 100).toFixed(1)}%</span>
                  <span className="text-muted-foreground">{t.roi}: {(Number(wallet.analysis.roi) * 100).toFixed(1)}%</span>
                  <span className="text-muted-foreground">{t.marketsTraded}: {wallet.analysis.markets_traded}</span>
                  <div className="ml-auto flex gap-1">
                    <Button
                      size="sm"
                      variant="outline"
                      className="h-6 px-2 text-xs"
                      disabled={pending}
                      onClick={() => runAction(() => setCopytradeWalletStatusAction(wallet.address, wallet.status === "active" ? "paused" : "active"))}
                    >
                      {wallet.status === "active" ? t.pause : t.resume}
                    </Button>
                    <Button
                      size="sm"
                      variant="destructive"
                      className="h-6 px-2 text-xs"
                      disabled={pending}
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

      {/* ── Source trades + orders + events ────────────────────────── */}
      <div className="grid gap-4 xl:grid-cols-[1.25fr_0.75fr]">
        <Card>
          <CardHeader className="border-b border-border/70">
            <CardTitle className="font-heading text-base">{t.detectedTrades}</CardTitle>
          </CardHeader>
          <CardContent className="max-h-80 overflow-auto">
            <table className="w-full text-xs">
              <thead className="sticky top-0 bg-card">
                <tr className="border-b border-border/60 text-left text-muted-foreground">
                  <th className="pb-2 pr-2">{t.sourceWallet}</th>
                  <th className="pb-2 pr-2">{t.price}</th>
                  <th className="pb-2 pr-2">{t.usdSize}</th>
                  <th className="pb-2 pr-2">{t.copied}</th>
                </tr>
              </thead>
              <tbody>
                {snapshot.source_trades.slice(tradesPagination.start, tradesPagination.end).map((trade) => (
                  <tr key={trade.id} className="border-b border-border/20">
                    <td className="py-1.5 pr-2 font-mono">{trade.wallet_address.slice(0, 6)}…{trade.wallet_address.slice(-4)}</td>
                    <td className="py-1.5 pr-2">{trade.price}</td>
                    <td className="py-1.5 pr-2">${trade.usd_size}</td>
                    <td className="py-1.5 pr-2">
                      <span className={`rounded px-1 text-xs ${trade.copied ? "bg-green-500/15 text-green-400" : "text-muted-foreground"}`}>
                        {trade.copied ? dictionary.common.completed : t.pending}
                      </span>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
            <PaginationBar pagination={tradesPagination} totalItems={snapshot.source_trades.length} className="mt-3 flex items-center justify-between border-t border-border/70 pt-3" />
          </CardContent>
        </Card>

        <Card>
          <CardHeader className="border-b border-border/70">
            <CardTitle className="font-heading text-base">{t.copyOrders} ({snapshot.orders.length})</CardTitle>
          </CardHeader>
          <CardContent className="max-h-80 overflow-auto">
            <table className="w-full text-xs">
              <thead className="sticky top-0 bg-card">
                <tr className="border-b border-border/60 text-left text-muted-foreground">
                  <th className="pb-2 pr-2">{t.sourceWallet}</th>
                  <th className="pb-2 pr-2">{t.price}</th>
                  <th className="pb-2 pr-2">{t.size}</th>
                  <th className="pb-2 pr-2">{dictionary.common.open}</th>
                </tr>
              </thead>
              <tbody>
                {snapshot.orders.slice(ordersPagination.start, ordersPagination.end).map((order) => (
                  <tr key={order.id} className="border-b border-border/20">
                    <td className="py-1.5 pr-2 font-mono">{order.wallet_address.slice(0, 6)}…{order.wallet_address.slice(-4)}</td>
                    <td className="py-1.5 pr-2">{order.price}</td>
                    <td className="py-1.5 pr-2">{order.size}</td>
                    <td className="py-1.5 pr-2">
                      <span className={`rounded px-1 text-xs font-medium ${
                        order.status === "filled" ? "bg-green-500/15 text-green-400" :
                        order.status === "planned" ? "bg-blue-500/15 text-blue-400" :
                        order.status === "cancelled" ? "text-muted-foreground" : ""
                      }`}>{order.status}</span>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
            <PaginationBar pagination={ordersPagination} totalItems={snapshot.orders.length} className="mt-3 flex items-center justify-between border-t border-border/70 pt-3" />
          </CardContent>
        </Card>
      </div>

      <Card>
        <CardHeader className="border-b border-border/70">
          <CardTitle className="font-heading text-base">{t.riskEvents}</CardTitle>
        </CardHeader>
        <CardContent className="max-h-64 overflow-auto">
          <div className="space-y-1">
            {snapshot.events.slice(eventsPagination.start, eventsPagination.end).map((event) => (
              <div key={event.id} className="flex items-start gap-2 text-xs">
                <span className={`mt-0.5 size-1.5 shrink-0 rounded-full ${
                  event.severity === "critical" ? "bg-red-500" :
                  event.severity === "warning" ? "bg-yellow-500" : "bg-green-500"
                }`} />
                <span className="font-mono text-muted-foreground">{event.created_at.replace("T", " ").slice(0, 19)}</span>
                <span className="font-medium">{event.event_type}</span>
                <span className="text-muted-foreground">{event.message}</span>
              </div>
            ))}
          </div>
          <PaginationBar pagination={eventsPagination} totalItems={snapshot.events.length} className="mt-3 flex items-center justify-between border-t border-border/70 pt-3" />
        </CardContent>
      </Card>
    </div>
  );
}

// ── Inline number input (minimal, no extra file needed) ─────────────────────

function NumberField({
  label,
  value,
  suffix,
  onChange,
}: {
  label: string;
  value: number | string;
  suffix?: string;
  onChange: (value: string) => void;
}) {
  return (
    <label className="space-y-1.5">
      <span className="text-xs font-medium text-muted-foreground">{label}</span>
      <div className="relative">
        <Input
          type="text"
          inputMode="decimal"
          value={value}
          onChange={(e) => onChange(e.target.value)}
          className="h-8 pr-6 text-sm"
        />
        {suffix && (
          <span className="pointer-events-none absolute right-2 top-1/2 -translate-y-1/2 text-xs text-muted-foreground">
            {suffix}
          </span>
        )}
      </div>
    </label>
  );
}
