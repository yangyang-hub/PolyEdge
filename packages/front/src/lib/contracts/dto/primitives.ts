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
export type RuntimeMode =
  | "live_auto"
  | "kill_switch_locked";
export type RuntimeEnvironment = "local" | "paper" | "staging" | "production";
export type RuntimeConfigValueType = "boolean" | "integer" | "decimal" | "text" | "url" | "json" | "enum";
export type NewsSourceType = "news" | "social" | "official" | "calendar" | "market";
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
  | "flatten_immediately"
  | "adaptive";
export type RewardExitStrategySource =
  | "configured"
  | "adaptive"
  | "external_inventory";
export type RewardQuoteMode = "double" | "auto";
export type RewardSelectionMode = "observe" | "enforce";
export type RewardEventTimeConfidence = "low" | "medium" | "high";
export type RewardUnknownEventTimeMode = "allow" | "observe" | "block";
export type RewardGammaEventDateMode = "ignore" | "observe" | "medium_confidence";
export type RewardEventWindowStatus =
  | "no_event_window"
  | "safe_before_window"
  | "stop_new_quotes"
  | "cancel_open_buys"
  | "in_event_window"
  | "post_event_cooldown"
  | "expired_or_resolved"
  | "untrusted_event_time";
export type RewardPlanQuoteMode = "double" | "single_yes" | "single_no" | "none";
export type RewardQuoteReadiness =
  | "ready_to_quote"
  | "waiting_orderbook"
  | "provider_pending"
  | "blocked";
export type RewardStrategyBucket = "standard" | "none";
export type RewardStrategyProfile = "standard" | "balanced_merge";
export type RewardAiProvider = "openai" | "anthropic";
export type RewardAiRequestFormat =
  | "openai_responses"
  | "openai_chat_completions"
  | "anthropic_messages";
export type RewardProviderAction =
  | "allow"
  | "reduce"
  | "stop_new"
  | "cancel_yes"
  | "cancel_no"
  | "cancel_all";
export type RewardInfoRiskLevel = "low" | "medium" | "high" | "critical" | "unknown";
export type RewardInfoRiskType =
  | "imminent_resolution"
  | "breaking_news"
  | "scheduled_event"
  | "official_result"
  | "rumor"
  | "stale"
  | "none"
  | "unknown";
export type RewardInfoDirectionalRisk = "yes" | "no" | "unclear";
export type RewardFillRole = "maker" | "taker";
export type RewardStrategyRunTrigger =
  | "poll"
  | "run_once"
  | "orderbook_event"
  | "control_command"
  | "replay";
export type RewardStrategyRunStatus = "running" | "completed" | "failed" | "cancelled";
export type RewardStrategyActionType =
  | "place_buy"
  | "submit_exit_sell"
  | "cancel_order"
  | "cancel_replace_exit"
  | "record_fill"
  | "create_merge_intent"
  | "execute_merge"
  | "skip";
export type RewardStrategyActionStatus =
  | "planned"
  | "executing"
  | "succeeded"
  | "failed"
  | "skipped"
  | "unknown";
export type DecimalValue = string | number;
