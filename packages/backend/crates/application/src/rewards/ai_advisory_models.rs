#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RewardAiSuitability {
    Allow,
    Watch,
    Avoid,
}

impl RewardAiSuitability {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Allow => "allow",
            Self::Watch => "watch",
            Self::Avoid => "avoid",
        }
    }
}

impl FromStr for RewardAiSuitability {
    type Err = AppError;

    fn from_str(value: &str) -> Result<Self> {
        match value {
            "allow" => Ok(Self::Allow),
            "watch" => Ok(Self::Watch),
            "avoid" => Ok(Self::Avoid),
            other => Err(AppError::invalid_input(
                "REWARD_AI_SUITABILITY_INVALID",
                format!("unknown reward AI suitability: {other}"),
            )),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RewardMarketAdvisory {
    pub condition_id: String,
    pub provider: RewardAiProvider,
    pub request_format: RewardAiRequestFormat,
    pub model: String,
    pub input_hash: String,
    pub suitability: RewardAiSuitability,
    pub quote_mode: RewardPlanQuoteMode,
    pub exit_policy: PostFillStrategy,
    pub confidence: Decimal,
    pub reasons: Vec<String>,
    pub metrics: Value,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub expires_at: OffsetDateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RewardAiAdvisoryDecision {
    pub suitability: RewardAiSuitability,
    pub quote_mode: RewardPlanQuoteMode,
    pub exit_policy: PostFillStrategy,
    pub confidence: Decimal,
    pub reasons: Vec<String>,
    pub metrics: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RewardAiAdvisoryRequest {
    pub condition_id: String,
    pub provider: RewardAiProvider,
    pub request_format: RewardAiRequestFormat,
    pub model: String,
    pub input_hash: String,
    pub payload: Value,
}

impl RewardAiAdvisoryDecision {
    #[must_use]
    pub fn into_advisory(
        self,
        request: &RewardAiAdvisoryRequest,
        ttl_sec: u64,
        now: OffsetDateTime,
    ) -> RewardMarketAdvisory {
        RewardMarketAdvisory {
            condition_id: request.condition_id.clone(),
            provider: request.provider,
            request_format: request.request_format,
            model: request.model.clone(),
            input_hash: request.input_hash.clone(),
            suitability: self.suitability,
            quote_mode: self.quote_mode,
            exit_policy: self.exit_policy,
            confidence: self.confidence,
            reasons: self.reasons,
            metrics: self.metrics,
            created_at: now,
            expires_at: now + TimeDuration::seconds(ttl_sec as i64),
        }
    }
}

pub fn build_reward_ai_advisory_request(
    market: &RewardMarket,
    plan: &RewardQuotePlan,
    account: &RewardAccountState,
    positions: &[RewardPosition],
    open_orders: &[ManagedRewardOrder],
    books: &HashMap<String, RewardOrderBook>,
    config: &RewardBotConfig,
    provider: RewardAiProvider,
    request_format: RewardAiRequestFormat,
    model: &str,
) -> Result<RewardAiAdvisoryRequest> {
    let market_positions = positions
        .iter()
        .filter(|position| position.condition_id == market.condition_id)
        .collect::<Vec<_>>();
    let market_open_orders = open_orders
        .iter()
        .filter(|order| order.condition_id == market.condition_id)
        .collect::<Vec<_>>();
    let book_payload = market
        .tokens
        .iter()
        .map(|token| {
            let book = books.get(&token.token_id);
            json!({
                "token_id": token.token_id,
                "outcome": token.outcome,
                "price": token.price,
                "bids": book.map(|book| top_reward_book_levels(&book.bids)),
                "asks": book.map(|book| top_reward_book_levels(&book.asks)),
            })
        })
        .collect::<Vec<_>>();
    let payload = json!({
        "schema_version": 1,
        "market": {
            "condition_id": market.condition_id,
            "question": market.question,
            "market_slug": market.market_slug,
            "category": market.category,
            "total_daily_rate": market.total_daily_rate,
            "liquidity_usd": market.liquidity_usd,
            "volume_24h_usd": market.volume_24h_usd,
            "market_spread_cents": market.market_spread_cents,
            "rewards_max_spread": market.rewards_max_spread,
            "rewards_min_size": market.rewards_min_size,
            "end_at": market.end_at,
        },
        "deterministic_plan": {
            "eligible": plan.eligible,
            "reason": plan.reason,
            "score": plan.score,
            "quote_mode": plan.quote_mode,
            "recommended_quote_mode": plan.recommended_quote_mode,
            "midpoint": plan.midpoint,
            "legs": plan.legs,
            "book_metrics": plan.book_metrics,
        },
        "account": {
            "account_id": account.account_id,
            "available_usd": account.available_usd,
            "external_buy_notional": account.external_buy_notional,
            "positions": market_positions,
            "open_orders": market_open_orders,
        },
        "strategy_config": {
            "quote_bid_rank": config.quote_bid_rank,
            "quote_mode": config.quote_mode,
            "selection_mode": config.selection_mode,
            "post_fill_strategy": config.post_fill_strategy,
            "max_spread_cents": config.max_spread_cents,
            "min_market_score": config.min_market_score,
            "max_position_usd": config.max_position_usd,
            "max_global_position_usd": config.max_global_position_usd,
        },
        "books": book_payload,
    });
    Ok(RewardAiAdvisoryRequest {
        condition_id: market.condition_id.clone(),
        provider,
        request_format,
        model: model.trim().to_string(),
        input_hash: reward_ai_input_hash(&payload)?,
        payload,
    })
}

pub fn apply_reward_ai_advisories(
    plans: &mut [RewardQuotePlan],
    advisories: &HashMap<String, RewardMarketAdvisory>,
    config: &RewardBotConfig,
    min_confidence: Decimal,
) {
    if !config.ai_advisory_enabled {
        return;
    }

    for plan in plans {
        let Some(advisory) = advisories.get(&plan.condition_id).cloned() else {
            reject_ai_gated_plan(
                plan,
                "AI advisory pending: market has not passed provider filter",
            );
            continue;
        };
        plan.ai_advisory = Some(advisory.clone());
        if advisory.confidence < min_confidence {
            reject_ai_gated_plan(
                plan,
                &format!(
                    "AI advisory confidence {} below required {}",
                    advisory.confidence, min_confidence
                ),
            );
            continue;
        }
        enforce_reward_ai_advisory(plan, &advisory, config);
    }
}

fn reject_ai_gated_plan(plan: &mut RewardQuotePlan, reason: &str) {
    if !plan.eligible {
        return;
    }
    plan.eligible = false;
    plan.quote_mode = RewardPlanQuoteMode::None;
    plan.legs.clear();
    plan.reason = reason.to_string();
}

fn enforce_reward_ai_advisory(
    plan: &mut RewardQuotePlan,
    advisory: &RewardMarketAdvisory,
    config: &RewardBotConfig,
) {
    if advisory.suitability != RewardAiSuitability::Allow
        || advisory.quote_mode == RewardPlanQuoteMode::None
    {
        plan.eligible = false;
        plan.quote_mode = RewardPlanQuoteMode::None;
        plan.legs.clear();
        plan.reason = format!(
            "AI advisory {}: {}",
            advisory.suitability.as_str(),
            advisory
                .reasons
                .first()
                .cloned()
                .unwrap_or_else(|| "advisory rejected this market".to_string())
        );
        return;
    }

    if !plan.eligible
        || config.selection_mode != RewardSelectionMode::Enforce
        || config.quote_mode != RewardQuoteMode::Auto
    {
        return;
    }

    match advisory.quote_mode {
        RewardPlanQuoteMode::SingleYes => keep_single_ai_leg(plan, "yes"),
        RewardPlanQuoteMode::SingleNo => keep_single_ai_leg(plan, "no"),
        RewardPlanQuoteMode::Double | RewardPlanQuoteMode::None => {}
    }
}

fn keep_single_ai_leg(plan: &mut RewardQuotePlan, outcome: &str) {
    if plan.legs.len() < 2 {
        return;
    }
    plan.legs
        .retain(|leg| leg.outcome.trim().eq_ignore_ascii_case(outcome));
    if plan.legs.len() == 1 {
        plan.quote_mode = if outcome == "yes" {
            RewardPlanQuoteMode::SingleYes
        } else {
            RewardPlanQuoteMode::SingleNo
        };
        plan.reason = format!("eligible with AI-assisted {} single-side quote", outcome);
    }
}

fn top_reward_book_levels(levels: &[RewardBookLevel]) -> Vec<RewardBookLevel> {
    levels
        .iter()
        .filter(|level| level.price > Decimal::ZERO && level.size > Decimal::ZERO)
        .take(5)
        .cloned()
        .collect()
}

fn reward_ai_input_hash(payload: &Value) -> Result<String> {
    let bytes = serde_json::to_vec(payload).map_err(|error| {
        AppError::internal(
            "REWARD_AI_INPUT_HASH_FAILED",
            format!("failed to serialize reward AI advisory input: {error}"),
        )
    })?;
    let digest = Sha256::digest(bytes);
    Ok(format!("{digest:x}"))
}
