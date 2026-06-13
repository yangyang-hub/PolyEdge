"use client";

import { startTransition, useState } from "react";
import { Search } from "lucide-react";

import { PageHeader } from "@/components/shared/page-header";
import { MetricCard } from "@/components/shared/metric-card";
import { PaginationBar } from "@/components/pagination-bar";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import type { WalletAnalysisReportDto } from "@/lib/contracts/dto";
import { formatUsdFixed, metricToneForPnl } from "@/lib/formatters";
import { usePagination } from "@/hooks/use-pagination";
import { dictionary } from "@/lib/i18n/dictionaries";
import { analyzeWallet } from "@/lib/api/wallet-analysis";

function toNum(v: string | number | undefined | null): number {
  if (typeof v === "number") return v;
  if (typeof v === "string") return Number.parseFloat(v) || 0;
  return 0;
}

function formatPct(v: string | number): string {
  return `${(toNum(v) * 100).toFixed(1)}%`;
}

function formatHours(v: string | number): string {
  const h = toNum(v);
  if (h < 1) return `${(h * 60).toFixed(0)}m`;
  if (h < 24) return `${h.toFixed(1)}h`;
  return `${(h / 24).toFixed(1)}d`;
}

function truncateAddr(addr: string): string {
  if (addr.length <= 12) return addr;
  return `${addr.slice(0, 6)}...${addr.slice(-4)}`;
}

const styleLabels: Record<string, string> = {
  scalper: "styleScalper",
  day_trader: "styleDayTrader",
  swing_trader: "styleSwingTrader",
  position_trader: "stylePositionTrader",
  mixed: "styleMixed",
};

export function WalletAnalysisWorkbench() {
  const t = dictionary.walletAnalysis;
  const [address, setAddress] = useState("");
  const [report, setReport] = useState<WalletAnalysisReportDto | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const tradesPagination = usePagination(report?.recent_trades.length ?? 0, 10);
  const closedPagination = usePagination(report?.recent_closed.length ?? 0, 10);

  function handleAnalyze() {
    const addr = address.trim();
    if (!addr) return;
    setLoading(true);
    setError(null);
    startTransition(() => {
      analyzeWallet(addr)
        .then((res) => {
          if (res.data) {
            setReport(res.data);
          } else {
            setError(t.errorPrefix);
          }
        })
        .catch((e: Error) => setError(e.message))
        .finally(() => setLoading(false));
    });
  }

  return (
    <div className="space-y-6">
      <PageHeader
        eyebrow={t.eyebrow}
        title={t.title}
        description={t.description}
      />

      {/* Input */}
      <Card>
        <CardContent className="flex gap-3 pt-6">
          <Input
            placeholder={t.inputPlaceholder}
            value={address}
            onChange={(e) => setAddress(e.target.value)}
            onKeyDown={(e) => e.key === "Enter" && handleAnalyze()}
            className="font-mono text-sm"
          />
          <Button onClick={handleAnalyze} disabled={loading || !address.trim()}>
            <Search className="mr-2 size-4" />
            {loading ? t.analyzing : t.analyze}
          </Button>
        </CardContent>
      </Card>

      {error && (
        <Card className="border-destructive">
          <CardContent className="pt-6 text-destructive">{error}</CardContent>
        </Card>
      )}

      {!report && !loading && !error && (
        <Card>
          <CardContent className="pt-6 text-center text-muted-foreground">
            {t.noData}
          </CardContent>
        </Card>
      )}

      {report && <AnalysisReport report={report} t={t} tradesPagination={tradesPagination} closedPagination={closedPagination} />}
    </div>
  );
}

function AnalysisReport({
  report,
  t,
  tradesPagination,
  closedPagination,
}: {
  report: WalletAnalysisReportDto;
  t: Record<string, string>;
  tradesPagination: ReturnType<typeof usePagination>;
  closedPagination: ReturnType<typeof usePagination>;
}) {
  const { profile, pnl, activity, categories, style, risk, top_markets, recent_trades, winners, losers, recent_closed } = report;

  return (
    <div className="space-y-6">
      {/* Profile */}
      <Card>
        <CardHeader>
          <CardTitle>{t.profile}</CardTitle>
        </CardHeader>
        <CardContent>
          <div className="grid grid-cols-2 gap-4 sm:grid-cols-3 lg:grid-cols-4">
            <InfoItem label={t.profile} value={profile.name || profile.pseudonym || truncateAddr(profile.address)} />
            {profile.x_username && <InfoItem label={t.xHandle} value={`@${profile.x_username}`} />}
            {profile.created_at && <InfoItem label={t.memberSince} value={profile.created_at.slice(0, 10)} />}
            {profile.verified_badge && <InfoItem label={t.verified} value="✓" />}
            {profile.leaderboard_rank > 0 && <InfoItem label={t.leaderboardRank} value={`#${profile.leaderboard_rank}`} />}
            <InfoItem label={t.portfolioValue} value={formatUsdFixed(profile.portfolio_value)} />
            <InfoItem label={t.totalMarketsTraded} value={String(profile.total_markets_traded)} />
            {toNum(profile.leaderboard_volume) > 0 && <InfoItem label={t.leaderboardVolume} value={formatUsdFixed(profile.leaderboard_volume)} />}
          </div>
        </CardContent>
      </Card>

      {/* P&L */}
      <div className="grid grid-cols-2 gap-4 sm:grid-cols-3 lg:grid-cols-5">
        <MetricCard title={t.totalPnl} value={formatUsdFixed(pnl.total_pnl)} hint="" accent={metricToneForPnl(pnl.total_pnl)} />
        <MetricCard title={t.overallRoi} value={formatPct(pnl.overall_roi)} hint="" accent={metricToneForPnl(pnl.overall_roi)} />
        <MetricCard title={t.winRateClosed} value={formatPct(pnl.win_rate_closed)} hint="" accent="primary" />
        <MetricCard title={t.largestWin} value={formatUsdFixed(pnl.largest_win)} hint="" accent="success" />
        <MetricCard title={t.largestLoss} value={formatUsdFixed(pnl.largest_loss)} hint="" accent="danger" />
      </div>

      <div className="grid grid-cols-2 gap-4 sm:grid-cols-4">
        <MetricCard title={t.totalRealizedPnl} value={formatUsdFixed(pnl.total_realized_pnl)} hint="" accent={metricToneForPnl(pnl.total_realized_pnl)} />
        <MetricCard title={t.totalUnrealizedPnl} value={formatUsdFixed(pnl.total_unrealized_pnl)} hint="" accent={metricToneForPnl(pnl.total_unrealized_pnl)} />
        <MetricCard title={t.closedPositions} value={String(pnl.closed_positions_count)} hint="" accent="primary" />
        <MetricCard title={t.openPositions} value={String(pnl.open_positions_count)} hint="" accent="primary" />
      </div>

      {/* Activity */}
      <div className="grid grid-cols-2 gap-4 sm:grid-cols-3 lg:grid-cols-4">
        <MetricCard title={t.totalVolume} value={formatUsdFixed(activity.total_volume_usd)} hint="" accent="primary" />
        <MetricCard title={t.totalTrades} value={String(activity.total_trades)} hint="" accent="primary" />
        <MetricCard title={t.avgTrade} value={formatUsdFixed(activity.avg_trade_usd)} hint="" accent="primary" />
        <MetricCard title={t.medianTrade} value={formatUsdFixed(activity.median_trade_usd)} hint="" accent="primary" />
        <MetricCard title={t.tradingDays} value={String(activity.trading_days)} hint="" accent="primary" />
        <MetricCard title={t.avgTradesPerDay} value={toNum(activity.avg_trades_per_day).toFixed(1)} hint="" accent="primary" />
        <MetricCard title={t.buyRatio} value={formatPct(activity.buy_ratio)} hint="" accent="primary" />
        <MetricCard title={t.buyVolume} value={formatUsdFixed(activity.total_buy_volume)} hint="" accent="primary" />
      </div>

      {/* Style */}
      <Card>
        <CardHeader><CardTitle>{t.style}</CardTitle></CardHeader>
        <CardContent>
          <div className="grid grid-cols-2 gap-4 sm:grid-cols-3 lg:grid-cols-4">
            <InfoItem label={t.styleLabel} value={t[styleLabels[style.style_label] as keyof typeof t] ?? style.style_label} highlight />
            <InfoItem label={t.avgHoldTime} value={formatHours(style.avg_hold_hours)} />
            <InfoItem label={t.directionalBias} value={formatPct(style.directional_bias)} />
            <InfoItem label={t.priceRange} value={`${toNum(style.preferred_price_range_low).toFixed(2)} — ${toNum(style.preferred_price_range_high).toFixed(2)}`} />
            <InfoItem label={t.priceConcentration} value={style.price_concentration} />
            <InfoItem label={t.tradeSizeStddev} value={formatUsdFixed(style.trade_size_stddev)} />
          </div>
        </CardContent>
      </Card>

      {/* Risk */}
      <Card>
        <CardHeader><CardTitle>{t.risk}</CardTitle></CardHeader>
        <CardContent>
          <div className="grid grid-cols-2 gap-4 sm:grid-cols-3 lg:grid-cols-5">
            <MetricCard title={t.maxExposure} value={formatPct(risk.max_single_market_exposure_pct)} hint="" accent="primary" />
            <MetricCard title={t.maxDrawdown} value={formatUsdFixed(risk.max_drawdown_estimate)} hint="" accent="danger" />
            <MetricCard title={t.avgPositionSize} value={formatPct(risk.avg_position_size_pct)} hint="" accent="primary" />
            <MetricCard title={t.diversification} value={toNum(risk.diversification_score).toFixed(2)} hint="" accent="primary" />
            <InfoItem label={t.concentration} value={risk.concentration_label} />
          </div>
        </CardContent>
      </Card>

      {/* Categories */}
      {categories.length > 0 && (
        <Card>
          <CardHeader><CardTitle>{t.categories}</CardTitle></CardHeader>
          <CardContent>
            <div className="overflow-x-auto">
              <table className="w-full text-sm">
                <thead>
                  <tr className="border-b text-left text-muted-foreground">
                    <th className="pb-2 pr-4">{t.category}</th>
                    <th className="pb-2 pr-4 text-right">{t.trades}</th>
                    <th className="pb-2 pr-4 text-right">{t.volume}</th>
                    <th className="pb-2 pr-4 text-right">{t.pnlLabel}</th>
                    <th className="pb-2 pr-4 text-right">{t.wins}</th>
                    <th className="pb-2 text-right">{t.losses}</th>
                  </tr>
                </thead>
                <tbody>
                  {categories.map((c) => (
                    <tr key={c.category} className="border-b border-dashed">
                      <td className="py-2 pr-4 font-medium capitalize">{c.category}</td>
                      <td className="py-2 pr-4 text-right tabular-nums">{c.trade_count}</td>
                      <td className="py-2 pr-4 text-right tabular-nums">{formatUsdFixed(c.volume_usd)}</td>
                      <td className={`py-2 pr-4 text-right tabular-nums ${metricToneForPnl(c.pnl) === "success" ? "text-positive" : metricToneForPnl(c.pnl) === "danger" ? "text-negative" : ""}`}>{formatUsdFixed(c.pnl)}</td>
                      <td className="py-2 pr-4 text-right tabular-nums text-positive">{c.win_count}</td>
                      <td className="py-2 text-right tabular-nums text-negative">{c.loss_count}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          </CardContent>
        </Card>
      )}

      {/* Top Markets */}
      {top_markets.length > 0 && (
        <Card>
          <CardHeader><CardTitle>{t.topMarkets}</CardTitle></CardHeader>
          <CardContent>
            <div className="overflow-x-auto">
              <table className="w-full text-sm">
                <thead>
                  <tr className="border-b text-left text-muted-foreground">
                    <th className="pb-2 pr-4">{t.market}</th>
                    <th className="pb-2 pr-4 text-right">{t.tradeCount}</th>
                    <th className="pb-2 pr-4 text-right">{t.volume}</th>
                    <th className="pb-2 pr-4 text-right">{t.pnlLabel}</th>
                    <th className="pb-2 pr-4 text-right">{t.buyCount}</th>
                    <th className="pb-2 text-right">{t.sellCount}</th>
                  </tr>
                </thead>
                <tbody>
                  {top_markets.map((m) => (
                    <tr key={m.condition_id} className="border-b border-dashed">
                      <td className="max-w-[300px] truncate py-2 pr-4 font-medium" title={m.title}>{m.title}</td>
                      <td className="py-2 pr-4 text-right tabular-nums">{m.trade_count}</td>
                      <td className="py-2 pr-4 text-right tabular-nums">{formatUsdFixed(m.volume_usd)}</td>
                      <td className={`py-2 pr-4 text-right tabular-nums ${metricToneForPnl(m.pnl) === "success" ? "text-positive" : metricToneForPnl(m.pnl) === "danger" ? "text-negative" : ""}`}>{formatUsdFixed(m.pnl)}</td>
                      <td className="py-2 pr-4 text-right tabular-nums">{m.buy_count}</td>
                      <td className="py-2 text-right tabular-nums">{m.sell_count}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          </CardContent>
        </Card>
      )}

      {/* Winners & Losers */}
      <div className="grid gap-6 lg:grid-cols-2">
        <WinnersLosersCard title={t.winners} items={winners} positive />
        <WinnersLosersCard title={t.losers} items={losers} />
      </div>

      {/* Recent Trades */}
      {recent_trades.length > 0 && (
        <Card>
          <CardHeader><CardTitle>{t.recentTrades}</CardTitle></CardHeader>
          <CardContent>
            <div className="overflow-x-auto">
              <table className="w-full text-sm">
                <thead>
                  <tr className="border-b text-left text-muted-foreground">
                    <th className="pb-2 pr-4">{t.side}</th>
                    <th className="pb-2 pr-4">{t.market}</th>
                    <th className="pb-2 pr-4">{t.outcome}</th>
                    <th className="pb-2 pr-4 text-right">{t.price}</th>
                    <th className="pb-2 pr-4 text-right">{t.notional}</th>
                    <th className="pb-2">{t.time}</th>
                  </tr>
                </thead>
                <tbody>
                  {recent_trades
                    .slice(tradesPagination.start, tradesPagination.end)
                    .map((tr, i) => (
                      <tr key={`${tr.timestamp}-${i}`} className="border-b border-dashed">
                        <td className={`py-2 pr-4 font-medium ${tr.side === "BUY" ? "text-positive" : "text-negative"}`}>{tr.side}</td>
                        <td className="max-w-[200px] truncate py-2 pr-4" title={tr.title}>{tr.title}</td>
                        <td className="py-2 pr-4">{tr.outcome}</td>
                        <td className="py-2 pr-4 text-right tabular-nums">{toNum(tr.price).toFixed(4)}</td>
                        <td className="py-2 pr-4 text-right tabular-nums">{formatUsdFixed(tr.notional_usd)}</td>
                        <td className="py-2 text-muted-foreground">{tr.timestamp ? new Date(tr.timestamp).toLocaleDateString() : "-"}</td>
                      </tr>
                    ))}
                </tbody>
              </table>
            </div>
            <PaginationBar pagination={tradesPagination} totalItems={recent_trades.length} />
          </CardContent>
        </Card>
      )}

      {/* Recent Closed */}
      {recent_closed.length > 0 && (
        <Card>
          <CardHeader><CardTitle>{t.recentClosed}</CardTitle></CardHeader>
          <CardContent>
            <div className="overflow-x-auto">
              <table className="w-full text-sm">
                <thead>
                  <tr className="border-b text-left text-muted-foreground">
                    <th className="pb-2 pr-4">{t.market}</th>
                    <th className="pb-2 pr-4">{t.outcome}</th>
                    <th className="pb-2 pr-4 text-right">{t.avgPrice}</th>
                    <th className="pb-2 pr-4 text-right">{t.totalBought}</th>
                    <th className="pb-2 text-right">{t.realizedPnl}</th>
                  </tr>
                </thead>
                <tbody>
                  {recent_closed
                    .slice(closedPagination.start, closedPagination.end)
                    .map((cp, i) => (
                      <tr key={`${cp.timestamp}-${i}`} className="border-b border-dashed">
                        <td className="max-w-[200px] truncate py-2 pr-4 font-medium" title={cp.title}>{cp.title}</td>
                        <td className="py-2 pr-4">{cp.outcome}</td>
                        <td className="py-2 pr-4 text-right tabular-nums">{toNum(cp.avg_price).toFixed(4)}</td>
                        <td className="py-2 pr-4 text-right tabular-nums">{toNum(cp.total_bought).toFixed(2)}</td>
                        <td className={`py-2 text-right tabular-nums font-medium ${metricToneForPnl(cp.realized_pnl) === "success" ? "text-positive" : metricToneForPnl(cp.realized_pnl) === "danger" ? "text-negative" : ""}`}>{formatUsdFixed(cp.realized_pnl)}</td>
                      </tr>
                    ))}
                </tbody>
              </table>
            </div>
            <PaginationBar pagination={closedPagination} totalItems={recent_closed.length} />
          </CardContent>
        </Card>
      )}
    </div>
  );
}

function InfoItem({ label, value, highlight }: { label: string; value: string; highlight?: boolean }) {
  return (
    <div className="space-y-1">
      <p className="text-xs text-muted-foreground">{label}</p>
      <p className={`text-sm ${highlight ? "font-semibold text-primary" : "font-medium"}`}>{value}</p>
    </div>
  );
}

function WinnersLosersCard({
  title,
  items,
  positive,
}: {
  title: string;
  items: WalletAnalysisReportDto["winners"];
  positive?: boolean;
}) {
  return (
    <Card>
      <CardHeader><CardTitle>{title}</CardTitle></CardHeader>
      <CardContent>
        {items.length === 0 ? (
          <p className="text-sm text-muted-foreground">-</p>
        ) : (
          <div className="space-y-2">
            {items.map((item, i) => (
              <div key={`${item.title}-${i}`} className="flex items-center justify-between border-b border-dashed py-2 last:border-0">
                <div className="max-w-[250px] truncate">
                  <p className="text-sm font-medium" title={item.title}>{item.title}</p>
                  <p className="text-xs text-muted-foreground">{item.outcome}</p>
                </div>
                <span className={`text-sm font-semibold tabular-nums ${positive ? "text-positive" : "text-negative"}`}>
                  {formatUsdFixed(item.realized_pnl)}
                </span>
              </div>
            ))}
          </div>
        )}
      </CardContent>
    </Card>
  );
}
