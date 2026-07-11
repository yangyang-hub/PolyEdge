pub fn apply_reward_market_selection_to_quote_plans(
    plans: &mut [RewardQuotePlan],
    config: &RewardBotConfig,
) {
    for plan in plans.iter_mut() {
        let metrics = build_reward_market_selection_metrics(plan, config);
        plan.selection_score = metrics.selection_score;
        plan.selection_metrics = Some(metrics);
    }
    sort_reward_quote_plans_by_selection(plans);
}

pub fn sort_reward_quote_plans_by_selection(plans: &mut [RewardQuotePlan]) {
    plans.sort_by(|left, right| {
        right
            .eligible
            .cmp(&left.eligible)
            .then_with(|| right.selection_score.cmp(&left.selection_score))
            .then_with(|| right.score.cmp(&left.score))
            .then_with(|| right.total_daily_rate.cmp(&left.total_daily_rate))
            .then_with(|| left.condition_id.cmp(&right.condition_id))
    });
}

fn build_reward_market_selection_metrics(
    plan: &RewardQuotePlan,
    config: &RewardBotConfig,
) -> RewardMarketSelectionMetrics {
    let base_quality_score = clamp_selection_score(plan.score);
    let opportunity_score = plan
        .opportunity_metrics
        .as_ref()
        .map_or(base_quality_score, |metrics| {
            clamp_selection_score(metrics.opportunity_score)
        });
    let reward_density_score = plan
        .opportunity_metrics
        .as_ref()
        .map_or(base_quality_score, |metrics| {
            clamp_selection_score(metrics.reward_score)
        });
    let exit_score = plan
        .opportunity_metrics
        .as_ref()
        .map_or(Decimal::ZERO, |metrics| {
            clamp_selection_score(metrics.exit_score)
        });
    let stability_score = plan
        .opportunity_metrics
        .as_ref()
        .map_or(Decimal::ZERO, |metrics| {
            clamp_selection_score(metrics.stability_score)
        });
    let fair_value_edge_score = reward_selection_fair_value_edge_score(plan);
    let competition_penalty = reward_selection_competition_penalty(plan);
    let allocation_penalty = reward_selection_allocation_penalty(plan);
    let risk_penalty = reward_selection_risk_penalty(plan, config);

    // LP reward density is an explicit secondary signal. The composite
    // opportunity score is retained for audit/configuration but is not added
    // here because it already contains reward and would double-count it.
    let positive = base_quality_score * decimal("0.15")
        + reward_density_score * decimal("0.10")
        + fair_value_edge_score * decimal("0.30")
        + exit_score * decimal("0.25")
        + stability_score * decimal("0.20");
    let penalty = competition_penalty * decimal("0.18")
        + allocation_penalty * decimal("0.10")
        + risk_penalty * decimal("0.45");
    let selection_score = clamp_selection_score((positive - penalty).round_dp(2));

    RewardMarketSelectionMetrics {
        base_quality_score,
        opportunity_score,
        reward_density_score,
        fair_value_edge_score,
        exit_score,
        stability_score,
        competition_penalty,
        allocation_penalty,
        risk_penalty,
        selection_score,
        reasons: reward_selection_reasons(plan, selection_score),
    }
}

fn reward_selection_fair_value_edge_score(plan: &RewardQuotePlan) -> Decimal {
    let Some(decision) = &plan.fair_value else {
        return decimal("50");
    };
    if !decision.passed {
        return Decimal::ZERO;
    }
    let passed_edges = decision
        .edges
        .iter()
        .filter(|edge| edge.passed)
        .collect::<Vec<_>>();
    if passed_edges.is_empty() {
        return Decimal::ZERO;
    }
    let total_edge = passed_edges.iter().fold(Decimal::ZERO, |sum, edge| {
        sum + edge.effective_edge_cents.max(Decimal::ZERO)
    });
    let average_edge = total_edge / Decimal::from(passed_edges.len() as u64);
    ratio_selection_score(average_edge, decimal("2"))
}

fn reward_selection_competition_penalty(plan: &RewardQuotePlan) -> Decimal {
    plan.opportunity_metrics
        .as_ref()
        .map_or(Decimal::ZERO, |metrics| {
            (decimal("100") - clamp_selection_score(metrics.competition_score)).max(Decimal::ZERO)
        })
}

fn reward_selection_allocation_penalty(plan: &RewardQuotePlan) -> Decimal {
    plan.opportunity_metrics
        .as_ref()
        .map_or(Decimal::ZERO, |metrics| {
            Decimal::max(metrics.account_allocation_bps, metrics.market_allocation_bps)
                / decimal("100")
        })
        .min(decimal("100"))
        .max(Decimal::ZERO)
}

fn reward_selection_risk_penalty(plan: &RewardQuotePlan, config: &RewardBotConfig) -> Decimal {
    if !plan.eligible {
        return decimal("100");
    }
    let mut penalty = Decimal::ZERO;
    match plan.quote_readiness {
        RewardQuoteReadiness::ReadyToQuote => {}
        RewardQuoteReadiness::WaitingOrderbook => penalty += decimal("15"),
        RewardQuoteReadiness::ProviderPending => penalty += decimal("25"),
        RewardQuoteReadiness::Blocked => penalty += decimal("40"),
    }
    if plan
        .fair_value
        .as_ref()
        .is_some_and(|decision| !decision.passed)
    {
        penalty += decimal("60");
    }
    if plan
        .event_window
        .as_ref()
        .is_some_and(|assessment| assessment.status.blocks_new_buy())
    {
        penalty += decimal("60");
    }
    if let Some(advisory) = &plan.ai_advisory {
        penalty += match reward_ai_effective_action(
            advisory,
            config.ai_action_min_confidence,
        ) {
            RewardProviderAction::Allow => Decimal::ZERO,
            RewardProviderAction::Reduce => decimal("15"),
            RewardProviderAction::StopNew => decimal("40"),
            RewardProviderAction::CancelYes
            | RewardProviderAction::CancelNo
            | RewardProviderAction::CancelAll => decimal("70"),
        };
    }
    if let Some(risk) = &plan.info_risk {
        penalty += match reward_info_risk_effective_action(
            risk,
            config.info_risk_avoid_level,
            config.info_risk_min_confidence,
        ) {
            RewardProviderAction::Allow => Decimal::ZERO,
            RewardProviderAction::Reduce => decimal("15"),
            RewardProviderAction::StopNew => decimal("40"),
            RewardProviderAction::CancelYes
            | RewardProviderAction::CancelNo
            | RewardProviderAction::CancelAll => decimal("70"),
        };
    }
    penalty.min(decimal("100"))
}

fn reward_selection_reasons(plan: &RewardQuotePlan, selection_score: Decimal) -> Vec<String> {
    let mut reasons = Vec::new();
    if plan.opportunity_metrics.is_none() {
        reasons.push("opportunity metrics unavailable".to_string());
    }
    if plan.fair_value.is_none() {
        reasons.push("fair-value edge unavailable".to_string());
    }
    if !plan.eligible {
        reasons.push(plan.reason.clone());
    } else if selection_score >= decimal("70") {
        reasons.push("high maker priority".to_string());
    } else if selection_score >= decimal("45") {
        reasons.push("moderate maker priority".to_string());
    } else {
        reasons.push("low maker priority".to_string());
    }
    reasons
}

fn ratio_selection_score(value: Decimal, target: Decimal) -> Decimal {
    if target <= Decimal::ZERO {
        return Decimal::ZERO;
    }
    clamp_selection_score((value / target * decimal("100")).round_dp(2))
}

fn clamp_selection_score(value: Decimal) -> Decimal {
    value.max(Decimal::ZERO).min(decimal("100")).round_dp(2)
}
