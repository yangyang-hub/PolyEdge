macro_rules! reward_event_string_enum {
    (
        $(#[$meta:meta])*
        pub enum $name:ident {
            $($variant:ident => $value:literal),+ $(,)?
        }
        default $default:ident;
        error $error_code:literal, $error_label:literal;
    ) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
        #[serde(rename_all = "snake_case")]
        pub enum $name {
            $($variant),+
        }

        impl Default for $name {
            fn default() -> Self {
                Self::$default
            }
        }

        impl $name {
            #[must_use]
            pub const fn as_str(self) -> &'static str {
                match self {
                    $(Self::$variant => $value),+
                }
            }
        }

        impl FromStr for $name {
            type Err = AppError;

            fn from_str(value: &str) -> Result<Self> {
                match value {
                    $($value => Ok(Self::$variant)),+,
                    other => Err(AppError::invalid_input(
                        $error_code,
                        format!("unknown {}: {other}", $error_label),
                    )),
                }
            }
        }
    };
}

reward_event_string_enum! {
    pub enum RewardEventTimeRole {
        EventOccurrence => "event_occurrence",
        MarketLifecycle => "market_lifecycle",
        ResolutionDeadline => "resolution_deadline",
        Unknown => "unknown",
    }
    default Unknown;
    error "REWARD_EVENT_TIME_ROLE_INVALID", "reward event time role";
}

reward_event_string_enum! {
    pub enum RewardEventTimePrecision {
        Exact => "exact",
        DateOnly => "date_only",
        Inferred => "inferred",
        Unknown => "unknown",
    }
    default Unknown;
    error "REWARD_EVENT_TIME_PRECISION_INVALID", "reward event time precision";
}

reward_event_string_enum! {
    pub enum RewardEventEndPolicy {
        Explicit => "explicit",
        Point => "point",
        UntilMarketClosed => "until_market_closed",
        Unknown => "unknown",
    }
    default Unknown;
    error "REWARD_EVENT_END_POLICY_INVALID", "reward event end policy";
}

reward_event_string_enum! {
    pub enum RewardEventScheduleStatus {
        Scheduled => "scheduled",
        Conflicting => "conflicting",
        Finished => "finished",
        Withdrawn => "withdrawn",
        Unknown => "unknown",
    }
    default Unknown;
    error "REWARD_EVENT_SCHEDULE_STATUS_INVALID", "reward event schedule status";
}

const fn default_reward_event_producer_version() -> u32 {
    1
}

fn reward_event_producer_version_is_one(value: &u32) -> bool {
    *value == 1
}

fn reward_event_bool_is_false(value: &bool) -> bool {
    !*value
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RewardEventWindowSourceCoverage {
    pub condition_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(with = "time::serde::rfc3339::option")]
    pub source_updated_at: Option<OffsetDateTime>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RewardEventWindowSourceSnapshot {
    pub source: String,
    #[serde(default = "default_reward_event_producer_version")]
    pub producer_version: u32,
    #[serde(with = "time::serde::rfc3339")]
    pub observed_at: OffsetDateTime,
    #[serde(default)]
    pub coverage: Vec<RewardEventWindowSourceCoverage>,
    #[serde(default)]
    pub windows: Vec<RewardMarketEventWindow>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct RewardEventWindowReplaceReport {
    pub source: String,
    pub covered_condition_count: usize,
    pub input_window_count: usize,
    pub upserted_window_count: u64,
    pub deactivated_window_count: u64,
    pub idempotent_window_count: u64,
    pub skipped_window_count: u64,
    pub ignored_stale_count: u64,
    pub ignored_stale_condition_count: u64,
    pub skipped_missing_parent_count: u64,
}
