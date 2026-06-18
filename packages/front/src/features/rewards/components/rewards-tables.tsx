"use client";

import { useCallback, useEffect, useRef, useState } from "react";
import { ArrowDown, ArrowUp, Search } from "lucide-react";

import { StatusPill } from "@/components/shared/status-pill";
import { PaginationBar } from "@/components/pagination-bar";
import { Input } from "@/components/ui/input";
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
  RewardFillDto,
  RewardListPageDto,
  RewardPositionDto,
  RewardQuotePlanDto,
  RewardRiskEventDto,
} from "@/lib/contracts/dto";
import {
  approvalSeverityTone,
  formatFixed,
  formatOptionalClock,
  formatSignedFixed,
  formatUsdFixed,
} from "@/lib/formatters";
import { usePagination } from "@/hooks/use-pagination";
import type { PaginationState } from "@/hooks/use-pagination";
import { dictionary } from "@/lib/i18n/dictionaries";

import { rewardTone } from "../lib/rewards-helpers";

function SortIndicator({ active, order }: { active: boolean; order: "asc" | "desc" }) {
  if (!active) return null;
  return order === "asc" ? <ArrowUp className="ml-1 inline size-3" /> : <ArrowDown className="ml-1 inline size-3" />;
}

function aiSuitabilityTone(suitability?: string | null) {
  if (suitability === "allow") return "success";
  if (suitability === "avoid") return "danger";
  if (suitability === "watch") return "warning";
  return "neutral";
}

function infoRiskTone(level?: string | null) {
  if (level === "critical" || level === "high") return "danger";
  if (level === "medium" || level === "unknown") return "warning";
  if (level === "low") return "success";
  return "neutral";
}

function FilterBar({
  search,
  onSearchChange,
  onSearchCommit,
  placeholder,
  tabs,
  activeTab,
  onTabChange,
}: {
  search: string;
  onSearchChange: (v: string) => void;
  onSearchCommit: () => void;
  placeholder: string;
  tabs: { key: string; label: string; count?: number }[];
  activeTab: string;
  onTabChange: (key: string) => void;
}) {
  return (
    <div className="flex flex-col gap-2 sm:flex-row sm:items-center sm:justify-between">
      <div className="relative w-full sm:max-w-xs">
        <Search className="pointer-events-none absolute left-2.5 top-1/2 size-3.5 -translate-y-1/2 text-muted-foreground" />
        <Input
          className="h-8 pl-8 text-sm"
          placeholder={placeholder}
          value={search}
          onChange={(e) => onSearchChange(e.target.value)}
          onKeyDown={(e) => { if (e.key === "Enter") onSearchCommit(); }}
          onBlur={onSearchCommit}
        />
      </div>
      <div className="flex flex-wrap gap-1">
        {tabs.map((tab) => (
          <button
            key={tab.key}
            type="button"
            className={
              "rounded-md px-2.5 py-1 text-xs font-medium transition-colors " +
              (activeTab === tab.key
                ? "bg-primary text-primary-foreground"
                : "bg-muted text-muted-foreground hover:bg-muted/80")
            }
            onClick={() => onTabChange(tab.key)}
          >
            {tab.label}
            {typeof tab.count === "number" ? <span className="ml-1 opacity-70">{tab.count}</span> : null}
          </button>
        ))}
      </div>
    </div>
  );
}

function DebouncedFilterBar({
  initialSearch,
  onSearchChange,
  placeholder,
  tabs,
  activeTab,
  onTabChange,
}: {
  initialSearch: string;
  onSearchChange: (value: string) => void;
  placeholder: string;
  tabs: { key: string; label: string; count?: number }[];
  activeTab: string;
  onTabChange: (key: string) => void;
}) {
  const debounceRef = useRef<ReturnType<typeof setTimeout>>(undefined);
  const [search, setSearch] = useState(initialSearch);
  const [lastInitialSearch, setLastInitialSearch] = useState(initialSearch);

  // 外部搜索词变化时同步到内部状态（render 期调整，避免 effect setState 与 key remount 失焦）。
  if (initialSearch !== lastInitialSearch) {
    setLastInitialSearch(initialSearch);
    setSearch(initialSearch);
  }

  useEffect(() => () => clearTimeout(debounceRef.current), []);

  const handleSearchChange = useCallback((value: string) => {
    setSearch(value);
    clearTimeout(debounceRef.current);
    debounceRef.current = setTimeout(() => onSearchChange(value), 300);
  }, [onSearchChange]);
  const handleSearchCommit = useCallback(() => {
    clearTimeout(debounceRef.current);
    onSearchChange(search);
  }, [onSearchChange, search]);

  return (
    <FilterBar
      search={search}
      onSearchChange={handleSearchChange}
      onSearchCommit={handleSearchCommit}
      placeholder={placeholder}
      tabs={tabs}
      activeTab={activeTab}
      onTabChange={onTabChange}
    />
  );
}

export function FillsTable({ fills }: { fills: RewardFillDto[] }) {
  const pagination = usePagination(fills.length, 15);

  if (fills.length === 0) {
    return <p className="py-6 text-center text-sm text-muted-foreground">{dictionary.rewards.none}</p>;
  }

  return (
    <div>
    <Table className="min-w-[700px]">
      <TableHeader>
        <TableRow>
          <TableHead>{dictionary.rewards.outcome}</TableHead>
          <TableHead>{dictionary.rewards.side}</TableHead>
          <TableHead>{dictionary.rewards.role}</TableHead>
          <TableHead>{dictionary.rewards.price}</TableHead>
          <TableHead>{dictionary.rewards.size}</TableHead>
          <TableHead>{dictionary.rewards.pnl}</TableHead>
          <TableHead>{dictionary.rewards.time}</TableHead>
        </TableRow>
      </TableHeader>
      <TableBody>
        {fills.slice(pagination.start, pagination.end).map((fill) => (
          <TableRow key={fill.id}>
            <TableCell>{fill.outcome}</TableCell>
            <TableCell>
              <StatusPill tone={fill.side === "buy" ? "success" : "warning"}>{fill.side}</StatusPill>
            </TableCell>
            <TableCell className="font-mono text-xs">{fill.role}</TableCell>
            <TableCell className="font-mono">{formatFixed(fill.price, 2)}</TableCell>
            <TableCell className="font-mono">{formatFixed(fill.size, 2)}</TableCell>
            <TableCell className="font-mono">{formatSignedFixed(fill.realized_pnl, 2)}</TableCell>
            <TableCell className="font-mono text-xs text-muted-foreground">
              {formatOptionalClock(fill.created_at)}
            </TableCell>
          </TableRow>
        ))}
      </TableBody>
    </Table>
    <PaginationBar pagination={pagination} totalItems={fills.length} />
    </div>
  );
}

export function PositionsTable({ positions }: { positions: RewardPositionDto[] }) {
  const pagination = usePagination(positions.length, 8);

  if (positions.length === 0) {
    return <p className="py-6 text-center text-sm text-muted-foreground">{dictionary.rewards.none}</p>;
  }

  return (
    <div>
      <Table className="min-w-[720px]">
        <TableHeader>
          <TableRow>
            <TableHead>{dictionary.rewards.market}</TableHead>
            <TableHead>{dictionary.rewards.outcome}</TableHead>
            <TableHead>{dictionary.rewards.size}</TableHead>
            <TableHead>{dictionary.rewards.avgPrice}</TableHead>
            <TableHead>{dictionary.rewards.pnl}</TableHead>
            <TableHead>{dictionary.rewards.time}</TableHead>
          </TableRow>
        </TableHeader>
        <TableBody>
          {positions.slice(pagination.start, pagination.end).map((position) => (
            <TableRow key={`${position.condition_id}:${position.token_id}`}>
              <TableCell className="max-w-[220px] whitespace-normal break-all font-mono text-xs leading-5 text-muted-foreground">
                {position.condition_id}
              </TableCell>
              <TableCell>{position.outcome}</TableCell>
              <TableCell className="font-mono">{formatFixed(position.size, 2)}</TableCell>
              <TableCell className="font-mono">{formatFixed(position.avg_price, 3)}</TableCell>
              <TableCell className="font-mono">{formatSignedFixed(position.realized_pnl, 2)}</TableCell>
              <TableCell className="font-mono text-xs text-muted-foreground">
                {formatOptionalClock(position.updated_at)}
              </TableCell>
            </TableRow>
          ))}
        </TableBody>
      </Table>
      <PaginationBar pagination={pagination} totalItems={positions.length} />
    </div>
  );
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

  const pagination: PaginationState = {
    page: plansPage.page,
    totalPages: plansPage.total_pages,
    start: 0,
    end: plans.length,
    setPage: onPageChange,
    goPrevious: () => onPageChange(Math.max(1, plansPage.page - 1)),
    goNext: () => onPageChange(Math.min(plansPage.total_pages, plansPage.page + 1)),
    reset: () => onPageChange(1),
    hasPrevious: plansPage.page > 1,
    hasNext: plansPage.page < plansPage.total_pages,
  };

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
      <Table className="min-w-[1180px] table-fixed">
        <TableHeader>
          <TableRow>
            <TableHead className="w-[34%]">{dictionary.rewards.market}</TableHead>
            <TableHead>{dictionary.rewards.state}</TableHead>
            <TableHead
              aria-sort={sortBy === "score" ? (sortOrder === "asc" ? "ascending" : "descending") : "none"}
            >
              <button
                type="button"
                onClick={() => handleSort("score")}
                className="inline-flex cursor-pointer select-none items-center"
              >
                {dictionary.rewards.score}
                <SortIndicator active={sortBy === "score"} order={sortOrder} />
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
              <TableRow key={plan.condition_id}>
                <TableCell className="whitespace-normal align-top">
                  <div className="space-y-1">
                    <p className="break-words font-medium leading-snug">{plan.question}</p>
                    <p className="break-words text-xs leading-5 text-muted-foreground">{plan.reason}</p>
                  </div>
                </TableCell>
                <TableCell className="align-top">
                  <StatusPill tone={plan.eligible ? "success" : "warning"}>
                    {plan.eligible ? dictionary.rewards.filterEligible : dictionary.rewards.filterIneligible}
                  </StatusPill>
                </TableCell>
                <TableCell className="align-top">
                  <StatusPill tone={plan.eligible ? "success" : "neutral"}>
                    {formatFixed(plan.score, 1)}
                  </StatusPill>
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
                        <StatusPill tone={infoRiskTone(plan.info_risk.risk_level)}>
                          {plan.info_risk.risk_level}
                        </StatusPill>
                        <span className="font-mono text-muted-foreground">
                          {plan.info_risk.risk_type} · {formatFixed(plan.info_risk.confidence, 2)}
                        </span>
                      </div>
                      <p className="break-words leading-5 text-muted-foreground">
                        {plan.info_risk.summary}
                      </p>
                    </div>
                  )}
                </TableCell>
                <TableCell className="whitespace-normal align-top text-xs">
                  {plan.ai_advisory == null ? (
                    <span className="text-muted-foreground">{dictionary.rewards.none}</span>
                  ) : (
                    <div className="space-y-1">
                      <div className="flex flex-wrap items-center gap-1">
                        <StatusPill tone={aiSuitabilityTone(plan.ai_advisory.suitability)}>
                          {plan.ai_advisory.suitability}
                        </StatusPill>
                        <span className="font-mono text-muted-foreground">
                          {plan.ai_advisory.quote_mode} · {formatFixed(plan.ai_advisory.confidence, 2)}
                        </span>
                      </div>
                      <p className="break-words leading-5 text-muted-foreground">
                        {plan.ai_advisory.reasons[0] ?? dictionary.rewards.none}
                      </p>
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
  orders, search, onSearchChange, status, onStatusChange,
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

  const pagination: PaginationState = {
    page: page.page,
    totalPages: page.total_pages,
    start: 0,
    end: orders.length,
    setPage: onPageChange,
    goPrevious: () => onPageChange(Math.max(1, page.page - 1)),
    goNext: () => onPageChange(Math.min(page.total_pages, page.page + 1)),
    reset: () => onPageChange(1),
    hasPrevious: page.page > 1,
    hasNext: page.page < page.total_pages,
  };

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
      <Table className="min-w-[780px] table-fixed">
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
            <TableHead className="w-[320px]">{dictionary.rewards.reason}</TableHead>
          </TableRow>
        </TableHeader>
        <TableBody>
          {orders.length === 0 ? (
            <TableRow>
              <TableCell colSpan={6} className="py-6 text-center text-sm text-muted-foreground">
                {dictionary.rewards.none}
              </TableCell>
            </TableRow>
          ) : (
            orders.map((order) => (
              <TableRow key={order.id}>
                <TableCell className="align-top">
                  <StatusPill tone={rewardTone(order.status)}>{order.status}</StatusPill>
                </TableCell>
                <TableCell className="align-top">{order.outcome}</TableCell>
                <TableCell className="align-top font-mono">{formatFixed(order.price, 2)}</TableCell>
                <TableCell className="align-top font-mono">{formatFixed(order.size, 2)}</TableCell>
                <TableCell className="align-top">{order.scoring ? dictionary.common.active : dictionary.common.idle}</TableCell>
                <TableCell className="whitespace-normal break-words align-top text-xs leading-5 text-muted-foreground">
                  {order.reason}
                </TableCell>
              </TableRow>
            ))
          )}
        </TableBody>
      </Table>
      <PaginationBar pagination={pagination} totalItems={page.total_items} />
    </div>
  );
}

export function EventsTable({ events }: { events: RewardRiskEventDto[] }) {
  const pagination = usePagination(events.length, 15);

  return (
    <div>
    <Table className="min-w-[760px] table-fixed">
      <TableHeader>
        <TableRow>
          <TableHead>{dictionary.rewards.severity}</TableHead>
          <TableHead>{dictionary.rewards.type}</TableHead>
          <TableHead className="w-[50%]">{dictionary.rewards.message}</TableHead>
          <TableHead>{dictionary.common.published}</TableHead>
        </TableRow>
      </TableHeader>
      <TableBody>
        {events.slice(pagination.start, pagination.end).map((event) => (
          <TableRow key={event.id}>
            <TableCell>
              <StatusPill tone={approvalSeverityTone(event.severity)}>{event.severity}</StatusPill>
            </TableCell>
            <TableCell className="font-mono text-xs">{event.event_type}</TableCell>
            <TableCell className="whitespace-normal break-words leading-5">{event.message}</TableCell>
            <TableCell className="font-mono text-xs text-muted-foreground">{formatOptionalClock(event.created_at)}</TableCell>
          </TableRow>
        ))}
      </TableBody>
    </Table>
    <PaginationBar pagination={pagination} totalItems={events.length} />
    </div>
  );
}
