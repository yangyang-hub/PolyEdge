/// The full set of state changes produced by a single rewards tick. The
/// store persists it atomically via `apply_tick_outcome`.
#[derive(Debug, Clone, PartialEq)]
pub struct RewardTickOutcome {
    pub account: RewardAccountState,
    pub markets: Vec<RewardMarket>,
    pub plans: Vec<RewardQuotePlan>,
    /// New and modified managed orders, keyed by `id` (upserted).
    pub orders: Vec<ManagedRewardOrder>,
    /// Positions to upsert, keyed by `(account_id, token_id)`.
    pub positions: Vec<RewardPosition>,
    pub fills: Vec<RewardFill>,
    pub merge_intents: Vec<RewardMergeIntent>,
    pub events: Vec<RewardRiskEvent>,
    pub report: RewardBotRunReport,
}
