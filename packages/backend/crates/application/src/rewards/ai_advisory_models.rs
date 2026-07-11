/// Bounded slow-risk action returned by the provider. Real-time quote prices,
/// direction and order lifecycle remain deterministic.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RewardProviderAction {
    #[default]
    Allow,
    Reduce,
    StopNew,
    CancelYes,
    CancelNo,
    CancelAll,
}

impl RewardProviderAction {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Allow => "allow",
            Self::Reduce => "reduce",
            Self::StopNew => "stop_new",
            Self::CancelYes => "cancel_yes",
            Self::CancelNo => "cancel_no",
            Self::CancelAll => "cancel_all",
        }
    }

    #[must_use]
    pub const fn blocks_new_quotes(self) -> bool {
        matches!(
            self,
            Self::StopNew | Self::CancelYes | Self::CancelNo | Self::CancelAll
        )
    }

    #[must_use]
    pub fn cancels_outcome(self, outcome: &str) -> bool {
        self == Self::CancelAll
            || (self == Self::CancelYes && outcome.eq_ignore_ascii_case("yes"))
            || (self == Self::CancelNo && outcome.eq_ignore_ascii_case("no"))
    }
}

impl FromStr for RewardProviderAction {
    type Err = AppError;

    fn from_str(value: &str) -> Result<Self> {
        match value {
            "allow" => Ok(Self::Allow),
            "reduce" => Ok(Self::Reduce),
            "stop_new" => Ok(Self::StopNew),
            "cancel_yes" => Ok(Self::CancelYes),
            "cancel_no" => Ok(Self::CancelNo),
            "cancel_all" => Ok(Self::CancelAll),
            other => Err(AppError::invalid_input(
                "REWARD_PROVIDER_ACTION_INVALID",
                format!("unknown reward provider action: {other}"),
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
    #[serde(default)]
    pub action: RewardProviderAction,
    #[serde(default = "default_reward_provider_size_multiplier")]
    pub size_multiplier: Decimal,
    #[serde(default)]
    pub edge_buffer_cents: Decimal,
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
    pub action: RewardProviderAction,
    pub size_multiplier: Decimal,
    pub edge_buffer_cents: Decimal,
    pub confidence: Decimal,
    pub reasons: Vec<String>,
    pub metrics: Value,
}

fn default_reward_provider_size_multiplier() -> Decimal {
    Decimal::ONE
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
            action: self.action,
            size_multiplier: self.size_multiplier,
            edge_buffer_cents: self.edge_buffer_cents,
            confidence: self.confidence,
            reasons: self.reasons,
            metrics: self.metrics,
            created_at: now,
            expires_at: reward_provider_cache_expires_at(
                now,
                ttl_sec,
                "ai_advisory",
                &[
                    request.condition_id.as_str(),
                    request.provider.as_str(),
                    request.request_format.as_str(),
                    request.model.as_str(),
                    request.input_hash.as_str(),
                ],
            ),
        }
    }
}

pub fn build_reward_ai_advisory_request(
    market: &RewardMarket,
    _plan: &RewardQuotePlan,
    _account: &RewardAccountState,
    _positions: &[RewardPosition],
    _open_orders: &[ManagedRewardOrder],
    candles: &[RewardMarketCandle],
    _config: &RewardBotConfig,
    ttl_sec: u64,
    provider: RewardAiProvider,
    request_format: RewardAiRequestFormat,
    model: &str,
) -> Result<RewardAiAdvisoryRequest> {
    let now = OffsetDateTime::now_utc();
    let ai_candles = reward_ai_coarse_candles(candles)?;
    let completed_ai_candles = reward_ai_completed_candles(market, &ai_candles);
    let candle_payload = reward_ai_candle_payload(market, &completed_ai_candles);
    let candle_summary = reward_ai_candle_summary(market, &completed_ai_candles);
    let cache_candle_summary = reward_ai_candle_cache_summary(market, &completed_ai_candles);
    let payload = json!({
        "schema_version": 6,
        "task": "Assess slow structural market-making risk for the configured cache horizon. Do not choose live prices, sides, bid ranks, or notional.",
        "market": {
            "condition_id": market.condition_id,
            "question": market.question,
            "market_slug": market.market_slug,
            "category": market.category,
            "end_at": market.end_at,
        },
        "decision_boundary": {
            "provider_may_assess": ["structural_ambiguity", "slow_regime_risk", "historical_instability"],
            "provider_must_not_use": ["live_orderbook", "quote_price", "quote_side", "bid_rank", "account_balance", "position_size"],
        },
        "candles": candle_payload,
        "candle_summary": candle_summary,
        "provider_cache_policy": reward_provider_cache_policy_payload(ttl_sec, now),
    });
    Ok(RewardAiAdvisoryRequest {
        condition_id: market.condition_id.clone(),
        provider,
        request_format,
        model: model.trim().to_string(),
        input_hash: reward_ai_input_hash(&reward_ai_advisory_cache_key_payload(
            market,
            &cache_candle_summary,
        ))?,
        payload,
    })
}

fn reward_ai_advisory_cache_key_payload(
    market: &RewardMarket,
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
        // schema_version 16: request payload and cache key both use completed
        // hourly candle buckets only; in-progress market noise is excluded.
        //
        // schema_version 15: independently synced event/ambiguity metadata is
        // removed from the request domain so payload and cache semantics agree.
        //
        // schema_version 14: LP reward terms and current liquidity are removed;
        // the advisory assesses structural market risk, not reward economics.
        //
        // schema_version 13: this cache domain contains only structural market
        // facts and completed coarse candles. Live plans, books, balances,
        // inventory and operator pricing policy are deliberately excluded.
        "schema_version": 16,
        "cache_domain": "reward_ai_advisory",
        "provider_decision_schema": "slow_risk_action_v2",
        "market": {
            "condition_id": market.condition_id,
            "question": market.question,
            "market_slug": market.market_slug,
            "category": market.category,
            "end_at": market.end_at,
            "tokens": tokens,
        },
        "candle_summary": candle_summary,
    })
}

pub fn apply_reward_ai_advisories(
    plans: &mut [RewardQuotePlan],
    advisories: &HashMap<String, RewardMarketAdvisory>,
    config: &RewardBotConfig,
    min_confidence: Decimal,
) {
    apply_reward_ai_advisories_at(
        plans,
        advisories,
        config,
        min_confidence,
        OffsetDateTime::now_utc(),
    );
}

/// Apply an already-resolved advisory snapshot using an injected clock.
///
/// Live callers use [`apply_reward_ai_advisories`]. Replay callers must use
/// this variant so provider-pending grace decisions are reproducible.
pub fn apply_reward_ai_advisories_at(
    plans: &mut [RewardQuotePlan],
    advisories: &HashMap<String, RewardMarketAdvisory>,
    config: &RewardBotConfig,
    min_confidence: Decimal,
    now: OffsetDateTime,
) {
    if !config.ai_advisory_enabled {
        return;
    }

    let grace = TimeDuration::seconds(config.ai_advisory_provider_pending_grace_sec as i64);

    for plan in plans {
        let Some(advisory) = advisories.get(&plan.condition_id).cloned() else {
            if plan.pre_ai_eligible {
                // Pre-AI-eligible plans get a grace period before being dropped.
                // This prevents the eligible count from oscillating to 0 while
                // the background provider refresh populates the cache.
                match plan.ai_advisory_pending_since {
                    None if grace.is_zero() => {
                        // Grace disabled — immediate drop (prior behaviour).
                        reject_ai_gated_plan(
                            plan,
                            "AI advisory pending: market has not passed provider filter",
                        );
                    }
                    None => {
                        plan.ai_advisory_pending_since = Some(now);
                        // Keep eligible; reason reflects pending state.
                        plan.reason =
                            "AI advisory pending: market has not passed provider filter"
                                .to_string();
                    }
                    Some(since) if now - since >= grace => {
                        reject_ai_gated_plan(
                            plan,
                            "AI advisory pending: market has not passed provider filter",
                        );
                    }
                    Some(_) => {
                        // Within grace period — keep eligible.
                        plan.reason =
                            "AI advisory pending: market has not passed provider filter"
                                .to_string();
                    }
                }
            } else {
                // Non-pre-ai-eligible plans (e.g., active exposure) drop
                // immediately — fail-closed for safety.
                reject_ai_gated_plan(
                    plan,
                    "AI advisory pending: market has not passed provider filter",
                );
            }
            continue;
        };
        // Cached advisory available — clear any pending grace state.
        plan.ai_advisory_pending_since = None;
        plan.ai_advisory = Some(advisory.clone());
        enforce_reward_ai_advisory(plan, &advisory, config, min_confidence);
    }
}

#[cfg(test)]
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
        enforce_reward_ai_advisory(plan, &advisory, config, min_confidence);
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
    _config: &RewardBotConfig,
    min_confidence: Decimal,
) {
    let action = reward_ai_effective_action(advisory, min_confidence);
    if action.blocks_new_quotes() {
        reject_ai_gated_plan(
            plan,
            &format!(
                "AI advisory {}: {}",
                action.as_str(),
                advisory
                    .reasons
                    .first()
                    .cloned()
                    .unwrap_or_else(|| "advisory rejected this market".to_string())
            ),
        );
    }
}

#[must_use]
pub fn reward_ai_effective_action(
    advisory: &RewardMarketAdvisory,
    min_confidence: Decimal,
) -> RewardProviderAction {
    let action = match advisory.action {
        RewardProviderAction::Allow
        | RewardProviderAction::Reduce
        | RewardProviderAction::StopNew => advisory.action,
        // Defence in depth for corrupted/legacy quote-plan JSON. Connector
        // parsing and the advisory table already reject cancel actions.
        RewardProviderAction::CancelYes
        | RewardProviderAction::CancelNo
        | RewardProviderAction::CancelAll => RewardProviderAction::StopNew,
    };
    if advisory.confidence >= min_confidence {
        return action;
    }
    match action {
        RewardProviderAction::Allow => RewardProviderAction::Allow,
        // Low-confidence slow-risk concerns may reduce size but never stop or
        // cancel quoting on their own.
        _ => RewardProviderAction::Reduce,
    }
}

#[must_use]
pub fn reward_ai_size_multiplier(
    advisory: &RewardMarketAdvisory,
    config: &RewardBotConfig,
) -> Decimal {
    if !config.ai_risk_adjustment_enabled {
        return Decimal::ONE;
    }
    match reward_ai_effective_action(advisory, config.ai_action_min_confidence) {
        RewardProviderAction::Allow => Decimal::ONE,
        RewardProviderAction::Reduce if advisory.action == RewardProviderAction::Reduce => {
            advisory
                .size_multiplier
                .max(decimal("0.10"))
                .min(Decimal::ONE)
        }
        // A low-confidence stop recommendation is downgraded to a real reduce,
        // not the stop action's normalized zero size (which would indirectly
        // block nearly every rewards minimum). Keep the fallback deterministic.
        RewardProviderAction::Reduce => decimal("0.50"),
        _ => Decimal::ZERO,
    }
}

#[must_use]
pub fn reward_ai_edge_buffer_cents(
    advisory: &RewardMarketAdvisory,
    config: &RewardBotConfig,
) -> Decimal {
    if !config.ai_risk_adjustment_enabled
        || reward_ai_effective_action(advisory, config.ai_action_min_confidence)
            == RewardProviderAction::Allow
    {
        return Decimal::ZERO;
    }
    advisory
        .edge_buffer_cents
        .max(Decimal::ZERO)
        .min(decimal("10"))
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
