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
    #[serde(default)]
    pub action: RewardProviderAction,
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
    pub action: RewardProviderAction,
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
            action: self.action,
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
    _plan: Option<&RewardQuotePlan>,
    _account: &RewardAccountState,
    _positions: &[RewardPosition],
    _open_orders: &[ManagedRewardOrder],
    config: &RewardBotConfig,
    provider: RewardAiProvider,
    request_format: RewardAiRequestFormat,
    model: &str,
) -> Result<RewardInfoRiskAssessmentRequest> {
    let query = reward_info_risk_query(market);
    let evaluation_time = OffsetDateTime::now_utc();
    let payload = json!({
        "schema_version": 6,
        "task": "Return an evidence-backed market-maker risk action. Distinguish reduce/stop-new from directional or full cancellation.",
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
            "category": market.category,
            "end_at": market.end_at,
        },
        "decision_boundary": {
            "provider_may_assess": ["fresh_attributable_event", "official_result", "confirmed_resolution_driver"],
            "provider_must_not_use": ["live_orderbook", "quote_price", "quote_side", "account_balance", "position_size"],
            "cancel_requires_sources": true,
            "directional_action_semantics": "directional_risk is the outcome whose resting BUY is unsafe and must match cancel_yes/cancel_no; it is not the predicted winner. Evidence raising YES probability generally makes the NO BUY unsafe (cancel_no), while evidence lowering YES probability makes the YES BUY unsafe (cancel_yes).",
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
        input_hash: reward_info_hash(reward_info_risk_cache_key_payload(market, &query))?,
        query,
        payload,
    })
}

fn reward_info_risk_cache_key_payload(
    market: &RewardMarket,
    query: &str,
) -> Value {
    json!({
        // schema_version 11: directional risk means the unsafe resting-BUY
        // outcome, not the predicted winner; invalidate ambiguous old results.
        //
        // schema_version 10: independently synced event/ambiguity metadata is
        // removed from the request domain so payload and cache semantics agree.
        //
        // schema_version 9: evidence risk is independent of current quote,
        // inventory and operator thresholds; only stable market identity and
        // the search query participate in cache invalidation.
        "schema_version": 11,
        "cache_domain": "reward_info_risk",
        "provider_decision_schema": "evidence_action_v3_unsafe_buy_direction",
        "evaluation_policy_version": 2,
        "search_query": query,
        "market": {
            "condition_id": market.condition_id,
            "question": market.question,
            "market_slug": market.market_slug,
            "category": market.category,
            "end_at": market.end_at,
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
        if !enforce {
            continue;
        }
        let action = reward_info_risk_effective_action(
            &risk,
            config.info_risk_avoid_level,
            min_confidence,
        );
        match action {
            RewardProviderAction::Allow | RewardProviderAction::Reduce => continue,
            RewardProviderAction::CancelYes => {
                plan.quote_mode = RewardPlanQuoteMode::SingleNo;
                plan.recommended_quote_mode = Some(RewardPlanQuoteMode::SingleNo);
                plan.reason = format!(
                    "info risk cancel_yes: {}; quoting complementary NO only",
                    risk.summary
                );
                continue;
            }
            RewardProviderAction::CancelNo => {
                plan.quote_mode = RewardPlanQuoteMode::SingleYes;
                plan.recommended_quote_mode = Some(RewardPlanQuoteMode::SingleYes);
                plan.reason = format!(
                    "info risk cancel_no: {}; quoting complementary YES only",
                    risk.summary
                );
                continue;
            }
            RewardProviderAction::StopNew | RewardProviderAction::CancelAll => {}
        }
        plan.eligible = false;
        plan.quote_mode = RewardPlanQuoteMode::None;
        plan.legs.clear();
        plan.reason = format!("info risk {}: {}", action.as_str(), risk.summary);
    }
}

#[must_use]
pub fn reward_info_risk_effective_action(
    risk: &RewardMarketInfoRisk,
    avoid_level: RewardInfoRiskLevel,
    min_confidence: Decimal,
) -> RewardProviderAction {
    let mut action = risk.action;
    if (action == RewardProviderAction::CancelYes
        && risk.directional_risk != RewardInfoDirectionalRisk::Yes)
        || (action == RewardProviderAction::CancelNo
            && risk.directional_risk != RewardInfoDirectionalRisk::No)
    {
        action = RewardProviderAction::StopNew;
    }
    if action == RewardProviderAction::Allow {
        let taxonomy_blocks = risk.resolution_imminent
            || matches!(risk.risk_type, RewardInfoRiskType::OfficialResult)
            || match avoid_level {
                RewardInfoRiskLevel::Low | RewardInfoRiskLevel::Medium => {
                    risk.risk_level.rank() >= avoid_level.rank()
                }
                RewardInfoRiskLevel::High | RewardInfoRiskLevel::Critical => {
                    risk.risk_level == RewardInfoRiskLevel::Critical
                }
                RewardInfoRiskLevel::Unknown => false,
            };
        if taxonomy_blocks {
            action = RewardProviderAction::StopNew;
        }
    }
    if matches!(
        action,
        RewardProviderAction::CancelYes
            | RewardProviderAction::CancelNo
            | RewardProviderAction::CancelAll
    ) && !reward_info_risk_has_fresh_cancel_evidence(risk)
    {
        // Cancellation is irreversible queue loss. A provider assertion without
        // a recent attributable source may stop risk growth, but cannot remove
        // otherwise safe resting liquidity.
        action = RewardProviderAction::StopNew;
    }
    if risk.confidence >= min_confidence {
        return action;
    }
    match action {
        RewardProviderAction::Allow => RewardProviderAction::Allow,
        RewardProviderAction::Reduce => RewardProviderAction::Reduce,
        // Low-confidence information can stop risk growth but never cancel a
        // resting order.
        _ => RewardProviderAction::StopNew,
    }
}

fn reward_info_risk_has_fresh_cancel_evidence(risk: &RewardMarketInfoRisk) -> bool {
    let required_independent_sources = match risk.risk_type {
        RewardInfoRiskType::OfficialResult | RewardInfoRiskType::ImminentResolution => 1,
        RewardInfoRiskType::BreakingNews => 2,
        RewardInfoRiskType::ScheduledEvent if risk.resolution_imminent => 1,
        _ => 0,
    };
    if required_independent_sources == 0 {
        return false;
    }
    let oldest = risk.created_at - TimeDuration::hours(24);
    let newest = risk.created_at + TimeDuration::minutes(5);
    let mut authorities = Vec::new();
    for source in &risk.sources {
        let url = source.url.trim();
        let Some(authority) = reward_info_risk_source_authority(url) else {
            continue;
        };
        if !source
            .published_at
            .is_some_and(|published_at| published_at >= oldest && published_at <= newest)
        {
            continue;
        }
        if !authorities.iter().any(|existing| existing == &authority) {
            authorities.push(authority);
        }
    }
    authorities.len() >= required_independent_sources
}

fn reward_info_risk_source_authority(url: &str) -> Option<String> {
    let (_, remainder) = url.split_once("://")?;
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return None;
    }
    let authority = remainder.split('/').next()?.trim().to_ascii_lowercase();
    (!authority.is_empty()).then_some(authority)
}

/// Deterministic size effect for the info-risk `reduce` action. The provider
/// does not choose capital amounts; evidence-backed moderate risk halves new
/// quote size, while allow and stop/cancel actions remain semantically distinct.
#[must_use]
pub fn reward_info_risk_size_multiplier(
    risk: &RewardMarketInfoRisk,
    config: &RewardBotConfig,
) -> Decimal {
    if !config.info_risk_enabled || config.info_risk_mode != RewardSelectionMode::Enforce {
        return Decimal::ONE;
    }
    match reward_info_risk_effective_action(
        risk,
        config.info_risk_avoid_level,
        config.info_risk_min_confidence,
    ) {
        RewardProviderAction::Allow => Decimal::ONE,
        RewardProviderAction::Reduce => decimal("0.50"),
        // Directional cancellation removes the risky outcome from the plan;
        // the complementary quote remains a valid hedge and keeps its budget.
        RewardProviderAction::CancelYes | RewardProviderAction::CancelNo => Decimal::ONE,
        RewardProviderAction::StopNew | RewardProviderAction::CancelAll => Decimal::ZERO,
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
