import type { ApiResponse } from "@/lib/contracts/api";
import type {
  RewardBotConfigPatchDto,
  RewardBotSnapshotDto,
  RewardOrderTransitionPageDto,
  RewardStrategyActionDto,
  RewardStrategyActionPageDto,
  RewardStrategyDecisionDto,
  RewardStrategyDecisionPageDto,
  RewardStrategyRunDto,
  RewardStrategyRunPageDto,
} from "@/lib/contracts/dto";
import {
  buildQueryString,
  fetchContract,
  fetchWriteContract,
  randomUUID,
  type InternalApiStepUpScope,
} from "@/lib/api/base";

export interface RewardWriteOptions {
  operatorNote?: string;
  stepUpCode?: string;
  stepUpScopes?: InternalApiStepUpScope[];
}

export interface RewardBotSnapshotQuery {
  plans_search?: string;
  plans_eligible?: boolean;
  plans_sort_by?: string;
  plans_sort_order?: string;
  plans_page?: number;
  plans_page_size?: number;
  orders_search?: string;
  orders_status?: string;
  orders_sort_by?: string;
  orders_sort_order?: string;
  orders_page?: number;
  orders_page_size?: number;
}

export interface RewardStrategyRunsQuery {
  account_id?: string;
  status?: string;
  page?: number;
  page_size?: number;
}

export interface RewardStrategyDecisionsQuery {
  search?: string;
  eligible?: boolean;
  page?: number;
  page_size?: number;
}

export interface RewardStrategyActionsQuery {
  status?: string;
  action_type?: string;
  page?: number;
  page_size?: number;
}

export interface RewardOrderTransitionsQuery {
  page?: number;
  page_size?: number;
}

export async function readRewardBotSnapshot(
  query?: RewardBotSnapshotQuery,
): Promise<ApiResponse<RewardBotSnapshotDto>> {
  return fetchContract<ApiResponse<RewardBotSnapshotDto>>(
    `/api/v1/rewards-bot${buildQueryString(query as Record<string, string | number | boolean | undefined>)}`,
  );
}

export async function updateRewardBotConfig(
  patch: RewardBotConfigPatchDto,
  options: RewardWriteOptions = {},
): Promise<ApiResponse<RewardBotSnapshotDto>> {
  return fetchWriteContract<ApiResponse<RewardBotSnapshotDto>>("/api/v1/rewards-bot/config", {
    method: "POST",
    idempotencyKey: `reward-config-${randomUUID()}`,
    body: {
      ...(patch as Record<string, unknown>),
      ...(options.operatorNote ? { operator_note: options.operatorNote } : {}),
    },
    stepUpCode: options.stepUpCode,
    stepUpScopes: options.stepUpScopes,
  });
}

export async function runRewardBotOnce(
  options: RewardWriteOptions = {},
): Promise<ApiResponse<RewardBotSnapshotDto>> {
  return fetchWriteContract<ApiResponse<RewardBotSnapshotDto>>("/api/v1/rewards-bot/run", {
    method: "POST",
    idempotencyKey: `reward-run-${randomUUID()}`,
    body: options.operatorNote ? { operator_note: options.operatorNote } : {},
    stepUpCode: options.stepUpCode,
    stepUpScopes: ["rewards_run_once"],
  });
}

export async function cancelRewardBotOrders(
  options: RewardWriteOptions = {},
): Promise<ApiResponse<RewardBotSnapshotDto>> {
  return fetchWriteContract<ApiResponse<RewardBotSnapshotDto>>("/api/v1/rewards-bot/cancel-all", {
    method: "POST",
    idempotencyKey: `reward-cancel-${randomUUID()}`,
    body: options.operatorNote ? { operator_note: options.operatorNote } : {},
  });
}

export async function resetRewardBot(
  options: RewardWriteOptions = {},
): Promise<ApiResponse<RewardBotSnapshotDto>> {
  return fetchWriteContract<ApiResponse<RewardBotSnapshotDto>>("/api/v1/rewards-bot/reset", {
    method: "POST",
    idempotencyKey: `reward-reset-${randomUUID()}`,
    body: options.operatorNote ? { operator_note: options.operatorNote } : {},
    stepUpCode: options.stepUpCode,
    stepUpScopes: ["rewards_state_reset"],
  });
}

export async function listRewardStrategyRuns(
  query?: RewardStrategyRunsQuery,
): Promise<ApiResponse<RewardStrategyRunPageDto>> {
  return fetchContract<ApiResponse<RewardStrategyRunPageDto>>(
    `/api/v1/rewards-bot/runs${buildQueryString(query as Record<string, string | number | boolean | undefined>)}`,
  );
}

export async function readRewardStrategyRun(
  runId: number,
): Promise<ApiResponse<RewardStrategyRunDto>> {
  return fetchContract<ApiResponse<RewardStrategyRunDto>>(`/api/v1/rewards-bot/runs/${runId}`);
}

export async function listRewardStrategyDecisions(
  runId: number,
  query?: RewardStrategyDecisionsQuery,
): Promise<ApiResponse<RewardStrategyDecisionPageDto>> {
  return fetchContract<ApiResponse<RewardStrategyDecisionPageDto>>(
    `/api/v1/rewards-bot/runs/${runId}/decisions${buildQueryString(query as Record<string, string | number | boolean | undefined>)}`,
  );
}

export async function listRewardStrategyActions(
  runId: number,
  query?: RewardStrategyActionsQuery,
): Promise<ApiResponse<RewardStrategyActionPageDto>> {
  return fetchContract<ApiResponse<RewardStrategyActionPageDto>>(
    `/api/v1/rewards-bot/runs/${runId}/actions${buildQueryString(query as Record<string, string | number | boolean | undefined>)}`,
  );
}

const STRATEGY_LEDGER_ANALYTICS_PAGE_SIZE = 500;
const STRATEGY_LEDGER_PAGE_CONCURRENCY = 2;

async function mapPagesWithConcurrency<T>(
  pages: number[],
  loadPage: (page: number) => Promise<T>,
): Promise<T[]> {
  if (pages.length === 0) return [];
  const results = new Array<T>(pages.length);
  let nextIndex = 0;

  async function worker() {
    while (nextIndex < pages.length) {
      const index = nextIndex;
      nextIndex += 1;
      const page = pages[index];
      if (page !== undefined) results[index] = await loadPage(page);
    }
  }

  await Promise.all(
    Array.from(
      { length: Math.min(STRATEGY_LEDGER_PAGE_CONCURRENCY, pages.length) },
      () => worker(),
    ),
  );
  return results;
}

export async function listAllRewardStrategyDecisions(
  runId: number,
): Promise<RewardStrategyDecisionDto[]> {
  const first = await listRewardStrategyDecisions(runId, {
    page: 1,
    page_size: STRATEGY_LEDGER_ANALYTICS_PAGE_SIZE,
  });
  const remainingPages = Array.from(
    { length: Math.max(0, first.data.page.total_pages - 1) },
    (_, index) => index + 2,
  );
  const remaining = await mapPagesWithConcurrency(remainingPages, (page) =>
    listRewardStrategyDecisions(runId, {
        page,
        page_size: STRATEGY_LEDGER_ANALYTICS_PAGE_SIZE,
      }),
  );
  return [first, ...remaining].flatMap((response) => response.data.items);
}

export async function listAllRewardStrategyActions(
  runId: number,
): Promise<RewardStrategyActionDto[]> {
  const first = await listRewardStrategyActions(runId, {
    page: 1,
    page_size: STRATEGY_LEDGER_ANALYTICS_PAGE_SIZE,
  });
  const remainingPages = Array.from(
    { length: Math.max(0, first.data.page.total_pages - 1) },
    (_, index) => index + 2,
  );
  const remaining = await mapPagesWithConcurrency(remainingPages, (page) =>
    listRewardStrategyActions(runId, {
        page,
        page_size: STRATEGY_LEDGER_ANALYTICS_PAGE_SIZE,
      }),
  );
  return [first, ...remaining].flatMap((response) => response.data.items);
}

export async function listRewardOrderTransitions(
  managedOrderId: string,
  query?: RewardOrderTransitionsQuery,
): Promise<ApiResponse<RewardOrderTransitionPageDto>> {
  return fetchContract<ApiResponse<RewardOrderTransitionPageDto>>(
    `/api/v1/rewards-bot/orders/${encodeURIComponent(managedOrderId)}/transitions${buildQueryString(query as Record<string, string | number | boolean | undefined>)}`,
  );
}
