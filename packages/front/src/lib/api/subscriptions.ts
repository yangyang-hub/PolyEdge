import { fetchListContract, fetchWriteContract, randomUUID } from "@/lib/api/base";
import type { CreateStrategySubscriptionRequest, StrategySubscriptionData } from "@/lib/contracts/dto";
export const listSubscriptions = () => fetchListContract<StrategySubscriptionData>("/api/v1/strategy-subscriptions");
export const createSubscription = (body: CreateStrategySubscriptionRequest) => fetchWriteContract("/api/v1/strategy-subscriptions", { body, idempotencyKey: randomUUID() });
