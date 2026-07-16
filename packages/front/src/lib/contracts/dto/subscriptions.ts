export type StrategySubscriptionStatus = "active" | "paused" | "stopped";
export type StrategySubscriptionDto = {
  id: number;
  follower_user_id: number;
  source_strategy_id: number;
  source_strategy_name: string;
  source_user_id: number;
  source_display_name: string;
  kind: "owner" | "follower";
  status: StrategySubscriptionStatus;
  active_until: string | null;
  effective_active_until: string;
  stopped_at: string | null;
  created_at: string;
  updated_at: string;
};
export type StrategySubscriptionWalletDto = {
  subscription_id: number;
  follower_user_id: number;
  wallet_id: number;
  enabled: boolean;
  created_at: string;
};
export type StrategySubscriptionData = { subscription: StrategySubscriptionDto; wallets: StrategySubscriptionWalletDto[] };
export type CreateStrategySubscriptionRequest = { source_strategy_id: number; wallet_ids: number[]; active_until?: string; operator_note?: string };
