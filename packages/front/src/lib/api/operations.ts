import { fetchContract, fetchListContract } from "@/lib/api/base";
import type { ApiListResponse, ApiResponse } from "@/lib/contracts/api";
import type {
  ExecutionBatchData,
  ManagedOrderDto,
  ManagedPositionDto,
} from "@/lib/contracts/dto";

export function listExecutionBatches(): Promise<ApiListResponse<ExecutionBatchData>> {
  return fetchListContract<ExecutionBatchData>("/api/v1/execution-batches");
}

export function getExecutionBatch(batchId: number): Promise<ApiResponse<ExecutionBatchData>> {
  return fetchContract<ApiResponse<ExecutionBatchData>>(`/api/v1/execution-batches/${batchId}`);
}

export function listOrders(): Promise<ApiListResponse<ManagedOrderDto>> {
  return fetchListContract<ManagedOrderDto>("/api/v1/orders");
}

export function listPositions(): Promise<ApiListResponse<ManagedPositionDto>> {
  return fetchListContract<ManagedPositionDto>("/api/v1/positions");
}
