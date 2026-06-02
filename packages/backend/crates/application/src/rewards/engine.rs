// Stateful, tick-based rewards market-making engine.
//
// Each tick the engine reconciles the existing resting orders against the
// freshest order books, validates fills (deterministic when the book crosses
// our price, probabilistic when it merely touches), applies the configured
// post-fill strategy, and finally tops up quotes for eligible markets while
// respecting the shared fund pool on fills. Resting buys reuse the
// pool across markets instead of
// hard-reserving cash per order.
//
// NOTE: Engine functions are currently only used by tests. The production live
// path uses the worker's direct Polymarket connector flow instead.

/// The full set of state changes produced by a single validation tick. The
/// store persists it atomically via `apply_simulation_tick`.
#[derive(Debug, Clone, PartialEq)]
pub struct RewardSimulationOutcome {
    pub account: RewardAccountState,
    pub markets: Vec<RewardMarket>,
    pub plans: Vec<RewardQuotePlan>,
    /// New and modified managed orders, keyed by `id` (upserted).
    pub orders: Vec<ManagedRewardOrder>,
    /// Positions to upsert, keyed by `(account_id, token_id)`.
    pub positions: Vec<RewardPosition>,
    pub fills: Vec<RewardFill>,
    pub events: Vec<RewardRiskEvent>,
    pub report: RewardBotRunReport,
}

#[allow(dead_code)]
struct TickContext {
    now: OffsetDateTime,
    config: RewardBotConfig,
    account: RewardAccountState,
    orders: Vec<ManagedRewardOrder>,
    positions: HashMap<String, RewardPosition>,
    fills: Vec<RewardFill>,
    events: Vec<RewardRiskEvent>,
    trace_id: String,
    seq: usize,
    filled_orders: usize,
    placed_orders: usize,
    cancelled_orders: usize,
    risk_cancelled_orders: usize,
    reward_accrued: Decimal,
}

/// Run a single validation tick over the supplied inputs.
///
/// `open_orders` should contain the account's currently open-like orders and
/// `positions` its non-zero inventory. `elapsed_seconds` is the wall-clock gap
/// since the previous tick and drives reward accrual.
#[must_use]
#[allow(dead_code)]
pub fn run_reward_simulation_tick(
    config: &RewardBotConfig,
    account: RewardAccountState,
    open_orders: Vec<ManagedRewardOrder>,
    positions: Vec<RewardPosition>,
    markets: &[RewardMarket],
    books: &HashMap<String, RewardOrderBook>,
    book_history: &HashMap<String, VecDeque<BookSnapshot>>,
    elapsed_seconds: i64,
    trace_id: &str,
) -> RewardSimulationOutcome {
    let now = OffsetDateTime::now_utc();
    let plans = build_reward_quote_plans(markets, books, config);
    let eligible_plans = plans.iter().filter(|plan| plan.eligible).count();

    let mut ctx = TickContext {
        now,
        config: config.clone(),
        account,
        orders: open_orders,
        positions: positions
            .into_iter()
            .map(|position| (position.token_id.clone(), position))
            .collect(),
        fills: Vec::new(),
        events: Vec::new(),
        trace_id: trace_id.to_string(),
        seq: 0,
        filled_orders: 0,
        placed_orders: 0,
        cancelled_orders: 0,
        risk_cancelled_orders: 0,
        reward_accrued: Decimal::ZERO,
    };

    let plan_index: HashMap<String, RewardQuotePlan> = plans
        .iter()
        .map(|plan| (plan.condition_id.clone(), plan.clone()))
        .collect();

    let elapsed = elapsed_seconds.clamp(1, 86_400);

    ctx.release_legacy_buy_reserves();
    ctx.reconcile_open_orders(&plan_index, books, book_history);
    ctx.accrue_rewards(&plan_index, books, elapsed);
    ctx.place_new_quotes(&plans, books);

    ctx.account.tick_index += 1;
    ctx.account.updated_at = now;

    let report = RewardBotRunReport {
        markets_scanned: markets.len(),
        books_fetched: books.len(),
        plans_built: plans.len(),
        eligible_plans,
        simulated_orders: ctx.placed_orders,
        cancelled_orders: ctx.cancelled_orders,
        filled_orders: ctx.filled_orders,
        risk_cancelled_orders: ctx.risk_cancelled_orders,
        reward_accrued: ctx.reward_accrued,
    };

    RewardSimulationOutcome {
        account: ctx.account,
        markets: markets.to_vec(),
        plans,
        orders: ctx.orders,
        positions: ctx.positions.into_values().collect(),
        fills: ctx.fills,
        events: ctx.events,
        report,
    }
}

/// Run a fast reconcile tick — reuses the supplied `plans` instead of
/// rebuilding them. Used by the high-frequency reconcile loop (every N
/// seconds) between full cycles.
#[must_use]
#[allow(dead_code)]
pub fn run_reconcile_tick(
    config: &RewardBotConfig,
    account: RewardAccountState,
    open_orders: Vec<ManagedRewardOrder>,
    positions: Vec<RewardPosition>,
    plans: Vec<RewardQuotePlan>,
    markets: Vec<RewardMarket>,
    books: &HashMap<String, RewardOrderBook>,
    book_history: &HashMap<String, VecDeque<BookSnapshot>>,
    elapsed_seconds: i64,
    trace_id: &str,
) -> RewardSimulationOutcome {
    let now = OffsetDateTime::now_utc();
    let eligible_plans = plans.iter().filter(|plan| plan.eligible).count();

    let mut ctx = TickContext {
        now,
        config: config.clone(),
        account,
        orders: open_orders,
        positions: positions
            .into_iter()
            .map(|p| (p.token_id.clone(), p))
            .collect(),
        fills: Vec::new(),
        events: Vec::new(),
        trace_id: trace_id.to_string(),
        seq: 0,
        filled_orders: 0,
        placed_orders: 0,
        cancelled_orders: 0,
        risk_cancelled_orders: 0,
        reward_accrued: Decimal::ZERO,
    };

    let plan_index: HashMap<String, RewardQuotePlan> = plans
        .iter()
        .map(|plan| (plan.condition_id.clone(), plan.clone()))
        .collect();

    let elapsed = elapsed_seconds.clamp(1, 86_400);

    ctx.release_legacy_buy_reserves();
    ctx.reconcile_open_orders(&plan_index, books, book_history);
    ctx.accrue_rewards(&plan_index, books, elapsed);
    if ctx.config.enabled {
        ctx.place_new_quotes(&plans, books);
    }

    ctx.account.tick_index += 1;
    ctx.account.updated_at = now;

    let report = RewardBotRunReport {
        markets_scanned: markets.len(),
        books_fetched: books.len(),
        plans_built: plans.len(),
        eligible_plans,
        simulated_orders: ctx.placed_orders,
        cancelled_orders: ctx.cancelled_orders,
        filled_orders: ctx.filled_orders,
        risk_cancelled_orders: ctx.risk_cancelled_orders,
        reward_accrued: ctx.reward_accrued,
    };

    RewardSimulationOutcome {
        account: ctx.account,
        markets,
        plans,
        orders: ctx.orders,
        positions: ctx.positions.into_values().collect(),
        fills: ctx.fills,
        events: ctx.events,
        report,
    }
}

include!("engine/reconcile.rs");
include!("engine/fills.rs");
include!("engine/quoting.rs");
include!("engine/rewards_calc.rs");
include!("engine/state.rs");
include!("engine/risk_checks.rs");
