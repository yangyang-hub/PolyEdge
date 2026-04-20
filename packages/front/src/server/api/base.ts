import "server-only";

import type {
  ApiErrorResponse,
  ApiListResponse,
  ApiMeta,
  ApiResponse,
  ContractListQuery,
  WriteResponse,
} from "@/lib/contracts/api";

const API_BASE_URL = process.env.POLYEDGE_API_BASE_URL?.replace(/\/$/, "");

export type BackendMode = "mock" | "live";

export class PolyEdgeApiError extends Error {
  code?: string;
  requestId?: string;
  traceId?: string;
  retryable?: boolean;

  constructor(
    message: string,
    options?: {
      code?: string;
      requestId?: string;
      traceId?: string;
      retryable?: boolean;
    },
  ) {
    super(message);
    this.name = "PolyEdgeApiError";
    this.code = options?.code;
    this.requestId = options?.requestId;
    this.traceId = options?.traceId;
    this.retryable = options?.retryable;
  }
}

function createMeta(resource: string): ApiMeta {
  return {
    request_id: `req_${resource}`,
    trace_id: `trc_${resource}`,
    generated_at: "2026-04-16T14:30:00Z",
  };
}

function clone<T>(value: T): T {
  return structuredClone(value);
}

export function getBackendMode(): BackendMode {
  return API_BASE_URL ? "live" : "mock";
}

export function getApiBaseUrl(): string | null {
  return API_BASE_URL ?? null;
}

export function buildQueryString(query?: ContractListQuery): string {
  if (!query) {
    return "";
  }

  const searchParams = new URLSearchParams();

  for (const [key, rawValue] of Object.entries(query)) {
    if (rawValue === undefined || rawValue === null) {
      continue;
    }

    if (Array.isArray(rawValue)) {
      for (const value of rawValue) {
        searchParams.append(key, value);
      }
      continue;
    }

    searchParams.set(key, String(rawValue));
  }

  const queryString = searchParams.toString();
  return queryString ? `?${queryString}` : "";
}

export function createListResponse<T>(resource: string, items: T[], limit?: number): ApiListResponse<T> {
  const normalizedLimit = limit ?? items.length;
  const slicedItems = items.slice(0, normalizedLimit);

  return {
    data: slicedItems,
    page: {
      limit: normalizedLimit,
      next_cursor: items.length > normalizedLimit ? `${resource}_cursor_01` : null,
      has_more: items.length > normalizedLimit,
    },
    meta: createMeta(resource),
  };
}

export function createResponse<T>(resource: string, data: T): ApiResponse<T> {
  return {
    data,
    meta: createMeta(resource),
  };
}

export function createWriteResponse(
  resource: string,
  resourceId: string,
  status: WriteResponse["data"]["status"] = "queued",
): WriteResponse {
  const operationId = `op_${resource}_${crypto.randomUUID().slice(0, 8)}`;

  return {
    data: {
      accepted: true,
      operation_id: operationId,
      resource_id: resourceId,
      status,
    },
    meta: createMeta(resource),
  };
}

export async function fetchContract<T>(path: string, fallback: T): Promise<T> {
  if (!API_BASE_URL) {
    return clone(fallback);
  }

  const response = await fetch(`${API_BASE_URL}${path}`, {
    headers: {
      Accept: "application/json",
    },
  });

  if (!response.ok) {
    throw new Error(`PolyEdge API request failed: ${response.status} ${response.statusText}`);
  }

  return (await response.json()) as T;
}

export async function fetchWriteContract<T>(
  path: string,
  init: {
    method?: "POST" | "PATCH";
    body: Record<string, string | number | boolean | null>;
    idempotencyKey: string;
  },
  fallback: T,
): Promise<T> {
  if (!API_BASE_URL) {
    return clone(fallback);
  }

  const response = await fetch(`${API_BASE_URL}${path}`, {
    method: init.method ?? "POST",
    headers: {
      Accept: "application/json",
      "Content-Type": "application/json",
      "Idempotency-Key": init.idempotencyKey,
    },
    body: JSON.stringify(init.body),
  });

  if (!response.ok) {
    let errorPayload: ApiErrorResponse | null = null;

    try {
      errorPayload = (await response.json()) as ApiErrorResponse;
    } catch {
      errorPayload = null;
    }

    throw new PolyEdgeApiError(errorPayload?.error.message ?? `PolyEdge API request failed: ${response.status}`, {
      code: errorPayload?.error.code,
      requestId: errorPayload?.meta.request_id,
      traceId: errorPayload?.meta.trace_id,
      retryable: errorPayload?.error.retryable,
    });
  }

  return (await response.json()) as T;
}
