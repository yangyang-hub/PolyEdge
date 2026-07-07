const REWARD_MARKET_MAKER_DECISION_HASH_VERSION: &str = "reward_market_maker_decision_v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RewardMarketMakerDecisionType {
    Quote,
    Skip,
    Cancel,
    Hold,
    Exit,
    Merge,
}

impl RewardMarketMakerDecisionType {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Quote => "quote",
            Self::Skip => "skip",
            Self::Cancel => "cancel",
            Self::Hold => "hold",
            Self::Exit => "exit",
            Self::Merge => "merge",
        }
    }
}

impl FromStr for RewardMarketMakerDecisionType {
    type Err = AppError;

    fn from_str(value: &str) -> Result<Self> {
        match value {
            "quote" => Ok(Self::Quote),
            "skip" => Ok(Self::Skip),
            "cancel" => Ok(Self::Cancel),
            "hold" => Ok(Self::Hold),
            "exit" => Ok(Self::Exit),
            "merge" => Ok(Self::Merge),
            other => Err(AppError::invalid_input(
                "REWARD_MARKET_MAKER_DECISION_TYPE_INVALID",
                format!("unknown reward market maker decision type: {other}"),
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RewardMarketMakerDecisionStatus {
    Allowed,
    Blocked,
    ShadowAllowed,
    ShadowBlocked,
}

impl RewardMarketMakerDecisionStatus {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Allowed => "allowed",
            Self::Blocked => "blocked",
            Self::ShadowAllowed => "shadow_allowed",
            Self::ShadowBlocked => "shadow_blocked",
        }
    }

    #[must_use]
    pub const fn is_allowed(self) -> bool {
        matches!(self, Self::Allowed | Self::ShadowAllowed)
    }
}

impl FromStr for RewardMarketMakerDecisionStatus {
    type Err = AppError;

    fn from_str(value: &str) -> Result<Self> {
        match value {
            "allowed" => Ok(Self::Allowed),
            "blocked" => Ok(Self::Blocked),
            "shadow_allowed" => Ok(Self::ShadowAllowed),
            "shadow_blocked" => Ok(Self::ShadowBlocked),
            other => Err(AppError::invalid_input(
                "REWARD_MARKET_MAKER_DECISION_STATUS_INVALID",
                format!("unknown reward market maker decision status: {other}"),
            )),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RewardMarketMakerFairValue {
    pub id: i64,
    pub condition_id: String,
    pub token_id: String,
    pub fair_yes_low: Decimal,
    pub fair_yes_mid: Decimal,
    pub fair_yes_high: Decimal,
    pub market_implied: Decimal,
    pub base_rate: Decimal,
    pub confidence: Decimal,
    pub uncertainty_cents: Decimal,
    pub sample_count: u64,
    pub bucket_key: String,
    pub fallback_level: u8,
    pub model_version: String,
    pub input_hash: String,
    pub reason_codes: Vec<String>,
    pub live_eligible: bool,
    #[serde(with = "time::serde::rfc3339")]
    pub computed_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub expires_at: OffsetDateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RewardMarketMakerDecision {
    pub id: String,
    pub run_id: String,
    pub account_id: String,
    pub condition_id: String,
    pub token_id: String,
    pub outcome: String,
    pub side: RewardOrderSide,
    pub strategy_mode: RewardStrategyMode,
    pub decision_type: RewardMarketMakerDecisionType,
    pub decision_status: RewardMarketMakerDecisionStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_price: Option<Decimal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_size: Option<Decimal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_notional_usd: Option<Decimal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fair_value_id: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reward_ev_id: Option<i64>,
    pub pricing_edge_cents: Decimal,
    pub reward_ev_cents: Decimal,
    pub exit_cost_cents: Decimal,
    pub adverse_selection_cost_cents: Decimal,
    pub inventory_penalty_cents: Decimal,
    pub uncertainty_buffer_cents: Decimal,
    pub total_ev_cents: Decimal,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_profitable_bid: Option<Decimal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fair_value: Option<RewardMarketMakerFairValue>,
    pub reason_codes: Vec<String>,
    pub inputs_hash: String,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RewardMarketMakerPlanMetrics {
    pub strategy_mode: RewardStrategyMode,
    pub decision_status: RewardMarketMakerDecisionStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub best_total_ev_cents: Option<Decimal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub best_pricing_edge_cents: Option<Decimal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub best_reward_ev_cents: Option<Decimal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fair_value: Option<RewardMarketMakerFairValue>,
    #[serde(default)]
    pub decisions: Vec<RewardMarketMakerDecision>,
    #[serde(default)]
    pub reason_codes: Vec<String>,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
}

#[must_use]
pub fn apply_reward_market_maker_decisions_to_quote_plans(
    plans: &mut [RewardQuotePlan],
    markets: &[RewardMarket],
    fair_values: &[RewardMarketMakerFairValue],
    open_orders: &[ManagedRewardOrder],
    positions: &[RewardPosition],
    config: &RewardBotConfig,
    run_id: &str,
    now: OffsetDateTime,
) -> Vec<RewardMarketMakerDecision> {
    if !config.strategy_mode.market_maker_enabled() || !config.market_maker_enabled {
        for plan in plans {
            plan.market_maker = None;
        }
        return Vec::new();
    }

    let account_id = config.account_id.clone();
    let fair_values_by_condition = fair_values
        .iter()
        .map(|value| (value.condition_id.as_str(), value))
        .collect::<HashMap<_, _>>();
    let market_categories = markets
        .iter()
        .map(|market| (market.condition_id.as_str(), market.category.as_str()))
        .collect::<HashMap<_, _>>();
    let inventory = RewardMarketMakerInventory::new(markets, positions);
    let mut all_decisions = Vec::new();

    for plan in plans {
        let condition_inventory_usd = inventory.condition_usd(&plan.condition_id);
        let category = market_categories
            .get(plan.condition_id.as_str())
            .copied()
            .unwrap_or_default();
        let category_inventory_usd = inventory.category_usd(category);
        let global_inventory_usd = inventory.global_usd;
        let fair_value = fair_values_by_condition
            .get(plan.condition_id.as_str())
            .copied()
            .cloned();
        let mut decisions = plan
            .legs
            .iter()
            .filter(|leg| leg.side == RewardOrderSide::Buy)
            .map(|leg| {
                build_reward_market_maker_decision_for_leg(
                    plan,
                    leg,
                    fair_value.clone(),
                    open_orders,
                    config,
                    run_id,
                    &account_id,
                    condition_inventory_usd,
                    category_inventory_usd,
                    global_inventory_usd,
                    now,
                )
            })
            .collect::<Vec<_>>();

        if decisions.is_empty() {
            plan.market_maker = Some(RewardMarketMakerPlanMetrics {
                strategy_mode: config.strategy_mode,
                decision_status: blocked_status(config),
                best_total_ev_cents: None,
                best_pricing_edge_cents: None,
                best_reward_ev_cents: None,
                fair_value,
                decisions: Vec::new(),
                reason_codes: vec!["no_buy_quote_legs".to_string()],
                created_at: now,
            });
            if config.strategy_mode.market_maker_guarded() {
                block_market_maker_plan(plan, "market maker blocked: no BUY quote legs", now);
            }
            continue;
        }

        if config.strategy_mode.market_maker_guarded() {
            apply_market_maker_guarded_plan(plan, &decisions, now);
        }

        decisions.sort_by(|left, right| {
            right
                .total_ev_cents
                .cmp(&left.total_ev_cents)
                .then_with(|| right.pricing_edge_cents.cmp(&left.pricing_edge_cents))
        });
        let best = decisions.first().cloned();
        let allowed = decisions
            .iter()
            .any(|decision| decision.decision_status.is_allowed());
        let decision_status = if allowed {
            allowed_status(config)
        } else {
            blocked_status(config)
        };
        let reason_codes = if allowed {
            Vec::new()
        } else {
            decisions
                .iter()
                .flat_map(|decision| decision.reason_codes.iter().cloned())
                .collect::<Vec<_>>()
        };

        plan.market_maker = Some(RewardMarketMakerPlanMetrics {
            strategy_mode: config.strategy_mode,
            decision_status,
            best_total_ev_cents: best.as_ref().map(|decision| decision.total_ev_cents),
            best_pricing_edge_cents: best.as_ref().map(|decision| decision.pricing_edge_cents),
            best_reward_ev_cents: best.as_ref().map(|decision| decision.reward_ev_cents),
            fair_value,
            decisions: decisions.clone(),
            reason_codes,
            created_at: now,
        });
        all_decisions.extend(decisions);
    }

    all_decisions
}

#[allow(clippy::too_many_arguments)]
fn build_reward_market_maker_decision_for_leg(
    plan: &RewardQuotePlan,
    leg: &RewardQuoteLeg,
    fair_value: Option<RewardMarketMakerFairValue>,
    open_orders: &[ManagedRewardOrder],
    config: &RewardBotConfig,
    run_id: &str,
    account_id: &str,
    condition_inventory_usd: Decimal,
    category_inventory_usd: Decimal,
    global_inventory_usd: Decimal,
    now: OffsetDateTime,
) -> RewardMarketMakerDecision {
    let mut reason_codes = Vec::new();
    let current_price = (leg.price > Decimal::ZERO).then_some(leg.price);
    if !plan.eligible {
        reason_codes.push("plan_not_eligible".to_string());
    }
    if current_price.is_none() {
        reason_codes.push("target_price_unavailable".to_string());
    }
    if open_orders.iter().any(|order| {
        order.condition_id == plan.condition_id
            && order.token_id == leg.token_id
            && order.side == RewardOrderSide::Sell
            && order.status.is_open_like()
    }) {
        reason_codes.push("sell_exit_open_for_token".to_string());
    }

    let fair_value_id = fair_value.as_ref().map(|value| value.id);
    let fair_probability = fair_value
        .as_ref()
        .and_then(|value| fair_probability_for_leg(value, leg));
    match fair_value.as_ref() {
        Some(value) => {
            if !value.live_eligible {
                reason_codes.push("fair_value_not_live_eligible".to_string());
            }
            if value.confidence < config.market_maker_min_fair_value_confidence {
                reason_codes.push("fair_value_confidence_below_threshold".to_string());
            }
            if value.uncertainty_cents > config.market_maker_max_uncertainty_cents {
                reason_codes.push("fair_value_uncertainty_above_threshold".to_string());
            }
            if value.expires_at <= now {
                reason_codes.push("fair_value_expired".to_string());
            }
        }
        None => reason_codes.push("fair_value_unavailable".to_string()),
    }

    if config.market_maker_max_condition_inventory_usd > Decimal::ZERO
        && condition_inventory_usd >= config.market_maker_max_condition_inventory_usd
    {
        reason_codes.push("condition_inventory_cap_reached".to_string());
    }
    if config.market_maker_max_category_inventory_usd > Decimal::ZERO
        && category_inventory_usd >= config.market_maker_max_category_inventory_usd
    {
        reason_codes.push("category_inventory_cap_reached".to_string());
    }
    if config.market_maker_max_global_inventory_usd > Decimal::ZERO
        && global_inventory_usd >= config.market_maker_max_global_inventory_usd
    {
        reason_codes.push("global_inventory_cap_reached".to_string());
    }

    let mut target_price = current_price;
    let mut target_size = (leg.size > Decimal::ZERO).then_some(leg.size);
    let mut target_notional = (leg.notional_usd > Decimal::ZERO).then_some(leg.notional_usd);
    let exit_cost_cents = plan
        .opportunity_metrics
        .as_ref()
        .and_then(|metrics| metrics.exit_slippage_cents)
        .unwrap_or_else(|| config.opportunity_max_entry_exit_slippage_cents)
        .round_dp(4);
    let adverse_selection_cost_cents = adverse_selection_cost_cents(plan, config);
    let inventory_penalty_cents = inventory_penalty_cents(condition_inventory_usd, config);
    let uncertainty_buffer_cents = fair_value
        .as_ref()
        .map_or(Decimal::ZERO, |value| value.uncertainty_cents)
        .round_dp(4);

    let mut reward_ev_cents = Decimal::ZERO;
    let mut pricing_edge_cents = Decimal::ZERO;
    let mut total_ev_cents = Decimal::ZERO;
    let mut max_profitable_bid = None;

    if let (Some(fair_probability), Some(price)) = (fair_probability, target_price) {
        reward_ev_cents = reward_ev_cents_for_leg(plan, price).round_dp(4);
        let max_bid = max_profitable_bid_for_leg(
            fair_probability,
            reward_ev_cents,
            exit_cost_cents,
            adverse_selection_cost_cents,
            inventory_penalty_cents,
            uncertainty_buffer_cents,
            config.market_maker_min_total_ev_cents,
        )
        .min(max_pricing_edge_bid_for_leg(
            fair_probability,
            reward_ev_cents,
            config,
        ));
        max_profitable_bid = Some(max_bid);
        if price > max_bid {
            let repriced = floor_to_tick(max_bid, DEFAULT_TICK);
            if repriced > Decimal::ZERO && repriced < price {
                target_price = Some(repriced);
                target_size = Some(minimum_live_quote_size(repriced, plan.rewards_min_size));
                target_notional = target_size.map(|size| (size * repriced).round_dp(4));
            }
        }
        let effective_price = target_price.unwrap_or(price);
        reward_ev_cents = reward_ev_cents_for_leg(plan, effective_price).round_dp(4);
        pricing_edge_cents = ((fair_probability - effective_price) * decimal("100")).round_dp(4);
        total_ev_cents = (pricing_edge_cents + reward_ev_cents
            - exit_cost_cents
            - adverse_selection_cost_cents
            - inventory_penalty_cents
            - uncertainty_buffer_cents)
            .round_dp(4);

        let pricing_edge_floor_cents = pricing_edge_floor_cents(reward_ev_cents, config);
        if pricing_edge_cents < pricing_edge_floor_cents {
            if pricing_edge_floor_cents < config.market_maker_min_pricing_edge_cents {
                reason_codes.push("pricing_edge_below_subsidy_floor".to_string());
            } else {
                reason_codes.push("pricing_edge_below_threshold".to_string());
            }
        }
        if config.market_maker_min_reward_ev_cents > Decimal::ZERO
            && reward_ev_cents < config.market_maker_min_reward_ev_cents
        {
            reason_codes.push("reward_ev_below_threshold".to_string());
        }
        if total_ev_cents < config.market_maker_min_total_ev_cents {
            reason_codes.push("total_ev_below_threshold".to_string());
        }
        if effective_price > max_bid {
            reason_codes.push("target_price_above_max_profitable_bid".to_string());
        }
    }

    if target_price.is_some_and(|price| price <= Decimal::ZERO) {
        reason_codes.push("target_price_unprofitable".to_string());
    }
    if target_size.is_none_or(|size| size < plan.rewards_min_size) {
        reason_codes.push("target_size_below_rewards_minimum".to_string());
    }

    reason_codes.sort();
    reason_codes.dedup();
    let allowed = reason_codes.is_empty();
    let decision_status = if allowed {
        allowed_status(config)
    } else {
        blocked_status(config)
    };
    let decision_type = if allowed {
        RewardMarketMakerDecisionType::Quote
    } else {
        RewardMarketMakerDecisionType::Skip
    };
    let inputs_hash = reward_market_maker_decision_hash(
        run_id,
        account_id,
        plan,
        leg,
        target_price,
        fair_value_id,
        pricing_edge_cents,
        reward_ev_cents,
        total_ev_cents,
        &reason_codes,
    );
    let id = reward_market_maker_decision_id(run_id, &inputs_hash);

    RewardMarketMakerDecision {
        id,
        run_id: run_id.to_string(),
        account_id: account_id.to_string(),
        condition_id: plan.condition_id.clone(),
        token_id: leg.token_id.clone(),
        outcome: leg.outcome.clone(),
        side: leg.side,
        strategy_mode: config.strategy_mode,
        decision_type,
        decision_status,
        target_price,
        target_size,
        target_notional_usd: target_notional,
        fair_value_id,
        reward_ev_id: None,
        pricing_edge_cents,
        reward_ev_cents,
        exit_cost_cents,
        adverse_selection_cost_cents,
        inventory_penalty_cents,
        uncertainty_buffer_cents,
        total_ev_cents,
        max_profitable_bid,
        fair_value,
        reason_codes,
        inputs_hash,
        created_at: now,
    }
}

fn apply_market_maker_guarded_plan(
    plan: &mut RewardQuotePlan,
    decisions: &[RewardMarketMakerDecision],
    now: OffsetDateTime,
) {
    let allowed_by_token = decisions
        .iter()
        .filter(|decision| decision.decision_status.is_allowed())
        .map(|decision| (decision.token_id.as_str(), decision))
        .collect::<HashMap<_, _>>();
    if allowed_by_token.is_empty() {
        let reason = decisions
            .iter()
            .flat_map(|decision| decision.reason_codes.iter())
            .next()
            .map_or("market maker blocked: no positive-EV quote".to_string(), |reason| {
                format!("market maker blocked: {reason}")
            });
        block_market_maker_plan(plan, reason, now);
        return;
    }

    plan.legs = plan
        .legs
        .iter()
        .filter_map(|leg| {
            let decision = allowed_by_token.get(leg.token_id.as_str())?;
            let mut leg = leg.clone();
            if let Some(price) = decision.target_price {
                leg.price = price;
            }
            if let Some(size) = decision.target_size {
                leg.size = size;
            }
            leg.notional_usd = decision
                .target_notional_usd
                .unwrap_or_else(|| (leg.price * leg.size).round_dp(4));
            Some(leg)
        })
        .collect();
    plan.quote_mode = quote_mode_for_market_maker_legs(&plan.legs);
    if plan.quote_mode == RewardPlanQuoteMode::None {
        block_market_maker_plan(plan, "market maker blocked: no positive-EV quote", now);
    } else {
        plan.eligible = true;
        plan.pre_ai_eligible = true;
        plan.reason = format!(
            "market maker guarded eligible for {} quotes",
            plan.quote_mode.as_str()
        );
        plan.updated_at = now;
        refresh_reward_quote_plan_readiness(plan);
    }
}

fn block_market_maker_plan(plan: &mut RewardQuotePlan, reason: impl Into<String>, now: OffsetDateTime) {
    plan.eligible = false;
    plan.pre_ai_eligible = false;
    plan.quote_mode = RewardPlanQuoteMode::None;
    plan.legs.clear();
    plan.reason = reason.into();
    plan.updated_at = now;
    refresh_reward_quote_plan_readiness(plan);
}

fn quote_mode_for_market_maker_legs(legs: &[RewardQuoteLeg]) -> RewardPlanQuoteMode {
    let has_yes = legs
        .iter()
        .any(|leg| leg.outcome.trim().eq_ignore_ascii_case("yes"));
    let has_no = legs
        .iter()
        .any(|leg| leg.outcome.trim().eq_ignore_ascii_case("no"));
    match (has_yes, has_no) {
        (true, true) => RewardPlanQuoteMode::Double,
        (true, false) => RewardPlanQuoteMode::SingleYes,
        (false, true) => RewardPlanQuoteMode::SingleNo,
        (false, false) => RewardPlanQuoteMode::None,
    }
}

fn fair_probability_for_leg(
    fair_value: &RewardMarketMakerFairValue,
    leg: &RewardQuoteLeg,
) -> Option<Decimal> {
    let outcome = leg.outcome.trim();
    if outcome.eq_ignore_ascii_case("yes") {
        Some(fair_value.fair_yes_low)
    } else if outcome.eq_ignore_ascii_case("no") {
        Some(Decimal::ONE - fair_value.fair_yes_high)
    } else {
        None
    }
}

fn reward_ev_cents_for_leg(plan: &RewardQuotePlan, price: Decimal) -> Decimal {
    let Some(metrics) = &plan.opportunity_metrics else {
        return Decimal::ZERO;
    };
    (metrics.estimated_reward_per_100_usd_day * price).max(Decimal::ZERO)
}

fn adverse_selection_cost_cents(plan: &RewardQuotePlan, config: &RewardBotConfig) -> Decimal {
    let Some(metrics) = &plan.opportunity_metrics else {
        return config.market_maker_max_uncertainty_cents;
    };
    let midpoint_component = metrics.midpoint_range_cents.unwrap_or_else(|| {
        config
            .opportunity_max_midpoint_range_cents
            .max(config.market_maker_max_uncertainty_cents)
    }) / decimal("2");
    let flip_component = metrics
        .top_of_book_flip_count
        .map(|flips| {
            if config.opportunity_max_top_of_book_flip_count == 0 {
                Decimal::ZERO
            } else {
                Decimal::from(flips) / Decimal::from(config.opportunity_max_top_of_book_flip_count)
            }
        })
        .unwrap_or(Decimal::ONE)
        .min(Decimal::ONE);
    (midpoint_component + flip_component).round_dp(4)
}

fn inventory_penalty_cents(condition_inventory_usd: Decimal, config: &RewardBotConfig) -> Decimal {
    if condition_inventory_usd <= Decimal::ZERO
        || config.market_maker_inventory_skew_cents_per_10_usd <= Decimal::ZERO
    {
        return Decimal::ZERO;
    }
    (condition_inventory_usd / decimal("10") * config.market_maker_inventory_skew_cents_per_10_usd)
        .round_dp(4)
}

fn max_profitable_bid_for_leg(
    fair_probability: Decimal,
    reward_ev_cents: Decimal,
    exit_cost_cents: Decimal,
    adverse_selection_cost_cents: Decimal,
    inventory_penalty_cents: Decimal,
    uncertainty_buffer_cents: Decimal,
    min_total_ev_cents: Decimal,
) -> Decimal {
    (fair_probability
        + (reward_ev_cents
            - exit_cost_cents
            - adverse_selection_cost_cents
            - inventory_penalty_cents
            - uncertainty_buffer_cents
            - min_total_ev_cents)
            / decimal("100"))
    .max(Decimal::ZERO)
    .min(Decimal::ONE)
    .round_dp(4)
}

fn max_pricing_edge_bid_for_leg(
    fair_probability: Decimal,
    reward_ev_cents: Decimal,
    config: &RewardBotConfig,
) -> Decimal {
    (fair_probability - pricing_edge_floor_cents(reward_ev_cents, config) / decimal("100"))
        .max(Decimal::ZERO)
        .min(Decimal::ONE)
        .round_dp(4)
}

fn pricing_edge_floor_cents(reward_ev_cents: Decimal, config: &RewardBotConfig) -> Decimal {
    if config.strategy_mode.market_maker_guarded() {
        return config
            .market_maker_min_pricing_edge_cents
            .max(Decimal::ZERO);
    }

    if reward_ev_cents > Decimal::ZERO
        && config.market_maker_max_reward_subsidized_negative_edge_cents > Decimal::ZERO
    {
        -config.market_maker_max_reward_subsidized_negative_edge_cents
    } else {
        config.market_maker_min_pricing_edge_cents
    }
}

struct RewardMarketMakerInventory {
    by_condition: HashMap<String, Decimal>,
    by_category: HashMap<String, Decimal>,
    global_usd: Decimal,
}

impl RewardMarketMakerInventory {
    fn new(markets: &[RewardMarket], positions: &[RewardPosition]) -> Self {
        let category_by_condition = markets
            .iter()
            .map(|market| {
                (
                    market.condition_id.as_str(),
                    market.category.trim().to_ascii_lowercase(),
                )
            })
            .collect::<HashMap<_, _>>();
        let mut by_condition: HashMap<String, Decimal> = HashMap::new();
        let mut by_category: HashMap<String, Decimal> = HashMap::new();
        let mut global_usd = Decimal::ZERO;

        for position in positions.iter().filter(|position| position.size > Decimal::ZERO) {
            let notional = (position.size * position.avg_price).max(Decimal::ZERO);
            *by_condition
                .entry(position.condition_id.clone())
                .or_insert(Decimal::ZERO) += notional;
            if let Some(category) = category_by_condition.get(position.condition_id.as_str()) {
                *by_category.entry(category.clone()).or_insert(Decimal::ZERO) += notional;
            }
            global_usd += notional;
        }

        Self {
            by_condition,
            by_category,
            global_usd: global_usd.round_dp(4),
        }
    }

    fn condition_usd(&self, condition_id: &str) -> Decimal {
        self.by_condition
            .get(condition_id)
            .copied()
            .unwrap_or_default()
            .round_dp(4)
    }

    fn category_usd(&self, category: &str) -> Decimal {
        self.by_category
            .get(&category.trim().to_ascii_lowercase())
            .copied()
            .unwrap_or_default()
            .round_dp(4)
    }
}

fn allowed_status(config: &RewardBotConfig) -> RewardMarketMakerDecisionStatus {
    if config.strategy_mode.market_maker_guarded() {
        RewardMarketMakerDecisionStatus::Allowed
    } else {
        RewardMarketMakerDecisionStatus::ShadowAllowed
    }
}

fn blocked_status(config: &RewardBotConfig) -> RewardMarketMakerDecisionStatus {
    if config.strategy_mode.market_maker_guarded() {
        RewardMarketMakerDecisionStatus::Blocked
    } else {
        RewardMarketMakerDecisionStatus::ShadowBlocked
    }
}

#[allow(clippy::too_many_arguments)]
fn reward_market_maker_decision_hash(
    run_id: &str,
    account_id: &str,
    plan: &RewardQuotePlan,
    leg: &RewardQuoteLeg,
    target_price: Option<Decimal>,
    fair_value_id: Option<i64>,
    pricing_edge_cents: Decimal,
    reward_ev_cents: Decimal,
    total_ev_cents: Decimal,
    reason_codes: &[String],
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(REWARD_MARKET_MAKER_DECISION_HASH_VERSION.as_bytes());
    hasher.update([0]);
    hasher.update(account_id.trim().as_bytes());
    hasher.update([0]);
    hasher.update(plan.condition_id.trim().as_bytes());
    hasher.update([0]);
    hasher.update(leg.token_id.trim().as_bytes());
    hasher.update([0]);
    hasher.update(run_id.trim().as_bytes());
    hasher.update([0]);
    hasher.update(format!("{target_price:?}").as_bytes());
    hasher.update([0]);
    hasher.update(format!("{fair_value_id:?}").as_bytes());
    hasher.update([0]);
    hasher.update(pricing_edge_cents.normalize().to_string().as_bytes());
    hasher.update([0]);
    hasher.update(reward_ev_cents.normalize().to_string().as_bytes());
    hasher.update([0]);
    hasher.update(total_ev_cents.normalize().to_string().as_bytes());
    for reason in reason_codes {
        hasher.update([0]);
        hasher.update(reason.as_bytes());
    }
    hex_digest(hasher.finalize().as_slice())
}

fn reward_market_maker_decision_id(run_id: &str, inputs_hash: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(run_id.trim().as_bytes());
    hasher.update([0]);
    hasher.update(inputs_hash.as_bytes());
    let hash = hex_digest(hasher.finalize().as_slice());
    format!("mm_{}", &hash[..32])
}

fn hex_digest(bytes: &[u8]) -> String {
    let mut hex = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        hex.push_str(&format!("{byte:02x}"));
    }
    hex
}
