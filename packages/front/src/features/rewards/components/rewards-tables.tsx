"use client";

import { StatusPill } from "@/components/shared/status-pill";
import { TruncateText } from "@/components/shared/truncate-text";
import { PaginationBar } from "@/components/pagination-bar";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import type {
  ManagedRewardOrderDto,
  PostFillStrategy,
  RewardExitStrategySource,
  RewardListPageDto,
  RewardProviderAction,
  RewardQuotePlanDto,
  RewardStrategyProfile,
  RewardTokenQuoteDto,
} from "@/lib/contracts/dto";
import { formatFixed, formatUsdFixed } from "@/lib/formatters";
import type { PaginationState } from "@/hooks/use-pagination";
import { dictionary } from "@/lib/i18n/dictionaries";

import {
  quoteReadinessLabel,
  quoteReadinessTone,
  rewardTone,
} from "../lib/rewards-helpers";
import { getPositionQuote } from "../lib/position-metrics";
import { OpportunitySummary } from "./rewards-opportunity-summary";
import { DebouncedFilterBar, SortIndicator } from "./rewards-table-controls";

export { FillsTable } from "./rewards-fills-table";
export { EventsTable } from "./rewards-events-table";
export { PositionsTable } from "./rewards-positions-table";

function providerActionTone(action: RewardProviderAction) {
  if (action === "allow") return "success" as const;
  if (action === "reduce") return "warning" as const;
  return "danger" as const;
}

function providerActionLabel(action: RewardProviderAction) {
  if (action === "allow") return dictionary.rewards.providerActionAllow;
  if (action === "reduce") return dictionary.rewards.providerActionReduce;
  if (action === "stop_new") return dictionary.rewards.providerActionStopNew;
  if (action === "cancel_yes") return dictionary.rewards.providerActionCancelYes;
  if (action === "cancel_no") return dictionary.rewards.providerActionCancelNo;
  return dictionary.rewards.providerActionCancelAll;
}

function strategyProfileLabel(profile?: RewardStrategyProfile | null) {
  if (profile === "balanced_merge") return dictionary.rewards.strategyProfileBalancedMerge;
  return dictionary.rewards.strategyProfileStandard;
}

function postFillStrategyLabel(strategy?: PostFillStrategy | null) {
  if (strategy === "exit_at_markup") return dictionary.rewards.strategyExitMarkup;
  if (strategy === "hold_and_requote") return dictionary.rewards.strategyHold;
  if (strategy === "flatten_immediately") return dictionary.rewards.strategyFlatten;
  if (strategy === "adaptive") return dictionary.rewards.strategyAdaptive;
  return dictionary.rewards.notAvailable;
}

function exitStrategySourceLabel(source?: RewardExitStrategySource | null) {
  if (source === "adaptive") return dictionary.rewards.exitSourceAdaptive;
  if (source === "external_inventory") return dictionary.rewards.exitSourceExternalInventory;
  if (source === "configured") return dictionary.rewards.exitSourceConfigured;
  return dictionary.rewards.notAvailable;
}

function orderExitStrategySummary(order: ManagedRewardOrderDto) {
  if (order.side !== "sell" && !order.exit_strategy_selected) {
    return dictionary.rewards.notAvailable;
  }
  const source = exitStrategySourceLabel(order.exit_strategy_source);
  const strategy = postFillStrategyLabel(order.exit_strategy_selected);
  const count = order.exit_reselect_count ?? 0;
  return count > 0 ? `${source} / ${strategy} / ${count}` : `${source} / ${strategy}`;
}

function paginationFromPage(
  page: RewardListPageDto,
  itemCount: number,
  onPageChange: (page: number) => void,
): PaginationState {
  return {
    page: page.page,
    totalPages: page.total_pages,
    start: 0,
    end: itemCount,
    setPage: onPageChange,
    goPrevious: () => onPageChange(Math.max(1, page.page - 1)),
    goNext: () => onPageChange(Math.min(page.total_pages, page.page + 1)),
    reset: () => onPageChange(1),
    hasPrevious: page.page > 1,
    hasNext: page.page < page.total_pages,
  };
}


interface QuotePlansTableProps {
  plans: RewardQuotePlanDto[];
  plansPage: RewardListPageDto;
  plansTotal: number;
  eligibleTotal: number;
  search: string;
  onSearchChange: (v: string) => void;
  eligibility: "all" | "eligible" | "ineligible";
  onEligibilityChange: (v: "all" | "eligible" | "ineligible") => void;
  sortBy: string;
  sortOrder: "asc" | "desc";
  onSortChange: (by: string, order: "asc" | "desc") => void;
  onPageChange: (page: number) => void;
  filtering?: boolean;
}

export function QuotePlansTable({
  plans, plansPage, plansTotal, eligibleTotal, search, onSearchChange, eligibility, onEligibilityChange,
  sortBy, sortOrder, onSortChange, onPageChange, filtering,
}: QuotePlansTableProps) {
  // Server-side pagination: plans are already filtered/sorted/paged by the API.
  const tabs = [
    { key: "all", label: dictionary.rewards.filterAll, count: plansTotal },
    { key: "eligible", label: dictionary.rewards.filterEligible, count: eligibleTotal },
    { key: "ineligible", label: dictionary.rewards.filterIneligible, count: plansTotal - eligibleTotal },
  ];

  function handleSort(field: string) {
    if (sortBy === field) {
      onSortChange(field, sortOrder === "asc" ? "desc" : "asc");
    } else {
      onSortChange(field, "desc");
    }
  }

  const pagination = paginationFromPage(plansPage, plans.length, onPageChange);

  return (
    <div className="space-y-3">
      <DebouncedFilterBar
        initialSearch={search}
        onSearchChange={onSearchChange}
        placeholder={dictionary.rewards.searchPlaceholder}
        tabs={tabs}
        activeTab={eligibility}
        onTabChange={(key) => onEligibilityChange(key as typeof eligibility)}
      />
      {filtering && <p className="text-xs text-muted-foreground">…</p>}
      <Table className="min-w-[1360px] table-fixed">
        <TableHeader>
          <TableRow>
            <TableHead className="w-[30%]">{dictionary.rewards.market}</TableHead>
            <TableHead className="w-[280px]">{dictionary.rewards.state}</TableHead>
            <TableHead
              aria-sort={sortBy === "selection_score" ? (sortOrder === "asc" ? "ascending" : "descending") : "none"}
            >
              <button
                type="button"
                onClick={() => handleSort("selection_score")}
                className="inline-flex cursor-pointer select-none items-center"
              >
                {dictionary.rewards.selectionScore}
                <SortIndicator active={sortBy === "selection_score"} order={sortOrder} />
              </button>
            </TableHead>
            <TableHead
              aria-sort={sortBy === "daily_reward" ? (sortOrder === "asc" ? "ascending" : "descending") : "none"}
            >
              <button
                type="button"
                onClick={() => handleSort("daily_reward")}
                className="inline-flex cursor-pointer select-none items-center"
              >
                {dictionary.rewards.dailyReward}
                <SortIndicator active={sortBy === "daily_reward"} order={sortOrder} />
              </button>
            </TableHead>
            <TableHead
              aria-sort={sortBy === "midpoint" ? (sortOrder === "asc" ? "ascending" : "descending") : "none"}
            >
              <button
                type="button"
                onClick={() => handleSort("midpoint")}
                className="inline-flex cursor-pointer select-none items-center"
              >
                {dictionary.rewards.midpoint}
                <SortIndicator active={sortBy === "midpoint"} order={sortOrder} />
              </button>
            </TableHead>
            <TableHead className="w-[180px]">{dictionary.rewards.quotes}</TableHead>
            <TableHead className="w-[230px]">{dictionary.rewards.infoRisk}</TableHead>
            <TableHead className="w-[230px]">{dictionary.rewards.aiAdvisory}</TableHead>
          </TableRow>
        </TableHeader>
        <TableBody>
          {plans.length === 0 ? (
            <TableRow>
              <TableCell colSpan={8} className="py-6 text-center text-sm text-muted-foreground">
                {dictionary.rewards.none}
              </TableCell>
            </TableRow>
          ) : (
            plans.map((plan) => (
              <TableRow key={`${plan.condition_id}:${plan.strategy_profile ?? "standard"}`}>
                <TableCell className="whitespace-normal align-top">
                  <div className="space-y-1">
                    <div className="flex flex-wrap items-center gap-1">
                      <StatusPill tone={plan.strategy_profile === "balanced_merge" ? "warning" : "neutral"}>
                        {strategyProfileLabel(plan.strategy_profile)}
                      </StatusPill>
                    </div>
                    <TruncateText text={plan.question} lines={2} className="font-medium leading-snug" />
                    <TruncateText text={plan.reason} lines={1} className="text-xs leading-5 text-muted-foreground" />
                  </div>
                </TableCell>
                <TableCell className="align-top">
                  <StatusPill tone={quoteReadinessTone(plan)}>
                    {quoteReadinessLabel(plan)}
                  </StatusPill>
                  <OpportunitySummary plan={plan} />
                </TableCell>
                <TableCell className="align-top">
                  <StatusPill tone={plan.eligible ? "success" : "neutral"}>
                    {formatFixed(plan.selection_score ?? plan.score, 1)}
                  </StatusPill>
                  <div className="mt-1 font-mono text-[11px] leading-4 text-muted-foreground">
                    {dictionary.rewards.baseScore} {formatFixed(plan.score, 1)}
                  </div>
                </TableCell>
                <TableCell className="align-top font-mono">{formatUsdFixed(plan.total_daily_rate)}</TableCell>
                <TableCell className="align-top font-mono">{plan.midpoint == null ? "n/a" : formatFixed(plan.midpoint, 3)}</TableCell>
                <TableCell className="whitespace-normal break-words align-top font-mono text-xs leading-5">
                  {plan.legs.length === 0
                    ? dictionary.rewards.none
                    : plan.legs.map((leg) => `${leg.outcome} ${formatFixed(leg.size, 2)}@${formatFixed(leg.price, 2)}`).join(" / ")}
                </TableCell>
                <TableCell className="whitespace-normal align-top text-xs">
                  {plan.info_risk == null ? (
                    <span className="text-muted-foreground">{dictionary.rewards.none}</span>
                  ) : (
                      <div className="space-y-1">
                        <div className="flex flex-wrap items-center gap-1">
                          <StatusPill tone={providerActionTone(plan.info_risk.action)}>
                            {providerActionLabel(plan.info_risk.action)}
                          </StatusPill>
                          <span className="font-mono text-muted-foreground">
                            {plan.info_risk.risk_level} · {dictionary.common.confidence} {formatFixed(plan.info_risk.confidence, 2)}
                          </span>
                        </div>
                        <TruncateText text={plan.info_risk.summary} lines={2} className="leading-5 text-muted-foreground" />
                      </div>
                  )}
                </TableCell>
                <TableCell className="whitespace-normal align-top text-xs">
                  {plan.ai_advisory == null ? (
                    <span className="text-muted-foreground">{dictionary.rewards.none}</span>
                  ) : (
                      <div className="space-y-1">
                        <div className="flex flex-wrap items-center gap-1">
                          <StatusPill tone={providerActionTone(plan.ai_advisory.action)}>
                            {providerActionLabel(plan.ai_advisory.action)}
                          </StatusPill>
                          <span className="font-mono text-muted-foreground">
                            {dictionary.common.confidence} {formatFixed(plan.ai_advisory.confidence, 2)}
                          </span>
                        </div>
                        <TruncateText
                          text={plan.ai_advisory.reasons[0] ?? dictionary.rewards.none}
                          lines={2}
                          className="leading-5 text-muted-foreground"
                        />
                        <div className="grid grid-cols-[auto_1fr] gap-x-2 gap-y-0.5 font-mono text-[11px] leading-4 text-muted-foreground">
                          <span>{dictionary.rewards.providerSizeMultiplier}</span>
                          <span>{formatFixed(plan.ai_advisory.size_multiplier, 2)}x</span>
                          <span>{dictionary.rewards.providerEdgeBuffer}</span>
                          <span>{formatFixed(plan.ai_advisory.edge_buffer_cents, 2)}c</span>
                        </div>
                      </div>
                  )}
                </TableCell>
              </TableRow>
            ))
          )}
        </TableBody>
      </Table>
      <PaginationBar pagination={pagination} totalItems={plansPage.total_items} />
    </div>
  );
}

interface OrdersTableProps {
  orders: ManagedRewardOrderDto[];
  tokenQuotes: Record<string, RewardTokenQuoteDto> | null | undefined;
  search: string;
  onSearchChange: (v: string) => void;
  status: "all" | "open" | "filled" | "cancelled" | "exit_pending";
  onStatusChange: (v: "all" | "open" | "filled" | "cancelled" | "exit_pending") => void;
  sortBy: string;
  sortOrder: "asc" | "desc";
  onSortChange: (by: string, order: "asc" | "desc") => void;
  page: RewardListPageDto;
  onPageChange: (page: number) => void;
  filtering?: boolean;
}

export function OrdersTable({
  orders, tokenQuotes, search, onSearchChange, status, onStatusChange,
  sortBy, sortOrder, onSortChange, page, onPageChange, filtering,
}: OrdersTableProps) {
  const tabs = [
    { key: "all", label: dictionary.rewards.filterAll },
    { key: "open", label: dictionary.rewards.filterOpen },
    { key: "filled", label: dictionary.rewards.filterFilled },
    { key: "cancelled", label: dictionary.rewards.filterCancelled },
    { key: "exit_pending", label: dictionary.rewards.filterExit },
  ];

  function handleSort(field: string) {
    if (sortBy === field) {
      onSortChange(field, sortOrder === "asc" ? "desc" : "asc");
    } else {
      onSortChange(field, "desc");
    }
  }

  const pagination = paginationFromPage(page, orders.length, onPageChange);

  return (
    <div className="space-y-3">
      <DebouncedFilterBar
        initialSearch={search}
        onSearchChange={onSearchChange}
        placeholder={dictionary.rewards.searchOrdersPlaceholder}
        tabs={tabs}
        activeTab={status}
        onTabChange={(key) => onStatusChange(key as typeof status)}
      />
      {filtering && <p className="text-xs text-muted-foreground">…</p>}
      <Table className="min-w-[980px] table-fixed">
        <TableHeader>
          <TableRow>
            <TableHead className="w-[120px]">{dictionary.rewards.state}</TableHead>
            <TableHead>{dictionary.rewards.outcome}</TableHead>
            <TableHead
              aria-sort={sortBy === "price" ? (sortOrder === "asc" ? "ascending" : "descending") : "none"}
            >
              <button
                type="button"
                onClick={() => handleSort("price")}
                className="inline-flex cursor-pointer select-none items-center"
              >
                {dictionary.rewards.price}
                <SortIndicator active={sortBy === "price"} order={sortOrder} />
              </button>
            </TableHead>
            <TableHead>{dictionary.rewards.bestBid}</TableHead>
            <TableHead>{dictionary.rewards.bestAsk}</TableHead>
            <TableHead
              aria-sort={sortBy === "size" ? (sortOrder === "asc" ? "ascending" : "descending") : "none"}
            >
              <button
                type="button"
                onClick={() => handleSort("size")}
                className="inline-flex cursor-pointer select-none items-center"
              >
                {dictionary.rewards.size}
                <SortIndicator active={sortBy === "size"} order={sortOrder} />
              </button>
            </TableHead>
            <TableHead>{dictionary.rewards.scoring}</TableHead>
            <TableHead>{dictionary.rewards.exitStrategy}</TableHead>
            <TableHead className="w-[320px]">{dictionary.rewards.reason}</TableHead>
          </TableRow>
        </TableHeader>
        <TableBody>
          {orders.length === 0 ? (
            <TableRow>
              <TableCell colSpan={9} className="py-6 text-center text-sm text-muted-foreground">
                {dictionary.rewards.none}
              </TableCell>
            </TableRow>
          ) : (
            orders.map((order) => {
              const orderQuote = getPositionQuote(tokenQuotes, order.token_id);
              const orderBestBid = orderQuote?.best_bid ?? null;
              const orderBestAsk = orderQuote?.best_ask ?? null;
              return (
                <TableRow key={order.id}>
                  <TableCell className="align-top">
                    <div className="flex flex-col items-start gap-1">
                      <StatusPill tone={rewardTone(order.status)}>{order.status}</StatusPill>
                      <StatusPill tone={order.strategy_profile === "balanced_merge" ? "warning" : "neutral"}>
                        {strategyProfileLabel(order.strategy_profile)}
                      </StatusPill>
                    </div>
                  </TableCell>
                  <TableCell className="align-top">{order.outcome}</TableCell>
                  <TableCell className="align-top font-mono">{formatFixed(order.price, 2)}</TableCell>
                  <TableCell className="align-top font-mono">
                    {orderBestBid != null ? formatFixed(orderBestBid, 3) : "—"}
                  </TableCell>
                  <TableCell className="align-top font-mono">
                    {orderBestAsk != null ? formatFixed(orderBestAsk, 3) : "—"}
                  </TableCell>
                  <TableCell className="align-top font-mono">{formatFixed(order.size, 2)}</TableCell>
                  <TableCell className="align-top">{order.scoring ? dictionary.common.active : dictionary.common.idle}</TableCell>
                  <TableCell className="align-top text-xs text-muted-foreground">
                    {orderExitStrategySummary(order)}
                  </TableCell>
                  <TableCell className="align-top text-xs leading-5 text-muted-foreground">
                    <TruncateText text={order.reason} lines={2} />
                  </TableCell>
                </TableRow>
              );
            })
          )}
        </TableBody>
      </Table>
      <PaginationBar pagination={pagination} totalItems={page.total_items} />
    </div>
  );
}
