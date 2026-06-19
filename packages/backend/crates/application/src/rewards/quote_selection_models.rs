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
    #[serde(rename = "openai", alias = "open_ai")]
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
            "openai" => Ok(Self::OpenAi),
            "anthropic" => Ok(Self::Anthropic),
            other => Err(AppError::invalid_input(
                "REWARD_AI_PROVIDER_INVALID",
                format!("unknown reward AI provider: {other}"),
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RewardAiRequestFormat {
    #[serde(rename = "openai_responses", alias = "open_ai_responses")]
    OpenAiResponses,
    #[serde(
        rename = "openai_chat_completions",
        alias = "open_ai_chat_completions"
    )]
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
