#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RewardQuoteMode {
    /// Keep the legacy YES + NO two-sided rewards maker plan.
    Double,
    /// Let deterministic book metrics choose double, single-sided, or skip.
    Auto,
}

impl RewardQuoteMode {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Double => "double",
            Self::Auto => "auto",
        }
    }
}

impl FromStr for RewardQuoteMode {
    type Err = AppError;

    fn from_str(value: &str) -> Result<Self> {
        match value {
            "double" => Ok(Self::Double),
            "auto" => Ok(Self::Auto),
            other => Err(AppError::invalid_input(
                "REWARD_QUOTE_MODE_INVALID",
                format!("unknown reward quote mode: {other}"),
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RewardSelectionMode {
    /// Compute metrics but keep legacy quoting behavior.
    Observe,
    /// Enforce deterministic auto-mode recommendations.
    Enforce,
}

impl RewardSelectionMode {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Observe => "observe",
            Self::Enforce => "enforce",
        }
    }
}

impl FromStr for RewardSelectionMode {
    type Err = AppError;

    fn from_str(value: &str) -> Result<Self> {
        match value {
            "observe" => Ok(Self::Observe),
            "enforce" => Ok(Self::Enforce),
            other => Err(AppError::invalid_input(
                "REWARD_SELECTION_MODE_INVALID",
                format!("unknown reward selection mode: {other}"),
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RewardLowCompetitionMode {
    Off,
    Observe,
    Enforce,
}

impl RewardLowCompetitionMode {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Observe => "observe",
            Self::Enforce => "enforce",
        }
    }

    #[must_use]
    pub const fn is_enabled(self) -> bool {
        !matches!(self, Self::Off)
    }
}

impl FromStr for RewardLowCompetitionMode {
    type Err = AppError;

    fn from_str(value: &str) -> Result<Self> {
        match value {
            "off" => Ok(Self::Off),
            "observe" => Ok(Self::Observe),
            "enforce" => Ok(Self::Enforce),
            other => Err(AppError::invalid_input(
                "REWARD_LOW_COMPETITION_MODE_INVALID",
                format!("unknown reward low competition mode: {other}"),
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RewardEventTimeConfidence {
    Low,
    Medium,
    High,
}

impl RewardEventTimeConfidence {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
        }
    }

    #[must_use]
    pub const fn rank(self) -> u8 {
        match self {
            Self::Low => 1,
            Self::Medium => 2,
            Self::High => 3,
        }
    }
}

impl FromStr for RewardEventTimeConfidence {
    type Err = AppError;

    fn from_str(value: &str) -> Result<Self> {
        match value {
            "low" => Ok(Self::Low),
            "medium" => Ok(Self::Medium),
            "high" => Ok(Self::High),
            other => Err(AppError::invalid_input(
                "REWARD_EVENT_TIME_CONFIDENCE_INVALID",
                format!("unknown reward event time confidence: {other}"),
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RewardUnknownEventTimeMode {
    Allow,
    Observe,
    Block,
}

impl RewardUnknownEventTimeMode {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Allow => "allow",
            Self::Observe => "observe",
            Self::Block => "block",
        }
    }
}

impl FromStr for RewardUnknownEventTimeMode {
    type Err = AppError;

    fn from_str(value: &str) -> Result<Self> {
        match value {
            "allow" => Ok(Self::Allow),
            "observe" => Ok(Self::Observe),
            "block" => Ok(Self::Block),
            other => Err(AppError::invalid_input(
                "REWARD_UNKNOWN_EVENT_TIME_MODE_INVALID",
                format!("unknown reward unknown-event-time mode: {other}"),
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RewardGammaEventDateMode {
    Ignore,
    Observe,
    MediumConfidence,
}

impl RewardGammaEventDateMode {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ignore => "ignore",
            Self::Observe => "observe",
            Self::MediumConfidence => "medium_confidence",
        }
    }
}

impl FromStr for RewardGammaEventDateMode {
    type Err = AppError;

    fn from_str(value: &str) -> Result<Self> {
        match value {
            "ignore" => Ok(Self::Ignore),
            "observe" => Ok(Self::Observe),
            "medium_confidence" => Ok(Self::MediumConfidence),
            other => Err(AppError::invalid_input(
                "REWARD_GAMMA_EVENT_DATE_MODE_INVALID",
                format!("unknown reward Gamma event date mode: {other}"),
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RewardEventWindowStatus {
    NoEventWindow,
    SafeBeforeWindow,
    StopNewQuotes,
    CancelOpenBuys,
    InEventWindow,
    PostEventCooldown,
    ExpiredOrResolved,
    UntrustedEventTime,
}

impl RewardEventWindowStatus {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NoEventWindow => "no_event_window",
            Self::SafeBeforeWindow => "safe_before_window",
            Self::StopNewQuotes => "stop_new_quotes",
            Self::CancelOpenBuys => "cancel_open_buys",
            Self::InEventWindow => "in_event_window",
            Self::PostEventCooldown => "post_event_cooldown",
            Self::ExpiredOrResolved => "expired_or_resolved",
            Self::UntrustedEventTime => "untrusted_event_time",
        }
    }

    #[must_use]
    pub const fn blocks_new_buy(self) -> bool {
        matches!(
            self,
            Self::StopNewQuotes
                | Self::CancelOpenBuys
                | Self::InEventWindow
                | Self::PostEventCooldown
                | Self::UntrustedEventTime
        )
    }

    #[must_use]
    pub const fn cancels_open_buy(self) -> bool {
        matches!(
            self,
            Self::CancelOpenBuys | Self::InEventWindow | Self::PostEventCooldown
        )
    }
}

impl FromStr for RewardEventWindowStatus {
    type Err = AppError;

    fn from_str(value: &str) -> Result<Self> {
        match value {
            "no_event_window" => Ok(Self::NoEventWindow),
            "safe_before_window" => Ok(Self::SafeBeforeWindow),
            "stop_new_quotes" => Ok(Self::StopNewQuotes),
            "cancel_open_buys" => Ok(Self::CancelOpenBuys),
            "in_event_window" => Ok(Self::InEventWindow),
            "post_event_cooldown" => Ok(Self::PostEventCooldown),
            "expired_or_resolved" => Ok(Self::ExpiredOrResolved),
            "untrusted_event_time" => Ok(Self::UntrustedEventTime),
            other => Err(AppError::invalid_input(
                "REWARD_EVENT_WINDOW_STATUS_INVALID",
                format!("unknown reward event window status: {other}"),
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RewardPlanQuoteMode {
    Double,
    SingleYes,
    SingleNo,
    None,
}

impl RewardPlanQuoteMode {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Double => "double",
            Self::SingleYes => "single_yes",
            Self::SingleNo => "single_no",
            Self::None => "none",
        }
    }
}

impl FromStr for RewardPlanQuoteMode {
    type Err = AppError;

    fn from_str(value: &str) -> Result<Self> {
        match value {
            "double" => Ok(Self::Double),
            "single_yes" => Ok(Self::SingleYes),
            "single_no" => Ok(Self::SingleNo),
            "none" => Ok(Self::None),
            other => Err(AppError::invalid_input(
                "REWARD_PLAN_QUOTE_MODE_INVALID",
                format!("unknown reward plan quote mode: {other}"),
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RewardQuoteReadiness {
    ReadyToQuote,
    WaitingOrderbook,
    ProviderPending,
    Blocked,
}

impl RewardQuoteReadiness {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ReadyToQuote => "ready_to_quote",
            Self::WaitingOrderbook => "waiting_orderbook",
            Self::ProviderPending => "provider_pending",
            Self::Blocked => "blocked",
        }
    }
}

impl FromStr for RewardQuoteReadiness {
    type Err = AppError;

    fn from_str(value: &str) -> Result<Self> {
        match value {
            "ready_to_quote" => Ok(Self::ReadyToQuote),
            "waiting_orderbook" => Ok(Self::WaitingOrderbook),
            "provider_pending" => Ok(Self::ProviderPending),
            "blocked" => Ok(Self::Blocked),
            other => Err(AppError::invalid_input(
                "REWARD_QUOTE_READINESS_INVALID",
                format!("unknown reward quote readiness: {other}"),
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RewardStrategyBucket {
    Standard,
    LowCompetition,
    None,
}

impl RewardStrategyBucket {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Standard => "standard",
            Self::LowCompetition => "low_competition",
            Self::None => "none",
        }
    }
}

impl FromStr for RewardStrategyBucket {
    type Err = AppError;

    fn from_str(value: &str) -> Result<Self> {
        match value {
            "standard" => Ok(Self::Standard),
            "low_competition" => Ok(Self::LowCompetition),
            "none" => Ok(Self::None),
            other => Err(AppError::invalid_input(
                "REWARD_STRATEGY_BUCKET_INVALID",
                format!("unknown reward strategy bucket: {other}"),
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RewardAiProvider {
    #[serde(
        rename = "openai",
        alias = "open_ai",
        alias = "glm",
        alias = "bigmodel",
        alias = "zhipu",
        alias = "deepseek",
        alias = "deep_seek"
    )]
    OpenAi,
    Anthropic,
}

impl RewardAiProvider {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::OpenAi => "openai",
            Self::Anthropic => "anthropic",
        }
    }
}

impl FromStr for RewardAiProvider {
    type Err = AppError;

    fn from_str(value: &str) -> Result<Self> {
        match value {
            "openai" | "open_ai" | "glm" | "bigmodel" | "zhipu" | "deepseek" | "deep_seek" => {
                Ok(Self::OpenAi)
            }
            "anthropic" => Ok(Self::Anthropic),
            other => Err(AppError::invalid_input(
                "REWARD_AI_PROVIDER_INVALID",
                format!("unknown reward AI provider: {other}"),
            )),
        }
    }
}

#[must_use]
pub fn reward_ai_model_requires_openai_chat_completions(model: &str) -> bool {
    let normalized = model.to_ascii_lowercase();
    normalized.contains("glm") || normalized.contains("deepseek")
}

/// Returns true for GLM reasoning models that enable chain-of-thought by
/// default (the glm-4.7 family, including `glm-4.7-flashx`). For these models
/// the worker disables thinking on chat-completions calls so the output budget
/// lands in `content` instead of being consumed by `reasoning_content`, which
/// otherwise truncates the message to an empty string under a tight `max_tokens`
/// (observed as `finish_reason: length`, `content: ""`).
#[must_use]
pub fn reward_ai_model_is_glm_reasoning(model: &str) -> bool {
    model.to_ascii_lowercase().contains("glm-4.7")
}

#[must_use]
pub fn reward_ai_effective_request_format(
    provider: RewardAiProvider,
    configured: RewardAiRequestFormat,
    model: &str,
) -> RewardAiRequestFormat {
    match provider {
        RewardAiProvider::Anthropic => RewardAiRequestFormat::AnthropicMessages,
        RewardAiProvider::OpenAi if reward_ai_model_requires_openai_chat_completions(model) => {
            RewardAiRequestFormat::OpenAiChatCompletions
        }
        RewardAiProvider::OpenAi
            if matches!(configured, RewardAiRequestFormat::AnthropicMessages) =>
        {
            RewardAiRequestFormat::OpenAiResponses
        }
        RewardAiProvider::OpenAi => configured,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RewardAiRequestFormat {
    #[serde(rename = "openai_responses", alias = "open_ai_responses")]
    OpenAiResponses,
    #[serde(rename = "openai_chat_completions", alias = "open_ai_chat_completions")]
    OpenAiChatCompletions,
    AnthropicMessages,
}

impl RewardAiRequestFormat {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::OpenAiResponses => "openai_responses",
            Self::OpenAiChatCompletions => "openai_chat_completions",
            Self::AnthropicMessages => "anthropic_messages",
        }
    }
}

impl FromStr for RewardAiRequestFormat {
    type Err = AppError;

    fn from_str(value: &str) -> Result<Self> {
        match value {
            "openai_responses" => Ok(Self::OpenAiResponses),
            "openai_chat_completions" => Ok(Self::OpenAiChatCompletions),
            "anthropic_messages" => Ok(Self::AnthropicMessages),
            other => Err(AppError::invalid_input(
                "REWARD_AI_REQUEST_FORMAT_INVALID",
                format!("unknown reward AI request format: {other}"),
            )),
        }
    }
}

fn normalize_reward_categories(categories: &[String]) -> Vec<String> {
    let mut normalized = Vec::new();
    for category in categories {
        let category = category.trim().to_ascii_lowercase();
        if category.is_empty() || normalized.contains(&category) {
            continue;
        }
        normalized.push(category);
    }
    normalized
}
