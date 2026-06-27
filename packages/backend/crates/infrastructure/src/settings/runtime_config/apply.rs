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
        "worker.poll_reward_bot" => {
            self.worker.poll_reward_bot = parse_bool_runtime_config(key, value)?;
        }
        "worker.poll_reward_info_risks" => {
            self.worker.poll_reward_info_risks = parse_bool_runtime_config(key, value)?;
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
        "worker.news_promotion_interval_secs" => {
            self.worker.news_promotion_interval_secs = parse_u64_runtime_config(key, value)?;
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
        "orderbook_stream.max_tokens" => {
            self.orderbook_stream.max_tokens = parse_usize_runtime_config(key, value)?;
        }
        "orderbook_stream.reward_candidate_token_cap" => {
            self.orderbook_stream.reward_candidate_token_cap =
                parse_usize_runtime_config(key, value)?;
        }
        "orderbook_stream.ws_chunk_size" => {
            self.orderbook_stream.ws_chunk_size = parse_usize_runtime_config(key, value)?;
        }
        "orderbook_stream.max_levels_per_side" => {
            self.orderbook_stream.max_levels_per_side = parse_usize_runtime_config(key, value)?;
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
        "orderbook_stream.orderbook_ws_incremental_reconcile" => {
            self.orderbook_stream.orderbook_ws_incremental_reconcile =
                parse_bool_runtime_config(key, value)?;
        }
        "orderbook_stream.orderbook_full_resync_interval_secs" => {
            self.orderbook_stream.orderbook_full_resync_interval_secs =
                parse_u64_runtime_config(key, value)?;
        }
        _ => {}
    }

    Ok(())
}
}
