export type ResourceVersion = {
  id: string;
  version: number;
};

export type MarketStatus = "open" | "closed" | "resolved";
export type AmbiguityLevel = "low" | "medium" | "high";
export type TradabilityStatus = "tradable" | "manual_review" | "observe_only" | "blocked";
export type EventStatus = "active" | "expired" | "invalidated" | "superseded";
export type EvidenceDirection = "supports_yes" | "supports_no" | "background";
export type EvidenceStatus = "active" | "expired" | "invalidated";
export type SignalAction = "buy" | "sell";
export type SignalSide = "yes" | "no";
export type SignalLifecycleState =
  | "new"
  | "active"
  | "weakened"
  | "executed"
  | "invalidated"
  | "reversed"
  | "expired";
export type RuntimeMode =
  | "research"
  | "paper_trade"
  | "manual_confirm"
  | "live_auto"
  | "kill_switch_locked";
export type RuntimeEnvironment = "local" | "paper" | "staging" | "production";
export type RuntimeConfigValueType = "boolean" | "integer" | "decimal" | "text" | "url" | "json" | "enum";
export type AlertSeverity = "warning" | "critical";
export type AlertStatus = "unresolved" | "watching" | "contained";
export type PositionSide = "yes" | "no";
export type BucketStatus = "healthy" | "watch" | "breach";
export type NewsSourceType = "news" | "social" | "official" | "calendar" | "market";
export type ReplayMomentKind = "event_ingested" | "evidence_generated" | "posterior_updated" | "signal_transition";
export type ArbitrageOpportunityType = "binary_buy_both" | "binary_sell_both";
export type ArbitrageOpportunityStatus = "observed" | "expired" | "repeated";
export type ArbitrageValidationStatus =
  | "unvalidated"
  | "valid"
  | "stale_book"
  | "insufficient_depth"
  | "price_moved"
  | "fees_exceed_edge"
  | "below_threshold"
  | "invalid_market"
  | "error";
export type RewardOrderSide = "buy" | "sell";
export type ManagedRewardOrderStatus =
  | "planned"
  | "open"
  | "cancelled"
  | "filled"
  | "exit_pending"
  | "error";
export type RewardRiskSeverity = "info" | "warning" | "critical";
export type PostFillStrategy =
  | "exit_at_markup"
  | "hold_and_requote"
  | "flatten_immediately";
export type RewardFillRole = "maker" | "taker";
export type CopyTradeMode = "paper" | "live";
export type CopySizingMode = "fixed_usd" | "proportional_to_source" | "capital_ratio" | "mirror_portfolio_weight";
export type CopyOrderSide = "buy" | "sell";
export type CopyOrderStatus = "planned" | "open" | "filled" | "cancelled" | "skipped" | "error";
export type TrackedWalletStatus = "active" | "paused";
export type CopyEventSeverity = "info" | "warning" | "critical";
export type DecimalValue = string | number;
