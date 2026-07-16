import type {
  DecimalValue,
  MarketStatus,
  QuoteOutcome,
  QuotePricingMode,
} from "./primitives";
import type { StrategySubscriptionData } from "./subscriptions";

export type StrategyStatus = "draft" | "active" | "paused" | "expired" | "archived";
export type StrategyVersionStatus = "draft" | "published" | "retired";

export type ManagedMarketDto = {
  id: number;
  condition_id: string;
  slug: string;
  question: string;
  polymarket_url: string | null;
  status: MarketStatus;
  created_at: string;
  updated_at: string;
};

export type ManagedMarketOutcomeDto = {
  id: number;
  market_id: number;
  outcome: QuoteOutcome;
  token_id: string;
};

export type StrategyRewardTermsDto = {
  strategy_version_id: number;
  minimum_size: DecimalValue;
  maximum_spread: DecimalValue;
  daily_rate: DecimalValue | null;
};

export type MarketStrategyDto = {
  id: number;
  market_id: number;
  name: string;
  owner_user_id: number;
  owner_display_name: string;
  visibility: "private" | "followable";
  active_from: string;
  active_until: string;
  status: StrategyStatus;
  created_at: string;
  updated_at: string;
};

export type StrategyVersionDto = {
  id: number;
  strategy_id: number;
  version_number: number;
  status: StrategyVersionStatus;
  book_freshness_ms: number;
  downward_reprice_confirm_ms: number;
  upward_reprice_confirm_ms: number;
  reprice_cooldown_ms: number;
  max_replaces_per_cycle: number;
  published_at: string | null;
  created_at: string;
};

export type StrategyQuoteSlotDto = {
  id: number;
  strategy_version_id: number;
  slot_key: string;
  outcome: QuoteOutcome;
  quantity: DecimalValue;
  pricing_mode: QuotePricingMode;
  fixed_price: DecimalValue | null;
  book_rank: number | null;
  price_offset: DecimalValue;
  minimum_price: DecimalValue;
  maximum_price: DecimalValue;
  post_only: boolean;
  enabled: boolean;
};

export type MarketStrategyData = {
  market: ManagedMarketDto;
  outcomes: ManagedMarketOutcomeDto[];
  reward_terms: StrategyRewardTermsDto;
  strategy: MarketStrategyDto;
  version: StrategyVersionDto;
  quote_slots: StrategyQuoteSlotDto[];
  current_user_subscription?: StrategySubscriptionData | null;
};

export type ManagedMarketInput = {
  condition_id: string;
  slug: string;
  question: string;
  polymarket_url?: string;
  yes_token_id: string;
  no_token_id: string;
};

export type QuoteSlotInput = {
  slot_key: string;
  outcome: QuoteOutcome;
  quantity: DecimalValue;
  pricing_mode: QuotePricingMode;
  fixed_price?: DecimalValue;
  book_rank?: number;
  price_offset: DecimalValue;
  minimum_price: DecimalValue;
  maximum_price: DecimalValue;
  post_only: boolean;
  enabled: boolean;
};

export type StrategyVersionInput = {
  reward_minimum_size: DecimalValue;
  reward_maximum_spread: DecimalValue;
  reward_daily_rate?: DecimalValue;
  book_freshness_ms: number;
  downward_reprice_confirm_ms: number;
  upward_reprice_confirm_ms: number;
  reprice_cooldown_ms: number;
  max_replaces_per_cycle: number;
  quote_slots: QuoteSlotInput[];
};

export type CreateMarketStrategyRequest = {
  name: string;
  visibility: "private" | "followable";
  active_from: string;
  active_until: string;
  market: ManagedMarketInput;
  version: StrategyVersionInput;
  wallet_ids: number[];
  operator_note?: string;
};
