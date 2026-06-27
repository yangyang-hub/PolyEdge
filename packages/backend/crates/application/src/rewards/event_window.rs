pub fn assess_reward_event_window(
    config: &RewardBotConfig,
    window: Option<&RewardMarketEventWindow>,
    now: OffsetDateTime,
) -> RewardEventWindowAssessment {
    if !config.event_window_enabled {
        return reward_event_window_assessment(
            RewardEventWindowStatus::NoEventWindow,
            "event window gate disabled",
            None,
        );
    }

    let Some(window) = window else {
        return match config.event_window_unknown_event_time_mode {
            RewardUnknownEventTimeMode::Block => reward_event_window_assessment(
                RewardEventWindowStatus::UntrustedEventTime,
                "event time unavailable; blocking new BUY quotes",
                None,
            ),
            RewardUnknownEventTimeMode::Allow | RewardUnknownEventTimeMode::Observe => {
                reward_event_window_assessment(
                    RewardEventWindowStatus::NoEventWindow,
                    "event time unavailable",
                    None,
                )
            }
        };
    };

    if !window.active
        || window.confidence.rank() < config.event_window_min_confidence.rank()
        || window.event_start_at.is_none()
    {
        return match config.event_window_unknown_event_time_mode {
            RewardUnknownEventTimeMode::Block => reward_event_window_assessment(
                RewardEventWindowStatus::UntrustedEventTime,
                "event time is missing or below configured confidence; blocking new BUY quotes",
                Some(window),
            ),
            RewardUnknownEventTimeMode::Allow | RewardUnknownEventTimeMode::Observe => {
                reward_event_window_assessment(
                    RewardEventWindowStatus::NoEventWindow,
                    "event time is missing or below configured confidence",
                    Some(window),
                )
            }
        };
    }

    let Some(start_at) = window.event_start_at else {
        return reward_event_window_assessment(
            RewardEventWindowStatus::NoEventWindow,
            "event time is missing",
            Some(window),
        );
    };
    let end_at = window.event_end_at.unwrap_or(start_at);
    let stop_new_at =
        start_at - TimeDuration::seconds(config.event_window_stop_new_quote_before_start_sec as i64);
    let cancel_at =
        start_at - TimeDuration::seconds(config.event_window_cancel_open_buy_before_start_sec as i64);
    let resume_at =
        end_at + TimeDuration::seconds(config.event_window_resume_after_event_end_sec as i64);

    let (status, reason) = if now < stop_new_at {
        (
            RewardEventWindowStatus::SafeBeforeWindow,
            "event window is outside the stop-new-quote threshold",
        )
    } else if now < cancel_at {
        (
            RewardEventWindowStatus::StopNewQuotes,
            "event starts soon; blocking new BUY quotes",
        )
    } else if now < start_at {
        (
            RewardEventWindowStatus::CancelOpenBuys,
            "event is inside the open-BUY cancel threshold",
        )
    } else if now <= end_at {
        (
            RewardEventWindowStatus::InEventWindow,
            "event is in progress; blocking and cancelling BUY quotes",
        )
    } else if now <= resume_at {
        (
            RewardEventWindowStatus::PostEventCooldown,
            "event is in post-event cooldown; blocking and cancelling BUY quotes",
        )
    } else {
        (
            RewardEventWindowStatus::ExpiredOrResolved,
            "event window has expired",
        )
    };

    reward_event_window_assessment(status, reason, Some(window))
}

pub fn apply_reward_event_windows_to_quote_plans(
    plans: &mut [RewardQuotePlan],
    windows: &[RewardMarketEventWindow],
    config: &RewardBotConfig,
    now: OffsetDateTime,
) {
    let windows_by_condition = windows
        .iter()
        .map(|window| (window.condition_id.as_str(), window))
        .collect::<HashMap<_, _>>();

    for plan in plans {
        let assessment = assess_reward_event_window(
            config,
            windows_by_condition.get(plan.condition_id.as_str()).copied(),
            now,
        );
        if assessment.status != RewardEventWindowStatus::NoEventWindow {
            plan.event_window = Some(assessment.clone());
        }
        if assessment.status.cancels_open_buy() {
            plan.eligible = false;
            plan.pre_ai_eligible = false;
            plan.quote_mode = RewardPlanQuoteMode::None;
            plan.legs.clear();
            plan.reason = format!("event window blocked: {}", assessment.reason);
            refresh_reward_quote_plan_readiness(plan);
        }
    }
}

pub fn reward_quote_plan_event_window_blocks_new_buy(plan: &RewardQuotePlan) -> bool {
    match plan.event_window.as_ref() {
        Some(assessment) => assessment.status.blocks_new_buy(),
        None => false,
    }
}

pub fn reward_quote_plan_event_window_cancels_open_buy(plan: &RewardQuotePlan) -> bool {
    match plan.event_window.as_ref() {
        Some(assessment) => assessment.status.cancels_open_buy(),
        None => false,
    }
}

fn reward_event_window_assessment(
    status: RewardEventWindowStatus,
    reason: impl Into<String>,
    window: Option<&RewardMarketEventWindow>,
) -> RewardEventWindowAssessment {
    RewardEventWindowAssessment {
        status,
        reason: reason.into(),
        event_start_at: window.and_then(|window| window.event_start_at),
        event_end_at: window.and_then(|window| window.event_end_at),
        source: window.map(|window| window.source.clone()),
        confidence: window.map(|window| window.confidence),
        event_type: window.map(|window| window.event_type.clone()),
    }
}

#[cfg(test)]
mod event_window_tests {
    use super::*;

    fn test_window(
        now: OffsetDateTime,
        start_after: TimeDuration,
        confidence: RewardEventTimeConfidence,
    ) -> RewardMarketEventWindow {
        RewardMarketEventWindow {
            condition_id: "cond1".to_string(),
            source: "manual".to_string(),
            event_type: "sports_start".to_string(),
            event_start_at: Some(now + start_after),
            event_end_at: Some(now + start_after + TimeDuration::hours(2)),
            confidence,
            source_url: None,
            source_payload: json!({}),
            notes: String::new(),
            active: true,
            reviewed_by: None,
            reviewed_at: None,
            updated_at: now,
        }
    }

    #[test]
    fn gamma_candidate_below_default_confidence_does_not_hard_gate() {
        let now = OffsetDateTime::now_utc();
        let window = test_window(now, TimeDuration::hours(1), RewardEventTimeConfidence::Medium);
        let assessment = assess_reward_event_window(&RewardBotConfig::default(), Some(&window), now);

        assert_eq!(assessment.status, RewardEventWindowStatus::NoEventWindow);
        assert!(!assessment.status.blocks_new_buy());
    }

    #[test]
    fn trusted_event_inside_stop_window_blocks_new_buys_only() {
        let now = OffsetDateTime::now_utc();
        let window = test_window(now, TimeDuration::hours(2), RewardEventTimeConfidence::High);
        let assessment = assess_reward_event_window(&RewardBotConfig::default(), Some(&window), now);

        assert_eq!(assessment.status, RewardEventWindowStatus::StopNewQuotes);
        assert!(assessment.status.blocks_new_buy());
        assert!(!assessment.status.cancels_open_buy());
    }

    #[test]
    fn trusted_event_inside_cancel_window_cancels_open_buys() {
        let now = OffsetDateTime::now_utc();
        let window = test_window(now, TimeDuration::minutes(30), RewardEventTimeConfidence::High);
        let assessment = assess_reward_event_window(&RewardBotConfig::default(), Some(&window), now);

        assert_eq!(assessment.status, RewardEventWindowStatus::CancelOpenBuys);
        assert!(assessment.status.blocks_new_buy());
        assert!(assessment.status.cancels_open_buy());
    }

    #[test]
    fn unknown_event_time_block_mode_blocks_new_buys() {
        let now = OffsetDateTime::now_utc();
        let config = RewardBotConfig {
            event_window_unknown_event_time_mode: RewardUnknownEventTimeMode::Block,
            ..RewardBotConfig::default()
        };
        let assessment = assess_reward_event_window(&config, None, now);

        assert_eq!(assessment.status, RewardEventWindowStatus::UntrustedEventTime);
        assert!(assessment.status.blocks_new_buy());
        assert!(!assessment.status.cancels_open_buy());
    }
}
