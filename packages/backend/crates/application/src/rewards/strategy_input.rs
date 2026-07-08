// Replay-friendly, serializable snapshot of every read-only input a single
// rewards decision tick consumes.
//
// Phase 2 scope: this captures the deterministic inputs for
// `RewardDecisionEngine` — config, candidate markets, the pre-application
// quote plans, order books and local book history, account/open orders/
// positions, and effective event windows — plus the tick timestamp and
// `force_orders` trigger flag. Provider cache (AI advisory / info-risk) is
// applied by the worker *between* engine phases via
// `apply_cached_reward_ai_advisories_to_cycle` /
// `apply_cached_reward_info_risks_to_cycle`; it is input-hash keyed and
// settings dependent, so it is intentionally not captured here. Full provider
// replay belongs to Phase 4 v2.
//
// The snapshot is the canonical tick input. The mutable `RewardLiveCycle` the
// engine mutates is derived from it via `RewardLiveCycle::from_strategy_input`,
// so the engine signature and live trading behavior stay unchanged.

/// Owned, serializable snapshot of a single rewards decision tick's read-only
/// inputs. See the module preamble above for scope and the provider-cache
/// deferral.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RewardStrategyInput {
    /// Logical tick timestamp; the single `now` threaded through planning,
    /// event-window application and the engine. Injected once by the builder so
    /// the snapshot is deterministic and replay-faithful.
    #[serde(with = "time::serde::rfc3339")]
    pub now: OffsetDateTime,
    /// Whether the tick was forced (run-once) rather than a normal poll.
    pub force_orders: bool,
    pub config: RewardBotConfig,
    pub candidate_markets: Vec<RewardCandidateMarket>,
    /// Quote plans as built from candidates, *before* the decision engine runs
    /// or provider cache is applied. Captured pre-application so replay can
    /// re-run the engine and apply steps deterministically.
    pub plans: Vec<RewardQuotePlan>,
    pub previous_plans: Vec<RewardQuotePlan>,
    pub pre_ai_eligible_condition_ids: Vec<String>,
    pub books: HashMap<String, RewardOrderBook>,
    /// Local book history keyed by token id. Stored as `Vec` (not the worker's
    /// `VecDeque`) for clean serialization; insertion order is preserved.
    pub book_history: HashMap<String, Vec<BookSnapshot>>,
    pub account: RewardAccountState,
    pub open_orders: Vec<ManagedRewardOrder>,
    pub positions: Vec<RewardPosition>,
    pub event_windows: Vec<RewardMarketEventWindow>,
}

impl RewardLiveCycle {
    /// Derive the mutable working cycle the engine mutates from a strategy
    /// input snapshot. Pure field copy except `markets` (projected from
    /// candidate markets) and `should_execute` (`config.enabled || force_orders`).
    /// Plans are NOT rebuilt from candidates, so this derivation is exact and
    /// behavior-equivalent.
    #[must_use]
    pub fn from_strategy_input(input: &RewardStrategyInput) -> RewardLiveCycle {
        let markets = input
            .candidate_markets
            .iter()
            .map(|candidate| candidate.market.clone())
            .collect::<Vec<_>>();
        RewardLiveCycle {
            config: input.config.clone(),
            account: input.account.clone(),
            markets,
            plans: input.plans.clone(),
            previous_plans: input.previous_plans.clone(),
            pre_ai_eligible_condition_ids: input.pre_ai_eligible_condition_ids.clone(),
            open_orders: input.open_orders.clone(),
            positions: input.positions.clone(),
            should_execute: input.config.enabled || input.force_orders,
        }
    }
}
