export type ExecutionBatchStatus =
  | "pending"
  | "running"
  | "partially_succeeded"
  | "succeeded"
  | "failed"
  | "cancelled";

export type WalletExecutionJobStatus =
  | "pending"
  | "running"
  | "succeeded"
  | "failed"
  | "cancelled";

export type ExecutionBatchDto = {
  id: number;
  strategy_version_id: number;
  status: ExecutionBatchStatus;
  requested_by: string;
  operator_note: string | null;
  created_at: string;
  started_at: string | null;
  completed_at: string | null;
};

export type WalletExecutionJobDto = {
  id: number;
  batch_id: number;
  wallet_id: number;
  status: WalletExecutionJobStatus;
  attempt_count: number;
  error_code: string | null;
  error_message: string | null;
  lease_epoch: number;
  lease_owner: string | null;
  lease_expires_at: string | null;
  created_at: string;
  updated_at: string;
};

export type ExecutionBatchData = {
  batch: ExecutionBatchDto;
  jobs: WalletExecutionJobDto[];
};

export type CreateExecutionBatchRequest = {
  strategy_id: number;
  wallet_ids: number[];
  operator_note?: string;
};

export type CreateCancellationBatchRequest = {
  wallet_ids: number[];
  condition_ids: string[];
  operator_note?: string;
};
