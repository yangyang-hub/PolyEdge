import { readFundingStatus } from "@/lib/api/funding";
import type { FundingStatusDto } from "@/lib/contracts/dto";

export type FundingPageData = {
  status: FundingStatusDto;
};

export async function getFundingPageData(): Promise<FundingPageData> {
  const response = await readFundingStatus();

  return {
    status: response.data,
  };
}
