const LOW_COMPETITION_REPORT_MIN_OBSERVATIONS: usize = 20;
const LOW_COMPETITION_REPORT_MIN_UNIQUE_MARKETS: usize = 3;

pub fn build_low_competition_observations(
    account_id: &str,
    plans: &[RewardQuotePlan],
    config: &RewardBotConfig,
    observed_at: OffsetDateTime,
) -> Vec<RewardLowCompetitionObservation> {
    if !config.low_competition_mode.is_enabled() {
        return Vec::new();
    }

    let standard_eligible_conditions = plans
        .iter()
        .filter(|plan| plan.strategy_bucket == RewardStrategyBucket::Standard && plan.eligible)
        .map(|plan| plan.condition_id.clone())
        .collect::<HashSet<_>>();

    plans
        .iter()
        .filter(|plan| plan.strategy_bucket == RewardStrategyBucket::LowCompetition)
        .filter_map(|plan| {
            let metrics = plan.low_competition_metrics.as_ref()?;
            let reason = plan.reason.to_ascii_lowercase();
            let ai_blocked = metrics.eligible_for_low_competition
                && !plan.eligible
                && reason.contains("ai advisory");
            let info_risk_blocked = metrics.eligible_for_low_competition
                && !plan.eligible
                && reason.contains("info risk");
            Some(RewardLowCompetitionObservation {
                id: low_competition_observation_id(account_id, &plan.condition_id, observed_at),
                account_id: account_id.to_string(),
                condition_id: plan.condition_id.clone(),
                market_slug: plan.market_slug.clone(),
                question: plan.question.clone(),
                observed_at,
                mode: config.low_competition_mode,
                planned_notional_usd: low_competition_plan_notional_usd(plan),
                qualified_competition_usd: metrics.qualified_competition_usd,
                estimated_reward_per_100_usd_day: metrics.estimated_reward_per_100_usd_day,
                competition_density: metrics.competition_density,
                exit_depth_usd: metrics.exit_depth_usd,
                exit_slippage_cents: metrics.exit_slippage_cents,
                midpoint_range_cents: metrics.midpoint_range_cents,
                top_of_book_flip_count: metrics.top_of_book_flip_count,
                sample_count: metrics.sample_count,
                sample_insufficient: low_competition_sample_insufficient(metrics),
                eligible_for_low_competition: metrics.eligible_for_low_competition,
                final_eligible: plan.eligible,
                ai_blocked,
                info_risk_blocked,
                standard_plan_overlap: standard_eligible_conditions.contains(&plan.condition_id),
                rejection_reasons: metrics.rejection_reasons.clone(),
                created_at: observed_at,
            })
        })
        .collect()
}

pub fn build_low_competition_shadow_report(
    observations: &[RewardLowCompetitionObservation],
    window_hours: u64,
    config: &RewardBotConfig,
    generated_at: OffsetDateTime,
) -> RewardLowCompetitionShadowReport {
    let total = observations.len();
    let gate_pass_count = observations
        .iter()
        .filter(|observation| observation.eligible_for_low_competition)
        .count();
    let final_pass_count = observations
        .iter()
        .filter(|observation| observation.final_eligible)
        .count();
    let sample_insufficient_count = observations
        .iter()
        .filter(|observation| observation.sample_insufficient)
        .count();
    let ai_blocked_count = observations
        .iter()
        .filter(|observation| observation.ai_blocked)
        .count();
    let info_risk_blocked_count = observations
        .iter()
        .filter(|observation| observation.info_risk_blocked)
        .count();
    let standard_overlap_count = observations
        .iter()
        .filter(|observation| observation.standard_plan_overlap)
        .count();
    let unique_markets = observations
        .iter()
        .map(|observation| observation.condition_id.as_str())
        .collect::<HashSet<_>>()
        .len();
    let latest_observed_at = observations
        .iter()
        .map(|observation| observation.observed_at)
        .max();

    let estimated_rewards = observations
        .iter()
        .map(|observation| observation.estimated_reward_per_100_usd_day)
        .collect::<Vec<_>>();
    let exit_depth_multiples = observations
        .iter()
        .filter_map(|observation| {
            if observation.planned_notional_usd > Decimal::ZERO {
                Some(
                    (observation.exit_depth_usd / observation.planned_notional_usd)
                        .round_dp(4),
                )
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    let midpoint_ranges = observations
        .iter()
        .filter_map(|observation| observation.midpoint_range_cents)
        .collect::<Vec<_>>();
    let exit_slippages = observations
        .iter()
        .filter_map(|observation| observation.exit_slippage_cents)
        .collect::<Vec<_>>();

    let estimated_reward_per_100_usd_day_median =
        percentile_nearest_decimal(estimated_rewards.clone(), 5_000);
    let estimated_reward_per_100_usd_day_p90 =
        percentile_nearest_decimal(estimated_rewards, 9_000);
    let exit_depth_multiple_median =
        percentile_nearest_decimal(exit_depth_multiples, 5_000);
    let midpoint_range_cents_p95 = percentile_nearest_decimal(midpoint_ranges, 9_500);
    let exit_slippage_cents_p95 = percentile_nearest_decimal(exit_slippages, 9_500);

    let gate_pass_ratio = decimal_count_ratio(gate_pass_count, total);
    let sample_insufficient_ratio = decimal_count_ratio(sample_insufficient_count, total);
    let ai_blocked_ratio = decimal_count_ratio(ai_blocked_count, total);
    let info_risk_blocked_ratio = decimal_count_ratio(info_risk_blocked_count, total);
    let mut recommendation_reasons = low_competition_recommendation_reasons(
        total,
        unique_markets,
        gate_pass_ratio,
        sample_insufficient_ratio,
        ai_blocked_ratio,
        info_risk_blocked_ratio,
        estimated_reward_per_100_usd_day_median,
        exit_depth_multiple_median,
        midpoint_range_cents_p95,
        config,
    );
    let should_consider_enforce = recommendation_reasons.is_empty();
    if should_consider_enforce {
        recommendation_reasons
            .push("shadow metrics support considering a small enforce sleeve".to_string());
    }

    RewardLowCompetitionShadowReport {
        window_hours,
        generated_at,
        latest_observed_at,
        observations: total,
        unique_markets,
        gate_pass_count,
        final_pass_count,
        sample_insufficient_count,
        ai_blocked_count,
        info_risk_blocked_count,
        standard_overlap_count,
        gate_pass_ratio,
        final_pass_ratio: decimal_count_ratio(final_pass_count, total),
        sample_insufficient_ratio,
        ai_blocked_ratio,
        info_risk_blocked_ratio,
        standard_overlap_ratio: decimal_count_ratio(standard_overlap_count, total),
        estimated_reward_per_100_usd_day_median,
        estimated_reward_per_100_usd_day_p90,
        exit_depth_multiple_median,
        midpoint_range_cents_p95,
        exit_slippage_cents_p95,
        should_consider_enforce,
        recommendation_reasons,
    }
}

fn low_competition_observation_id(
    account_id: &str,
    condition_id: &str,
    observed_at: OffsetDateTime,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(account_id.as_bytes());
    hasher.update(b"\0");
    hasher.update(condition_id.as_bytes());
    hasher.update(b"\0");
    hasher.update(observed_at.unix_timestamp_nanos().to_string().as_bytes());
    format!("{:x}", hasher.finalize())
}

fn low_competition_plan_notional_usd(plan: &RewardQuotePlan) -> Decimal {
    plan.legs
        .iter()
        .map(|leg| {
            if leg.notional_usd > Decimal::ZERO {
                leg.notional_usd
            } else {
                leg.price * leg.size
            }
        })
        .sum::<Decimal>()
        .round_dp(4)
}

fn low_competition_sample_insufficient(metrics: &RewardLowCompetitionMetrics) -> bool {
    metrics.rejection_reasons.iter().any(|reason| {
        reason.contains("book history samples")
            || reason.contains("book history midpoint range unavailable")
    })
}

fn low_competition_recommendation_reasons(
    observations: usize,
    unique_markets: usize,
    gate_pass_ratio: Decimal,
    sample_insufficient_ratio: Decimal,
    ai_blocked_ratio: Decimal,
    info_risk_blocked_ratio: Decimal,
    reward_median: Option<Decimal>,
    exit_depth_multiple_median: Option<Decimal>,
    midpoint_range_cents_p95: Option<Decimal>,
    config: &RewardBotConfig,
) -> Vec<String> {
    let mut reasons = Vec::new();
    if observations < LOW_COMPETITION_REPORT_MIN_OBSERVATIONS {
        reasons.push(format!(
            "observations {observations} below required {LOW_COMPETITION_REPORT_MIN_OBSERVATIONS}"
        ));
    }
    if unique_markets < LOW_COMPETITION_REPORT_MIN_UNIQUE_MARKETS {
        reasons.push(format!(
            "unique markets {unique_markets} below required {LOW_COMPETITION_REPORT_MIN_UNIQUE_MARKETS}"
        ));
    }
    if gate_pass_ratio < decimal("0.40") {
        reasons.push(format!("low-competition gate pass ratio {gate_pass_ratio} below 0.40"));
    }
    if sample_insufficient_ratio > decimal("0.20") {
        reasons.push(format!(
            "sample insufficiency ratio {sample_insufficient_ratio} above 0.20"
        ));
    }
    if ai_blocked_ratio > decimal("0.40") {
        reasons.push(format!("AI block ratio {ai_blocked_ratio} above 0.40"));
    }
    if info_risk_blocked_ratio > decimal("0.20") {
        reasons.push(format!(
            "info-risk block ratio {info_risk_blocked_ratio} above 0.20"
        ));
    }
    match reward_median {
        Some(value) if value >= config.low_competition_min_reward_per_100_usd_day => {}
        Some(value) => reasons.push(format!(
            "median estimated reward/100/day {value} below {}",
            config.low_competition_min_reward_per_100_usd_day
        )),
        None => reasons.push("median estimated reward unavailable".to_string()),
    }
    match exit_depth_multiple_median {
        Some(value) if value >= config.low_competition_min_exit_depth_multiple => {}
        Some(value) => reasons.push(format!(
            "median exit depth multiple {value} below {}",
            config.low_competition_min_exit_depth_multiple
        )),
        None => reasons.push("median exit depth multiple unavailable".to_string()),
    }
    match midpoint_range_cents_p95 {
        Some(value) if value <= config.low_competition_max_midpoint_range_cents => {}
        Some(value) => reasons.push(format!(
            "midpoint range p95 {value}c above {}c",
            config.low_competition_max_midpoint_range_cents
        )),
        None => reasons.push("midpoint range p95 unavailable".to_string()),
    }
    reasons
}

fn decimal_count_ratio(numerator: usize, denominator: usize) -> Decimal {
    if denominator == 0 {
        return Decimal::ZERO;
    }
    (Decimal::from(numerator as u64) / Decimal::from(denominator as u64)).round_dp(4)
}

fn percentile_nearest_decimal(mut values: Vec<Decimal>, percentile_bps: u64) -> Option<Decimal> {
    if values.is_empty() {
        return None;
    }
    values.sort();
    let len = values.len() as u64;
    let rank = ((len * percentile_bps).saturating_add(9_999) / 10_000).max(1);
    let index = usize::try_from(rank.saturating_sub(1))
        .unwrap_or(usize::MAX)
        .min(values.len() - 1);
    Some(values[index].round_dp(4))
}
