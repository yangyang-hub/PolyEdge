#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RewardInfoRiskLevel {
    Low,
    Medium,
    High,
    Critical,
    Unknown,
}

impl RewardInfoRiskLevel {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::Critical => "critical",
            Self::Unknown => "unknown",
        }
    }

    #[must_use]
    pub const fn rank(self) -> u8 {
        match self {
            Self::Low => 1,
            Self::Medium => 2,
            Self::High => 3,
            Self::Critical => 4,
            Self::Unknown => 0,
        }
    }
}

impl FromStr for RewardInfoRiskLevel {
    type Err = AppError;

    fn from_str(value: &str) -> Result<Self> {
        match value {
            "low" => Ok(Self::Low),
            "medium" => Ok(Self::Medium),
            "high" => Ok(Self::High),
            "critical" => Ok(Self::Critical),
            "unknown" => Ok(Self::Unknown),
            other => Err(AppError::invalid_input(
                "REWARD_INFO_RISK_LEVEL_INVALID",
                format!("unknown reward info risk level: {other}"),
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RewardInfoRiskType {
    ImminentResolution,
    BreakingNews,
    ScheduledEvent,
    OfficialResult,
    Rumor,
    Stale,
    None,
    Unknown,
}

impl RewardInfoRiskType {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ImminentResolution => "imminent_resolution",
            Self::BreakingNews => "breaking_news",
            Self::ScheduledEvent => "scheduled_event",
            Self::OfficialResult => "official_result",
            Self::Rumor => "rumor",
            Self::Stale => "stale",
            Self::None => "none",
            Self::Unknown => "unknown",
        }
    }
}

impl FromStr for RewardInfoRiskType {
    type Err = AppError;

    fn from_str(value: &str) -> Result<Self> {
        match value {
            "imminent_resolution" => Ok(Self::ImminentResolution),
            "breaking_news" => Ok(Self::BreakingNews),
            "scheduled_event" => Ok(Self::ScheduledEvent),
            "official_result" => Ok(Self::OfficialResult),
            "rumor" => Ok(Self::Rumor),
            "stale" => Ok(Self::Stale),
            "none" => Ok(Self::None),
            "unknown" => Ok(Self::Unknown),
            other => Err(AppError::invalid_input(
                "REWARD_INFO_RISK_TYPE_INVALID",
                format!("unknown reward info risk type: {other}"),
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RewardInfoDirectionalRisk {
    Yes,
    No,
    Unclear,
}

impl RewardInfoDirectionalRisk {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Yes => "yes",
            Self::No => "no",
            Self::Unclear => "unclear",
        }
    }
}

impl FromStr for RewardInfoDirectionalRisk {
    type Err = AppError;

    fn from_str(value: &str) -> Result<Self> {
        match value {
            "yes" => Ok(Self::Yes),
            "no" => Ok(Self::No),
            "unclear" => Ok(Self::Unclear),
            other => Err(AppError::invalid_input(
                "REWARD_INFO_DIRECTIONAL_RISK_INVALID",
                format!("unknown reward info directional risk: {other}"),
            )),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RewardInfoRiskSource {
    pub url: String,
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(with = "time::serde::rfc3339::option")]
    pub published_at: Option<OffsetDateTime>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snippet: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RewardMarketInfoRisk {
    pub condition_id: String,
    pub provider: RewardAiProvider,
    pub request_format: RewardAiRequestFormat,
    pub model: String,
    pub query_hash: String,
    pub input_hash: String,
    pub risk_level: RewardInfoRiskLevel,
    pub risk_type: RewardInfoRiskType,
    pub directional_risk: RewardInfoDirectionalRisk,
    pub resolution_imminent: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(with = "time::serde::rfc3339::option")]
    pub expected_event_at: Option<OffsetDateTime>,
    pub confidence: Decimal,
    pub summary: String,
    pub sources: Vec<RewardInfoRiskSource>,
    pub metrics: Value,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub expires_at: OffsetDateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RewardInfoRiskAssessmentDecision {
    pub risk_level: RewardInfoRiskLevel,
    pub risk_type: RewardInfoRiskType,
    pub directional_risk: RewardInfoDirectionalRisk,
    pub resolution_imminent: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(with = "time::serde::rfc3339::option")]
    pub expected_event_at: Option<OffsetDateTime>,
    pub confidence: Decimal,
    pub summary: String,
    pub sources: Vec<RewardInfoRiskSource>,
    pub metrics: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RewardInfoRiskAssessmentRequest {
    pub condition_id: String,
    pub provider: RewardAiProvider,
    pub request_format: RewardAiRequestFormat,
    pub model: String,
    pub query: String,
    pub query_hash: String,
    pub input_hash: String,
    pub payload: Value,
}

impl RewardInfoRiskAssessmentDecision {
    #[must_use]
    pub fn into_info_risk(
        self,
        request: &RewardInfoRiskAssessmentRequest,
        ttl_sec: u64,
        now: OffsetDateTime,
    ) -> RewardMarketInfoRisk {
        RewardMarketInfoRisk {
            condition_id: request.condition_id.clone(),
            provider: request.provider,
            request_format: request.request_format,
            model: request.model.clone(),
            query_hash: request.query_hash.clone(),
            input_hash: request.input_hash.clone(),
            risk_level: self.risk_level,
            risk_type: self.risk_type,
            directional_risk: self.directional_risk,
            resolution_imminent: self.resolution_imminent,
            expected_event_at: self.expected_event_at,
            confidence: self.confidence,
            summary: self.summary,
            sources: self.sources,
            metrics: self.metrics,
            created_at: now,
            expires_at: reward_provider_cache_expires_at(
                now,
                ttl_sec,
                "info_risk",
                &[
                    request.condition_id.as_str(),
                    request.provider.as_str(),
                    request.request_format.as_str(),
                    request.model.as_str(),
                    request.query_hash.as_str(),
                    request.input_hash.as_str(),
                ],
            ),
        }
    }
}

pub fn build_reward_info_risk_assessment_request(
    market: &RewardMarket,
    plan: Option<&RewardQuotePlan>,
    account: &RewardAccountState,
    positions: &[RewardPosition],
    open_orders: &[ManagedRewardOrder],
    config: &RewardBotConfig,
    provider: RewardAiProvider,
    request_format: RewardAiRequestFormat,
    model: &str,
) -> Result<RewardInfoRiskAssessmentRequest> {
    let market_positions = positions
        .iter()
        .filter(|position| position.condition_id == market.condition_id)
        .collect::<Vec<_>>();
    let market_open_orders = open_orders
        .iter()
        .filter(|order| order.condition_id == market.condition_id)
        .collect::<Vec<_>>();
    let query = reward_info_risk_query(market);
    let evaluation_time = OffsetDateTime::now_utc();
    let payload = json!({
        "schema_version": 3,
        "task": "Return a binary allow_quote decision for this Polymarket rewards market before maker quoting.",
        "evaluation_time_utc": evaluation_time,
        "imminent_resolution_policy": {
            "current_time_source": "evaluation_time_utc",
            "definition": "Set resolution_imminent=true only when an official result/resolution has been announced or a confirmed resolution-driving event is expected within 7 days of evaluation_time_utc. Distant scheduled events, stale dates, or unsupported current-news claims are not imminent by themselves.",
        },
        "provider_cache_policy": reward_provider_cache_policy_payload(
            config.info_risk_ttl_sec,
            evaluation_time,
        ),
        "search_query": query,
        "market": {
            "condition_id": market.condition_id,
            "question": market.question,
            "market_slug": market.market_slug,
            "event_slug": market.event_slug,
            "category": market.category,
            "total_daily_rate": market.total_daily_rate,
            "liquidity_usd": market.liquidity_usd,
            "volume_24h_usd": market.volume_24h_usd,
            "market_spread_cents": market.market_spread_cents,
            "rewards_max_spread": market.rewards_max_spread,
            "rewards_min_size": market.rewards_min_size,
            "end_at": market.end_at,
            "ambiguity_level": market.ambiguity_level,
            "market_synced_at": market.market_synced_at,
        },
        "current_quote_plan": plan.map(|plan| json!({
            "eligible": plan.eligible,
            "reason": plan.reason,
            "score": plan.score,
            "quote_mode": plan.quote_mode,
            "recommended_quote_mode": plan.recommended_quote_mode,
            "midpoint": plan.midpoint,
            "book_metrics": plan.book_metrics,
            "legs": plan.legs,
            "ai_advisory": plan.ai_advisory,
        })),
        "account_exposure": {
            "account_id": account.account_id,
            "available_usd": account.available_usd,
            "positions": market_positions,
            "open_orders": market_open_orders,
        },
        "strategy_config": {
            "info_risk_mode": config.info_risk_mode,
            "info_risk_avoid_level": config.info_risk_avoid_level,
            "selection_mode": config.selection_mode,
            "quote_mode": config.quote_mode,
            "dominant_min_probability": config.dominant_min_probability,
            "dominant_max_probability": config.dominant_max_probability,
            "min_hours_to_end": config.min_hours_to_end,
            "preferred_categories": config.preferred_categories,
        },
        "suggested_search_queries": [
            &query,
            format!("{} latest news result official source", market.question),
            format!("{} Polymarket resolution source result date", market.question)
        ],
    });
    Ok(RewardInfoRiskAssessmentRequest {
        condition_id: market.condition_id.clone(),
        provider,
        request_format,
        model: model.trim().to_string(),
        query_hash: reward_info_hash(json!({ "query": &query }))?,
        input_hash: reward_info_hash(reward_info_risk_cache_key_payload(
            market, plan, config, &query,
        ))?,
        query,
        payload,
    })
}

fn reward_info_risk_cache_key_payload(
    market: &RewardMarket,
    plan: Option<&RewardQuotePlan>,
    config: &RewardBotConfig,
    query: &str,
) -> Value {
    json!({
        // schema_version 7: drop `event_slug` and `ambiguity_level` from the
        // cache key. These are the only info-risk key fields NOT also present
        // in the (stable) AI advisory key, and they drift: `event_slug` comes
        // from the rewards-catalog sync (CLOB) while `ambiguity_level` comes
        // from the Gamma markets table, and the two independent sync loops
        // (rewards catalog ~5min + Gamma priority/full) write marginally
        // differing values for the same condition across cycles. That drift
        // made the info-risk lookup key oscillate away from the cached row's
        // key and back, so a still-valid risk row was missed on alternate
        // ticks → `info risk pending` → eligible dropped to 0 intermittently.
        // The market is now identified by the same stable fields the advisory
        // key uses (condition_id / question / market_slug / category / end_at).
        //
        // schema_version 6: drop `quote_mode` / `recommended_quote_mode` from
        // the cache key. The materialized quote mode flips between double and
        // single_no every tick for markets sitting on the funding boundary, and
        // because it was part of the key those flips invalidated the info-risk
        // cache lookup — marking markets `info risk pending` and (under enforce
        // mode + require_info_risk_before_first_quote) dropping eligible to 0
        // even though the cached risk assessment was still valid. Info-risk
        // evaluates market/event resolution risk, which is independent of how we
        // happen to size the quote, so the per-tick mode must not churn the key.
        //
        // schema_version 5: legacy low-competition sleeve settings are no
        // longer part of strategy context; all candidates use the unified
        // info-risk policy.
        //
        // schema_version 4: provider output contract is binary allow_quote.
        // Keep detailed risk taxonomy as internal compatibility fields only.
        "schema_version": 7,
        "cache_domain": "reward_info_risk",
        "provider_decision_schema": "binary_allow_quote_v1",
        "evaluation_policy_version": 1,
        "search_query": query,
        "market": {
            "condition_id": market.condition_id,
            "question": market.question,
            "market_slug": market.market_slug,
            "category": market.category,
            "end_at": market.end_at,
        },
        "current_quote_plan": plan.map(|plan| json!({
            "strategy_bucket": plan.strategy_bucket,
            "strategy_profile": plan.strategy_profile,
        })),
        "strategy_config": {
            "info_risk_mode": config.info_risk_mode,
            "info_risk_avoid_level": config.info_risk_avoid_level,
            "selection_mode": config.selection_mode,
            "quote_mode": config.quote_mode,
            "dominant_min_probability": config.dominant_min_probability,
            "dominant_max_probability": config.dominant_max_probability,
            "min_hours_to_end": config.min_hours_to_end,
            "preferred_categories": config.preferred_categories,
        },
    })
}

pub fn apply_reward_info_risks(
    plans: &mut [RewardQuotePlan],
    risks: &HashMap<String, RewardMarketInfoRisk>,
    config: &RewardBotConfig,
    min_confidence: Decimal,
) {
    if !config.info_risk_enabled {
        return;
    }

    let enforce = config.info_risk_mode == RewardSelectionMode::Enforce;
    let now = OffsetDateTime::now_utc();
    let grace = TimeDuration::seconds(config.info_risk_provider_pending_grace_sec as i64);

    for plan in plans {
        let Some(risk) = risks.get(&plan.condition_id).cloned() else {
            if enforce && plan.eligible {
                if plan.pre_ai_eligible {
                    // Pre-AI-eligible plans get a grace period before being
                    // dropped. This prevents the eligible count from
                    // oscillating to 0 while the background provider refresh
                    // populates the cache.
                    match plan.info_risk_pending_since {
                        None if grace.is_zero() => {
                            // Grace disabled — immediate drop (prior behaviour).
                            plan.eligible = false;
                            plan.quote_mode = RewardPlanQuoteMode::None;
                            plan.legs.clear();
                            plan.reason =
                                "info risk pending: market has not passed provider risk filter"
                                    .to_string();
                        }
                        None => {
                            plan.info_risk_pending_since = Some(now);
                            // Keep eligible; preserve reason for display.
                        }
                        Some(since) if now - since >= grace => {
                            plan.eligible = false;
                            plan.quote_mode = RewardPlanQuoteMode::None;
                            plan.legs.clear();
                            plan.reason =
                                "info risk pending: market has not passed provider risk filter"
                                    .to_string();
                        }
                        Some(_) => {
                            // Within grace period — keep eligible.
                        }
                    }
                } else {
                    // Non-pre-ai-eligible plans drop immediately
                    // (fail-closed for active-exposure plans).
                    plan.eligible = false;
                    plan.quote_mode = RewardPlanQuoteMode::None;
                    plan.legs.clear();
                    plan.reason =
                        "info risk pending: market has not passed provider risk filter"
                            .to_string();
                }
            }
            continue;
        };
        // Cached risk available — clear any pending grace state.
        plan.info_risk_pending_since = None;
        plan.info_risk = Some(risk.clone());
        let avoid_level = config.info_risk_avoid_level;
        if !enforce || !reward_info_risk_blocks_quote(&risk, avoid_level) {
            continue;
        }
        if risk.confidence < min_confidence && risk.risk_level != RewardInfoRiskLevel::Critical {
            continue;
        }
        plan.eligible = false;
        plan.quote_mode = RewardPlanQuoteMode::None;
        plan.legs.clear();
        plan.reason = format!("info risk {}: {}", risk.risk_level.as_str(), risk.summary);
    }
}

fn reward_info_risk_blocks_quote(
    risk: &RewardMarketInfoRisk,
    avoid_level: RewardInfoRiskLevel,
) -> bool {
    risk.resolution_imminent
        || matches!(risk.risk_type, RewardInfoRiskType::OfficialResult)
        || match avoid_level {
            RewardInfoRiskLevel::Low | RewardInfoRiskLevel::Medium => {
                risk.risk_level.rank() >= avoid_level.rank()
            }
            RewardInfoRiskLevel::High | RewardInfoRiskLevel::Critical => {
                risk.risk_level == RewardInfoRiskLevel::Critical
            }
            RewardInfoRiskLevel::Unknown => false,
        }
}

fn reward_info_risk_query(market: &RewardMarket) -> String {
    let mut parts = vec![market.question.trim().to_string()];
    if !market.market_slug.trim().is_empty() {
        parts.push(market.market_slug.trim().to_string());
    }
    if !market.category.trim().is_empty() {
        parts.push(market.category.trim().to_string());
    }
    parts.push("latest news official result scheduled event".to_string());
    parts
        .into_iter()
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

fn reward_info_hash(value: Value) -> Result<String> {
    let bytes = serde_json::to_vec(&value).map_err(|error| {
        AppError::internal(
            "REWARD_INFO_RISK_HASH_FAILED",
            format!("failed to serialize reward info risk input: {error}"),
        )
    })?;
    let digest = Sha256::digest(bytes);
    Ok(format!("{digest:x}"))
}
