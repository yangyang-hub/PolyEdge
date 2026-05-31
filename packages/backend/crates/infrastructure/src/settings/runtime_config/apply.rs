impl Settings {
pub fn apply_runtime_config_values(
    &mut self,
    values: BTreeMap<String, String>,
) -> polyedge_domain::Result<()> {
    for (key, value) in values {
        self.apply_runtime_config_value(&key, value.trim())?;
    }
    Ok(())
}

pub fn validate_runtime_config_keys(
    values: &BTreeMap<String, String>,
) -> polyedge_domain::Result<()> {
    let allowed = Self::default().runtime_config_value_map();
    if let Some(key) = values.keys().find(|key| !allowed.contains_key(*key)) {
        return Err(AppError::invalid_input(
            "CONFIG_RUNTIME_KEY_UNSUPPORTED",
            format!("runtime config key {key} is not supported"),
        ));
    }
    Ok(())
}

fn apply_runtime_config_value(
    &mut self,
    key: &str,
    value: &str,
) -> polyedge_domain::Result<()> {
    match key {
        "risk.exposure_reference_nav" => {
            self.risk.exposure_reference_nav = parse_usd_amount_runtime_config(key, value)?;
        }
        "risk.min_signal_confidence" => {
            self.risk.min_signal_confidence = parse_probability_runtime_config(key, value)?;
        }
        "risk.min_edge_to_execute" => {
            self.risk.min_edge_to_execute = parse_probability_runtime_config(key, value)?;
        }
        "risk.max_open_alerts" => {
            self.risk.max_open_alerts = parse_u32_runtime_config(key, value)?;
        }
        "risk.max_daily_loss" => {
            self.risk.max_daily_loss = parse_usd_amount_runtime_config(key, value)?;
        }
        "risk.max_gross_exposure" => {
            self.risk.max_gross_exposure = parse_exposure_ratio_runtime_config(key, value)?;
        }
        "risk.max_net_exposure" => {
            self.risk.max_net_exposure = parse_exposure_ratio_runtime_config(key, value)?;
        }
        "polymarket.account_id" => self.polymarket.account_id = value.to_string(),
        "polymarket.chain_id" => {
            self.polymarket.chain_id = parse_u64_runtime_config(key, value)?;
        }
        "polymarket.signature_type" => {
            self.polymarket.signature_type = PolymarketSignatureType::from_str(value)?;
        }
        "polymarket.funder" => self.polymarket.funder = optional_runtime_config(value),
        "polymarket.clob_host" => self.polymarket.clob_host = value.to_string(),
        "polymarket.ws_host" => self.polymarket.ws_host = value.to_string(),
        "polymarket.gamma_host" => self.polymarket.gamma_host = value.to_string(),
        "polymarket.data_api_host" => self.polymarket.data_api_host = value.to_string(),
        "polymarket.order_status_poll_limit" => {
            self.polymarket.order_status_poll_limit = parse_u16_runtime_config(key, value)?;
        }
        "polymarket.fill_poll_limit" => {
            self.polymarket.fill_poll_limit = parse_u16_runtime_config(key, value)?;
        }
        "polymarket.ws_max_instruments" => {
            self.polymarket.ws_max_instruments = parse_usize_runtime_config(key, value)?;
        }
        "polymarket.ws_idle_warn_secs" => {
            self.polymarket.ws_idle_warn_secs = parse_u64_runtime_config(key, value)?;
        }
        "polymarket.ws_stale_after_secs" => {
            self.polymarket.ws_stale_after_secs = parse_u64_runtime_config(key, value)?;
        }
        "arbitrage.enabled" => {
            self.arbitrage.enabled = parse_bool_runtime_config(key, value)?;
        }
        "arbitrage.poll_interval_secs" => {
            self.arbitrage.poll_interval_secs = parse_u64_runtime_config(key, value)?;
        }
        "arbitrage.scan_limit" => {
            self.arbitrage.scan_limit = parse_u16_runtime_config(key, value)?;
        }
        "arbitrage.scanner_version" => self.arbitrage.scanner_version = value.to_string(),
        "arbitrage.book_source" => {
            self.arbitrage.book_source = match value {
                "" | "market_snapshot" => "market_snapshot".to_string(),
                "polymarket" => "polymarket".to_string(),
                other => {
                    return Err(AppError::invalid_input(
                        "CONFIG_RUNTIME_BOOK_SOURCE_INVALID",
                        format!(
                            "runtime config key {key} does not support book_source={other}"
                        ),
                    ));
                }
            };
        }
        "arbitrage.analysis_lookback_hours" => {
            self.arbitrage.analysis_lookback_hours = parse_u16_runtime_config(key, value)?;
        }
        "arbitrage.max_book_age_ms" => {
            self.arbitrage.max_book_age_ms = parse_u64_runtime_config(key, value)?;
        }
        "arbitrage.opportunity_ttl_secs" => {
            self.arbitrage.opportunity_ttl_secs = parse_u64_runtime_config(key, value)?;
        }
        "arbitrage.event_retention_hours" => {
            self.arbitrage.event_retention_hours = parse_u64_runtime_config(key, value)?;
        }
        "arbitrage.min_gross_edge" => {
            self.arbitrage.min_gross_edge = parse_edge_runtime_config(key, value)?;
        }
        "arbitrage.min_capacity" => {
            self.arbitrage.min_capacity = parse_quantity_runtime_config(key, value)?;
        }
        "arbitrage.fee_buffer" => {
            self.arbitrage.fee_buffer = parse_edge_runtime_config(key, value)?;
        }
        "arbitrage.slippage_buffer" => {
            self.arbitrage.slippage_buffer = parse_edge_runtime_config(key, value)?;
        }
        "rewards.enabled" => self.rewards.enabled = parse_bool_runtime_config(key, value)?,
        "rewards.poll_interval_secs" => {
            self.rewards.poll_interval_secs = parse_u64_runtime_config(key, value)?;
        }
        "news.enabled" => self.news.enabled = parse_bool_runtime_config(key, value)?,
        "news.poll_interval_secs" => {
            self.news.poll_interval_secs = parse_u64_runtime_config(key, value)?;
        }
        "news.request_timeout_secs" => {
            self.news.request_timeout_secs = parse_u64_runtime_config(key, value)?;
        }
        "news.max_items_per_source" => {
            self.news.max_items_per_source = parse_usize_runtime_config(key, value)?;
        }
        "news.sources_json" => {
            self.news.sources = serde_json::from_str(value).map_err(|error| {
                AppError::invalid_input(
                    "CONFIG_RUNTIME_JSON_INVALID",
                    format!("runtime config key {key} must be valid JSON: {error}"),
                )
            })?;
        }
        "worker.poll_news" => self.worker.poll_news = parse_bool_runtime_config(key, value)?,
        "worker.promote_news_events" => {
            self.worker.promote_news_events = parse_bool_runtime_config(key, value)?;
        }
        "worker.poll_arbitrage_radar" => {
            self.worker.poll_arbitrage_radar = parse_bool_runtime_config(key, value)?;
        }
        "worker.analyze_arbitrage_opportunities" => {
            self.worker.analyze_arbitrage_opportunities =
                parse_bool_runtime_config(key, value)?;
        }
        "worker.poll_reward_bot" => {
            self.worker.poll_reward_bot = parse_bool_runtime_config(key, value)?;
        }
        "worker.drain_execution_queue" => {
            self.worker.drain_execution_queue = parse_bool_runtime_config(key, value)?;
        }
        "worker.poll_paper_order_statuses" => {
            self.worker.poll_paper_order_statuses = parse_bool_runtime_config(key, value)?;
        }
        "worker.reconcile_paper_fills" => {
            self.worker.reconcile_paper_fills = parse_bool_runtime_config(key, value)?;
        }
        "worker.poll_polymarket_order_statuses" => {
            self.worker.poll_polymarket_order_statuses = parse_bool_runtime_config(key, value)?;
        }
        "worker.reconcile_polymarket_fills" => {
            self.worker.reconcile_polymarket_fills = parse_bool_runtime_config(key, value)?;
        }
        "worker.consume_polymarket_user_events" => {
            self.worker.consume_polymarket_user_events = parse_bool_runtime_config(key, value)?;
        }
        "worker.consume_orderbook_stream" => {
            self.worker.consume_orderbook_stream = parse_bool_runtime_config(key, value)?;
        }
        "worker.recompute_signals" => {
            self.worker.recompute_signals = parse_bool_runtime_config(key, value)?;
        }
        "worker.news_promotion_interval_secs" => {
            self.worker.news_promotion_interval_secs = parse_u64_runtime_config(key, value)?;
        }
        "worker.signal_recompute_interval_secs" => {
            self.worker.signal_recompute_interval_secs =
                parse_u64_runtime_config(key, value)?;
        }
        "worker.arbitrage_analysis_interval_secs" => {
            self.worker.arbitrage_analysis_interval_secs =
                parse_u64_runtime_config(key, value)?;
        }
        "worker.execution_drain_interval_secs" => {
            self.worker.execution_drain_interval_secs = parse_u64_runtime_config(key, value)?;
        }
        "worker.order_status_poll_interval_secs" => {
            self.worker.order_status_poll_interval_secs = parse_u64_runtime_config(key, value)?;
        }
        "worker.fill_reconciliation_interval_secs" => {
            self.worker.fill_reconciliation_interval_secs =
                parse_u64_runtime_config(key, value)?;
        }
        "worker.polymarket_user_event_restart_interval_secs" => {
            self.worker.polymarket_user_event_restart_interval_secs =
                parse_u64_runtime_config(key, value)?;
        }
        "worker.task_limit" => self.worker.task_limit = parse_u16_runtime_config(key, value)?,
        "orderbook_stream.enabled" => {
            self.orderbook_stream.enabled = parse_bool_runtime_config(key, value)?;
        }
        "orderbook_stream.max_tokens" => {
            self.orderbook_stream.max_tokens = parse_usize_runtime_config(key, value)?;
        }
        "orderbook_stream.poll_reconcile_interval_secs" => {
            self.orderbook_stream.poll_reconcile_interval_secs =
                parse_u64_runtime_config(key, value)?;
        }
        "orderbook_stream.stale_threshold_ms" => {
            self.orderbook_stream.stale_threshold_ms = parse_u64_runtime_config(key, value)?;
        }
        "orderbook_stream.book_ttl_ms" => {
            self.orderbook_stream.book_ttl_ms = parse_u64_runtime_config(key, value)?;
        }
        "orderbook_stream.token_refresh_interval_secs" => {
            self.orderbook_stream.token_refresh_interval_secs = parse_u64_runtime_config(key, value)?;
        }
        "orderbook_stream.restart_interval_secs" => {
            self.orderbook_stream.restart_interval_secs = parse_u64_runtime_config(key, value)?;
        }
        _ => {}
    }

    Ok(())
}
}
