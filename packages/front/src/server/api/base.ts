import "server-only";

import type {
  ApiErrorResponse,
  ApiListResponse,
  ApiMeta,
  ApiResponse,
  WriteResponse,
} from "@/lib/contracts/api";
import type { InternalApiRequestKind, InternalApiStepUpScope } from "@/server/auth/internal-api-token";
import { createInternalApiHeaders } from "@/server/auth/internal-api-token";

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

function createCursorPage(limit: number) {
  return {
    limit,
    next_cursor: null,
    has_more: false,
  };
}

export function getBackendMode(): BackendMode {
  return getApiBaseUrl() ? "live" : "mock";
}

export function getApiBaseUrl(): string | null {
  const value = process.env.POLYEDGE_API_BASE_URL?.trim().replace(/\/$/, "");
  return value ? value : null;
}

export function buildQueryString(
  query?: Record<
    string,
    string | number | boolean | null | undefined | Array<string | number | boolean>
  >,
): string {
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
        searchParams.append(key, String(value));
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

async function fetchJson<T>(
  path: string,
  init: RequestInit,
  auth: {
    kind: InternalApiRequestKind;
    stepUpCode?: string;
    stepUpScopes?: InternalApiStepUpScope[];
  },
): Promise<T> {
  const apiBaseUrl = getApiBaseUrl();

  if (!apiBaseUrl) {
    throw new PolyEdgeApiError("PolyEdge API base URL is not configured.");
  }

  const authHeaders = await createInternalApiHeaders(auth);
  const headers = new Headers(init.headers);

  authHeaders.forEach((value, key) => {
    headers.set(key, value);
  });

  const response = await fetch(`${apiBaseUrl}${path}`, {
    ...init,
    headers,
    cache: "no-store",
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

export async function fetchContract<T>(path: string, fallback: T): Promise<T> {
  if (!getApiBaseUrl()) {
    return clone(fallback);
  }

  return fetchJson<T>(
    path,
    {
      headers: {
        Accept: "application/json",
      },
    },
    {
      kind: "read",
    },
  );
}

export async function fetchListContract<TLive, TFront = TLive>(
  path: string,
  fallback: ApiListResponse<TFront>,
  options?: {
    mapItem?: (item: TLive) => TFront;
  },
): Promise<ApiListResponse<TFront>> {
  if (!getApiBaseUrl()) {
    return clone(fallback);
  }

  const payload = await fetchJson<ApiResponse<TLive[]>>(
    path,
    {
      headers: {
        Accept: "application/json",
      },
    },
    {
      kind: "read",
    },
  );
  const items = options?.mapItem ? payload.data.map(options.mapItem) : (payload.data as unknown as TFront[]);

  return {
    data: items,
    page: createCursorPage(items.length),
    meta: payload.meta,
  };
}

export async function fetchWriteContract<TLive, TFront = TLive>(
  path: string,
  init: {
    method?: "POST" | "PATCH";
    body: Record<string, unknown>;
    idempotencyKey: string;
    stepUpCode?: string;
    stepUpScopes?: InternalApiStepUpScope[];
  },
  fallback: TFront,
  options?: {
    mapLiveResponse?: (payload: TLive) => TFront;
  },
): Promise<TFront> {
  if (!getApiBaseUrl()) {
    return clone(fallback);
  }

  const payload = await fetchJson<TLive>(
    path,
    {
      method: init.method ?? "POST",
      headers: {
        Accept: "application/json",
        "Content-Type": "application/json",
        "Idempotency-Key": init.idempotencyKey,
      },
      body: JSON.stringify(init.body),
    },
    {
      kind: "write",
      stepUpCode: init.stepUpCode,
      stepUpScopes: init.stepUpScopes,
    },
  );

  return options?.mapLiveResponse ? options.mapLiveResponse(payload) : ((payload as unknown) as TFront);
}

export async function fetchUnsupportedContract<T>(fallback: T): Promise<T> {
  return clone(fallback);
}

export async function fetchUnsupportedListContract<T>(fallback: ApiListResponse<T>): Promise<ApiListResponse<T>> {
  return clone(fallback);
}
