import type { FundingTransferDto } from "@/lib/contracts/dto";

export type FundingActionState = "idle" | "submitting" | "submitted" | "error";

export type FundingSubmissionSnapshot = {
  status: FundingActionState;
  message: string | null;
  transfer: FundingTransferDto | null;
};
