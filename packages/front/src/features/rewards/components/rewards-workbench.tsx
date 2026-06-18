"use client";

import { startTransition, useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Activity, CircleDollarSign, ShieldCheck } from "lucide-react";
import { toast } from "sonner";

import { ActionDialog } from "@/components/shared/action-dialog";
import { OperationFeedbackBanner } from "@/components/shared/operation-feedback-banner";
import { PageHeader } from "@/components/shared/page-header";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import type { RewardBotConfigDto, RewardBotSnapshotDto } from "@/lib/contracts/dto";
import { dictionary } from "@/lib/i18n/dictionaries";
import {
  cancelRewardBotOrdersAction,
  resetRewardBotAction,
  runRewardBotOnceAction,
  updateRewardBotConfigAction,
  type RewardBotActionResult,
} from "@/lib/api/actions";
import { readRewardBotSnapshot, type RewardBotSnapshotQuery } from "@/lib/api/rewards";

import type { NumberConfigKey } from "../types";
import { EventsPanel } from "./rewards-events-panel";
import { RewardsConfigPanel } from "./rewards-config-panel";
import {
  CommandPanel,
  ModeStatusPanel,
  SummaryStrip,
  countRewardEvents,
} from "./rewards-overview-cards";
import { RiskControlConfig } from "./rewards-risk-config";
import { OrdersTable, PositionsTable, QuotePlansTable } from "./rewards-tables";

const REWARD_ORDERS_PAGE_SIZE = 15;
const REWARD_PLANS_PAGE_SIZE = 15;
const REWARD_SNAPSHOT_REFRESH_MS = 10_000;

type OrderStatusFilter = "all" | "open" | "filled" | "cancelled" | "exit_pending";
type PlansEligibilityFilter = "all" | "eligible" | "ineligible";
type SortOrder = "asc" | "desc";
type ConfirmKind = "cancel" | "reset";

export function RewardsWorkbench({ initialSnapshot }: { initialSnapshot: RewardBotSnapshotDto }) {
  const [snapshot, setSnapshot] = useState(initialSnapshot);
  const [draft, setDraft] = useState<RewardBotConfigDto>(initialSnapshot.config);
  const [feedback, setFeedback] = useState<RewardBotActionResult | null>(null);
  const [pending, setPending] = useState(false);
  const [filtering, setFiltering] = useState(false);
  const refetchSequence = useRef(0);
  const [confirm, setConfirm] = useState<{ kind: ConfirmKind; note: string } | null>(null);

  const [plansSearch, setPlansSearch] = useState("");
  const [plansEligible, setPlansEligible] = useState<PlansEligibilityFilter>("eligible");
  const [plansSortBy, setPlansSortBy] = useState("score");
  const [plansSortOrder, setPlansSortOrder] = useState<SortOrder>("desc");
  const [plansPage, setPlansPage] = useState(initialSnapshot.plans_page?.page ?? 1);

  const [ordersSearch, setOrdersSearch] = useState("");
  const [ordersStatus, setOrdersStatus] = useState<OrderStatusFilter>("all");
  const [ordersSortBy, setOrdersSortBy] = useState("status");
  const [ordersSortOrder, setOrdersSortOrder] = useState<SortOrder>("desc");
  const [ordersPage, setOrdersPage] = useState(initialSnapshot.orders_page?.page ?? 1);

  const eventCounts = useMemo(() => countRewardEvents(snapshot), [snapshot]);

  const isDirty = JSON.stringify(draft) !== JSON.stringify(snapshot.config);

  const buildQuery = useCallback(
    (
      overrides: {
        search?: string;
        status?: OrderStatusFilter;
        sortBy?: string;
        sortOrder?: SortOrder;
        page?: number;
        plansSearch?: string;
        plansEligible?: PlansEligibilityFilter;
        plansSortBy?: string;
        plansSortOrder?: SortOrder;
        plansPage?: number;
      } = {},
    ): RewardBotSnapshotQuery => {
      const search = overrides.search ?? ordersSearch;
      const status = overrides.status ?? ordersStatus;
      const q: RewardBotSnapshotQuery = {};
      const resolvedPlansSearch = overrides.plansSearch ?? plansSearch;
      const resolvedPlansEligible = overrides.plansEligible ?? plansEligible;
      const resolvedPlansSortBy = overrides.plansSortBy ?? plansSortBy;
      const resolvedPlansSortOrder = overrides.plansSortOrder ?? plansSortOrder;
      const resolvedPlansPage = overrides.plansPage ?? plansPage;
      if (resolvedPlansSearch.trim()) q.plans_search = resolvedPlansSearch.trim();
      if (resolvedPlansEligible === "eligible") q.plans_eligible = true;
      else if (resolvedPlansEligible === "ineligible") q.plans_eligible = false;
      q.plans_sort_by = resolvedPlansSortBy;
      q.plans_sort_order = resolvedPlansSortOrder;
      q.plans_page = resolvedPlansPage;
      q.plans_page_size = REWARD_PLANS_PAGE_SIZE;
      if (search.trim()) q.orders_search = search.trim();
      if (status !== "all") q.orders_status = status;
      q.orders_sort_by = overrides.sortBy ?? ordersSortBy;
      q.orders_sort_order = overrides.sortOrder ?? ordersSortOrder;
      q.orders_page = overrides.page ?? ordersPage;
      q.orders_page_size = REWARD_ORDERS_PAGE_SIZE;
      return q;
    },
    [
      ordersPage,
      ordersSearch,
      ordersSortBy,
      ordersSortOrder,
      ordersStatus,
      plansEligible,
      plansPage,
      plansSearch,
      plansSortBy,
      plansSortOrder,
    ],
  );

  const refetchWithFilters = useCallback(
    (
      overrides?: Parameters<typeof buildQuery>[0],
      options: { silent?: boolean } = {},
    ) => {
      const sequence = refetchSequence.current + 1;
      refetchSequence.current = sequence;
      const requestedOrdersPage = overrides?.page ?? ordersPage;
      const requestedPlansPage = overrides?.plansPage ?? plansPage;
      if (!options.silent) setFiltering(true);
      void readRewardBotSnapshot(buildQuery(overrides))
        .then((response) => {
          if (sequence !== refetchSequence.current) return;
          setSnapshot(response.data);
          setOrdersPage(response.data.orders_page?.page ?? requestedOrdersPage);
          setPlansPage(response.data.plans_page?.page ?? requestedPlansPage);
        })
        .catch((error: unknown) => {
          if (sequence !== refetchSequence.current) return;
          setFeedback({
            ok: false,
            message:
              error instanceof Error
                ? error.message
                : dictionary.rewards.snapshotRefreshFailed,
          });
        })
        .finally(() => {
          if (sequence === refetchSequence.current && !options.silent) {
            setFiltering(false);
          }
        });
    },
    [buildQuery, ordersPage, plansPage],
  );

  useEffect(() => {
    const tick = () => {
      if (document.hidden) return;
      refetchWithFilters(undefined, { silent: true });
    };
    const interval = window.setInterval(tick, REWARD_SNAPSHOT_REFRESH_MS);
    const handleVisibilityChange = () => {
      if (!document.hidden) refetchWithFilters(undefined, { silent: true });
    };
    document.addEventListener("visibilitychange", handleVisibilityChange);
    return () => {
      window.clearInterval(interval);
      document.removeEventListener("visibilitychange", handleVisibilityChange);
    };
  }, [refetchWithFilters]);

  function applyResult(result: RewardBotActionResult) {
    setFeedback(result);
    if (result.ok) {
      toast.success(result.message);
    } else {
      toast.error(result.message);
    }
    if (result.snapshot) {
      setSnapshot(result.snapshot);
      setDraft(result.snapshot.config);
      refetchWithFilters();
    }
  }

  function runAction(action: () => Promise<RewardBotActionResult>) {
    setPending(true);
    startTransition(() => {
      void action()
        .then(applyResult)
        .finally(() => setPending(false));
    });
  }

  function updateNumber(key: NumberConfigKey, value: string) {
    const nextValue = Number(value);
    setDraft((current) => ({
      ...current,
      [key]: Number.isFinite(nextValue) ? Math.max(0, nextValue) : 0,
    }));
  }

  return (
    <div className="space-y-5">
      <PageHeader
        eyebrow={dictionary.rewards.eyebrow}
        title={dictionary.rewards.title}
        description={dictionary.rewards.description}
      />

      {feedback ? <OperationFeedbackBanner feedback={feedback} onDismiss={() => setFeedback(null)} /> : null}

      <section className="grid items-start gap-4 xl:grid-cols-[1.15fr_0.85fr]">
        <ModeStatusPanel snapshot={snapshot} eventCounts={eventCounts} />
        <CommandPanel
          pending={pending}
          isDirty={isDirty}
          onRun={() => runAction(runRewardBotOnceAction)}
          onCancel={() => setConfirm({ kind: "cancel", note: "" })}
          onReset={() => setConfirm({ kind: "reset", note: "" })}
          onSave={() => runAction(() => updateRewardBotConfigAction(draft))}
        />
      </section>

      <SummaryStrip snapshot={snapshot} eventCounts={eventCounts} />

      <Tabs defaultValue="activity" className="gap-4">
        <TabsList className="h-auto w-full flex-wrap justify-start">
          <TabsTrigger value="activity">
            <Activity className="size-4" />
            {dictionary.rewards.activityTab}
          </TabsTrigger>
          <TabsTrigger value="config">
            <CircleDollarSign className="size-4" />
            {dictionary.rewards.strategyTab}
          </TabsTrigger>
          <TabsTrigger value="risk">
            <ShieldCheck className="size-4" />
            {dictionary.rewards.riskTab}
          </TabsTrigger>
        </TabsList>

        <TabsContent value="activity" className="space-y-4">
          <Card>
            <CardHeader className="border-b border-border/70">
              <CardTitle>{dictionary.rewards.quotePlans}</CardTitle>
              <CardDescription>{dictionary.rewards.quotePlansDescription}</CardDescription>
            </CardHeader>
            <CardContent>
              <QuotePlansTable
                plans={snapshot.quote_plans}
                plansPage={snapshot.plans_page}
                plansTotal={snapshot.status.plans_total}
                eligibleTotal={snapshot.status.eligible_markets}
                search={plansSearch}
                onSearchChange={(v) => {
                  setPlansSearch(v);
                  setPlansPage(1);
                  refetchWithFilters({ plansSearch: v, plansPage: 1 });
                }}
                eligibility={plansEligible}
                onEligibilityChange={(v) => {
                  setPlansEligible(v);
                  setPlansPage(1);
                  refetchWithFilters({ plansEligible: v, plansPage: 1 });
                }}
                sortBy={plansSortBy}
                sortOrder={plansSortOrder}
                onSortChange={(by, order) => {
                  setPlansSortBy(by);
                  setPlansSortOrder(order);
                  refetchWithFilters({ plansSortBy: by, plansSortOrder: order });
                }}
                onPageChange={(p) => {
                  setPlansPage(p);
                  refetchWithFilters({ plansPage: p });
                }}
                filtering={filtering}
              />
            </CardContent>
          </Card>

          <div className="grid items-start gap-4 xl:grid-cols-[minmax(0,1fr)_minmax(340px,0.65fr)]">
            <Card>
              <CardHeader className="border-b border-border/70">
                <CardTitle>{dictionary.rewards.managedOrders}</CardTitle>
                <CardDescription>{dictionary.rewards.managedOrdersDescription}</CardDescription>
              </CardHeader>
              <CardContent>
                <OrdersTable
                  orders={snapshot.orders}
                  search={ordersSearch}
                  onSearchChange={(v) => {
                    setOrdersSearch(v);
                    setOrdersPage(1);
                    refetchWithFilters({ search: v, page: 1 });
                  }}
                  status={ordersStatus}
                  onStatusChange={(v) => {
                    setOrdersStatus(v);
                    setOrdersPage(1);
                    refetchWithFilters({ status: v, page: 1 });
                  }}
                  sortBy={ordersSortBy}
                  sortOrder={ordersSortOrder}
                  onSortChange={(by, order) => {
                    setOrdersSortBy(by);
                    setOrdersSortOrder(order);
                    setOrdersPage(1);
                    refetchWithFilters({ sortBy: by, sortOrder: order, page: 1 });
                  }}
                  page={snapshot.orders_page}
                  onPageChange={(page) => {
                    setOrdersPage(page);
                    refetchWithFilters({ page });
                  }}
                  filtering={filtering}
                />
              </CardContent>
            </Card>

            <Card>
              <CardHeader className="border-b border-border/70">
                <CardTitle>{dictionary.rewards.positions}</CardTitle>
                <CardDescription>{dictionary.rewards.positionsDescription}</CardDescription>
              </CardHeader>
              <CardContent>
                <PositionsTable positions={snapshot.positions} />
              </CardContent>
            </Card>
          </div>

          <Card>
            <CardHeader className="border-b border-border/70">
              <CardTitle>{dictionary.rewards.riskEvents}</CardTitle>
              <CardDescription>{dictionary.rewards.eventsDescription}</CardDescription>
            </CardHeader>
            <CardContent>
              <EventsPanel events={snapshot.events} fills={snapshot.fills} />
            </CardContent>
          </Card>
        </TabsContent>

        <TabsContent value="config">
          <RewardsConfigPanel draft={draft} setDraft={setDraft} updateNumber={updateNumber} />
        </TabsContent>

        <TabsContent value="risk">
          <RiskControlConfig draft={draft} updateNumber={updateNumber} />
        </TabsContent>
      </Tabs>

      {confirm ? (
        <ActionDialog
          open
          onOpenChange={(open) => {
            if (!open) setConfirm(null);
          }}
          title={
            confirm.kind === "cancel"
              ? dictionary.rewards.cancelConfirmTitle
              : dictionary.rewards.resetConfirmTitle
          }
          description={
            confirm.kind === "cancel"
              ? dictionary.rewards.cancelConfirmDescription
              : dictionary.rewards.resetConfirmDescription
          }
          confirmLabel={dictionary.rewards.confirmAction}
          confirmVariant={confirm.kind === "cancel" ? "destructive" : "default"}
          isPending={pending}
          note={confirm.note}
          onNoteChange={(value) =>
            setConfirm((current) => (current ? { ...current, note: value } : current))
          }
          requiresStepUp={false}
          stepUpCode=""
          onStepUpCodeChange={() => {}}
          feedback={null}
          onSubmit={() => {
            const kind = confirm.kind;
            setConfirm(null);
            if (kind === "cancel") {
              runAction(cancelRewardBotOrdersAction);
            } else {
              runAction(resetRewardBotAction);
            }
          }}
        />
      ) : null}
    </div>
  );
}
