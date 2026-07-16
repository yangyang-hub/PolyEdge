"use client";

import Link from "next/link";
import { useEffect, useState } from "react";

import { PageHeader } from "@/components/shared/page-header";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { listExecutionBatches, listOrders } from "@/lib/api/operations";
import { listStrategies } from "@/lib/api/strategies";
import { listWallets } from "@/lib/api/wallets";
import { dictionary } from "@/lib/i18n/dictionaries";

type Metrics = {
  strategies: number;
  wallets: number;
  openOrders: number;
  pendingBatches: number;
};

export function DashboardOverview() {
  const d = dictionary.dashboard;
  const [metrics, setMetrics] = useState<Metrics | null>(null);

  useEffect(() => {
    void Promise.all([listStrategies(), listWallets(), listOrders(), listExecutionBatches()])
      .then(([strategies, wallets, orders, batches]) => {
        setMetrics({
          strategies: strategies.data.length,
          wallets: wallets.data.filter((wallet) => wallet.account.trading_enabled).length,
          openOrders: orders.data.filter((order) => ["open", "partially_filled"].includes(order.status)).length,
          pendingBatches: batches.data.filter(({ batch }) => ["pending", "running"].includes(batch.status)).length,
        });
      })
      .catch(() => setMetrics(null));
  }, []);

  const cards = [
    [d.strategies, metrics?.strategies],
    [d.wallets, metrics?.wallets],
    [d.openOrders, metrics?.openOrders],
    [d.pendingBatches, metrics?.pendingBatches],
  ] as const;

  return (
    <div className="space-y-8">
      <PageHeader eyebrow={d.eyebrow} title={d.title} description={d.description} />
      <div className="grid gap-4 md:grid-cols-4">
        {cards.map(([label, value]) => (
          <Card key={label}>
            <CardHeader><CardTitle className="text-sm text-muted-foreground">{label}</CardTitle></CardHeader>
            <CardContent><p className="text-3xl font-semibold">{value ?? "—"}</p><p className="text-xs text-muted-foreground">{d.liveHint}</p></CardContent>
          </Card>
        ))}
      </div>
      <Card>
        <CardHeader><CardTitle>{d.quickStart}</CardTitle></CardHeader>
        <CardContent className="grid gap-3 md:grid-cols-3">
          {[["/strategies", d.strategyHint], ["/wallets", d.walletHint], ["/operations", d.operationHint]].map(([href, label]) => (
            <Link key={href} href={href} className="rounded-lg border border-border bg-background p-4 text-sm transition hover:border-primary hover:bg-accent">
              <span className="font-medium text-foreground">{label}</span>
              <span className="mt-2 block text-xs text-muted-foreground">{d.openWorkbench}</span>
            </Link>
          ))}
        </CardContent>
      </Card>
    </div>
  );
}
