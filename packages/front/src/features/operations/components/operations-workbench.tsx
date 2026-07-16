"use client";

import { useCallback, useEffect, useState, useTransition } from "react";

import { ActionDialog } from "@/components/shared/action-dialog";
import { OperationFeedbackBanner } from "@/components/shared/operation-feedback-banner";
import { PageHeader } from "@/components/shared/page-header";
import { StatusPill } from "@/components/shared/status-pill";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { cancelOrders, executeBatch, type OperationActionResult } from "@/lib/api/actions";
import {
  listExecutionBatches,
  listOrders,
  listPositions,
} from "@/lib/api/operations";
import { listStrategies } from "@/lib/api/strategies";
import { listWallets } from "@/lib/api/wallets";
import type {
  ExecutionBatchData,
  ManagedOrderDto,
  ManagedPositionDto,
  MarketStrategyData,
  WalletAccountData,
} from "@/lib/contracts/dto";
import { dictionary, translateEnum } from "@/lib/i18n/dictionaries";
import { canWriteMarkets, useAuth } from "@/components/shared/auth-provider";

type DialogMode = "execute" | "cancel" | null;

export function OperationsWorkbench() {
  const d = dictionary.operations;
  const { user } = useAuth();
  const writable = canWriteMarkets(user?.role);
  const [strategies, setStrategies] = useState<MarketStrategyData[]>([]);
  const [wallets, setWallets] = useState<WalletAccountData[]>([]);
  const [batches, setBatches] = useState<ExecutionBatchData[]>([]);
  const [orders, setOrders] = useState<ManagedOrderDto[]>([]);
  const [positions, setPositions] = useState<ManagedPositionDto[]>([]);
  const [strategyId, setStrategyId] = useState("");
  const [walletIds, setWalletIds] = useState("");
  const [conditionIds, setConditionIds] = useState("");
  const [dialogMode, setDialogMode] = useState<DialogMode>(null);
  const [note, setNote] = useState("");
  const [feedback, setFeedback] = useState<OperationActionResult | null>(null);
  const [loadError, setLoadError] = useState("");
  const [isPending, startTransition] = useTransition();

  const reload = useCallback(() => {
    void Promise.all([
      listStrategies(),
      listWallets(),
      listExecutionBatches(),
      listOrders(),
      listPositions(),
    ])
      .then(([strategyResult, walletResult, batchResult, orderResult, positionResult]) => {
        setStrategies(strategyResult.data);
        setWallets(walletResult.data);
        setBatches(batchResult.data);
        setOrders(orderResult.data);
        setPositions(positionResult.data);
        setLoadError("");
      })
      .catch(() => setLoadError(d.loadFailed));
  }, [d.loadFailed]);

  useEffect(reload, [reload]);

  const selectedWalletIds = parseIds(walletIds);
  const selectedConditionIds = parseValues(conditionIds);
  const selectedStrategyId = Number(strategyId);

  const submitDialog = () => {
    if (!dialogMode) return;
    startTransition(async () => {
      const result = dialogMode === "execute"
        ? await executeBatch({
            request: {
              strategy_id: selectedStrategyId,
              wallet_ids: selectedWalletIds,
              operator_note: note.trim() || undefined,
            },
          })
        : await cancelOrders({
            request: {
              wallet_ids: selectedWalletIds,
              condition_ids: selectedConditionIds,
              operator_note: note.trim() || undefined,
            },
          });
      setFeedback(result);
      if (result.ok) {
        setDialogMode(null);
        setNote("");
        reload();
      }
    });
  };

  const openDialog = (mode: Exclude<DialogMode, null>) => {
    setFeedback(null);
    setDialogMode(mode);
  };

  return (
    <div className="space-y-8">
      <PageHeader eyebrow={d.eyebrow} title={d.title} description={d.description} />
      {feedback ? <OperationFeedbackBanner feedback={feedback} /> : null}
      {loadError ? <p className="text-sm text-destructive">{loadError}</p> : null}

      <Card>
        <CardHeader>
          <CardTitle>{d.preview}</CardTitle>
        </CardHeader>
        <CardContent className="grid gap-4 md:grid-cols-3">
          <label className="space-y-2 text-sm">
            <span>{d.strategy}</span>
            <select
              className="h-9 w-full rounded-md border border-input bg-background px-3 text-sm"
              value={strategyId}
              onChange={(event) => setStrategyId(event.target.value)}
            >
              <option value="">{d.selectStrategy}</option>
              {strategies.map((item) => (
                <option key={item.strategy.id} value={item.strategy.id}>
                  #{item.strategy.id} {item.strategy.name}
                </option>
              ))}
            </select>
          </label>
          <label className="space-y-2 text-sm">
            <span>{d.wallet}</span>
            <Input value={walletIds} onChange={(event) => setWalletIds(event.target.value)} placeholder={d.walletIdsPlaceholder} />
            <span className="text-xs text-muted-foreground">{wallets.length} {d.walletsAvailable}</span>
          </label>
          <label className="space-y-2 text-sm">
            <span>{d.conditionIds}</span>
            <Input value={conditionIds} onChange={(event) => setConditionIds(event.target.value)} placeholder={d.conditionIdsPlaceholder} />
          </label>
          <div className="flex flex-wrap gap-2 md:col-span-3">
            <Button disabled={!writable || !Number.isSafeInteger(selectedStrategyId) || selectedStrategyId <= 0 || selectedWalletIds.length === 0} onClick={() => openDialog("execute")}>
              {d.execute}
            </Button>
            <Button variant="destructive" disabled={!writable || (selectedWalletIds.length === 0 && selectedConditionIds.length === 0)} onClick={() => openDialog("cancel")}>
              {d.cancel}
            </Button>
            <Button variant="outline" onClick={reload}>{d.refresh}</Button>
          </div>
        </CardContent>
      </Card>

      <div className="grid gap-6 xl:grid-cols-2">
        <RecordCard title={d.batches} empty={d.empty}>
          {batches.map(({ batch, jobs }) => (
            <div key={batch.id} className="flex flex-wrap items-center justify-between gap-3 rounded-lg border p-3 text-sm">
              <div>
                <p className="font-medium">#{batch.id} · version {batch.strategy_version_id}</p>
                <p className="text-xs text-muted-foreground">{batch.requested_by} · {jobs.length} {d.jobs}</p>
              </div>
              <StatusPill>{translateEnum(batch.status)}</StatusPill>
            </div>
          ))}
        </RecordCard>
        <RecordCard title={d.orders} empty={d.empty}>
          {orders.map((order) => (
            <div key={order.id} className="flex flex-wrap items-center justify-between gap-3 rounded-lg border p-3 text-sm">
              <div>
                <p className="font-medium">#{order.id} · wallet {order.wallet_id} · {order.outcome.toUpperCase()}</p>
                <p className="text-xs text-muted-foreground">{order.quantity} @ {order.price} · {order.filled_quantity} {d.filled}</p>
              </div>
              <StatusPill>{translateEnum(order.status)}</StatusPill>
            </div>
          ))}
        </RecordCard>
        <RecordCard title={d.positions} empty={d.empty}>
          {positions.map((position) => (
            <div key={position.id} className="flex flex-wrap items-center justify-between gap-3 rounded-lg border p-3 text-sm">
              <div>
                <p className="font-medium">wallet {position.wallet_id} · market {position.market_id} · {position.outcome.toUpperCase()}</p>
                <p className="text-xs text-muted-foreground">{position.quantity} @ {position.average_price}</p>
              </div>
              <StatusPill>{d.pnl} {position.realized_pnl}</StatusPill>
            </div>
          ))}
        </RecordCard>
      </div>

      <ActionDialog
        open={dialogMode !== null}
        onOpenChange={(open) => { if (!open) setDialogMode(null); }}
        title={dialogMode === "cancel" ? d.cancelDialogTitle : d.executeDialogTitle}
        description={dialogMode === "cancel" ? d.cancelDialogDescription : d.executeDialogDescription}
        confirmLabel={dialogMode === "cancel" ? d.cancelConfirm : d.executeConfirm}
        confirmVariant={dialogMode === "cancel" ? "destructive" : "default"}
        isPending={isPending}
        note={note}
        onNoteChange={setNote}
        onSubmit={submitDialog}
        context={
          <p>
            {dialogMode === "cancel"
              ? `${selectedWalletIds.length} ${d.walletTargets} · ${selectedConditionIds.length} ${d.marketTargets}`
              : `${d.strategy} #${selectedStrategyId} · ${selectedWalletIds.length} ${d.walletTargets}`}
          </p>
        }
      />
    </div>
  );
}

function RecordCard({ title, empty, children }: { title: string; empty: string; children: React.ReactNode }) {
  const hasChildren = Array.isArray(children) ? children.length > 0 : Boolean(children);
  return <Card><CardHeader><CardTitle>{title}</CardTitle></CardHeader><CardContent className="space-y-2">{hasChildren ? children : <div className="rounded border border-dashed p-6 text-center text-sm text-muted-foreground">{empty}</div>}</CardContent></Card>;
}

function parseIds(value: string): number[] {
  return [...new Set(value.split(",").map((item) => Number(item.trim())).filter((id) => Number.isSafeInteger(id) && id > 0))];
}

function parseValues(value: string): string[] {
  return [...new Set(value.split(",").map((item) => item.trim()).filter(Boolean))];
}
