#[cfg(test)]
pub fn assess_reward_event_window(
    config: &RewardBotConfig,
    window: Option<&RewardMarketEventWindow>,
    now: OffsetDateTime,
) -> RewardEventWindowAssessment {
    match window {
        Some(window) => assess_reward_event_windows(config, std::slice::from_ref(&window), now),
        None => reward_event_window_assessment(
            RewardEventWindowStatus::NoEventWindow,
            "no discrete event window applies",
            None,
            None,
        ),
    }
}

pub fn assess_reward_event_windows(
    config: &RewardBotConfig,
    windows: &[&RewardMarketEventWindow],
    now: OffsetDateTime,
) -> RewardEventWindowAssessment {
    if !config.event_window_enabled {
        return reward_event_window_assessment(
            RewardEventWindowStatus::NoEventWindow,
            "event window gate disabled",
            None,
            None,
        );
    }

    if windows.is_empty() {
        return reward_event_window_assessment(
            RewardEventWindowStatus::NoEventWindow,
            "no discrete event window applies",
            None,
            None,
        );
    }

    let mut best_by_event = HashMap::<String, &RewardMarketEventWindow>::new();
    for window in windows {
        let key = if window.event_key.trim().is_empty() {
            format!("{}:legacy", window.source)
        } else {
            window.event_key.clone()
        };
        let should_replace = best_by_event
            .get(&key)
            .is_none_or(|existing| reward_event_candidate_precedes(window, existing));
        if should_replace {
            best_by_event.insert(key, window);
        }
    }

    let mut assessments = best_by_event
        .into_values()
        .map(|window| assess_single_reward_event_window(config, window, now))
        .collect::<Vec<_>>();
    assessments.sort_by(reward_event_assessment_order);
    assessments.pop().unwrap_or_else(|| {
        reward_event_window_assessment(
            RewardEventWindowStatus::NoEventWindow,
            "no discrete event window applies",
            None,
            None,
        )
    })
}

fn assess_single_reward_event_window(
    config: &RewardBotConfig,
    window: &RewardMarketEventWindow,
    now: OffsetDateTime,
) -> RewardEventWindowAssessment {
    if !window.active || window.expires_at.is_some_and(|expires_at| expires_at <= now) {
        return reward_event_window_assessment(
            RewardEventWindowStatus::ExpiredOrResolved,
            "event candidate is inactive or expired",
            Some(window),
            Some(window.confidence),
        );
    }

    if window.event_time_role != RewardEventTimeRole::EventOccurrence {
        return reward_event_window_assessment(
            RewardEventWindowStatus::NoEventWindow,
            "candidate is market lifecycle/deadline metadata, not a discrete event",
            Some(window),
            Some(window.confidence),
        );
    }

    if matches!(
        window.schedule_status,
        RewardEventScheduleStatus::Finished | RewardEventScheduleStatus::Withdrawn
    ) {
        return reward_event_window_assessment(
            RewardEventWindowStatus::ExpiredOrResolved,
            "scheduled event is finished or withdrawn",
            Some(window),
            Some(window.confidence),
        );
    }

    if window.source == "gamma"
        && !reward_gamma_event_dates_reviewed(window)
        && config.event_window_gamma_unreviewed_dates_mode
            != RewardGammaEventDateMode::MediumConfidence
    {
        return reward_event_window_assessment(
            RewardEventWindowStatus::NoEventWindow,
            "unreviewed Gamma schedule is observe-only",
            Some(window),
            Some(RewardEventTimeConfidence::Low),
        );
    }

    let confidence = if window.source == "gamma"
        && !reward_gamma_event_dates_reviewed(window)
        && config.event_window_gamma_unreviewed_dates_mode
            == RewardGammaEventDateMode::MediumConfidence
    {
        RewardEventTimeConfidence::Medium
    } else {
        window.confidence
    };

    if !window.active
        || !window.hard_gate_eligible
        || window.schedule_status != RewardEventScheduleStatus::Scheduled
        || window.time_precision != RewardEventTimePrecision::Exact
        || window.event_start_at.is_none()
        || window.start_source_field.as_deref().is_none_or(str::is_empty)
        || confidence.rank() < config.event_window_min_confidence.rank()
    {
        return reward_untrusted_event_window_assessment(config, window, confidence);
    }

    let Some(start_at) = window.event_start_at else {
        return reward_untrusted_event_window_assessment(config, window, confidence);
    };
    let end_at = match window.end_policy {
        RewardEventEndPolicy::Explicit => match window.event_end_at {
            Some(end_at) if end_at >= start_at => Some(end_at),
            _ => return reward_untrusted_event_window_assessment(config, window, confidence),
        },
        RewardEventEndPolicy::Point => Some(start_at),
        RewardEventEndPolicy::UntilMarketClosed => None,
        RewardEventEndPolicy::Unknown => {
            return reward_untrusted_event_window_assessment(config, window, confidence);
        }
    };
    let stop_new_at =
        start_at - TimeDuration::seconds(config.event_window_stop_new_quote_before_start_sec as i64);
    let cancel_at =
        start_at - TimeDuration::seconds(config.event_window_cancel_open_buy_before_start_sec as i64);
    let resume_at = end_at
        .map(|end_at| {
            end_at + TimeDuration::seconds(config.event_window_resume_after_event_end_sec as i64)
        });

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
    } else if end_at.is_none_or(|end_at| now <= end_at) {
        (
            RewardEventWindowStatus::InEventWindow,
            if window.end_policy == RewardEventEndPolicy::UntilMarketClosed {
                "event has started; blocking BUY quotes until the market closes"
            } else {
                "event is in progress; blocking and cancelling BUY quotes"
            },
        )
    } else if resume_at.is_some_and(|resume_at| now <= resume_at) {
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

    reward_event_window_assessment(status, reason, Some(window), Some(confidence))
}

fn reward_untrusted_event_window_assessment(
    config: &RewardBotConfig,
    window: &RewardMarketEventWindow,
    confidence: RewardEventTimeConfidence,
) -> RewardEventWindowAssessment {
    match config.event_window_unknown_event_time_mode {
        RewardUnknownEventTimeMode::Block => reward_event_window_assessment(
            RewardEventWindowStatus::UntrustedEventTime,
            "expected event schedule is missing, conflicting or below configured confidence; blocking new BUY quotes",
            Some(window),
            Some(confidence),
        ),
        RewardUnknownEventTimeMode::Allow | RewardUnknownEventTimeMode::Observe => {
            reward_event_window_assessment(
                RewardEventWindowStatus::NoEventWindow,
                "expected event schedule is missing, conflicting or below configured confidence",
                Some(window),
                Some(confidence),
            )
        }
    }
}

fn reward_gamma_event_dates_reviewed(window: &RewardMarketEventWindow) -> bool {
    window
        .source_payload
        .get("has_reviewed_dates")
        .and_then(Value::as_bool)
        .unwrap_or(window.confidence.rank() >= RewardEventTimeConfidence::Medium.rank())
}

fn reward_event_candidate_precedes(
    candidate: &RewardMarketEventWindow,
    existing: &RewardMarketEventWindow,
) -> bool {
    reward_event_candidate_rank(candidate)
        .cmp(&reward_event_candidate_rank(existing))
        .then_with(|| candidate.source_updated_at.cmp(&existing.source_updated_at))
        .then_with(|| candidate.observed_at.cmp(&existing.observed_at))
        .then_with(|| candidate.updated_at.cmp(&existing.updated_at))
        .is_gt()
}

fn reward_event_candidate_rank(window: &RewardMarketEventWindow) -> (u8, u8, u8, u8) {
    (
        reward_event_window_source_priority(&window.source),
        u8::from(window.hard_gate_eligible),
        u8::from(window.schedule_status == RewardEventScheduleStatus::Scheduled),
        window.confidence.rank(),
    )
}

fn reward_event_window_source_priority(source: &str) -> u8 {
    match source {
        "manual" => 6,
        "official" | "sports_api" | "economic_calendar" | "earnings_calendar"
        | "governance_calendar" => 5,
        "gamma" => 4,
        "news" | "rss" => 2,
        "ai_extracted" => 1,
        _ => 0,
    }
}

fn reward_event_assessment_order(
    left: &RewardEventWindowAssessment,
    right: &RewardEventWindowAssessment,
) -> std::cmp::Ordering {
    reward_event_assessment_rank(left.status)
        .cmp(&reward_event_assessment_rank(right.status))
        .then_with(|| right.event_start_at.cmp(&left.event_start_at))
        .then_with(|| left.source.cmp(&right.source))
        .then_with(|| left.event_key.cmp(&right.event_key))
}

fn reward_event_assessment_rank(status: RewardEventWindowStatus) -> u8 {
    match status {
        RewardEventWindowStatus::InEventWindow => 7,
        RewardEventWindowStatus::CancelOpenBuys | RewardEventWindowStatus::PostEventCooldown => 6,
        RewardEventWindowStatus::StopNewQuotes | RewardEventWindowStatus::UntrustedEventTime => 5,
        RewardEventWindowStatus::SafeBeforeWindow => 3,
        RewardEventWindowStatus::ExpiredOrResolved => 2,
        RewardEventWindowStatus::NoEventWindow => 1,
    }
}

pub fn apply_reward_event_windows_to_quote_plans(
    plans: &mut [RewardQuotePlan],
    windows: &[RewardMarketEventWindow],
    config: &RewardBotConfig,
    now: OffsetDateTime,
) {
    let mut windows_by_condition = HashMap::<&str, Vec<&RewardMarketEventWindow>>::new();
    for window in windows {
        windows_by_condition
            .entry(window.condition_id.as_str())
            .or_default()
            .push(window);
    }

    for plan in plans {
        let assessment = assess_reward_event_windows(
            config,
            windows_by_condition
                .get(plan.condition_id.as_str())
                .map(Vec::as_slice)
                .unwrap_or(&[]),
            now,
        );
        if assessment.status != RewardEventWindowStatus::NoEventWindow
            || assessment.event_key.is_some()
        {
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
    confidence: Option<RewardEventTimeConfidence>,
) -> RewardEventWindowAssessment {
    RewardEventWindowAssessment {
        status,
        reason: reason.into(),
        event_key: window
            .map(|window| window.event_key.clone())
            .filter(|event_key| !event_key.is_empty()),
        event_time_role: window.map(|window| window.event_time_role),
        schedule_status: window.map(|window| window.schedule_status),
        time_precision: window.map(|window| window.time_precision),
        start_source_field: window.and_then(|window| window.start_source_field.clone()),
        end_policy: window.map(|window| window.end_policy),
        hard_gate_eligible: window.map(|window| window.hard_gate_eligible),
        producer_version: window.map(|window| window.producer_version),
        source_updated_at: window.and_then(|window| window.source_updated_at),
        observed_at: window.and_then(|window| window.observed_at),
        expires_at: window.and_then(|window| window.expires_at),
        event_start_at: window.and_then(|window| window.event_start_at),
        event_end_at: window.and_then(|window| window.event_end_at),
        source: window.map(|window| window.source.clone()),
        confidence,
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
            event_key: "primary".to_string(),
            event_type: "sports_start".to_string(),
            event_time_role: RewardEventTimeRole::EventOccurrence,
            schedule_status: RewardEventScheduleStatus::Scheduled,
            time_precision: RewardEventTimePrecision::Exact,
            start_source_field: Some("manual.event_start_at".to_string()),
            end_policy: RewardEventEndPolicy::Explicit,
            event_start_at: Some(now + start_after),
            event_end_at: Some(now + start_after + TimeDuration::hours(2)),
            confidence,
            source_url: None,
            source_payload: json!({}),
            notes: String::new(),
            active: true,
            hard_gate_eligible: true,
            producer_version: 1,
            source_updated_at: Some(now),
            observed_at: Some(now),
            expires_at: None,
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
    fn no_event_is_not_applicable_even_when_unknown_mode_blocks() {
        let now = OffsetDateTime::now_utc();
        let config = RewardBotConfig {
            event_window_unknown_event_time_mode: RewardUnknownEventTimeMode::Block,
            ..RewardBotConfig::default()
        };
        let assessment = assess_reward_event_window(&config, None, now);

        assert_eq!(assessment.status, RewardEventWindowStatus::NoEventWindow);
        assert!(!assessment.status.blocks_new_buy());
        assert!(!assessment.status.cancels_open_buy());
    }

    #[test]
    fn conflicting_expected_event_uses_unknown_time_policy() {
        let now = OffsetDateTime::now_utc();
        let mut window = test_window(
            now,
            TimeDuration::hours(1),
            RewardEventTimeConfidence::High,
        );
        window.schedule_status = RewardEventScheduleStatus::Conflicting;
        window.hard_gate_eligible = false;
        let config = RewardBotConfig {
            event_window_unknown_event_time_mode: RewardUnknownEventTimeMode::Block,
            ..RewardBotConfig::default()
        };

        let assessment = assess_reward_event_window(&config, Some(&window), now);

        assert_eq!(assessment.status, RewardEventWindowStatus::UntrustedEventTime);
        assert!(assessment.status.blocks_new_buy());
        assert!(!assessment.status.cancels_open_buy());
    }

    #[test]
    fn lifecycle_metadata_can_never_become_a_hard_event_window() {
        let now = OffsetDateTime::now_utc();
        let mut window = test_window(
            now,
            -TimeDuration::days(30),
            RewardEventTimeConfidence::High,
        );
        window.event_time_role = RewardEventTimeRole::MarketLifecycle;
        window.event_end_at = Some(now + TimeDuration::days(180));

        let assessment = assess_reward_event_window(
            &RewardBotConfig::production_live_drill_defaults(),
            Some(&window),
            now,
        );

        assert_eq!(assessment.status, RewardEventWindowStatus::NoEventWindow);
        assert!(!assessment.status.blocks_new_buy());
    }

    #[test]
    fn until_market_closed_never_resumes_while_candidate_is_active() {
        let now = OffsetDateTime::now_utc();
        let mut window = test_window(
            now,
            -TimeDuration::days(2),
            RewardEventTimeConfidence::High,
        );
        window.end_policy = RewardEventEndPolicy::UntilMarketClosed;
        window.event_end_at = None;

        let assessment = assess_reward_event_window(
            &RewardBotConfig::production_live_drill_defaults(),
            Some(&window),
            now,
        );

        assert_eq!(assessment.status, RewardEventWindowStatus::InEventWindow);
        assert!(assessment.status.cancels_open_buy());
    }

    #[test]
    fn multiple_events_aggregate_to_the_most_restrictive_action() {
        let now = OffsetDateTime::now_utc();
        let safe = test_window(
            now,
            TimeDuration::days(3),
            RewardEventTimeConfidence::High,
        );
        let mut imminent = test_window(
            now,
            TimeDuration::minutes(30),
            RewardEventTimeConfidence::High,
        );
        imminent.event_key = "imminent".to_string();

        let assessment = assess_reward_event_windows(
            &RewardBotConfig::production_live_drill_defaults(),
            &[&safe, &imminent],
            now,
        );

        assert_eq!(assessment.status, RewardEventWindowStatus::CancelOpenBuys);
        assert_eq!(assessment.event_key.as_deref(), Some("imminent"));
    }

    #[test]
    fn higher_priority_manual_withdrawal_suppresses_gamma_for_the_same_event() {
        let now = OffsetDateTime::now_utc();
        let mut gamma = test_window(
            now,
            TimeDuration::minutes(30),
            RewardEventTimeConfidence::Medium,
        );
        gamma.source = "gamma".to_string();
        gamma.source_payload = json!({"has_reviewed_dates": true});

        let mut manual = gamma.clone();
        manual.source = "manual".to_string();
        manual.schedule_status = RewardEventScheduleStatus::Withdrawn;
        manual.hard_gate_eligible = false;
        manual.event_start_at = None;
        manual.event_end_at = None;
        manual.start_source_field = None;
        manual.end_policy = RewardEventEndPolicy::Unknown;
        manual.confidence = RewardEventTimeConfidence::High;

        let assessment = assess_reward_event_windows(
            &RewardBotConfig::production_live_drill_defaults(),
            &[&gamma, &manual],
            now,
        );

        assert_eq!(assessment.status, RewardEventWindowStatus::ExpiredOrResolved);
        assert_eq!(assessment.source.as_deref(), Some("manual"));
        assert!(!assessment.status.blocks_new_buy());
    }
}
