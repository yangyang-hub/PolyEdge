impl Default for RewardBotConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            account_id: "reward_bot".to_string(),
            max_markets: 3,
            max_open_orders: 12,
            per_market_usd: decimal("20"),
            quote_size_usd: decimal("10"),
            min_daily_reward: decimal("1"),
            min_market_liquidity_usd: decimal("1000"),
            min_market_volume_24h_usd: decimal("1000"),
            min_hours_to_end: 48,
            max_market_spread_cents: decimal("10"),
            max_market_data_age_minutes: 15,
            min_market_score: decimal("15"),
            max_spread_cents: decimal("8"),
            quote_mode: RewardQuoteMode::Double,
            selection_mode: RewardSelectionMode::Observe,
            quote_bid_rank: 1,
            dominant_single_side_enabled: false,
            dominant_min_probability: decimal("0.90"),
            dominant_max_probability: decimal("0.97"),
            dominant_min_exit_depth_usd: decimal("50"),
            max_top1_depth_share: decimal("1"),
            max_top3_depth_share: decimal("1"),
            max_book_hhi: decimal("1"),
            preferred_categories: vec![
                "politics".to_string(),
                "elections".to_string(),
                "geopolitics".to_string(),
            ],
            preferred_category_score_bonus: decimal("0"),
            opportunity_metrics_enabled: true,
            opportunity_probe_notional_usd: decimal("10"),
            opportunity_min_reward_per_100_usd_day: decimal("0.75"),
            opportunity_max_competition_multiple: decimal("4"),
            opportunity_competition_hard_gate_enabled: true,
            opportunity_competition_hard_gate_multiple: decimal("1000"),
            opportunity_max_account_allocation_bps: 1_500,
            opportunity_max_market_allocation_bps: 500,
            opportunity_min_exit_depth_usd: decimal("60"),
            opportunity_min_exit_depth_multiple: decimal("2.5"),
            opportunity_max_entry_exit_slippage_cents: decimal("2"),
            opportunity_max_bad_fill_recovery_days: decimal("3"),
            opportunity_observation_window_sec: 1800,
            opportunity_min_book_samples: 30,
            opportunity_max_midpoint_range_cents: decimal("3"),
            opportunity_max_top_of_book_flip_count: 8,
            opportunity_reward_weight: decimal("35"),
            opportunity_competition_weight: decimal("30"),
            opportunity_exit_weight: decimal("25"),
            opportunity_stability_weight: decimal("10"),
            fair_value_enabled: true,
            fair_value_record_history_enabled: true,
            fair_value_min_confidence: decimal("0.55"),
            fair_value_min_raw_edge_cents: decimal("0.25"),
            fair_value_min_effective_edge_cents: decimal("0.75"),
            fair_value_uncertainty_buffer_cents: decimal("0.75"),
            fair_value_rebate_haircut: decimal("0.25"),
            fair_value_max_reward_rebate_cents: decimal("2"),
            fair_value_max_midpoint_deviation_cents: decimal("3"),
            fair_value_history_window_sec: 300,
            fair_value_min_history_samples: 3,
            ai_advisory_enabled: false,
            ai_provider: RewardAiProvider::OpenAi,
            ai_request_format: RewardAiRequestFormat::OpenAiResponses,
            ai_advisory_ttl_sec: 3600,
            ai_provider_concurrency_enabled: false,
            ai_provider_primary_max_concurrency: 1,
            ai_provider_fallback_max_concurrency: 1,
            ai_strategy_hint_enabled: true,
            ai_strategy_hint_min_confidence: decimal("0.75"),
            info_risk_enabled: false,
            info_risk_mode: RewardSelectionMode::Observe,
            info_risk_avoid_level: RewardInfoRiskLevel::High,
            info_risk_ttl_sec: 3600,
            ai_advisory_provider_pending_grace_sec: 120,
            info_risk_provider_pending_grace_sec: 120,
            event_window_enabled: true,
            event_window_min_confidence: RewardEventTimeConfidence::High,
            event_window_stop_new_quote_before_start_sec: 10_800,
            event_window_cancel_open_buy_before_start_sec: 3_600,
            event_window_resume_after_event_end_sec: 3_600,
            event_window_unknown_event_time_mode: RewardUnknownEventTimeMode::Observe,
            event_window_gamma_unreviewed_dates_mode: RewardGammaEventDateMode::Ignore,
            require_info_risk_before_first_quote: true,
            first_quote_quarantine_sec: 600,
            safety_margin_cents: decimal("1"),
            min_midpoint: decimal("0.1"),
            max_midpoint: decimal("0.9"),
            stale_book_ms: 45_000,
            min_scoring_check_sec: 45,
            max_position_usd: decimal("20"),
            max_global_position_usd: decimal("50"),
            exit_markup_cents: Decimal::ZERO,
            cancel_on_fill: true,
            account_capital_usd: decimal("1000"),
            requote_drift_cents: decimal("2"),
            requote_drift_confirm_sec: 60,
            requote_drift_cooldown_sec: 120,
            requote_drift_max_cancels_per_cycle: 2,
            post_fill_strategy: PostFillStrategy::ExitAtMarkup,
            adaptive_flatten_min_bid_depth_usd: decimal("5"),
            adaptive_flatten_min_depth_multiple: decimal("1.25"),
            adaptive_flatten_min_surplus_cents: Decimal::ZERO,
            adaptive_flatten_when_plan_ineligible: true,
            adaptive_flatten_when_event_risk: true,
            adaptive_hold_when_plan_eligible: true,
            adaptive_fallback_strategy: PostFillStrategy::ExitAtMarkup,
            adaptive_exit_recheck_sec: 30,
            adaptive_exit_reselect_cooldown_sec: 120,
            adaptive_exit_max_reselects_per_order: 3,
            adaptive_exit_min_strategy_improvement_cents: decimal("1"),
            adaptive_exit_cancel_replace_enabled: true,
            balanced_merge_enabled: false,
            balanced_merge_max_markets: 2,
            balanced_merge_max_open_orders: 4,
            balanced_merge_min_edge_cents: decimal("3"),
            balanced_merge_min_market_score: decimal("8"),
            balanced_merge_min_market_liquidity_usd: Decimal::ZERO,
            balanced_merge_min_market_volume_24h_usd: Decimal::ZERO,
            balanced_merge_max_market_spread_cents: decimal("20"),
            balanced_merge_quote_bid_rank: 1,
            balanced_merge_max_unpaired_position_usd: decimal("20"),
            balanced_merge_auto_execute_enabled: false,
            // Risk control defaults: all disabled (0 = off)
            min_depth_usd: Decimal::ZERO,
            cancel_bid_rank: 0,
            depth_drop_pct: Decimal::ZERO,
            depth_drop_window_sec: 10,
            fill_velocity_usd: Decimal::ZERO,
            fill_velocity_window_sec: 10,
            mass_cancel_pct: Decimal::ZERO,
            mass_cancel_window_sec: 10,
            requote_interval_sec: 0,
            requote_jitter_sec: 0,
            reconcile_interval_sec: 5,
        }
    }
}

impl RewardBotConfig {
    #[must_use]
    pub fn normalized(mut self) -> Self {
        self.account_id = normalize_account_id(&self.account_id);
        self.max_markets = clamp_u16(self.max_markets, 0, u16::MAX);
        self.max_open_orders = clamp_u16(self.max_open_orders, 0, u16::MAX);
        self.per_market_usd = clamp_decimal(self.per_market_usd, Decimal::ZERO, decimal("1000000"));
        self.quote_size_usd = clamp_decimal(self.quote_size_usd, Decimal::ZERO, decimal("1000000"));
        self.min_daily_reward =
            clamp_decimal(self.min_daily_reward, Decimal::ZERO, decimal("100000"));
        self.min_market_liquidity_usd = clamp_decimal(
            self.min_market_liquidity_usd,
            Decimal::ZERO,
            decimal("1000000000"),
        );
        self.min_market_volume_24h_usd = clamp_decimal(
            self.min_market_volume_24h_usd,
            Decimal::ZERO,
            decimal("1000000000"),
        );
        self.min_hours_to_end = self.min_hours_to_end.clamp(0, 24 * 365 * 10);
        self.max_market_spread_cents =
            clamp_decimal(self.max_market_spread_cents, decimal("0.1"), decimal("100"));
        self.max_market_data_age_minutes = self.max_market_data_age_minutes.clamp(1, 1440);
        self.min_market_score = clamp_decimal(self.min_market_score, Decimal::ZERO, decimal("100"));
        self.max_spread_cents = clamp_decimal(self.max_spread_cents, decimal("0.1"), decimal("99"));
        self.quote_bid_rank = self.quote_bid_rank.clamp(1, 3);
        self.dominant_min_probability = clamp_decimal(
            self.dominant_min_probability,
            decimal("0.51"),
            decimal("0.99"),
        );
        self.dominant_max_probability = clamp_decimal(
            self.dominant_max_probability,
            self.dominant_min_probability,
            decimal("0.99"),
        );
        self.dominant_min_exit_depth_usd = clamp_decimal(
            self.dominant_min_exit_depth_usd,
            Decimal::ZERO,
            decimal("1000000"),
        );
        self.max_top1_depth_share =
            clamp_decimal(self.max_top1_depth_share, Decimal::ZERO, Decimal::ONE);
        self.max_top3_depth_share =
            clamp_decimal(self.max_top3_depth_share, Decimal::ZERO, Decimal::ONE);
        if self.max_top3_depth_share < self.max_top1_depth_share {
            self.max_top3_depth_share = self.max_top1_depth_share;
        }
        self.max_book_hhi = clamp_decimal(self.max_book_hhi, Decimal::ZERO, Decimal::ONE);
        self.preferred_categories = normalize_reward_categories(&self.preferred_categories);
        self.preferred_category_score_bonus = clamp_decimal(
            self.preferred_category_score_bonus,
            Decimal::ZERO,
            decimal("20"),
        );
        self.opportunity_probe_notional_usd = clamp_decimal(
            self.opportunity_probe_notional_usd,
            Decimal::ZERO,
            decimal("1000000"),
        );
        self.opportunity_min_reward_per_100_usd_day = clamp_decimal(
            self.opportunity_min_reward_per_100_usd_day,
            Decimal::ZERO,
            decimal("100000"),
        );
        self.opportunity_max_competition_multiple = clamp_decimal(
            self.opportunity_max_competition_multiple,
            Decimal::ZERO,
            decimal("1000000"),
        );
        self.opportunity_competition_hard_gate_multiple = clamp_decimal(
            self.opportunity_competition_hard_gate_multiple,
            Decimal::ZERO,
            decimal("1000000"),
        );
        self.opportunity_max_account_allocation_bps =
            self.opportunity_max_account_allocation_bps.clamp(0, 10_000);
        self.opportunity_max_market_allocation_bps =
            self.opportunity_max_market_allocation_bps.clamp(0, 10_000);
        self.opportunity_min_exit_depth_usd = clamp_decimal(
            self.opportunity_min_exit_depth_usd,
            Decimal::ZERO,
            decimal("1000000"),
        );
        self.opportunity_min_exit_depth_multiple = clamp_decimal(
            self.opportunity_min_exit_depth_multiple,
            Decimal::ZERO,
            decimal("100"),
        );
        self.opportunity_max_entry_exit_slippage_cents = clamp_decimal(
            self.opportunity_max_entry_exit_slippage_cents,
            Decimal::ZERO,
            decimal("99"),
        );
        self.opportunity_max_bad_fill_recovery_days = clamp_decimal(
            self.opportunity_max_bad_fill_recovery_days,
            Decimal::ZERO,
            decimal("365"),
        );
        self.opportunity_observation_window_sec =
            self.opportunity_observation_window_sec.clamp(60, 86_400);
        self.opportunity_min_book_samples = self.opportunity_min_book_samples.clamp(1, 10_000);
        self.opportunity_max_midpoint_range_cents = clamp_decimal(
            self.opportunity_max_midpoint_range_cents,
            Decimal::ZERO,
            decimal("100"),
        );
        self.opportunity_max_top_of_book_flip_count =
            self.opportunity_max_top_of_book_flip_count.clamp(0, 10_000);
        self.opportunity_reward_weight = clamp_decimal(
            self.opportunity_reward_weight,
            Decimal::ZERO,
            decimal("1000"),
        );
        self.opportunity_competition_weight = clamp_decimal(
            self.opportunity_competition_weight,
            Decimal::ZERO,
            decimal("1000"),
        );
        self.opportunity_exit_weight =
            clamp_decimal(self.opportunity_exit_weight, Decimal::ZERO, decimal("1000"));
        self.opportunity_stability_weight = clamp_decimal(
            self.opportunity_stability_weight,
            Decimal::ZERO,
            decimal("1000"),
        );
        self.fair_value_min_confidence = clamp_decimal(
            self.fair_value_min_confidence,
            Decimal::ZERO,
            Decimal::ONE,
        );
        self.fair_value_min_raw_edge_cents = clamp_decimal(
            self.fair_value_min_raw_edge_cents,
            Decimal::ZERO,
            decimal("50"),
        );
        self.fair_value_min_effective_edge_cents = clamp_decimal(
            self.fair_value_min_effective_edge_cents,
            Decimal::ZERO,
            decimal("50"),
        );
        self.fair_value_uncertainty_buffer_cents = clamp_decimal(
            self.fair_value_uncertainty_buffer_cents,
            Decimal::ZERO,
            decimal("50"),
        );
        self.fair_value_rebate_haircut = clamp_decimal(
            self.fair_value_rebate_haircut,
            Decimal::ZERO,
            Decimal::ONE,
        );
        self.fair_value_max_reward_rebate_cents = clamp_decimal(
            self.fair_value_max_reward_rebate_cents,
            Decimal::ZERO,
            decimal("50"),
        );
        self.fair_value_max_midpoint_deviation_cents = clamp_decimal(
            self.fair_value_max_midpoint_deviation_cents,
            Decimal::ZERO,
            decimal("50"),
        );
        self.fair_value_history_window_sec = self.fair_value_history_window_sec.clamp(0, 86_400);
        self.fair_value_min_history_samples = self.fair_value_min_history_samples.clamp(0, 10_000);
        self.ai_advisory_ttl_sec = self.ai_advisory_ttl_sec.clamp(60, 86_400);
        self.ai_provider_primary_max_concurrency =
            self.ai_provider_primary_max_concurrency.clamp(1, 10);
        self.ai_provider_fallback_max_concurrency =
            self.ai_provider_fallback_max_concurrency.clamp(1, 10);
        self.ai_strategy_hint_min_confidence = clamp_decimal(
            self.ai_strategy_hint_min_confidence,
            Decimal::ZERO,
            Decimal::ONE,
        );
        self.info_risk_ttl_sec = self.info_risk_ttl_sec.clamp(60, 86_400);
        self.ai_advisory_provider_pending_grace_sec = self
            .ai_advisory_provider_pending_grace_sec
            .clamp(0, 86_400);
        self.info_risk_provider_pending_grace_sec = self
            .info_risk_provider_pending_grace_sec
            .clamp(0, 86_400);
        self.event_window_stop_new_quote_before_start_sec = self
            .event_window_stop_new_quote_before_start_sec
            .clamp(0, 86_400 * 30);
        self.event_window_cancel_open_buy_before_start_sec = self
            .event_window_cancel_open_buy_before_start_sec
            .clamp(0, 86_400 * 30);
        self.event_window_resume_after_event_end_sec = self
            .event_window_resume_after_event_end_sec
            .clamp(0, 86_400 * 30);
        if self.event_window_cancel_open_buy_before_start_sec
            > self.event_window_stop_new_quote_before_start_sec
        {
            self.event_window_cancel_open_buy_before_start_sec =
                self.event_window_stop_new_quote_before_start_sec;
        }
        self.first_quote_quarantine_sec = self.first_quote_quarantine_sec.clamp(0, 86_400);
        match self.ai_provider {
            RewardAiProvider::Anthropic => {
                self.ai_request_format = RewardAiRequestFormat::AnthropicMessages;
            }
            RewardAiProvider::OpenAi
                if matches!(
                    self.ai_request_format,
                    RewardAiRequestFormat::AnthropicMessages
                ) =>
            {
                self.ai_request_format = RewardAiRequestFormat::OpenAiResponses;
            }
            RewardAiProvider::OpenAi => {}
        }
        self.safety_margin_cents =
            clamp_decimal(self.safety_margin_cents, Decimal::ZERO, decimal("20"));
        self.min_midpoint = clamp_decimal(self.min_midpoint, Decimal::ZERO, decimal("0.49"));
        self.max_midpoint = clamp_decimal(self.max_midpoint, decimal("0.51"), Decimal::ONE);
        if self.max_midpoint <= self.min_midpoint {
            self.max_midpoint = Decimal::min(Decimal::ONE, self.min_midpoint + decimal("0.1"));
        }
        self.stale_book_ms = self.stale_book_ms.clamp(5_000, 120_000);
        self.min_scoring_check_sec = self.min_scoring_check_sec.clamp(15, 600);
        self.max_position_usd =
            clamp_decimal(self.max_position_usd, Decimal::ZERO, decimal("1000000"));
        self.max_global_position_usd = clamp_decimal(
            self.max_global_position_usd,
            Decimal::ZERO,
            decimal("10000000"),
        );
        self.exit_markup_cents =
            clamp_decimal(self.exit_markup_cents, Decimal::ZERO, decimal("50"));
        self.account_capital_usd =
            clamp_decimal(self.account_capital_usd, decimal("1"), decimal("100000000"));
        self.requote_drift_cents =
            clamp_decimal(self.requote_drift_cents, Decimal::ZERO, decimal("99"));
        self.requote_drift_confirm_sec = self.requote_drift_confirm_sec.clamp(0, 3600);
        self.requote_drift_cooldown_sec = self.requote_drift_cooldown_sec.clamp(0, 86_400);
        self.requote_drift_max_cancels_per_cycle =
            self.requote_drift_max_cancels_per_cycle.clamp(0, 100);
        self.adaptive_flatten_min_bid_depth_usd = clamp_decimal(
            self.adaptive_flatten_min_bid_depth_usd,
            Decimal::ZERO,
            decimal("1000000"),
        );
        self.adaptive_flatten_min_depth_multiple = clamp_decimal(
            self.adaptive_flatten_min_depth_multiple,
            Decimal::ZERO,
            decimal("100"),
        );
        self.adaptive_flatten_min_surplus_cents = clamp_decimal(
            self.adaptive_flatten_min_surplus_cents,
            Decimal::ZERO,
            decimal("50"),
        );
        if self.adaptive_fallback_strategy == PostFillStrategy::Adaptive {
            self.adaptive_fallback_strategy = PostFillStrategy::ExitAtMarkup;
        }
        self.adaptive_exit_recheck_sec = self.adaptive_exit_recheck_sec.clamp(5, 86_400);
        self.adaptive_exit_reselect_cooldown_sec =
            self.adaptive_exit_reselect_cooldown_sec.clamp(0, 86_400);
        self.adaptive_exit_max_reselects_per_order =
            self.adaptive_exit_max_reselects_per_order.clamp(0, 100);
        self.adaptive_exit_min_strategy_improvement_cents = clamp_decimal(
            self.adaptive_exit_min_strategy_improvement_cents,
            Decimal::ZERO,
            decimal("50"),
        );
        self.balanced_merge_max_markets = self.balanced_merge_max_markets.clamp(0, u16::MAX);
        self.balanced_merge_max_open_orders =
            self.balanced_merge_max_open_orders.clamp(0, u16::MAX);
        self.balanced_merge_min_edge_cents = clamp_decimal(
            self.balanced_merge_min_edge_cents,
            Decimal::ZERO,
            decimal("20"),
        );
        self.balanced_merge_min_market_score = clamp_decimal(
            self.balanced_merge_min_market_score,
            Decimal::ZERO,
            decimal("100"),
        );
        self.balanced_merge_min_market_liquidity_usd = clamp_decimal(
            self.balanced_merge_min_market_liquidity_usd,
            Decimal::ZERO,
            decimal("1000000000"),
        );
        self.balanced_merge_min_market_volume_24h_usd = clamp_decimal(
            self.balanced_merge_min_market_volume_24h_usd,
            Decimal::ZERO,
            decimal("1000000000"),
        );
        self.balanced_merge_max_market_spread_cents = clamp_decimal(
            self.balanced_merge_max_market_spread_cents,
            decimal("0.1"),
            decimal("100"),
        );
        self.balanced_merge_quote_bid_rank = self.balanced_merge_quote_bid_rank.clamp(1, 3);
        self.balanced_merge_max_unpaired_position_usd = clamp_decimal(
            self.balanced_merge_max_unpaired_position_usd,
            Decimal::ZERO,
            decimal("1000000"),
        );
        if !self.balanced_merge_enabled {
            self.balanced_merge_auto_execute_enabled = false;
        }
        // Risk control clamps
        self.min_depth_usd = clamp_decimal(self.min_depth_usd, Decimal::ZERO, decimal("1000000"));
        self.cancel_bid_rank = self
            .cancel_bid_rank
            .clamp(0, self.quote_bid_rank.saturating_sub(1));
        self.depth_drop_pct = clamp_decimal(self.depth_drop_pct, Decimal::ZERO, decimal("100"));
        self.depth_drop_window_sec = self.depth_drop_window_sec.clamp(0, 300);
        self.fill_velocity_usd =
            clamp_decimal(self.fill_velocity_usd, Decimal::ZERO, decimal("1000000"));
        self.fill_velocity_window_sec = self.fill_velocity_window_sec.clamp(0, 300);
        self.mass_cancel_pct = clamp_decimal(self.mass_cancel_pct, Decimal::ZERO, decimal("100"));
        self.mass_cancel_window_sec = self.mass_cancel_window_sec.clamp(0, 300);
        self.requote_interval_sec = self.requote_interval_sec.clamp(0, 3600);
        self.requote_jitter_sec = self.requote_jitter_sec.clamp(0, 600);
        self.reconcile_interval_sec = self.reconcile_interval_sec.clamp(1, 60);
        self
    }

    /// Build a `RewardCandidateFilter` from this config for SQL-level filtering.
    #[must_use]
    pub fn candidate_filter(&self) -> RewardCandidateFilter {
        RewardCandidateFilter {
            min_daily_reward: self.min_daily_reward,
            min_midpoint: self.min_midpoint,
            max_midpoint: self.max_midpoint,
            min_market_liquidity_usd: self.min_market_liquidity_usd,
            min_market_volume_24h_usd: self.min_market_volume_24h_usd,
            min_hours_to_end: self.min_hours_to_end,
            max_market_spread_cents: self.max_market_spread_cents,
            max_market_data_age_minutes: self.max_market_data_age_minutes,
            max_rewards_spread_cents: self.max_spread_cents,
            allow_dominant_single_side: self.quote_mode == RewardQuoteMode::Auto
                && self.dominant_single_side_enabled,
            dominant_min_probability: self.dominant_min_probability,
            dominant_max_probability: self.dominant_max_probability,
            prefer_sparse_market_ordering: false,
        }
    }

    #[must_use]
    pub fn balanced_merge_candidate_filter(&self) -> Option<RewardCandidateFilter> {
        if !self.balanced_merge_enabled
            || self.balanced_merge_max_markets == 0
            || self.balanced_merge_max_open_orders == 0
        {
            return None;
        }

        let mut filter = self.candidate_filter();
        filter.min_market_liquidity_usd = self.balanced_merge_min_market_liquidity_usd;
        filter.min_market_volume_24h_usd = self.balanced_merge_min_market_volume_24h_usd;
        filter.max_market_spread_cents = self.balanced_merge_max_market_spread_cents;
        filter.allow_dominant_single_side = false;
        filter.prefer_sparse_market_ordering = true;
        Some(filter)
    }

    #[must_use]
    pub fn config_for_strategy_bucket(&self, _bucket: RewardStrategyBucket) -> Self {
        self.clone()
    }

    #[must_use]
    pub fn config_for_strategy_profile(&self, profile: RewardStrategyProfile) -> Self {
        let mut config = self.clone();
        if profile != RewardStrategyProfile::BalancedMerge {
            return config;
        }

        if !self.balanced_merge_enabled {
            config.max_markets = 0;
            config.max_open_orders = 0;
            return config.normalized();
        }

        config.max_markets = self.balanced_merge_max_markets;
        config.max_open_orders = self.balanced_merge_max_open_orders;
        config.min_market_score = self.balanced_merge_min_market_score;
        config.min_market_liquidity_usd = self.balanced_merge_min_market_liquidity_usd;
        config.min_market_volume_24h_usd = self.balanced_merge_min_market_volume_24h_usd;
        config.max_market_spread_cents = self.balanced_merge_max_market_spread_cents;
        config.quote_mode = RewardQuoteMode::Double;
        config.selection_mode = RewardSelectionMode::Observe;
        config.dominant_single_side_enabled = false;
        config.quote_bid_rank = self.balanced_merge_quote_bid_rank;
        config.safety_margin_cents = self.balanced_merge_min_edge_cents;
        config.max_position_usd = self.balanced_merge_max_unpaired_position_usd;
        config.normalized()
    }

    #[must_use]
    pub fn apply_patch(&self, patch: RewardBotConfigPatch) -> Self {
        let mut next = self.clone();
        if let Some(enabled) = patch.enabled {
            next.enabled = enabled;
        }
        if let Some(account_id) = patch.account_id {
            next.account_id = account_id;
        }
        if let Some(max_markets) = patch.max_markets {
            next.max_markets = max_markets;
        }
        if let Some(max_open_orders) = patch.max_open_orders {
            next.max_open_orders = max_open_orders;
        }
        if let Some(per_market_usd) = patch.per_market_usd {
            next.per_market_usd = per_market_usd;
        }
        if let Some(quote_size_usd) = patch.quote_size_usd {
            next.quote_size_usd = quote_size_usd;
        }
        if let Some(min_daily_reward) = patch.min_daily_reward {
            next.min_daily_reward = min_daily_reward;
        }
        if let Some(value) = patch.min_market_liquidity_usd {
            next.min_market_liquidity_usd = value;
        }
        if let Some(value) = patch.min_market_volume_24h_usd {
            next.min_market_volume_24h_usd = value;
        }
        if let Some(value) = patch.min_hours_to_end {
            next.min_hours_to_end = value;
        }
        if let Some(value) = patch.max_market_spread_cents {
            next.max_market_spread_cents = value;
        }
        if let Some(value) = patch.max_market_data_age_minutes {
            next.max_market_data_age_minutes = value;
        }
        if let Some(min_market_score) = patch.min_market_score {
            next.min_market_score = min_market_score;
        }
        if let Some(max_spread_cents) = patch.max_spread_cents {
            next.max_spread_cents = max_spread_cents;
        }
        if let Some(quote_mode) = patch.quote_mode {
            next.quote_mode = quote_mode;
        }
        if let Some(selection_mode) = patch.selection_mode {
            next.selection_mode = selection_mode;
        }
        if let Some(quote_bid_rank) = patch.quote_bid_rank {
            next.quote_bid_rank = quote_bid_rank;
        }
        if let Some(value) = patch.dominant_single_side_enabled {
            next.dominant_single_side_enabled = value;
        }
        if let Some(value) = patch.dominant_min_probability {
            next.dominant_min_probability = value;
        }
        if let Some(value) = patch.dominant_max_probability {
            next.dominant_max_probability = value;
        }
        if let Some(value) = patch.dominant_min_exit_depth_usd {
            next.dominant_min_exit_depth_usd = value;
        }
        if let Some(value) = patch.max_top1_depth_share {
            next.max_top1_depth_share = value;
        }
        if let Some(value) = patch.max_top3_depth_share {
            next.max_top3_depth_share = value;
        }
        if let Some(value) = patch.max_book_hhi {
            next.max_book_hhi = value;
        }
        if let Some(value) = patch.preferred_categories {
            next.preferred_categories = value;
        }
        if let Some(value) = patch.preferred_category_score_bonus {
            next.preferred_category_score_bonus = value;
        }
        if let Some(value) = patch.opportunity_metrics_enabled {
            next.opportunity_metrics_enabled = value;
        }
        if let Some(value) = patch.opportunity_probe_notional_usd {
            next.opportunity_probe_notional_usd = value;
        }
        if let Some(value) = patch.opportunity_min_reward_per_100_usd_day {
            next.opportunity_min_reward_per_100_usd_day = value;
        }
        if let Some(value) = patch.opportunity_max_competition_multiple {
            next.opportunity_max_competition_multiple = value;
        }
        if let Some(value) = patch.opportunity_competition_hard_gate_enabled {
            next.opportunity_competition_hard_gate_enabled = value;
        }
        if let Some(value) = patch.opportunity_competition_hard_gate_multiple {
            next.opportunity_competition_hard_gate_multiple = value;
        }
        if let Some(value) = patch.opportunity_max_account_allocation_bps {
            next.opportunity_max_account_allocation_bps = value;
        }
        if let Some(value) = patch.opportunity_max_market_allocation_bps {
            next.opportunity_max_market_allocation_bps = value;
        }
        if let Some(value) = patch.opportunity_min_exit_depth_usd {
            next.opportunity_min_exit_depth_usd = value;
        }
        if let Some(value) = patch.opportunity_min_exit_depth_multiple {
            next.opportunity_min_exit_depth_multiple = value;
        }
        if let Some(value) = patch.opportunity_max_entry_exit_slippage_cents {
            next.opportunity_max_entry_exit_slippage_cents = value;
        }
        if let Some(value) = patch.opportunity_max_bad_fill_recovery_days {
            next.opportunity_max_bad_fill_recovery_days = value;
        }
        if let Some(value) = patch.opportunity_observation_window_sec {
            next.opportunity_observation_window_sec = value;
        }
        if let Some(value) = patch.opportunity_min_book_samples {
            next.opportunity_min_book_samples = value;
        }
        if let Some(value) = patch.opportunity_max_midpoint_range_cents {
            next.opportunity_max_midpoint_range_cents = value;
        }
        if let Some(value) = patch.opportunity_max_top_of_book_flip_count {
            next.opportunity_max_top_of_book_flip_count = value;
        }
        if let Some(value) = patch.opportunity_reward_weight {
            next.opportunity_reward_weight = value;
        }
        if let Some(value) = patch.opportunity_competition_weight {
            next.opportunity_competition_weight = value;
        }
        if let Some(value) = patch.opportunity_exit_weight {
            next.opportunity_exit_weight = value;
        }
        if let Some(value) = patch.opportunity_stability_weight {
            next.opportunity_stability_weight = value;
        }
        if let Some(value) = patch.fair_value_enabled {
            next.fair_value_enabled = value;
        }
        if let Some(value) = patch.fair_value_record_history_enabled {
            next.fair_value_record_history_enabled = value;
        }
        if let Some(value) = patch.fair_value_min_confidence {
            next.fair_value_min_confidence = value;
        }
        if let Some(value) = patch.fair_value_min_raw_edge_cents {
            next.fair_value_min_raw_edge_cents = value;
        }
        if let Some(value) = patch.fair_value_min_effective_edge_cents {
            next.fair_value_min_effective_edge_cents = value;
        }
        if let Some(value) = patch.fair_value_uncertainty_buffer_cents {
            next.fair_value_uncertainty_buffer_cents = value;
        }
        if let Some(value) = patch.fair_value_rebate_haircut {
            next.fair_value_rebate_haircut = value;
        }
        if let Some(value) = patch.fair_value_max_reward_rebate_cents {
            next.fair_value_max_reward_rebate_cents = value;
        }
        if let Some(value) = patch.fair_value_max_midpoint_deviation_cents {
            next.fair_value_max_midpoint_deviation_cents = value;
        }
        if let Some(value) = patch.fair_value_history_window_sec {
            next.fair_value_history_window_sec = value;
        }
        if let Some(value) = patch.fair_value_min_history_samples {
            next.fair_value_min_history_samples = value;
        }
        if let Some(value) = patch.ai_advisory_enabled {
            next.ai_advisory_enabled = value;
        }
        if let Some(value) = patch.ai_provider {
            next.ai_provider = value;
        }
        if let Some(value) = patch.ai_request_format {
            next.ai_request_format = value;
        }
        if let Some(value) = patch.ai_advisory_ttl_sec {
            next.ai_advisory_ttl_sec = value;
        }
        if let Some(value) = patch.ai_provider_concurrency_enabled {
            next.ai_provider_concurrency_enabled = value;
        }
        if let Some(value) = patch.ai_provider_primary_max_concurrency {
            next.ai_provider_primary_max_concurrency = value;
        }
        if let Some(value) = patch.ai_provider_fallback_max_concurrency {
            next.ai_provider_fallback_max_concurrency = value;
        }
        if let Some(value) = patch.ai_strategy_hint_enabled {
            next.ai_strategy_hint_enabled = value;
        }
        if let Some(value) = patch.ai_strategy_hint_min_confidence {
            next.ai_strategy_hint_min_confidence = value;
        }
        if let Some(value) = patch.info_risk_enabled {
            next.info_risk_enabled = value;
        }
        if let Some(value) = patch.info_risk_mode {
            next.info_risk_mode = value;
        }
        if let Some(value) = patch.info_risk_avoid_level {
            next.info_risk_avoid_level = value;
        }
        if let Some(value) = patch.info_risk_ttl_sec {
            next.info_risk_ttl_sec = value;
        }
        if let Some(value) = patch.ai_advisory_provider_pending_grace_sec {
            next.ai_advisory_provider_pending_grace_sec = value;
        }
        if let Some(value) = patch.info_risk_provider_pending_grace_sec {
            next.info_risk_provider_pending_grace_sec = value;
        }
        if let Some(value) = patch.event_window_enabled {
            next.event_window_enabled = value;
        }
        if let Some(value) = patch.event_window_min_confidence {
            next.event_window_min_confidence = value;
        }
        if let Some(value) = patch.event_window_stop_new_quote_before_start_sec {
            next.event_window_stop_new_quote_before_start_sec = value;
        }
        if let Some(value) = patch.event_window_cancel_open_buy_before_start_sec {
            next.event_window_cancel_open_buy_before_start_sec = value;
        }
        if let Some(value) = patch.event_window_resume_after_event_end_sec {
            next.event_window_resume_after_event_end_sec = value;
        }
        if let Some(value) = patch.event_window_unknown_event_time_mode {
            next.event_window_unknown_event_time_mode = value;
        }
        if let Some(value) = patch.event_window_gamma_unreviewed_dates_mode {
            next.event_window_gamma_unreviewed_dates_mode = value;
        }
        if let Some(value) = patch.require_info_risk_before_first_quote {
            next.require_info_risk_before_first_quote = value;
        }
        if let Some(value) = patch.first_quote_quarantine_sec {
            next.first_quote_quarantine_sec = value;
        }
        if let Some(safety_margin_cents) = patch.safety_margin_cents {
            next.safety_margin_cents = safety_margin_cents;
        }
        if let Some(min_midpoint) = patch.min_midpoint {
            next.min_midpoint = min_midpoint;
        }
        if let Some(max_midpoint) = patch.max_midpoint {
            next.max_midpoint = max_midpoint;
        }
        if let Some(stale_book_ms) = patch.stale_book_ms {
            next.stale_book_ms = stale_book_ms;
        }
        if let Some(min_scoring_check_sec) = patch.min_scoring_check_sec {
            next.min_scoring_check_sec = min_scoring_check_sec;
        }
        if let Some(max_position_usd) = patch.max_position_usd {
            next.max_position_usd = max_position_usd;
        }
        if let Some(max_global_position_usd) = patch.max_global_position_usd {
            next.max_global_position_usd = max_global_position_usd;
        }
        if let Some(exit_markup_cents) = patch.exit_markup_cents {
            next.exit_markup_cents = exit_markup_cents;
        }
        if let Some(cancel_on_fill) = patch.cancel_on_fill {
            next.cancel_on_fill = cancel_on_fill;
        }
        if let Some(account_capital_usd) = patch.account_capital_usd {
            next.account_capital_usd = account_capital_usd;
        }
        if let Some(requote_drift_cents) = patch.requote_drift_cents {
            next.requote_drift_cents = requote_drift_cents;
        }
        if let Some(requote_drift_confirm_sec) = patch.requote_drift_confirm_sec {
            next.requote_drift_confirm_sec = requote_drift_confirm_sec;
        }
        if let Some(requote_drift_cooldown_sec) = patch.requote_drift_cooldown_sec {
            next.requote_drift_cooldown_sec = requote_drift_cooldown_sec;
        }
        if let Some(requote_drift_max_cancels_per_cycle) = patch.requote_drift_max_cancels_per_cycle
        {
            next.requote_drift_max_cancels_per_cycle = requote_drift_max_cancels_per_cycle;
        }
        if let Some(post_fill_strategy) = patch.post_fill_strategy {
            next.post_fill_strategy = post_fill_strategy;
        }
        if let Some(value) = patch.adaptive_flatten_min_bid_depth_usd {
            next.adaptive_flatten_min_bid_depth_usd = value;
        }
        if let Some(value) = patch.adaptive_flatten_min_depth_multiple {
            next.adaptive_flatten_min_depth_multiple = value;
        }
        if let Some(value) = patch.adaptive_flatten_min_surplus_cents {
            next.adaptive_flatten_min_surplus_cents = value;
        }
        if let Some(value) = patch.adaptive_flatten_when_plan_ineligible {
            next.adaptive_flatten_when_plan_ineligible = value;
        }
        if let Some(value) = patch.adaptive_flatten_when_event_risk {
            next.adaptive_flatten_when_event_risk = value;
        }
        if let Some(value) = patch.adaptive_hold_when_plan_eligible {
            next.adaptive_hold_when_plan_eligible = value;
        }
        if let Some(value) = patch.adaptive_fallback_strategy {
            next.adaptive_fallback_strategy = value;
        }
        if let Some(value) = patch.adaptive_exit_recheck_sec {
            next.adaptive_exit_recheck_sec = value;
        }
        if let Some(value) = patch.adaptive_exit_reselect_cooldown_sec {
            next.adaptive_exit_reselect_cooldown_sec = value;
        }
        if let Some(value) = patch.adaptive_exit_max_reselects_per_order {
            next.adaptive_exit_max_reselects_per_order = value;
        }
        if let Some(value) = patch.adaptive_exit_min_strategy_improvement_cents {
            next.adaptive_exit_min_strategy_improvement_cents = value;
        }
        if let Some(value) = patch.adaptive_exit_cancel_replace_enabled {
            next.adaptive_exit_cancel_replace_enabled = value;
        }
        if let Some(value) = patch.balanced_merge_enabled {
            next.balanced_merge_enabled = value;
        }
        if let Some(value) = patch.balanced_merge_max_markets {
            next.balanced_merge_max_markets = value;
        }
        if let Some(value) = patch.balanced_merge_max_open_orders {
            next.balanced_merge_max_open_orders = value;
        }
        if let Some(value) = patch.balanced_merge_min_edge_cents {
            next.balanced_merge_min_edge_cents = value;
        }
        if let Some(value) = patch.balanced_merge_min_market_score {
            next.balanced_merge_min_market_score = value;
        }
        if let Some(value) = patch.balanced_merge_min_market_liquidity_usd {
            next.balanced_merge_min_market_liquidity_usd = value;
        }
        if let Some(value) = patch.balanced_merge_min_market_volume_24h_usd {
            next.balanced_merge_min_market_volume_24h_usd = value;
        }
        if let Some(value) = patch.balanced_merge_max_market_spread_cents {
            next.balanced_merge_max_market_spread_cents = value;
        }
        if let Some(value) = patch.balanced_merge_quote_bid_rank {
            next.balanced_merge_quote_bid_rank = value;
        }
        if let Some(value) = patch.balanced_merge_max_unpaired_position_usd {
            next.balanced_merge_max_unpaired_position_usd = value;
        }
        if let Some(value) = patch.balanced_merge_auto_execute_enabled {
            next.balanced_merge_auto_execute_enabled = value;
        }
        // Risk control patches
        if let Some(v) = patch.min_depth_usd {
            next.min_depth_usd = v;
        }
        if let Some(v) = patch.cancel_bid_rank {
            next.cancel_bid_rank = v;
        }
        if let Some(v) = patch.depth_drop_pct {
            next.depth_drop_pct = v;
        }
        if let Some(v) = patch.depth_drop_window_sec {
            next.depth_drop_window_sec = v;
        }
        if let Some(v) = patch.fill_velocity_usd {
            next.fill_velocity_usd = v;
        }
        if let Some(v) = patch.fill_velocity_window_sec {
            next.fill_velocity_window_sec = v;
        }
        if let Some(v) = patch.mass_cancel_pct {
            next.mass_cancel_pct = v;
        }
        if let Some(v) = patch.mass_cancel_window_sec {
            next.mass_cancel_window_sec = v;
        }
        if let Some(v) = patch.requote_interval_sec {
            next.requote_interval_sec = v;
        }
        if let Some(v) = patch.requote_jitter_sec {
            next.requote_jitter_sec = v;
        }
        if let Some(v) = patch.reconcile_interval_sec {
            next.reconcile_interval_sec = v;
        }
        next.normalized()
    }
}

#[cfg(test)]
mod reward_config_tests {
    use super::*;

    #[test]
    fn reward_config_patch_accepts_console_payload() {
        let payload = r#"{
            "enabled": true,
            "account_id": "reward_bot",
            "max_markets": 10,
            "max_open_orders": 100,
            "per_market_usd": 35,
            "quote_size_usd": 15,
            "min_daily_reward": 10,
            "min_market_liquidity_usd": 1000,
            "min_market_volume_24h_usd": 1000,
            "min_hours_to_end": 48,
            "max_market_spread_cents": 4,
            "max_market_data_age_minutes": 15,
            "min_market_score": 30,
            "max_spread_cents": 8,
            "quote_mode": "auto",
            "selection_mode": "enforce",
            "quote_bid_rank": 3,
            "dominant_single_side_enabled": true,
            "dominant_min_probability": 0.9,
            "dominant_max_probability": 0.97,
            "dominant_min_exit_depth_usd": 50,
            "max_top1_depth_share": 1,
            "max_top3_depth_share": 1,
            "max_book_hhi": 1,
            "preferred_categories": ["politics", "elections", "geopolitics"],
            "preferred_category_score_bonus": 0,
            "ai_advisory_enabled": true,
            "ai_provider": "openai",
            "ai_request_format": "openai_chat_completions",
            "ai_advisory_ttl_sec": 36000,
            "ai_provider_concurrency_enabled": true,
            "ai_provider_primary_max_concurrency": 4,
            "ai_provider_fallback_max_concurrency": 2,
            "ai_strategy_hint_enabled": true,
            "ai_strategy_hint_min_confidence": 0.8,
            "info_risk_enabled": true,
            "info_risk_mode": "enforce",
            "info_risk_avoid_level": "high",
            "info_risk_ttl_sec": 36000,
            "event_window_enabled": true,
            "event_window_min_confidence": "medium",
            "event_window_stop_new_quote_before_start_sec": 7200,
            "event_window_cancel_open_buy_before_start_sec": 1800,
            "event_window_resume_after_event_end_sec": 900,
            "event_window_unknown_event_time_mode": "block",
            "event_window_gamma_unreviewed_dates_mode": "observe",
            "require_info_risk_before_first_quote": true,
            "first_quote_quarantine_sec": 300,
            "safety_margin_cents": 2,
            "min_midpoint": 0.4,
            "max_midpoint": 0.6,
            "stale_book_ms": 45000,
            "min_scoring_check_sec": 30,
            "max_position_usd": 20,
            "max_global_position_usd": 1000,
            "exit_markup_cents": 0,
            "cancel_on_fill": true,
            "account_capital_usd": 1000,
            "requote_drift_cents": 2,
            "requote_drift_confirm_sec": 90,
            "requote_drift_cooldown_sec": 240,
            "requote_drift_max_cancels_per_cycle": 2,
            "post_fill_strategy": "adaptive",
            "adaptive_flatten_min_bid_depth_usd": 12,
            "adaptive_flatten_min_depth_multiple": 1.5,
            "adaptive_flatten_min_surplus_cents": 1,
            "adaptive_flatten_when_plan_ineligible": true,
            "adaptive_flatten_when_event_risk": true,
            "adaptive_hold_when_plan_eligible": false,
            "adaptive_fallback_strategy": "hold_and_requote",
            "adaptive_exit_recheck_sec": 45,
            "adaptive_exit_reselect_cooldown_sec": 180,
            "adaptive_exit_max_reselects_per_order": 4,
            "adaptive_exit_min_strategy_improvement_cents": 2,
            "adaptive_exit_cancel_replace_enabled": false,
            "min_depth_usd": 100,
            "cancel_bid_rank": 2,
            "depth_drop_pct": 30,
            "depth_drop_window_sec": 3,
            "fill_velocity_usd": 300,
            "fill_velocity_window_sec": 3,
            "mass_cancel_pct": 30,
            "mass_cancel_window_sec": 3,
            "requote_interval_sec": 300,
            "requote_jitter_sec": 305,
            "reconcile_interval_sec": 3
        }"#;

        let patch: RewardBotConfigPatch =
            serde_json::from_str(payload).expect("console rewards config payload deserializes");
        let config = RewardBotConfig::default().apply_patch(patch);

        assert_eq!(config.quote_bid_rank, 3);
        assert!(config.ai_provider_concurrency_enabled);
        assert_eq!(config.ai_provider_primary_max_concurrency, 4);
        assert_eq!(config.ai_provider_fallback_max_concurrency, 2);
        assert_eq!(config.ai_strategy_hint_min_confidence, decimal("0.8"));
        assert!(config.event_window_enabled);
        assert_eq!(
            config.event_window_min_confidence,
            RewardEventTimeConfidence::Medium
        );
        assert_eq!(config.event_window_stop_new_quote_before_start_sec, 7200);
        assert_eq!(config.event_window_cancel_open_buy_before_start_sec, 1800);
        assert_eq!(config.event_window_resume_after_event_end_sec, 900);
        assert_eq!(
            config.event_window_unknown_event_time_mode,
            RewardUnknownEventTimeMode::Block
        );
        assert_eq!(
            config.event_window_gamma_unreviewed_dates_mode,
            RewardGammaEventDateMode::Observe
        );
        assert!(config.require_info_risk_before_first_quote);
        assert_eq!(config.first_quote_quarantine_sec, 300);
        assert_eq!(config.cancel_bid_rank, 2);
        assert_eq!(config.requote_jitter_sec, 305);
        assert_eq!(config.requote_drift_confirm_sec, 90);
        assert_eq!(config.requote_drift_cooldown_sec, 240);
        assert_eq!(config.requote_drift_max_cancels_per_cycle, 2);
        assert_eq!(config.post_fill_strategy, PostFillStrategy::Adaptive);
        assert_eq!(config.adaptive_flatten_min_bid_depth_usd, decimal("12"));
        assert_eq!(
            config.adaptive_flatten_min_depth_multiple,
            decimal("1.5")
        );
        assert_eq!(config.adaptive_flatten_min_surplus_cents, decimal("1"));
        assert!(!config.adaptive_hold_when_plan_eligible);
        assert_eq!(
            config.adaptive_fallback_strategy,
            PostFillStrategy::HoldAndRequote
        );
        assert_eq!(config.adaptive_exit_recheck_sec, 45);
        assert_eq!(config.adaptive_exit_reselect_cooldown_sec, 180);
        assert_eq!(config.adaptive_exit_max_reselects_per_order, 4);
        assert_eq!(
            config.adaptive_exit_min_strategy_improvement_cents,
            decimal("2")
        );
        assert!(!config.adaptive_exit_cancel_replace_enabled);

        let serialized = serde_json::to_value(config).expect("config serializes");
        assert_eq!(serialized["ai_provider"], "openai");
        assert_eq!(serialized["ai_request_format"], "openai_chat_completions");
        assert_eq!(serialized["event_window_min_confidence"], "medium");
        assert_eq!(serialized["event_window_unknown_event_time_mode"], "block");
        assert_eq!(
            serialized["event_window_gamma_unreviewed_dates_mode"],
            "observe"
        );
        assert_eq!(serialized["post_fill_strategy"], "adaptive");
        assert_eq!(serialized["adaptive_fallback_strategy"], "hold_and_requote");
        assert_eq!(serialized["adaptive_exit_recheck_sec"], 45);
        assert_eq!(serialized["adaptive_exit_reselect_cooldown_sec"], 180);
        assert_eq!(serialized["adaptive_exit_max_reselects_per_order"], 4);
        assert_eq!(
            serialized["adaptive_exit_min_strategy_improvement_cents"],
            "2"
        );
        assert_eq!(serialized["adaptive_exit_cancel_replace_enabled"], false);
    }

    #[test]
    fn reward_config_normalization_keeps_live_sync_guardrails_enabled() {
        let config = RewardBotConfig {
            stale_book_ms: 0,
            min_scoring_check_sec: 0,
            ..RewardBotConfig::default()
        }
        .normalized();

        assert_eq!(config.stale_book_ms, 5_000);
        assert_eq!(config.min_scoring_check_sec, 15);
    }

    #[test]
    fn reward_ai_provider_aliases_openai_compatible_models_to_openai() {
        assert_eq!(
            RewardAiProvider::from_str("glm").expect("parse glm alias"),
            RewardAiProvider::OpenAi
        );
        assert_eq!(
            RewardAiProvider::from_str("deepseek").expect("parse deepseek alias"),
            RewardAiProvider::OpenAi
        );
        assert_eq!(
            RewardAiProvider::from_str("agnes").expect("parse agnes alias"),
            RewardAiProvider::OpenAi
        );
        assert_eq!(
            reward_ai_effective_request_format(
                RewardAiProvider::OpenAi,
                RewardAiRequestFormat::OpenAiResponses,
                "glm-4.7",
            ),
            RewardAiRequestFormat::OpenAiChatCompletions
        );
        assert_eq!(
            reward_ai_effective_request_format(
                RewardAiProvider::OpenAi,
                RewardAiRequestFormat::OpenAiResponses,
                "deepseek-v4-flash",
            ),
            RewardAiRequestFormat::OpenAiChatCompletions
        );
        assert_eq!(
            reward_ai_effective_request_format(
                RewardAiProvider::OpenAi,
                RewardAiRequestFormat::OpenAiResponses,
                "agnes-2.0-flash",
            ),
            RewardAiRequestFormat::OpenAiChatCompletions
        );
    }

}
