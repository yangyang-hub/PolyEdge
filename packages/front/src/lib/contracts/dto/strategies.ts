import type {
  DecimalValue,
  MarketStatus,
  QuoteOutcome,
  QuotePricingMode,
} from "./primitives";

export type StrategyStatus = "draft" | "active" | "paused" | "archived";
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

export type MarketRewardTermsDto = {
  market_id: number;
  minimum_size: DecimalValue;
  maximum_spread: DecimalValue;
  daily_rate: DecimalValue | null;
  updated_at: string;
};

export type MarketStrategyDto = {
  id: number;
  market_id: number;
  name: string;
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

export type StrategyWalletTargetDto = {
  strategy_id: number;
  wallet_id: number;
  enabled: boolean;
  created_at: string;
};

export type MarketStrategyData = {
  market: ManagedMarketDto;
  outcomes: ManagedMarketOutcomeDto[];
  reward_terms: MarketRewardTermsDto;
  strategy: MarketStrategyDto;
  version: StrategyVersionDto;
  quote_slots: StrategyQuoteSlotDto[];
  wallet_targets: StrategyWalletTargetDto[];
};

export type ManagedMarketInput = {
  condition_id: string;
  slug: string;
  question: string;
  polymarket_url?: string;
  yes_token_id: string;
  no_token_id: string;
  reward_minimum_size: DecimalValue;
  reward_maximum_spread: DecimalValue;
  reward_daily_rate?: DecimalValue;
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
  book_freshness_ms: number;
  downward_reprice_confirm_ms: number;
  upward_reprice_confirm_ms: number;
  reprice_cooldown_ms: number;
  max_replaces_per_cycle: number;
  quote_slots: QuoteSlotInput[];
  wallet_ids: number[];
};

export type CreateMarketStrategyRequest = {
  name: string;
  market: ManagedMarketInput;
  version: StrategyVersionInput;
  operator_note?: string;
};
