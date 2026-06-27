"use client";

import { Activity, XCircle } from "lucide-react";

import { PaginationBar } from "@/components/pagination-bar";
import { StatusPill } from "@/components/shared/status-pill";
import { TruncateText } from "@/components/shared/truncate-text";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { usePagination } from "@/hooks/use-pagination";
import type { SmartMoneySnapshotDto, SmartSignalStatus } from "@/lib/contracts/dto";
import { formatShortAddress } from "@/lib/format-address";
import {
  formatFixed,
  formatOptionalClock,
  formatUsdFixed,
  uppercaseEnum,
  type Tone,
} from "@/lib/formatters";
import { dictionary } from "@/lib/i18n/dictionaries";

function signalStatusTone(status: SmartSignalStatus): Tone {
  if (status === "observe" || status === "paper" || status === "live_ready") {
    return "success";
  }

  if (status === "approval_required" || status === "new") {
    return "primary";
  }

  if (status === "rejected" || status === "expired") {
    return "danger";
  }

  return "neutral";
}

export function SmartMoneySignalsPanel({
  snapshot,
}: {
  snapshot: SmartMoneySnapshotDto;
}) {
  const t = dictionary.copytrade.smartSignals;
  const pagination = usePagination(snapshot.recent_signals.length, 12);

  return (
    <Card>
      <CardHeader className="flex flex-col gap-4 border-b border-border/70 xl:flex-row xl:items-center xl:justify-between">
        <div>
          <CardTitle className="font-heading text-base">{t.title}</CardTitle>
          <CardDescription>{t.description}</CardDescription>
        </div>
        <div className="flex flex-wrap gap-2 text-xs text-muted-foreground">
          <span>
            {t.total}: {snapshot.status.recent_signals}
          </span>
          <span>
            {t.trades}: {snapshot.status.recent_trades}
          </span>
        </div>
      </CardHeader>
      <CardContent>
        {snapshot.recent_signals.length === 0 ? (
          <p className="py-8 text-center text-sm text-muted-foreground">{t.noSignals}</p>
        ) : (
          <div className="overflow-auto">
            <table className="w-full min-w-[920px] text-xs">
              <thead>
                <tr className="border-b border-border/60 text-left text-muted-foreground">
                  <th className="pb-2 pr-3">{t.wallet}</th>
                  <th className="pb-2 pr-3">{t.market}</th>
                  <th className="pb-2 pr-3">{t.side}</th>
                  <th className="pb-2 pr-3">{t.price}</th>
                  <th className="pb-2 pr-3">{t.slippage}</th>
                  <th className="pb-2 pr-3">{t.status}</th>
                  <th className="pb-2 pr-3">{t.reason}</th>
                  <th className="pb-2 pr-3">{t.created}</th>
                </tr>
              </thead>
              <tbody>
                {snapshot.recent_signals.slice(pagination.start, pagination.end).map((signal) => (
                  <tr key={signal.id} className="border-b border-border/20">
                    <td className="py-3 pr-3 font-mono text-foreground">
                      {formatShortAddress(signal.wallet_address)}
                    </td>
                    <td className="py-3 pr-3 text-muted-foreground">
                      <TruncateText text={signal.condition_id} lines={1} />
                      {signal.token_id ? (
                        <p className="mt-1 font-mono text-[11px]">{signal.token_id}</p>
                      ) : null}
                    </td>
                    <td className="py-3 pr-3">
                      <span className="inline-flex items-center gap-1">
                        {signal.side === "buy" ? (
                          <Activity className="size-3 text-secondary" />
                        ) : (
                          <XCircle className="size-3 text-muted-foreground" />
                        )}
                        {uppercaseEnum(signal.side)}
                      </span>
                    </td>
                    <td className="py-3 pr-3 text-muted-foreground">
                      <p>{t.source}: {formatFixed(signal.source_price, 3)}</p>
                      <p>{t.current}: {signal.current_price == null ? "n/a" : formatFixed(signal.current_price, 3)}</p>
                      <p>{formatUsdFixed(signal.source_notional_usd)}</p>
                    </td>
                    <td className="py-3 pr-3 text-muted-foreground">
                      {signal.price_slippage_cents == null
                        ? "n/a"
                        : `${formatFixed(signal.price_slippage_cents, 2)}c`}
                    </td>
                    <td className="py-3 pr-3">
                      <StatusPill tone={signalStatusTone(signal.status)}>
                        {t.statusLabels[signal.status]}
                      </StatusPill>
                    </td>
                    <td className="py-3 pr-3 text-muted-foreground">
                      {signal.reason ? <TruncateText text={signal.reason} lines={2} /> : dictionary.common.none}
                    </td>
                    <td className="py-3 pr-3 text-muted-foreground">
                      {formatOptionalClock(signal.created_at)}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
            <PaginationBar
              pagination={pagination}
              totalItems={snapshot.recent_signals.length}
              className="mt-3 flex items-center justify-between border-t border-border/70 pt-3"
            />
          </div>
        )}
      </CardContent>
    </Card>
  );
}
