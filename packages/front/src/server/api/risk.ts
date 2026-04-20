import "server-only";

import type { ApiListResponse, ApiResponse, ContractListQuery, WriteResponse } from "@/lib/contracts/api";
import type { RiskAlertDto, RiskBucketDto, RiskStateDto } from "@/lib/contracts/dto";
import { riskAlertFixtures, riskBucketFixtures, riskStateFixture } from "@/lib/server/polyedge-mock-data";
import {
  buildQueryString,
  createListResponse,
  createResponse,
  createWriteResponse,
  fetchContract,
  fetchWriteContract,
} from "@/server/api/base";

export async function readRiskState(): Promise<ApiResponse<RiskStateDto>> {
  return fetchContract("/api/risk/state", createResponse("risk_state", riskStateFixture));
}

export async function listRiskAlerts(query?: ContractListQuery): Promise<ApiListResponse<RiskAlertDto>> {
  const filtered = riskAlertFixtures.filter((alert) => {
    if (query?.status && !query.status.includes(alert.status)) {
      return false;
    }

    return true;
  });

  return fetchContract(
    `/api/risk/alerts${buildQueryString(query)}`,
    createListResponse("risk_alerts", filtered, query?.limit),
  );
}

export async function listRiskBuckets(query?: ContractListQuery): Promise<ApiListResponse<RiskBucketDto>> {
  return fetchContract(
    `/api/risk/buckets${buildQueryString(query)}`,
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
    "/api/system/mode",
    {
      method: "POST",
      idempotencyKey: `mode-${input.currentMode}-${input.targetMode}-${crypto.randomUUID()}`,
      body: {
        current_mode: input.currentMode,
        target_mode: input.targetMode,
        note: input.note,
        step_up_code: input.stepUpCode,
      },
    },
    createWriteResponse(`mode_switch_${input.targetMode}`, "runtime_mode", "queued"),
  );
}

export async function releaseRiskControls(input: {
  note: string;
  stepUpCode: string;
}): Promise<WriteResponse> {
  return fetchWriteContract(
    "/api/risk/release-controls",
    {
      method: "POST",
      idempotencyKey: `risk-release-${crypto.randomUUID()}`,
      body: {
        note: input.note,
        step_up_code: input.stepUpCode,
      },
    },
    createWriteResponse("risk_release", "risk_state_global", "queued"),
  );
}

export async function setKillSwitchState(input: {
  enabled: boolean;
  note: string;
  stepUpCode: string;
}): Promise<WriteResponse> {
  return fetchWriteContract(
    "/api/risk/kill-switch",
    {
      method: "POST",
      idempotencyKey: `kill-switch-${input.enabled ? "on" : "off"}-${crypto.randomUUID()}`,
      body: {
        enabled: input.enabled,
        note: input.note,
        step_up_code: input.stepUpCode,
      },
    },
    createWriteResponse("kill_switch", "kill_switch_global", "queued"),
  );
}
