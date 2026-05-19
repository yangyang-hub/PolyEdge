import "server-only";

import type { ApiListResponse, ApiResponse, ContractListQuery, WriteResponse } from "@/lib/contracts/api";
import type { RiskAlertDto, RiskBucketDto, RiskStateDto } from "@/lib/contracts/dto";
import { riskAlertFixtures, riskBucketFixtures, riskStateFixture } from "@/lib/server/polyedge-mock-data";
import {
  buildQueryString,
  createListResponse,
  createResponse,
  fetchContract,
  fetchListContract,
  createWriteResponse,
  fetchWriteContract,
} from "@/server/api/base";

type LiveSystemModeWriteResponse = ApiResponse<{
  mode: RiskStateDto["mode"];
  environment: RiskStateDto["environment"];
  version: number;
  updated_at: string;
}>;

export async function readRiskState(): Promise<ApiResponse<RiskStateDto>> {
  if (!process.env.POLYEDGE_API_BASE_URL) {
    return createResponse("risk_state", structuredClone(riskStateFixture));
  }

  return fetchContract<ApiResponse<RiskStateDto>>(
    "/api/v1/risk/state",
    createResponse("risk_state", riskStateFixture),
  );
}

export async function listRiskAlerts(query?: ContractListQuery): Promise<ApiListResponse<RiskAlertDto>> {
  const applyFilters = (alerts: RiskAlertDto[]) => alerts.filter((alert) => {
    if (query?.status && !query.status.includes(alert.status)) {
      return false;
    }

    return true;
  });

  const fallback = createListResponse("risk_alerts", applyFilters(riskAlertFixtures), query?.limit);

  return fetchListContract(
    `/api/v1/risk/alerts${buildQueryString({
      status: query?.status?.[0],
      limit: query?.limit,
    })}`,
    fallback,
  );
}

export async function listRiskBuckets(query?: ContractListQuery): Promise<ApiListResponse<RiskBucketDto>> {
  return fetchListContract(
    `/api/v1/risk/buckets${buildQueryString({ limit: query?.limit })}`,
    createListResponse("risk_buckets", riskBucketFixtures, query?.limit),
  );
}

export async function requestModeSwitch(input: {
  currentMode: string;
  targetMode: string;
  note: string;
  stepUpCode: string;
}): Promise<WriteResponse> {
  return fetchWriteContract(
    "/api/v1/system/mode",
    {
      method: "POST",
      idempotencyKey: `mode-${input.currentMode}-${input.targetMode}-${crypto.randomUUID()}`,
      body: {
        to_mode: input.targetMode,
        reason: input.note,
      },
      stepUpCode: input.stepUpCode,
      stepUpScopes: ["system_mode_switch"],
    },
    {
      mapLiveResponse: (payload: LiveSystemModeWriteResponse) =>
        createWriteResponse(`mode_switch_${payload.data.mode}`, "runtime_mode", "completed"),
    },
  );
}

export async function releaseRiskControls(input: {
  note: string;
  stepUpCode: string;
}): Promise<WriteResponse> {
  return fetchWriteContract(
    "/api/v1/system/kill-switch/release",
    {
      method: "POST",
      idempotencyKey: `risk-release-${crypto.randomUUID()}`,
      body: {
        reason: input.note,
        to_mode: "manual_confirm",
      },
      stepUpCode: input.stepUpCode,
      stepUpScopes: ["system_kill_switch_release"],
    },
    {
      mapLiveResponse: () =>
        createWriteResponse("risk_release", "risk_state_global", "completed"),
    },
  );
}

export async function setKillSwitchState(input: {
  enabled: boolean;
  note: string;
  stepUpCode: string;
}): Promise<WriteResponse> {
  if (input.enabled) {
    return fetchWriteContract(
      "/api/v1/system/kill-switch/trigger",
      {
        method: "POST",
        idempotencyKey: `kill-switch-on-${crypto.randomUUID()}`,
        body: {
          reason: input.note,
        },
        stepUpCode: input.stepUpCode,
        stepUpScopes: ["system_kill_switch_trigger"],
      },
      {
        mapLiveResponse: () =>
          createWriteResponse("kill_switch", "kill_switch_global", "completed"),
      },
    );
  }

  return releaseRiskControls({ note: input.note, stepUpCode: input.stepUpCode });
}
