fn apply_reward_config_value(config: &mut RewardBotConfig, key: &str, value: &str) -> Result<()> {
    match key {
        "enabled" => config.enabled = parse_bool_config(key, value)?,
        "execution_mode" => {
            // Legacy key: accept and ignore; always live.
        }
        "account_id" => config.account_id = value.to_string(),
        "max_markets" => config.max_markets = parse_u16_config(key, value)?,
        "max_open_orders" => config.max_open_orders = parse_u16_config(key, value)?,
        "per_market_usd" => config.per_market_usd = parse_decimal_config(key, value)?,
        "quote_size_usd" => config.quote_size_usd = parse_decimal_config(key, value)?,
        "min_daily_reward" => config.min_daily_reward = parse_decimal_config(key, value)?,
        "min_market_liquidity_usd" => {
            config.min_market_liquidity_usd = parse_decimal_config(key, value)?;
        }
        "min_market_volume_24h_usd" => {
            config.min_market_volume_24h_usd = parse_decimal_config(key, value)?;
        }
        "min_hours_to_end" => config.min_hours_to_end = parse_u64_config(key, value)?,
        "max_market_spread_cents" => {
            config.max_market_spread_cents = parse_decimal_config(key, value)?;
        }
        "max_market_data_age_minutes" => {
            config.max_market_data_age_minutes = parse_u64_config(key, value)?;
        }
        "min_market_score" => config.min_market_score = parse_decimal_config(key, value)?,
        "max_spread_cents" => config.max_spread_cents = parse_decimal_config(key, value)?,
        "quote_mode" => config.quote_mode = RewardQuoteMode::from_str(value)?,
        "selection_mode" => config.selection_mode = RewardSelectionMode::from_str(value)?,
        "quote_edge_cents" => {
            // Legacy key: midpoint-offset quoting was replaced by bid-rank quoting.
        }
        "quote_bid_rank" => config.quote_bid_rank = parse_u16_config(key, value)?,
        "dominant_single_side_enabled" => {
            config.dominant_single_side_enabled = parse_bool_config(key, value)?;
        }
        "dominant_min_probability" => {
            config.dominant_min_probability = parse_decimal_config(key, value)?;
        }
        "dominant_max_probability" => {
            config.dominant_max_probability = parse_decimal_config(key, value)?;
        }
        "dominant_min_exit_depth_usd" => {
            config.dominant_min_exit_depth_usd = parse_decimal_config(key, value)?;
        }
        "max_top1_depth_share" => {
            config.max_top1_depth_share = parse_decimal_config(key, value)?;
        }
        "max_top3_depth_share" => {
            config.max_top3_depth_share = parse_decimal_config(key, value)?;
        }
        "max_book_hhi" => config.max_book_hhi = parse_decimal_config(key, value)?,
        "preferred_categories" => config.preferred_categories = parse_csv_config(value),
        "preferred_category_score_bonus" => {
            config.preferred_category_score_bonus = parse_decimal_config(key, value)?;
        }
        "low_competition_mode" => {
            config.low_competition_mode = RewardLowCompetitionMode::from_str(value)?;
        }
        "low_competition_max_markets" => {
            config.low_competition_max_markets = parse_u16_config(key, value)?;
        }
        "low_competition_max_open_orders" => {
            config.low_competition_max_open_orders = parse_u16_config(key, value)?;
        }
        "low_competition_per_market_usd" => {
            config.low_competition_per_market_usd = parse_decimal_config(key, value)?;
        }
        "low_competition_max_position_usd" => {
            config.low_competition_max_position_usd = parse_decimal_config(key, value)?;
        }
        "low_competition_probe_notional_usd" => {
            config.low_competition_probe_notional_usd = parse_decimal_config(key, value)?;
        }
        "low_competition_min_competition_share_bps" => {
            config.low_competition_min_competition_share_bps = parse_u16_config(key, value)?;
        }
        "low_competition_max_competition_multiple" => {
            config.low_competition_max_competition_multiple = parse_decimal_config(key, value)?;
        }
        "low_competition_candidate_max_competition_multiple" => {
            config.low_competition_candidate_max_competition_multiple =
                parse_decimal_config(key, value)?;
        }
        "low_competition_max_account_allocation_bps" => {
            config.low_competition_max_account_allocation_bps = parse_u16_config(key, value)?;
        }
        "low_competition_max_market_allocation_bps" => {
            config.low_competition_max_market_allocation_bps = parse_u16_config(key, value)?;
        }
        "low_competition_candidate_liquidity_filter_enabled" => {
            config.low_competition_candidate_liquidity_filter_enabled =
                parse_bool_config(key, value)?;
        }
        "low_competition_candidate_volume_filter_enabled" => {
            config.low_competition_candidate_volume_filter_enabled = parse_bool_config(key, value)?;
        }
        "low_competition_min_market_liquidity_usd" => {
            config.low_competition_min_market_liquidity_usd = parse_decimal_config(key, value)?;
        }
        "low_competition_min_market_volume_24h_usd" => {
            config.low_competition_min_market_volume_24h_usd = parse_decimal_config(key, value)?;
        }
        "low_competition_max_competition_usd" => {
            config.low_competition_max_competition_usd = parse_decimal_config(key, value)?;
        }
        "low_competition_min_reward_per_100_usd_day" => {
            config.low_competition_min_reward_per_100_usd_day =
                parse_decimal_config(key, value)?;
        }
        "low_competition_min_exit_depth_usd" => {
            config.low_competition_min_exit_depth_usd = parse_decimal_config(key, value)?;
        }
        "low_competition_min_exit_depth_multiple" => {
            config.low_competition_min_exit_depth_multiple = parse_decimal_config(key, value)?;
        }
        "low_competition_max_entry_exit_slippage_cents" => {
            config.low_competition_max_entry_exit_slippage_cents = parse_decimal_config(key, value)?;
        }
        "low_competition_max_bad_fill_recovery_days" => {
            config.low_competition_max_bad_fill_recovery_days = parse_decimal_config(key, value)?;
        }
        "low_competition_max_midpoint_range_cents" => {
            config.low_competition_max_midpoint_range_cents = parse_decimal_config(key, value)?;
        }
        "low_competition_max_top_of_book_flip_count" => {
            config.low_competition_max_top_of_book_flip_count = parse_u64_config(key, value)?;
        }
        "low_competition_observation_window_sec" => {
            config.low_competition_observation_window_sec = parse_u64_config(key, value)?;
        }
        "low_competition_min_book_samples" => {
            config.low_competition_min_book_samples = parse_u64_config(key, value)?;
        }
        "low_competition_quote_bid_rank" => {
            config.low_competition_quote_bid_rank = parse_u16_config(key, value)?;
        }
        "low_competition_safety_margin_cents" => {
            config.low_competition_safety_margin_cents = parse_decimal_config(key, value)?;
        }
        "low_competition_max_spread_cents" => {
            config.low_competition_max_spread_cents = parse_decimal_config(key, value)?;
        }
        "low_competition_max_market_spread_cents" => {
            config.low_competition_max_market_spread_cents = parse_decimal_config(key, value)?;
        }
        "low_competition_min_market_score" => {
            config.low_competition_min_market_score = parse_decimal_config(key, value)?;
        }
        "low_competition_require_ai_allow" => {
            config.low_competition_require_ai_allow = parse_bool_config(key, value)?;
        }
        "low_competition_info_risk_avoid_level" => {
            config.low_competition_info_risk_avoid_level = RewardInfoRiskLevel::from_str(value)?;
        }
        "low_competition_cancel_confirm_sec" => {
            config.low_competition_cancel_confirm_sec = parse_u64_config(key, value)?;
        }
        "low_competition_cancel_share_threshold_ratio_bps" => {
            config.low_competition_cancel_share_threshold_ratio_bps = parse_u16_config(key, value)?;
        }
        "low_competition_cancel_competition_multiple_factor" => {
            config.low_competition_cancel_competition_multiple_factor =
                parse_decimal_config(key, value)?;
        }
        "low_competition_cancel_max_exit_slippage_cents" => {
            config.low_competition_cancel_max_exit_slippage_cents =
                parse_decimal_config(key, value)?;
        }
        "low_competition_cancel_min_exit_depth_usd" => {
            config.low_competition_cancel_min_exit_depth_usd = parse_decimal_config(key, value)?;
        }
        "low_competition_cancel_exit_depth_multiple" => {
            config.low_competition_cancel_exit_depth_multiple = parse_decimal_config(key, value)?;
        }
        "low_competition_cancel_midpoint_range_floor_cents" => {
            config.low_competition_cancel_midpoint_range_floor_cents =
                parse_decimal_config(key, value)?;
        }
        "low_competition_global_open_order_share_bps" => {
            config.low_competition_global_open_order_share_bps = parse_u16_config(key, value)?;
        }
        "ai_advisory_enabled" => config.ai_advisory_enabled = parse_bool_config(key, value)?,
        "ai_provider" => config.ai_provider = RewardAiProvider::from_str(value)?,
        "ai_request_format" => {
            config.ai_request_format = RewardAiRequestFormat::from_str(value)?;
        }
        "ai_advisory_ttl_sec" => config.ai_advisory_ttl_sec = parse_u64_config(key, value)?,
        "ai_advisory_batch_size" => {
            config.ai_advisory_batch_size = parse_u16_config(key, value)?;
        }
        "info_risk_enabled" => config.info_risk_enabled = parse_bool_config(key, value)?,
        "info_risk_mode" => config.info_risk_mode = RewardSelectionMode::from_str(value)?,
        "info_risk_avoid_level" => {
            config.info_risk_avoid_level = RewardInfoRiskLevel::from_str(value)?;
        }
        "info_risk_ttl_sec" => config.info_risk_ttl_sec = parse_u64_config(key, value)?,
        "info_risk_batch_size" => {
            config.info_risk_batch_size = parse_u16_config(key, value)?;
        }
        "require_info_risk_before_first_quote" => {
            config.require_info_risk_before_first_quote = parse_bool_config(key, value)?;
        }
        "first_quote_quarantine_sec" => {
            config.first_quote_quarantine_sec = parse_u64_config(key, value)?;
        }
        "safety_margin_cents" => config.safety_margin_cents = parse_decimal_config(key, value)?,
        "min_midpoint" => config.min_midpoint = parse_decimal_config(key, value)?,
        "max_midpoint" => config.max_midpoint = parse_decimal_config(key, value)?,
        "stale_book_ms" => config.stale_book_ms = parse_u64_config(key, value)?,
        "min_scoring_check_sec" => config.min_scoring_check_sec = parse_u64_config(key, value)?,
        "max_position_usd" => config.max_position_usd = parse_decimal_config(key, value)?,
        "max_global_position_usd" => {
            config.max_global_position_usd = parse_decimal_config(key, value)?;
        }
        "exit_markup_cents" => config.exit_markup_cents = parse_decimal_config(key, value)?,
        "cancel_on_fill" => config.cancel_on_fill = parse_bool_config(key, value)?,
        "account_capital_usd" => config.account_capital_usd = parse_decimal_config(key, value)?,
        "reward_competition_factor" | "single_sided_divisor_c" | "fill_rate_per_tick"
        | "max_fill_ratio" => {
            // Legacy validation/simulation settings; rewards execution is live-only.
        }
        "requote_drift_cents" => config.requote_drift_cents = parse_decimal_config(key, value)?,
        "requote_drift_confirm_sec" => {
            config.requote_drift_confirm_sec = parse_u64_config(key, value)?;
        }
        "requote_drift_cooldown_sec" => {
            config.requote_drift_cooldown_sec = parse_u64_config(key, value)?;
        }
        "requote_drift_max_cancels_per_cycle" => {
            config.requote_drift_max_cancels_per_cycle = parse_u16_config(key, value)?;
        }
        "post_fill_strategy" => config.post_fill_strategy = PostFillStrategy::from_str(value)?,
        "min_depth_usd" => config.min_depth_usd = parse_decimal_config(key, value)?,
        "cancel_bid_rank" => config.cancel_bid_rank = parse_u16_config(key, value)?,
        "depth_drop_pct" => config.depth_drop_pct = parse_decimal_config(key, value)?,
        "depth_drop_window_sec" => config.depth_drop_window_sec = parse_u64_config(key, value)?,
        "fill_velocity_usd" => config.fill_velocity_usd = parse_decimal_config(key, value)?,
        "fill_velocity_window_sec" => {
            config.fill_velocity_window_sec = parse_u64_config(key, value)?;
        }
        "mass_cancel_pct" => config.mass_cancel_pct = parse_decimal_config(key, value)?,
        "mass_cancel_window_sec" => config.mass_cancel_window_sec = parse_u64_config(key, value)?,
        "requote_interval_sec" => config.requote_interval_sec = parse_u64_config(key, value)?,
        "requote_jitter_sec" => config.requote_jitter_sec = parse_u64_config(key, value)?,
        "reconcile_interval_sec" => config.reconcile_interval_sec = parse_u64_config(key, value)?,
        "auto_cancel_stale_minutes" => {
            // Legacy unsafe setting: unresolved external orders are never
            // force-cancelled from local state without exchange confirmation.
        }
        _ => {}
    }
    Ok(())
}

fn reward_config_entries(config: &RewardBotConfig) -> Vec<(&'static str, String)> {
    vec![
        ("enabled", config.enabled.to_string()),
        ("account_id", config.account_id.clone()),
        ("max_markets", config.max_markets.to_string()),
        ("max_open_orders", config.max_open_orders.to_string()),
        ("per_market_usd", config.per_market_usd.to_string()),
        ("quote_size_usd", config.quote_size_usd.to_string()),
        ("min_daily_reward", config.min_daily_reward.to_string()),
        (
            "min_market_liquidity_usd",
            config.min_market_liquidity_usd.to_string(),
        ),
        (
            "min_market_volume_24h_usd",
            config.min_market_volume_24h_usd.to_string(),
        ),
        ("min_hours_to_end", config.min_hours_to_end.to_string()),
        (
            "max_market_spread_cents",
            config.max_market_spread_cents.to_string(),
        ),
        (
            "max_market_data_age_minutes",
            config.max_market_data_age_minutes.to_string(),
        ),
        ("min_market_score", config.min_market_score.to_string()),
        ("max_spread_cents", config.max_spread_cents.to_string()),
        ("quote_mode", config.quote_mode.as_str().to_string()),
        ("selection_mode", config.selection_mode.as_str().to_string()),
        ("quote_bid_rank", config.quote_bid_rank.to_string()),
        (
            "dominant_single_side_enabled",
            config.dominant_single_side_enabled.to_string(),
        ),
        (
            "dominant_min_probability",
            config.dominant_min_probability.to_string(),
        ),
        (
            "dominant_max_probability",
            config.dominant_max_probability.to_string(),
        ),
        (
            "dominant_min_exit_depth_usd",
            config.dominant_min_exit_depth_usd.to_string(),
        ),
        (
            "max_top1_depth_share",
            config.max_top1_depth_share.to_string(),
        ),
        (
            "max_top3_depth_share",
            config.max_top3_depth_share.to_string(),
        ),
        ("max_book_hhi", config.max_book_hhi.to_string()),
        ("preferred_categories", config.preferred_categories.join(",")),
        (
            "preferred_category_score_bonus",
            config.preferred_category_score_bonus.to_string(),
        ),
        (
            "low_competition_mode",
            config.low_competition_mode.as_str().to_string(),
        ),
        (
            "low_competition_max_markets",
            config.low_competition_max_markets.to_string(),
        ),
        (
            "low_competition_max_open_orders",
            config.low_competition_max_open_orders.to_string(),
        ),
        (
            "low_competition_per_market_usd",
            config.low_competition_per_market_usd.to_string(),
        ),
        (
            "low_competition_max_position_usd",
            config.low_competition_max_position_usd.to_string(),
        ),
        (
            "low_competition_probe_notional_usd",
            config.low_competition_probe_notional_usd.to_string(),
        ),
        (
            "low_competition_min_competition_share_bps",
            config.low_competition_min_competition_share_bps.to_string(),
        ),
        (
            "low_competition_max_competition_multiple",
            config.low_competition_max_competition_multiple.to_string(),
        ),
        (
            "low_competition_candidate_max_competition_multiple",
            config
                .low_competition_candidate_max_competition_multiple
                .to_string(),
        ),
        (
            "low_competition_max_account_allocation_bps",
            config.low_competition_max_account_allocation_bps.to_string(),
        ),
        (
            "low_competition_max_market_allocation_bps",
            config.low_competition_max_market_allocation_bps.to_string(),
        ),
        (
            "low_competition_candidate_liquidity_filter_enabled",
            config
                .low_competition_candidate_liquidity_filter_enabled
                .to_string(),
        ),
        (
            "low_competition_candidate_volume_filter_enabled",
            config
                .low_competition_candidate_volume_filter_enabled
                .to_string(),
        ),
        (
            "low_competition_min_market_liquidity_usd",
            config
                .low_competition_min_market_liquidity_usd
                .to_string(),
        ),
        (
            "low_competition_min_market_volume_24h_usd",
            config.low_competition_min_market_volume_24h_usd.to_string(),
        ),
        (
            "low_competition_max_competition_usd",
            config.low_competition_max_competition_usd.to_string(),
        ),
        (
            "low_competition_min_reward_per_100_usd_day",
            config
                .low_competition_min_reward_per_100_usd_day
                .to_string(),
        ),
        (
            "low_competition_min_exit_depth_usd",
            config.low_competition_min_exit_depth_usd.to_string(),
        ),
        (
            "low_competition_min_exit_depth_multiple",
            config.low_competition_min_exit_depth_multiple.to_string(),
        ),
        (
            "low_competition_max_entry_exit_slippage_cents",
            config.low_competition_max_entry_exit_slippage_cents
                .to_string(),
        ),
        (
            "low_competition_max_bad_fill_recovery_days",
            config.low_competition_max_bad_fill_recovery_days
                .to_string(),
        ),
        (
            "low_competition_max_midpoint_range_cents",
            config.low_competition_max_midpoint_range_cents.to_string(),
        ),
        (
            "low_competition_max_top_of_book_flip_count",
            config.low_competition_max_top_of_book_flip_count.to_string(),
        ),
        (
            "low_competition_observation_window_sec",
            config.low_competition_observation_window_sec.to_string(),
        ),
        (
            "low_competition_min_book_samples",
            config.low_competition_min_book_samples.to_string(),
        ),
        (
            "low_competition_quote_bid_rank",
            config.low_competition_quote_bid_rank.to_string(),
        ),
        (
            "low_competition_safety_margin_cents",
            config.low_competition_safety_margin_cents.to_string(),
        ),
        (
            "low_competition_max_spread_cents",
            config.low_competition_max_spread_cents.to_string(),
        ),
        (
            "low_competition_max_market_spread_cents",
            config.low_competition_max_market_spread_cents.to_string(),
        ),
        (
            "low_competition_min_market_score",
            config.low_competition_min_market_score.to_string(),
        ),
        (
            "low_competition_require_ai_allow",
            config.low_competition_require_ai_allow.to_string(),
        ),
        (
            "low_competition_info_risk_avoid_level",
            config.low_competition_info_risk_avoid_level
                .as_str()
                .to_string(),
        ),
        (
            "low_competition_cancel_confirm_sec",
            config.low_competition_cancel_confirm_sec.to_string(),
        ),
        (
            "low_competition_cancel_share_threshold_ratio_bps",
            config
                .low_competition_cancel_share_threshold_ratio_bps
                .to_string(),
        ),
        (
            "low_competition_cancel_competition_multiple_factor",
            config
                .low_competition_cancel_competition_multiple_factor
                .to_string(),
        ),
        (
            "low_competition_cancel_max_exit_slippage_cents",
            config
                .low_competition_cancel_max_exit_slippage_cents
                .to_string(),
        ),
        (
            "low_competition_cancel_min_exit_depth_usd",
            config.low_competition_cancel_min_exit_depth_usd.to_string(),
        ),
        (
            "low_competition_cancel_exit_depth_multiple",
            config.low_competition_cancel_exit_depth_multiple.to_string(),
        ),
        (
            "low_competition_cancel_midpoint_range_floor_cents",
            config
                .low_competition_cancel_midpoint_range_floor_cents
                .to_string(),
        ),
        (
            "low_competition_global_open_order_share_bps",
            config.low_competition_global_open_order_share_bps
                .to_string(),
        ),
        (
            "ai_advisory_enabled",
            config.ai_advisory_enabled.to_string(),
        ),
        ("ai_provider", config.ai_provider.as_str().to_string()),
        (
            "ai_request_format",
            config.ai_request_format.as_str().to_string(),
        ),
        (
            "ai_advisory_ttl_sec",
            config.ai_advisory_ttl_sec.to_string(),
        ),
        (
            "ai_advisory_batch_size",
            config.ai_advisory_batch_size.to_string(),
        ),
        (
            "info_risk_enabled",
            config.info_risk_enabled.to_string(),
        ),
        (
            "info_risk_mode",
            config.info_risk_mode.as_str().to_string(),
        ),
        (
            "info_risk_avoid_level",
            config.info_risk_avoid_level.as_str().to_string(),
        ),
        (
            "info_risk_ttl_sec",
            config.info_risk_ttl_sec.to_string(),
        ),
        (
            "info_risk_batch_size",
            config.info_risk_batch_size.to_string(),
        ),
        (
            "require_info_risk_before_first_quote",
            config.require_info_risk_before_first_quote.to_string(),
        ),
        (
            "first_quote_quarantine_sec",
            config.first_quote_quarantine_sec.to_string(),
        ),
        (
            "safety_margin_cents",
            config.safety_margin_cents.to_string(),
        ),
        ("min_midpoint", config.min_midpoint.to_string()),
        ("max_midpoint", config.max_midpoint.to_string()),
        ("stale_book_ms", config.stale_book_ms.to_string()),
        (
            "min_scoring_check_sec",
            config.min_scoring_check_sec.to_string(),
        ),
        ("max_position_usd", config.max_position_usd.to_string()),
        (
            "max_global_position_usd",
            config.max_global_position_usd.to_string(),
        ),
        ("exit_markup_cents", config.exit_markup_cents.to_string()),
        ("cancel_on_fill", config.cancel_on_fill.to_string()),
        (
            "account_capital_usd",
            config.account_capital_usd.to_string(),
        ),
        (
            "requote_drift_cents",
            config.requote_drift_cents.to_string(),
        ),
        (
            "requote_drift_confirm_sec",
            config.requote_drift_confirm_sec.to_string(),
        ),
        (
            "requote_drift_cooldown_sec",
            config.requote_drift_cooldown_sec.to_string(),
        ),
        (
            "requote_drift_max_cancels_per_cycle",
            config.requote_drift_max_cancels_per_cycle.to_string(),
        ),
        (
            "post_fill_strategy",
            config.post_fill_strategy.as_str().to_string(),
        ),
        ("min_depth_usd", config.min_depth_usd.to_string()),
        ("cancel_bid_rank", config.cancel_bid_rank.to_string()),
        ("depth_drop_pct", config.depth_drop_pct.to_string()),
        (
            "depth_drop_window_sec",
            config.depth_drop_window_sec.to_string(),
        ),
        ("fill_velocity_usd", config.fill_velocity_usd.to_string()),
        (
            "fill_velocity_window_sec",
            config.fill_velocity_window_sec.to_string(),
        ),
        ("mass_cancel_pct", config.mass_cancel_pct.to_string()),
        (
            "mass_cancel_window_sec",
            config.mass_cancel_window_sec.to_string(),
        ),
        (
            "requote_interval_sec",
            config.requote_interval_sec.to_string(),
        ),
        ("requote_jitter_sec", config.requote_jitter_sec.to_string()),
        (
            "reconcile_interval_sec",
            config.reconcile_interval_sec.to_string(),
        ),
    ]
}
