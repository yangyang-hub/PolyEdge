import type { ApiListResponse, ApiResponse, ContractListQuery, WriteResponse } from "@/lib/contracts/api";
import type { RiskAlertDto, RiskBucketDto, RiskStateDto } from "@/lib/contracts/dto";
import {
  buildQueryString,
  createWriteResponse,
  fetchContract,
  fetchListContract,
  fetchWriteContract,
  randomUUID,
} from "@/lib/api/base";

export async function readRiskState(): Promise<ApiResponse<RiskStateDto>> {
  return fetchContract<ApiResponse<RiskStateDto>>("/api/v1/risk/state");
}

export async function listRiskAlerts(query?: ContractListQuery): Promise<ApiListResponse<RiskAlertDto>> {
  return fetchListContract(
    `/api/v1/risk/alerts${buildQueryString({
      status: query?.status?.[0],
      limit: query?.limit,
    })}`,
  );
}

export async function listRiskBuckets(query?: ContractListQuery): Promise<ApiListResponse<RiskBucketDto>> {
  return fetchListContract(`/api/v1/risk/buckets${buildQueryString({ limit: query?.limit })}`);
}

export async function releaseRiskControls(input: {
  note: string;
  stepUpCode: string;
}): Promise<WriteResponse> {
  return fetchWriteContract(
    "/api/v1/system/kill-switch/release",
    {
      method: "POST",
      idempotencyKey: `risk-release-${randomUUID()}`,
      body: {
        reason: input.note,
        to_mode: "live_auto",
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
        idempotencyKey: `kill-switch-on-${randomUUID()}`,
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
