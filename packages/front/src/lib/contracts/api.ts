export type SortOrder = "asc" | "desc";

export type ContractListQuery = {
  limit?: number;
  cursor?: string;
  status?: string[];
  market_id?: string;
  event_id?: string;
  signal_state?: string[];
  source_type?: string;
  from?: string;
  to?: string;
  q?: string;
  sort_by?: string;
  sort_order?: SortOrder;
};

export type ApiMeta = {
  request_id: string;
  trace_id: string;
  generated_at?: string;
};

export type CursorPage = {
  limit: number;
  next_cursor: string | null;
  has_more: boolean;
};

export type ApiResponse<T> = {
  data: T;
  meta: ApiMeta;
};

export type ApiListResponse<T> = {
  data: T[];
  page: CursorPage;
  meta: ApiMeta;
};

export type WriteOperationResult = {
  accepted: boolean;
  operation_id: string;
  resource_id: string;
  status: "queued" | "completed" | "rejected";
};

export type WriteResponse = ApiResponse<WriteOperationResult>;

export type ApiError = {
  code: string;
  message: string;
  details?: Record<string, string | number | boolean>;
  retryable: boolean;
};

export type ApiErrorResponse = {
  error: ApiError;
  meta: Pick<ApiMeta, "request_id" | "trace_id">;
};
