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
            ai_advisory_enabled: false,
            ai_provider: RewardAiProvider::OpenAi,
            ai_request_format: RewardAiRequestFormat::OpenAiResponses,
            ai_advisory_ttl_sec: 3600,
            info_risk_enabled: false,
            info_risk_mode: RewardSelectionMode::Observe,
            info_risk_avoid_level: RewardInfoRiskLevel::High,
            info_risk_ttl_sec: 3600,
            safety_margin_cents: decimal("1"),
            min_midpoint: decimal("0.1"),
            max_midpoint: decimal("0.9"),
            stale_book_ms: 45_000,
            min_scoring_check_sec: 45,
            max_position_usd: decimal("20"),
            max_global_position_usd: decimal("50"),
            exit_markup_cents: decimal("1"),
            cancel_on_fill: true,
            account_capital_usd: decimal("1000"),
            requote_drift_cents: decimal("2"),
            post_fill_strategy: PostFillStrategy::ExitAtMarkup,
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
        let quote_size_cap = if self.per_market_usd == Decimal::ZERO {
            decimal("1000000")
        } else {
            self.per_market_usd
        };
        self.quote_size_usd = clamp_decimal(self.quote_size_usd, Decimal::ZERO, quote_size_cap);
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
        self.max_market_spread_cents = clamp_decimal(
            self.max_market_spread_cents,
            decimal("0.1"),
            decimal("100"),
        );
        self.max_market_data_age_minutes = self.max_market_data_age_minutes.clamp(1, 1440);
        self.min_market_score = clamp_decimal(self.min_market_score, Decimal::ZERO, decimal("100"));
        self.max_spread_cents =
            clamp_decimal(self.max_spread_cents, decimal("0.1"), decimal("99"));
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
        self.ai_advisory_ttl_sec = self.ai_advisory_ttl_sec.clamp(60, 86_400);
        self.info_risk_ttl_sec = self.info_risk_ttl_sec.clamp(60, 86_400);
        if matches!(self.ai_provider, RewardAiProvider::Anthropic) {
            self.ai_request_format = RewardAiRequestFormat::AnthropicMessages;
        } else if matches!(
            self.ai_request_format,
            RewardAiRequestFormat::AnthropicMessages
        ) {
            self.ai_request_format = RewardAiRequestFormat::OpenAiResponses;
        }
        self.safety_margin_cents =
            clamp_decimal(self.safety_margin_cents, Decimal::ZERO, decimal("20"));
        self.min_midpoint = clamp_decimal(self.min_midpoint, Decimal::ZERO, decimal("0.49"));
        self.max_midpoint = clamp_decimal(self.max_midpoint, decimal("0.51"), Decimal::ONE);
        if self.max_midpoint <= self.min_midpoint {
            self.max_midpoint = Decimal::min(Decimal::ONE, self.min_midpoint + decimal("0.1"));
        }
        self.stale_book_ms = self.stale_book_ms.clamp(0, 120_000);
        self.min_scoring_check_sec = self.min_scoring_check_sec.clamp(0, 600);
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
        // Risk control clamps
        self.min_depth_usd =
            clamp_decimal(self.min_depth_usd, Decimal::ZERO, decimal("1000000"));
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
            per_market_usd: self.per_market_usd,
            min_market_liquidity_usd: self.min_market_liquidity_usd,
            min_market_volume_24h_usd: self.min_market_volume_24h_usd,
            min_hours_to_end: self.min_hours_to_end,
            max_market_spread_cents: self.max_market_spread_cents,
            max_market_data_age_minutes: self.max_market_data_age_minutes,
            max_rewards_spread_cents: self.max_spread_cents,
            allow_dominant_single_side: self.quote_mode == RewardQuoteMode::Auto
                && self.dominant_single_side_enabled,
            allow_single_side_budget_fallback: self.quote_mode == RewardQuoteMode::Auto
                && self.selection_mode == RewardSelectionMode::Enforce
                && self.dominant_single_side_enabled,
            dominant_min_probability: self.dominant_min_probability,
            dominant_max_probability: self.dominant_max_probability,
        }
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
        if let Some(post_fill_strategy) = patch.post_fill_strategy {
            next.post_fill_strategy = post_fill_strategy;
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
            "info_risk_enabled": true,
            "info_risk_mode": "enforce",
            "info_risk_avoid_level": "high",
            "info_risk_ttl_sec": 36000,
            "safety_margin_cents": 2,
            "min_midpoint": 0.4,
            "max_midpoint": 0.6,
            "stale_book_ms": 45000,
            "min_scoring_check_sec": 30,
            "max_position_usd": 20,
            "max_global_position_usd": 1000,
            "exit_markup_cents": 1,
            "cancel_on_fill": true,
            "account_capital_usd": 1000,
            "requote_drift_cents": 2,
            "post_fill_strategy": "flatten_immediately",
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
        assert_eq!(config.cancel_bid_rank, 2);
        assert_eq!(config.requote_jitter_sec, 305);

        let serialized = serde_json::to_value(config).expect("config serializes");
        assert_eq!(serialized["ai_provider"], "openai");
        assert_eq!(
            serialized["ai_request_format"],
            "openai_chat_completions"
        );
    }
}
