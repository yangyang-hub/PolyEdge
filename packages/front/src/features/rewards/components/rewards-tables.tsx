"use client";

import { StatusPill } from "@/components/shared/status-pill";
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
import { useI18n } from "@/lib/i18n/client";

import { rewardTone } from "../lib/rewards-helpers";

export function FillsTable({ fills }: { fills: RewardFillDto[] }) {
  const { dictionary } = useI18n();

  if (fills.length === 0) {
    return <p className="py-6 text-center text-sm text-muted-foreground">{dictionary.rewards.none}</p>;
  }

  return (
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
        {fills.map((fill) => (
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
  );
}

export function QuotePlansTable({ plans }: { plans: RewardQuotePlanDto[] }) {
  const { dictionary } = useI18n();

  return (
    <Table>
      <TableHeader>
        <TableRow>
          <TableHead>{dictionary.rewards.market}</TableHead>
          <TableHead>{dictionary.rewards.score}</TableHead>
          <TableHead>{dictionary.rewards.dailyReward}</TableHead>
          <TableHead>{dictionary.rewards.midpoint}</TableHead>
          <TableHead>{dictionary.rewards.quotes}</TableHead>
        </TableRow>
      </TableHeader>
      <TableBody>
        {plans.map((plan) => (
          <TableRow key={plan.condition_id}>
            <TableCell className="max-w-[360px]">
              <div className="space-y-1">
                <p className="truncate font-medium">{plan.question}</p>
                <p className="text-xs text-muted-foreground">{plan.reason}</p>
              </div>
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
        ))}
      </TableBody>
    </Table>
  );
}

export function OrdersTable({ orders }: { orders: ManagedRewardOrderDto[] }) {
  const { dictionary } = useI18n();

  return (
    <Table>
      <TableHeader>
        <TableRow>
          <TableHead>{dictionary.rewards.state}</TableHead>
          <TableHead>{dictionary.rewards.outcome}</TableHead>
          <TableHead>{dictionary.rewards.price}</TableHead>
          <TableHead>{dictionary.rewards.size}</TableHead>
          <TableHead>{dictionary.rewards.scoring}</TableHead>
        </TableRow>
      </TableHeader>
      <TableBody>
        {orders.map((order) => (
          <TableRow key={order.id}>
            <TableCell>
              <StatusPill tone={rewardTone(order.status)}>{order.status}</StatusPill>
            </TableCell>
            <TableCell>{order.outcome}</TableCell>
            <TableCell className="font-mono">{formatFixed(order.price, 2)}</TableCell>
            <TableCell className="font-mono">{formatFixed(order.size, 2)}</TableCell>
            <TableCell>{order.scoring ? dictionary.common.active : dictionary.common.idle}</TableCell>
          </TableRow>
        ))}
      </TableBody>
    </Table>
  );
}

export function EventsTable({ events }: { events: RewardRiskEventDto[] }) {
  const { dictionary } = useI18n();

  return (
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
        {events.map((event) => (
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
  );
}
