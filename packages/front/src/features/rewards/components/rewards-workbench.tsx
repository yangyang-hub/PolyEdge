"use client";

import { startTransition, useMemo, useState } from "react";
import { Activity, CircleDollarSign, ShieldCheck } from "lucide-react";

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

type OrderStatusFilter = "all" | "open" | "filled" | "cancelled" | "exit_pending";
type PlansEligibilityFilter = "all" | "eligible" | "ineligible";
type SortOrder = "asc" | "desc";

export function RewardsWorkbench({ initialSnapshot }: { initialSnapshot: RewardBotSnapshotDto }) {
  const [snapshot, setSnapshot] = useState(initialSnapshot);
  const [draft, setDraft] = useState<RewardBotConfigDto>(initialSnapshot.config);
  const [feedback, setFeedback] = useState<RewardBotActionResult | null>(null);
  const [pending, setPending] = useState(false);
  const [filtering, setFiltering] = useState(false);

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

  function buildQuery(
    overrides: {
      search?: string;
      status?: OrderStatusFilter;
      sortBy?: string;
      sortOrder?: SortOrder;
      page?: number;
    } = {},
  ): RewardBotSnapshotQuery {
    const search = overrides.search ?? ordersSearch;
    const status = overrides.status ?? ordersStatus;
    const q: RewardBotSnapshotQuery = {};
    // Plans pagination
    if (plansSearch.trim()) q.plans_search = plansSearch.trim();
    if (plansEligible === "eligible") q.plans_eligible = true;
    else if (plansEligible === "ineligible") q.plans_eligible = false;
    q.plans_sort_by = plansSortBy;
    q.plans_sort_order = plansSortOrder;
    q.plans_page = plansPage;
    q.plans_page_size = REWARD_PLANS_PAGE_SIZE;
    // Orders pagination
    if (search.trim()) q.orders_search = search.trim();
    if (status !== "all") q.orders_status = status;
    q.orders_sort_by = overrides.sortBy ?? ordersSortBy;
    q.orders_sort_order = overrides.sortOrder ?? ordersSortOrder;
    q.orders_page = overrides.page ?? ordersPage;
    q.orders_page_size = REWARD_ORDERS_PAGE_SIZE;
    return q;
  }

  function refetchWithFilters(overrides?: Parameters<typeof buildQuery>[0]) {
    const requestedOrdersPage = overrides?.page ?? ordersPage;
    setFiltering(true);
    void readRewardBotSnapshot(buildQuery(overrides))
      .then((response) => {
        setSnapshot(response.data);
        setOrdersPage(response.data.orders_page?.page ?? requestedOrdersPage);
        setPlansPage(response.data.plans_page?.page ?? plansPage);
      })
      .finally(() => setFiltering(false));
  }

  function applyResult(result: RewardBotActionResult) {
    setFeedback(result);
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
      [key]: Number.isFinite(nextValue) ? nextValue : 0,
    }));
  }

  return (
    <div className="space-y-5">
      <PageHeader
        eyebrow={dictionary.rewards.eyebrow}
        title={dictionary.rewards.title}
        description={dictionary.rewards.description}
      />

      {feedback ? <OperationFeedbackBanner feedback={feedback} /> : null}

      <section className="grid gap-4 xl:grid-cols-[1.15fr_0.85fr]">
        <ModeStatusPanel snapshot={snapshot} eventCounts={eventCounts} />
        <CommandPanel
          config={draft}
          pending={pending}
          onRun={() => runAction(runRewardBotOnceAction)}
          onCancel={() => runAction(cancelRewardBotOrdersAction)}
          onReset={() => runAction(resetRewardBotAction)}
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
          <div className="grid gap-4 2xl:grid-cols-[1.25fr_0.75fr]">
            <Card>
              <CardHeader className="border-b border-border/70">
                <CardTitle>{dictionary.rewards.quotePlans}</CardTitle>
                <CardDescription>{dictionary.rewards.quotePlansDescription}</CardDescription>
              </CardHeader>
              <CardContent>
                <QuotePlansTable
                  plans={snapshot.quote_plans}
                  plansPage={snapshot.plans_page}
                  search={plansSearch}
                  onSearchChange={(v) => {
                    setPlansSearch(v);
                    setPlansPage(1);
                    refetchWithFilters();
                  }}
                  eligibility={plansEligible}
                  onEligibilityChange={(v) => {
                    setPlansEligible(v);
                    setPlansPage(1);
                    refetchWithFilters();
                  }}
                  sortBy={plansSortBy}
                  sortOrder={plansSortOrder}
                  onSortChange={(by, order) => {
                    setPlansSortBy(by);
                    setPlansSortOrder(order);
                    refetchWithFilters();
                  }}
                  onPageChange={(p) => {
                    setPlansPage(p);
                    refetchWithFilters();
                  }}
                  filtering={filtering}
                />
              </CardContent>
            </Card>

            <div className="space-y-4">
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
    </div>
  );
}
