"use client";

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
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
import { useI18n } from "@/lib/i18n/client";

import { rewardTone } from "../lib/rewards-helpers";

function SortIndicator({ active, order }: { active: boolean; order: "asc" | "desc" }) {
  if (!active) return null;
  return order === "asc" ? <ArrowUp className="ml-1 inline size-3" /> : <ArrowDown className="ml-1 inline size-3" />;
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
  tabs: { key: string; label: string; count: number }[];
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
      <div className="flex gap-1">
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
            <span className="ml-1 opacity-70">{tab.count}</span>
          </button>
        ))}
      </div>
    </div>
  );
}

export function FillsTable({ fills }: { fills: RewardFillDto[] }) {
  const { dictionary } = useI18n();
  const pagination = usePagination(fills.length, 15);

  if (fills.length === 0) {
    return <p className="py-6 text-center text-sm text-muted-foreground">{dictionary.rewards.none}</p>;
  }

  return (
    <div>
    <Table>
      <TableHeader>
        <TableRow>
          <TableHead>{dictionary.rewards.outcome}</TableHead>
          <TableHead>{dictionary.rewards.state}</TableHead>
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

interface QuotePlansTableProps {
  plans: RewardQuotePlanDto[];
  search: string;
  onSearchChange: (v: string) => void;
  eligibility: "all" | "eligible" | "ineligible";
  onEligibilityChange: (v: "all" | "eligible" | "ineligible") => void;
  sortBy: string;
  sortOrder: "asc" | "desc";
  onSortChange: (by: string, order: "asc" | "desc") => void;
  filtering?: boolean;
}

export function QuotePlansTable({
  plans, search, onSearchChange, eligibility, onEligibilityChange,
  sortBy, sortOrder, onSortChange, filtering,
}: QuotePlansTableProps) {
  const { dictionary } = useI18n();
  const debounceRef = useRef<ReturnType<typeof setTimeout>>(undefined);
  const [localSearch, setLocalSearch] = useState(search);

  useEffect(() => { setLocalSearch(search); }, [search]);

  const handleSearchChange = useCallback((v: string) => {
    setLocalSearch(v);
    clearTimeout(debounceRef.current);
    debounceRef.current = setTimeout(() => onSearchChange(v), 300);
  }, [onSearchChange]);

  const searchedPlans = useMemo(() => {
    const query = localSearch.trim().toLowerCase();
    if (!query) {
      return plans;
    }
    return plans.filter((plan) =>
      plan.question.toLowerCase().includes(query)
        || plan.market_slug.toLowerCase().includes(query)
        || plan.reason.toLowerCase().includes(query)
    );
  }, [plans, localSearch]);

  const visiblePlans = useMemo(() => {
    const next = searchedPlans.filter((plan) => {
      if (eligibility === "eligible") {
        return plan.eligible;
      }
      if (eligibility === "ineligible") {
        return !plan.eligible;
      }
      return true;
    });
    next.sort((a, b) => {
      const ord = (() => {
        if (sortBy === "daily_reward") {
          return Number(a.total_daily_rate) - Number(b.total_daily_rate);
        }
        if (sortBy === "midpoint") {
          if (a.midpoint == null && b.midpoint == null) return 0;
          if (a.midpoint == null) return -1;
          if (b.midpoint == null) return 1;
          return Number(a.midpoint) - Number(b.midpoint);
        }
        return Number(a.score) - Number(b.score);
      })();
      return sortOrder === "asc" ? ord : -ord;
    });
    return next;
  }, [searchedPlans, eligibility, sortBy, sortOrder]);

  const tabs = [
    { key: "all", label: dictionary.rewards.filterAll, count: searchedPlans.length },
    { key: "eligible", label: dictionary.rewards.filterEligible, count: searchedPlans.filter((p) => p.eligible).length },
    { key: "ineligible", label: dictionary.rewards.filterIneligible, count: searchedPlans.filter((p) => !p.eligible).length },
  ];

  function handleSort(field: string) {
    if (sortBy === field) {
      onSortChange(field, sortOrder === "asc" ? "desc" : "asc");
    } else {
      onSortChange(field, "desc");
    }
  }

  const pagination = usePagination(visiblePlans.length, 15);

  return (
    <div className="space-y-3">
      <FilterBar
        search={localSearch}
        onSearchChange={handleSearchChange}
        onSearchCommit={() => onSearchChange(localSearch)}
        placeholder={dictionary.rewards.searchPlaceholder}
        tabs={tabs}
        activeTab={eligibility}
        onTabChange={(key) => onEligibilityChange(key as typeof eligibility)}
      />
      {filtering && <p className="text-xs text-muted-foreground">…</p>}
      <Table>
        <TableHeader>
          <TableRow>
            <TableHead>{dictionary.rewards.market}</TableHead>
            <TableHead>{dictionary.rewards.state}</TableHead>
            <TableHead className="cursor-pointer select-none" onClick={() => handleSort("score")}>
              {dictionary.rewards.score}
              <SortIndicator active={sortBy === "score"} order={sortOrder} />
            </TableHead>
            <TableHead className="cursor-pointer select-none" onClick={() => handleSort("daily_reward")}>
              {dictionary.rewards.dailyReward}
              <SortIndicator active={sortBy === "daily_reward"} order={sortOrder} />
            </TableHead>
            <TableHead className="cursor-pointer select-none" onClick={() => handleSort("midpoint")}>
              {dictionary.rewards.midpoint}
              <SortIndicator active={sortBy === "midpoint"} order={sortOrder} />
            </TableHead>
            <TableHead>{dictionary.rewards.quotes}</TableHead>
          </TableRow>
        </TableHeader>
        <TableBody>
          {visiblePlans.length === 0 ? (
            <TableRow>
              <TableCell colSpan={6} className="py-6 text-center text-sm text-muted-foreground">
                {dictionary.rewards.none}
              </TableCell>
            </TableRow>
          ) : (
            visiblePlans.slice(pagination.start, pagination.end).map((plan) => (
              <TableRow key={plan.condition_id}>
                <TableCell className="max-w-[360px]">
                  <div className="space-y-1">
                    <p className="truncate font-medium">{plan.question}</p>
                    <p className="text-xs text-muted-foreground">{plan.reason}</p>
                  </div>
                </TableCell>
                <TableCell>
                  <StatusPill tone={plan.eligible ? "success" : "warning"}>
                    {plan.eligible ? dictionary.rewards.filterEligible : dictionary.rewards.filterIneligible}
                  </StatusPill>
                </TableCell>
                <TableCell>
                  <StatusPill tone={plan.eligible ? "success" : "neutral"}>
                    {formatFixed(plan.score, 1)}
                  </StatusPill>
                </TableCell>
                <TableCell className="font-mono">{formatUsdFixed(plan.total_daily_rate)}</TableCell>
                <TableCell className="font-mono">{plan.midpoint == null ? "n/a" : formatFixed(plan.midpoint, 3)}</TableCell>
                <TableCell className="font-mono text-xs">
                  {plan.legs.length === 0
                    ? dictionary.rewards.none
                    : plan.legs.map((leg) => `${leg.outcome} ${formatFixed(leg.size, 2)}@${formatFixed(leg.price, 2)}`).join(" / ")}
                </TableCell>
              </TableRow>
            ))
          )}
        </TableBody>
      </Table>
      <PaginationBar pagination={pagination} totalItems={visiblePlans.length} />
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
  filtering?: boolean;
}

export function OrdersTable({
  orders, search, onSearchChange, status, onStatusChange,
  sortBy, sortOrder, onSortChange, filtering,
}: OrdersTableProps) {
  const { dictionary } = useI18n();
  const debounceRef = useRef<ReturnType<typeof setTimeout>>(undefined);
  const [localSearch, setLocalSearch] = useState(search);

  useEffect(() => { setLocalSearch(search); }, [search]);

  const handleSearchChange = useCallback((v: string) => {
    setLocalSearch(v);
    clearTimeout(debounceRef.current);
    debounceRef.current = setTimeout(() => onSearchChange(v), 300);
  }, [onSearchChange]);

  const tabs = [
    { key: "all", label: dictionary.rewards.filterAll, count: orders.length },
    { key: "open", label: dictionary.rewards.filterOpen, count: orders.filter((o) => o.status === "open" || o.status === "planned").length },
    { key: "filled", label: dictionary.rewards.filterFilled, count: orders.filter((o) => o.status === "filled").length },
    { key: "cancelled", label: dictionary.rewards.filterCancelled, count: orders.filter((o) => o.status === "cancelled").length },
    { key: "exit_pending", label: dictionary.rewards.filterExit, count: orders.filter((o) => o.status === "exit_pending").length },
  ];

  function handleSort(field: string) {
    if (sortBy === field) {
      onSortChange(field, sortOrder === "asc" ? "desc" : "asc");
    } else {
      onSortChange(field, "desc");
    }
  }

  const pagination = usePagination(orders.length, 15);

  return (
    <div className="space-y-3">
      <FilterBar
        search={localSearch}
        onSearchChange={handleSearchChange}
        onSearchCommit={() => onSearchChange(localSearch)}
        placeholder={dictionary.rewards.searchOrdersPlaceholder}
        tabs={tabs}
        activeTab={status}
        onTabChange={(key) => onStatusChange(key as typeof status)}
      />
      {filtering && <p className="text-xs text-muted-foreground">…</p>}
      <Table>
        <TableHeader>
          <TableRow>
            <TableHead>{dictionary.rewards.state}</TableHead>
            <TableHead>{dictionary.rewards.outcome}</TableHead>
            <TableHead className="cursor-pointer select-none" onClick={() => handleSort("price")}>
              {dictionary.rewards.price}
              <SortIndicator active={sortBy === "price"} order={sortOrder} />
            </TableHead>
            <TableHead className="cursor-pointer select-none" onClick={() => handleSort("size")}>
              {dictionary.rewards.size}
              <SortIndicator active={sortBy === "size"} order={sortOrder} />
            </TableHead>
            <TableHead>{dictionary.rewards.scoring}</TableHead>
          </TableRow>
        </TableHeader>
        <TableBody>
          {orders.length === 0 ? (
            <TableRow>
              <TableCell colSpan={5} className="py-6 text-center text-sm text-muted-foreground">
                {dictionary.rewards.none}
              </TableCell>
            </TableRow>
          ) : (
            orders.slice(pagination.start, pagination.end).map((order) => (
              <TableRow key={order.id}>
                <TableCell>
                  <StatusPill tone={rewardTone(order.status)}>{order.status}</StatusPill>
                </TableCell>
                <TableCell>{order.outcome}</TableCell>
                <TableCell className="font-mono">{formatFixed(order.price, 2)}</TableCell>
                <TableCell className="font-mono">{formatFixed(order.size, 2)}</TableCell>
                <TableCell>{order.scoring ? dictionary.common.active : dictionary.common.idle}</TableCell>
              </TableRow>
            ))
          )}
        </TableBody>
      </Table>
      <PaginationBar pagination={pagination} totalItems={orders.length} />
    </div>
  );
}

export function EventsTable({ events }: { events: RewardRiskEventDto[] }) {
  const { dictionary } = useI18n();
  const pagination = usePagination(events.length, 15);

  return (
    <div>
    <Table>
      <TableHeader>
        <TableRow>
          <TableHead>{dictionary.rewards.severity}</TableHead>
          <TableHead>{dictionary.rewards.type}</TableHead>
          <TableHead>{dictionary.rewards.message}</TableHead>
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
            <TableCell className="max-w-[520px] truncate">{event.message}</TableCell>
            <TableCell className="font-mono text-xs text-muted-foreground">{formatOptionalClock(event.created_at)}</TableCell>
          </TableRow>
        ))}
      </TableBody>
    </Table>
    <PaginationBar pagination={pagination} totalItems={events.length} />
    </div>
  );
}
