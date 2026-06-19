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
            expires_at: now + TimeDuration::seconds(ttl_sec as i64),
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
    let payload = json!({
        "schema_version": 1,
        "task": "Assess recent external information risk for this Polymarket rewards market before maker quoting.",
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
        "schema_version": 2,
        "cache_domain": "reward_info_risk",
        "search_query": query,
        "market": {
            "condition_id": market.condition_id,
            "question": market.question,
            "market_slug": market.market_slug,
            "event_slug": market.event_slug,
            "category": market.category,
            "end_at": market.end_at,
            "ambiguity_level": market.ambiguity_level,
        },
        "current_quote_plan": plan.map(|plan| json!({
            "quote_mode": plan.quote_mode,
            "recommended_quote_mode": plan.recommended_quote_mode,
            "strategy_bucket": plan.strategy_bucket,
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

    for plan in plans {
        let Some(risk) = risks.get(&plan.condition_id).cloned() else {
            if config.info_risk_mode == RewardSelectionMode::Enforce && plan.eligible {
                plan.eligible = false;
                plan.quote_mode = RewardPlanQuoteMode::None;
                plan.legs.clear();
                plan.reason =
                    "info risk pending: market has not passed provider risk filter".to_string();
            }
            continue;
        };
        plan.info_risk = Some(risk.clone());
        if config.info_risk_mode != RewardSelectionMode::Enforce
            || risk.confidence < min_confidence
            || !reward_info_risk_blocks_quote(&risk, config.info_risk_avoid_level)
        {
            continue;
        }
        plan.eligible = false;
        plan.quote_mode = RewardPlanQuoteMode::None;
        plan.legs.clear();
        plan.reason = format!(
            "info risk {}: {}",
            risk.risk_level.as_str(),
            risk.summary
        );
    }
}

fn reward_info_risk_blocks_quote(
    risk: &RewardMarketInfoRisk,
    avoid_level: RewardInfoRiskLevel,
) -> bool {
    risk.resolution_imminent
        || matches!(
            risk.risk_type,
            RewardInfoRiskType::ImminentResolution | RewardInfoRiskType::OfficialResult
        )
        || risk.risk_level.rank() >= avoid_level.rank()
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
