fn reward_provider_refresh_candidate_condition_ids(
    condition_ids: &[String],
    plans: &[RewardQuotePlan],
    open_orders: &[ManagedRewardOrder],
    positions: &[RewardPosition],
    config: &RewardBotConfig,
) -> Vec<String> {
    // Return the full priority queue. `ai_provider_max_markets` /
    // `info_risk_max_markets_per_cycle` only cap real HTTP requests later;
    // truncating here starves lower-score markets when the head is cache-fresh.
    let available_conditions = condition_ids
        .iter()
        .filter_map(|condition_id| reward_provider_normalized_condition_id(condition_id))
        .collect::<HashSet<_>>();
    let mut seen = HashSet::new();
    let mut ordered = Vec::with_capacity(condition_ids.len());
    let plans_by_condition = reward_provider_plans_by_condition(plans);

    for order in open_orders {
        push_reward_provider_available_condition(
            &mut ordered,
            &mut seen,
            &available_conditions,
            &order.condition_id,
        );
    }
    for position in positions {
        push_reward_provider_available_condition(
            &mut ordered,
            &mut seen,
            &available_conditions,
            &position.condition_id,
        );
    }

    let mut standard_candidates = Vec::new();
    for condition_id in condition_ids {
        let Some(condition_id) = reward_provider_normalized_condition_id(condition_id) else {
            continue;
        };
        if !available_conditions.contains(&condition_id) || seen.contains(&condition_id) {
            continue;
        }
        let Some(condition_plans) = plans_by_condition.get(condition_id.as_str()) else {
            continue;
        };
        if !condition_plans.iter().any(|plan| {
            matches!(
                reward_provider_pre_llm_candidate_kind(plan, config, false),
                Some(RewardProviderPreLlmCandidateKind::Standard)
            )
        }) {
            continue;
        }
        let selection_score = condition_plans
            .iter()
            .map(|plan| plan.selection_score)
            .max()
            .unwrap_or(Decimal::ZERO);
        let missing_info_risk = condition_plans
            .iter()
            .any(|plan| plan.info_risk.is_none());
        standard_candidates.push((condition_id, missing_info_risk, selection_score));
    }
    // Prefer markets still missing info-risk (first-quote gate), then higher
    // selection_score. Cache-fresh heads are skipped without consuming the
    // request budget, so lower-priority misses still get reached.
    standard_candidates.sort_by(
        |(left_id, left_missing_info, left_score),
         (right_id, right_missing_info, right_score)| {
            right_missing_info
                .cmp(left_missing_info)
                .then_with(|| right_score.cmp(left_score))
                .then_with(|| left_id.cmp(right_id))
        },
    );

    for (condition_id, _, _) in standard_candidates {
        if seen.insert(condition_id.clone()) {
            ordered.push(condition_id);
        }
    }
    ordered
}

fn push_reward_provider_available_condition(
    ordered: &mut Vec<String>,
    seen: &mut HashSet<String>,
    available_conditions: &HashSet<String>,
    condition_id: &str,
) {
    let Some(condition_id) = reward_provider_normalized_condition_id(condition_id) else {
        return;
    };
    if !available_conditions.contains(&condition_id) {
        return;
    }
    if seen.insert(condition_id.clone()) {
        ordered.push(condition_id);
    }
}

fn reward_provider_normalized_condition_id(condition_id: &str) -> Option<String> {
    let condition_id = condition_id.trim();
    if condition_id.is_empty() {
        return None;
    }
    Some(condition_id.to_string())
}

fn reward_provider_max_conditions_per_cycle(state: &AppState, config: &RewardBotConfig) -> usize {
    let settings_cap = usize::from(state.settings.rewards.info_risk_max_markets_per_cycle);
    let config_cap = usize::from(config.ai_provider_max_markets);
    settings_cap.min(config_cap)
}

#[cfg(test)]
mod reward_provider_refresh_candidate_tests {
    use super::*;

    fn d(value: &str) -> Decimal {
        value.parse().expect("decimal literal")
    }

    fn candidate_plan(condition_id: &str, selection_score: Decimal) -> RewardQuotePlan {
        RewardQuotePlan {
            condition_id: condition_id.to_string(),
            market_slug: "market".to_string(),
            question: "Question?".to_string(),
            score: Decimal::ONE,
            selection_score,
            eligible: true,
            pre_ai_eligible: true,
            quote_readiness: RewardQuoteReadiness::Blocked,
            reason: String::new(),
            strategy_bucket: RewardStrategyBucket::Standard,
            strategy_profile: RewardStrategyProfile::Standard,
            latest_run_id: None,
            quote_mode: RewardPlanQuoteMode::Double,
            recommended_quote_mode: None,
            book_metrics: None,
            opportunity_metrics: None,
            selection_metrics: None,
            fair_value: None,
            ai_advisory: None,
            info_risk: None,
            event_window: None,
            midpoint: Some(d("0.5")),
            live_skip_until: None,
            live_skip_reason: None,
            first_quote_observed_at: None,
            ai_advisory_pending_since: None,
            info_risk_pending_since: None,
            total_daily_rate: d("5"),
            rewards_max_spread: d("3"),
            rewards_min_size: d("20"),
            orderbook_token_ids: vec!["yes".to_string(), "no".to_string()],
            legs: Vec::new(),
            updated_at: OffsetDateTime::now_utc(),
        }
    }

    #[test]
    fn refresh_candidates_keep_full_priority_queue_and_exposure_first() {
        let mut config = RewardBotConfig::default();
        // Request budget stays 3, but the ordered queue must still include every
        // standard candidate so cache-fresh heads cannot starve lower scores.
        config.ai_provider_max_markets = 3;
        let plans = vec![
            candidate_plan("cond_low", d("1")),
            candidate_plan("cond_high", d("90")),
            candidate_plan("cond_mid", d("50")),
            candidate_plan("cond_exposure", d("2")),
        ];
        let now = OffsetDateTime::now_utc();
        let open_orders = vec![ManagedRewardOrder {
            id: "order-1".to_string(),
            account_id: "acct".to_string(),
            condition_id: "cond_exposure".to_string(),
            token_id: "token".to_string(),
            outcome: "YES".to_string(),
            side: RewardOrderSide::Buy,
            price: d("0.5"),
            size: d("10"),
            strategy_bucket: RewardStrategyBucket::Standard,
            strategy_profile: RewardStrategyProfile::Standard,
            exit_strategy_source: RewardExitStrategySource::Configured,
            exit_strategy_selected: None,
            exit_floor_price: None,
            exit_reselect_count: 0,
            exit_last_reselected_at: None,
            external_order_id: Some("ext-1".to_string()),
            status: ManagedRewardOrderStatus::Open,
            scoring: true,
            reason: String::new(),
            filled_size: Decimal::ZERO,
            reward_earned: Decimal::ZERO,
            last_scored_at: None,
            created_at: now,
            updated_at: now,
        }];
        let union = plans
            .iter()
            .map(|plan| plan.condition_id.clone())
            .collect::<Vec<_>>();
        let ordered = reward_provider_refresh_candidate_condition_ids(
            &union,
            &plans,
            &open_orders,
            &[],
            &config,
        );
        assert_eq!(
            ordered,
            vec![
                "cond_exposure".to_string(),
                "cond_high".to_string(),
                "cond_mid".to_string(),
                "cond_low".to_string(),
            ]
        );
    }

    #[test]
    fn refresh_candidates_prefer_missing_info_risk_before_score() {
        let config = RewardBotConfig::default();
        let now = OffsetDateTime::now_utc();
        let mut with_info = candidate_plan("cond_cached", d("90"));
        with_info.info_risk = Some(RewardMarketInfoRisk {
            condition_id: "cond_cached".to_string(),
            provider: config.ai_provider,
            request_format: config.ai_request_format,
            model: "model".to_string(),
            query_hash: "query-hash".to_string(),
            input_hash: "input-hash".to_string(),
            action: polyedge_application::RewardProviderAction::Allow,
            risk_level: polyedge_application::RewardInfoRiskLevel::Low,
            risk_type: polyedge_application::RewardInfoRiskType::None,
            directional_risk: polyedge_application::RewardInfoDirectionalRisk::Unclear,
            resolution_imminent: false,
            expected_event_at: None,
            confidence: Decimal::ONE,
            summary: "ok".to_string(),
            sources: Vec::new(),
            metrics: json!({}),
            created_at: now,
            expires_at: now + time::Duration::hours(1),
        });
        let missing = candidate_plan("cond_missing", d("1"));
        let plans = vec![with_info, missing];
        let union = plans
            .iter()
            .map(|plan| plan.condition_id.clone())
            .collect::<Vec<_>>();
        let ordered = reward_provider_refresh_candidate_condition_ids(
            &union,
            &plans,
            &[],
            &[],
            &config,
        );
        assert_eq!(
            ordered,
            vec![
                "cond_missing".to_string(),
                "cond_cached".to_string(),
            ]
        );
    }
}
