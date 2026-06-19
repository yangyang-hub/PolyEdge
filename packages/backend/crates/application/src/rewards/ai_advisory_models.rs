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
pub struct RewardAiAdvisoryBatchItem {
    pub condition_id: String,
    pub decision: RewardAiAdvisoryDecision,
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

/// Whether every token of a rewards market has a populated orderbook (at least
/// one bid and one ask). The AI advisory provider refresh uses this to defer
/// requests for markets whose books the orderbook service has not yet
/// subscribed/published, so it never caches a meaningless "no orderbook"
/// watch/avoid that would block the market for the full advisory TTL even after
/// the book arrives in a later tick.
pub fn reward_market_books_available(
    market: &RewardMarket,
    books: &HashMap<String, RewardOrderBook>,
) -> bool {
    if market.tokens.is_empty() {
        return false;
    }
    market.tokens.iter().all(|token| {
        books
            .get(&token.token_id)
            .is_some_and(|book| !book.bids.is_empty() && !book.asks.is_empty())
    })
}

pub fn build_reward_ai_advisory_request(
    market: &RewardMarket,
    plan: &RewardQuotePlan,
    account: &RewardAccountState,
    positions: &[RewardPosition],
    open_orders: &[ManagedRewardOrder],
    books: &HashMap<String, RewardOrderBook>,
    candles: &[RewardMarketCandle],
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
    let candle_payload = reward_ai_candle_payload(market, candles);
    let candle_summary = reward_ai_candle_summary(market, candles);
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
        "candles": candle_payload,
        "candle_summary": candle_summary,
    });
    Ok(RewardAiAdvisoryRequest {
        condition_id: market.condition_id.clone(),
        provider,
        request_format,
        model: model.trim().to_string(),
        input_hash: reward_ai_input_hash(&reward_ai_advisory_cache_key_payload(
            market,
            plan,
            config,
            &candle_summary,
        ))?,
        payload,
    })
}

fn reward_ai_advisory_cache_key_payload(
    market: &RewardMarket,
    plan: &RewardQuotePlan,
    config: &RewardBotConfig,
    candle_summary: &Value,
) -> Value {
    let mut tokens = market
        .tokens
        .iter()
        .map(|token| {
            json!({
                "token_id": token.token_id,
                "outcome": token.outcome,
            })
        })
        .collect::<Vec<_>>();
    tokens.sort_by_key(|token| {
        token
            .get("token_id")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string()
    });

    json!({
        // schema_version 4: reward AI advisory now receives orderbook-derived
        // midpoint candles. The cache key includes only the candle summary,
        // not the full candle array, to avoid per-tick provider churn.
        //
        // schema_version 3: provider refresh now defers requests until the
        // orderbook service has published real books, so advisories cached
        // before that change (null bids/asks "no orderbook" watch/avoid) are
        // invalidated and re-evaluated against live books.
        "schema_version": 4,
        "cache_domain": "reward_ai_advisory",
        "market": {
            "condition_id": market.condition_id,
            "question": market.question,
            "market_slug": market.market_slug,
            "category": market.category,
            "rewards_max_spread": market.rewards_max_spread,
            "rewards_min_size": market.rewards_min_size,
            "end_at": market.end_at,
            "tokens": tokens,
        },
        "deterministic_plan": {
            "quote_mode": plan.quote_mode,
            "recommended_quote_mode": plan.recommended_quote_mode,
            "strategy_bucket": plan.strategy_bucket,
        },
        "strategy_config": {
            "quote_bid_rank": config.quote_bid_rank,
            "quote_mode": config.quote_mode,
            "selection_mode": config.selection_mode,
            "post_fill_strategy": config.post_fill_strategy,
            "max_spread_cents": config.max_spread_cents,
            "min_market_score": config.min_market_score,
            "min_midpoint": config.min_midpoint,
            "max_midpoint": config.max_midpoint,
            "dominant_single_side_enabled": config.dominant_single_side_enabled,
            "dominant_min_probability": config.dominant_min_probability,
            "dominant_max_probability": config.dominant_max_probability,
        },
        "candle_summary": candle_summary,
    })
}

fn reward_ai_candle_payload(market: &RewardMarket, candles: &[RewardMarketCandle]) -> Value {
    let mut groups = Vec::new();
    for token in &market.tokens {
        let mut items = candles
            .iter()
            .filter(|candle| candle.token_id == token.token_id)
            .cloned()
            .collect::<Vec<_>>();
        items.sort_by_key(|candle| candle.bucket_start);
        groups.push(json!({
            "token_id": token.token_id,
            "outcome": token.outcome,
            "interval_sec": REWARD_AI_CANDLE_INTERVAL_SEC,
            "items": items.into_iter().map(|candle| {
                json!({
                    "bucket_start": candle.bucket_start,
                    "open": candle.open,
                    "high": candle.high,
                    "low": candle.low,
                    "close": candle.close,
                    "best_bid_close": candle.best_bid_close,
                    "best_ask_close": candle.best_ask_close,
                    "spread_cents_close": candle.spread_cents_close,
                    "sample_count": candle.sample_count,
                })
            }).collect::<Vec<_>>(),
        }));
    }
    Value::Array(groups)
}

fn reward_ai_candle_summary(market: &RewardMarket, candles: &[RewardMarketCandle]) -> Value {
    let mut token_summaries = Vec::new();
    let mut latest_bucket: Option<OffsetDateTime> = None;
    let mut total_samples = 0i64;
    let mut missing_tokens = 0usize;

    for token in &market.tokens {
        let mut items = candles
            .iter()
            .filter(|candle| candle.token_id == token.token_id)
            .collect::<Vec<_>>();
        items.sort_by_key(|candle| candle.bucket_start);
        let Some(first) = items.first().copied() else {
            missing_tokens += 1;
            token_summaries.push(json!({
                "token_id": token.token_id,
                "outcome": token.outcome,
                "sample_count": 0,
                "missing": true,
            }));
            continue;
        };
        let last = items[items.len() - 1];
        latest_bucket = Some(latest_bucket.map_or(last.bucket_start, |current| {
            current.max(last.bucket_start)
        }));
        let min_low = items
            .iter()
            .map(|candle| candle.low)
            .min()
            .unwrap_or(first.low);
        let max_high = items
            .iter()
            .map(|candle| candle.high)
            .max()
            .unwrap_or(first.high);
        let max_spread = items
            .iter()
            .map(|candle| candle.spread_cents_close)
            .max()
            .unwrap_or(Decimal::ZERO);
        let sample_count = items
            .iter()
            .map(|candle| i64::from(candle.sample_count.max(0)))
            .sum::<i64>();
        total_samples += sample_count;
        token_summaries.push(json!({
            "token_id": token.token_id,
            "outcome": token.outcome,
            "interval_sec": REWARD_AI_CANDLE_INTERVAL_SEC,
            "first_bucket_start": first.bucket_start,
            "last_bucket_start": last.bucket_start,
            "open": first.open,
            "close": last.close,
            "return_cents": ((last.close - first.open) * decimal("100")).round_dp(8),
            "range_cents": ((max_high - min_low) * decimal("100")).round_dp(8),
            "max_spread_cents": max_spread,
            "sample_count": sample_count,
            "missing": false,
        }));
    }

    json!({
        "schema_version": 1,
        "interval_sec": REWARD_AI_CANDLE_INTERVAL_SEC,
        "limit_per_token": REWARD_AI_CANDLE_LIMIT_PER_TOKEN,
        "latest_bucket_start": latest_bucket,
        "token_count": market.tokens.len(),
        "missing_token_count": missing_tokens,
        "sample_count": total_samples,
        "stale": missing_tokens > 0,
        "tokens": token_summaries,
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

pub fn apply_existing_reward_ai_advisories(
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

#[must_use]
pub fn reward_ai_advisories_from_quote_plans(
    plans: &[RewardQuotePlan],
    config: &RewardBotConfig,
    model: &str,
    now: OffsetDateTime,
) -> HashMap<String, RewardMarketAdvisory> {
    let model = model.trim();
    plans
        .iter()
        .filter_map(|plan| plan.ai_advisory.as_ref())
        .filter(|advisory| {
            advisory.expires_at > now
                && advisory.provider == config.ai_provider
                && advisory.request_format == config.ai_request_format
                && advisory.model == model
        })
        .map(|advisory| (advisory.condition_id.clone(), advisory.clone()))
        .collect()
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
    plan.quote_mode = if outcome == "yes" {
        RewardPlanQuoteMode::SingleYes
    } else {
        RewardPlanQuoteMode::SingleNo
    };
    plan.reason = format!("eligible with AI-assisted {} single-side quote", outcome);
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
