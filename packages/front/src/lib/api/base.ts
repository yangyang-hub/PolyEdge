import type {
  ApiErrorResponse,
  ApiListResponse,
  ApiMeta,
  ApiResponse,
  WriteResponse,
} from "@/lib/contracts/api";

export type BackendMode = "live";
export type InternalApiStepUpScope =
  | "signal_approve"
  | "signal_reject"
  | "execution_submit"
  | "order_cancel_force"
  | "system_mode_switch"
  | "system_kill_switch_trigger"
  | "system_kill_switch_release"
  | "risk_threshold_update";

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

function createCursorPage(limit: number) {
  return {
    limit,
    next_cursor: null,
    has_more: false,
  };
}

export function getBackendMode(): BackendMode {
  return "live";
}

export function getConfiguredApiBaseUrl(): string | null {
  const value = process.env.NEXT_PUBLIC_POLYEDGE_API_BASE_URL?.trim().replace(/\/$/, "");
  return value ? value : null;
}

export function getApiBaseUrl(): string {
  return getConfiguredApiBaseUrl() ?? "";
}

export function randomUUID(): string {
  if (typeof crypto !== "undefined" && "randomUUID" in crypto) {
    return crypto.randomUUID();
  }

  return "xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx".replace(/[xy]/g, (c) => {
    const r = (Math.random() * 16) | 0;
    const v = c === "x" ? r : (r & 0x3) | 0x8;
    return v.toString(16);
  });
}

function createRequestId(): string {
  return `req_${randomUUID().replace(/-/g, "")}`;
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

export function createWriteResponse(
  resource: string,
  resourceId: string,
  status: WriteResponse["data"]["status"] = "queued",
): WriteResponse {
  const operationId = `op_${resource}_${randomUUID().slice(0, 8)}`;

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
    stepUpCode?: string;
    stepUpScopes?: InternalApiStepUpScope[];
  },
): Promise<T> {
  const apiBaseUrl = getApiBaseUrl();
  const headers = new Headers(init.headers);
  const requestId = createRequestId();

  headers.set("X-Request-Id", requestId);

  if (process.env.NEXT_PUBLIC_POLYEDGE_INTERNAL_AUTH_DEV_BYPASS === "1") {
    headers.set("X-PolyEdge-Dev-Auth", "local");
    headers.set("X-PolyEdge-Console-Role", "admin");
    headers.set("X-PolyEdge-Console-User", encodeURIComponent("Static Console"));
  }

  if (auth.stepUpCode?.trim()) {
    headers.set("X-PolyEdge-Step-Up-Code", auth.stepUpCode.trim());
  }

  if (auth.stepUpScopes?.length) {
    headers.set("X-PolyEdge-Step-Up-Scopes", auth.stepUpScopes.join(","));
  }

  const response = await fetch(`${apiBaseUrl}${path}`, {
    ...init,
    headers,
    cache: "no-store",
    credentials: "same-origin",
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
      requestId: errorPayload?.meta.request_id ?? requestId,
      traceId: errorPayload?.meta.trace_id,
      retryable: errorPayload?.error.retryable,
    });
  }

  return (await response.json()) as T;
}

export async function fetchContract<T>(path: string): Promise<T> {
  return fetchJson<T>(
    path,
    {
      headers: {
        Accept: "application/json",
      },
    },
    {
    },
  );
}

export async function fetchListContract<TLive, TFront = TLive>(
  path: string,
  options?: {
    mapItem?: (item: TLive) => TFront;
  },
): Promise<ApiListResponse<TFront>> {
  const payload = await fetchJson<ApiResponse<TLive[]>>(
    path,
    {
      headers: {
        Accept: "application/json",
      },
    },
    {
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
  options?: {
    mapLiveResponse?: (payload: TLive) => TFront;
  },
): Promise<TFront> {
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
      stepUpCode: init.stepUpCode,
      stepUpScopes: init.stepUpScopes,
    },
  );

  return options?.mapLiveResponse ? options.mapLiveResponse(payload) : ((payload as unknown) as TFront);
}
