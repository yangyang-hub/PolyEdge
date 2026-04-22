import "server-only";

import type { ApiListResponse, ApiResponse, ContractListQuery, WriteResponse } from "@/lib/contracts/api";
import type { RiskAlertDto, RiskBucketDto, RiskStateDto } from "@/lib/contracts/dto";
import { riskAlertFixtures, riskBucketFixtures, riskStateFixture } from "@/lib/server/polyedge-mock-data";
import {
  createListResponse,
  createResponse,
  createWriteResponse,
  fetchWriteContract,
} from "@/server/api/base";
import {
  readDerivedLiveRiskAlerts,
  readDerivedLiveRiskBuckets,
  readDerivedLiveRiskState,
} from "@/server/api/live-console-derived";

type LiveSystemModeWriteResponse = ApiResponse<{
  mode: RiskStateDto["mode"];
  environment: RiskStateDto["environment"];
  version: number;
  updated_at: string;
}>;

export async function readRiskState(): Promise<ApiResponse<RiskStateDto>> {
  if (!process.env.POLYEDGE_API_BASE_URL) {
    return createResponse("risk_state", riskStateFixture);
  }

  return readDerivedLiveRiskState();
}

export async function listRiskAlerts(query?: ContractListQuery): Promise<ApiListResponse<RiskAlertDto>> {
  const applyFilters = (alerts: RiskAlertDto[]) => alerts.filter((alert) => {
    if (query?.status && !query.status.includes(alert.status)) {
      return false;
    }

    return true;
  });

  if (!process.env.POLYEDGE_API_BASE_URL) {
    return createListResponse("risk_alerts", applyFilters(riskAlertFixtures), query?.limit);
  }

  const { data, meta } = await readDerivedLiveRiskAlerts();
  const filtered = applyFilters(data);
  const limited = query?.limit ? filtered.slice(0, query.limit) : filtered;

  return {
    data: limited,
    page: {
      limit: query?.limit ?? limited.length,
      next_cursor: null,
      has_more: filtered.length > limited.length,
    },
    meta,
  };
}

export async function listRiskBuckets(query?: ContractListQuery): Promise<ApiListResponse<RiskBucketDto>> {
  if (!process.env.POLYEDGE_API_BASE_URL) {
    return createListResponse("risk_buckets", riskBucketFixtures, query?.limit);
  }

  const { data, meta } = await readDerivedLiveRiskBuckets();
  const limited = query?.limit ? data.slice(0, query.limit) : data;

  return {
    data: limited,
    page: {
      limit: query?.limit ?? limited.length,
      next_cursor: null,
      has_more: data.length > limited.length,
    },
    meta,
  };
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
    createWriteResponse(`mode_switch_${input.targetMode}`, "runtime_mode", "queued"),
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
    createWriteResponse("risk_release", "risk_state_global", "queued"),
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
      createWriteResponse("kill_switch", "kill_switch_global", "queued"),
      {
        mapLiveResponse: () =>
          createWriteResponse("kill_switch", "kill_switch_global", "completed"),
      },
    );
  }

  return fetchWriteContract(
    "/api/v1/system/kill-switch/release",
    {
      method: "POST",
      idempotencyKey: `kill-switch-off-${crypto.randomUUID()}`,
      body: {
        reason: input.note,
        to_mode: "manual_confirm",
      },
      stepUpCode: input.stepUpCode,
      stepUpScopes: ["system_kill_switch_release"],
    },
    createWriteResponse("kill_switch", "kill_switch_global", "queued"),
    {
      mapLiveResponse: () =>
        createWriteResponse("kill_switch", "kill_switch_global", "completed"),
    },
  );
}
