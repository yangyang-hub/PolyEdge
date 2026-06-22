impl Settings {
#[must_use]
pub fn runtime_config_entries(&self) -> Vec<RuntimeConfigEntry> {
    let defaults = Self::default();
    let mut entries = Vec::new();

    push_runtime_config_entry(
        &mut entries,
        RuntimeConfigEntryDraft {
            section: "risk",
            field: "exposure_reference_nav",
            label: "Risk reference NAV",
            env_name: "POLYEDGE_RISK__EXPOSURE_REFERENCE_NAV",
            value: self.risk.exposure_reference_nav.to_string(),
            default_value: defaults.risk.exposure_reference_nav.to_string(),
            value_type: RuntimeConfigValueType::Decimal,
            options: Vec::new(),
        },
    );
    push_runtime_config_entry(
        &mut entries,
        RuntimeConfigEntryDraft {
            section: "risk",
            field: "min_signal_confidence",
            label: "Minimum signal confidence",
            env_name: "POLYEDGE_RISK__MIN_SIGNAL_CONFIDENCE",
            value: self.risk.min_signal_confidence.to_string(),
            default_value: defaults.risk.min_signal_confidence.to_string(),
            value_type: RuntimeConfigValueType::Decimal,
            options: Vec::new(),
        },
    );
    push_runtime_config_entry(
        &mut entries,
        RuntimeConfigEntryDraft {
            section: "risk",
            field: "min_edge_to_execute",
            label: "Minimum execution edge",
            env_name: "POLYEDGE_RISK__MIN_EDGE_TO_EXECUTE",
            value: self.risk.min_edge_to_execute.to_string(),
            default_value: defaults.risk.min_edge_to_execute.to_string(),
            value_type: RuntimeConfigValueType::Decimal,
            options: Vec::new(),
        },
    );
    push_runtime_config_entry(
        &mut entries,
        RuntimeConfigEntryDraft {
            section: "risk",
            field: "max_open_alerts",
            label: "Max open alerts",
            env_name: "POLYEDGE_RISK__MAX_OPEN_ALERTS",
            value: self.risk.max_open_alerts.to_string(),
            default_value: defaults.risk.max_open_alerts.to_string(),
            value_type: RuntimeConfigValueType::Integer,
            options: Vec::new(),
        },
    );
    push_runtime_config_entry(
        &mut entries,
        RuntimeConfigEntryDraft {
            section: "risk",
            field: "max_daily_loss",
            label: "Max daily loss",
            env_name: "POLYEDGE_RISK__MAX_DAILY_LOSS",
            value: self.risk.max_daily_loss.to_string(),
            default_value: defaults.risk.max_daily_loss.to_string(),
            value_type: RuntimeConfigValueType::Decimal,
            options: Vec::new(),
        },
    );
    push_runtime_config_entry(
        &mut entries,
        RuntimeConfigEntryDraft {
            section: "risk",
            field: "max_gross_exposure",
            label: "Max gross exposure",
            env_name: "POLYEDGE_RISK__MAX_GROSS_EXPOSURE",
            value: self.risk.max_gross_exposure.to_string(),
            default_value: defaults.risk.max_gross_exposure.to_string(),
            value_type: RuntimeConfigValueType::Decimal,
            options: Vec::new(),
        },
    );
    push_runtime_config_entry(
        &mut entries,
        RuntimeConfigEntryDraft {
            section: "risk",
            field: "max_net_exposure",
            label: "Max net exposure",
            env_name: "POLYEDGE_RISK__MAX_NET_EXPOSURE",
            value: self.risk.max_net_exposure.to_string(),
            default_value: defaults.risk.max_net_exposure.to_string(),
            value_type: RuntimeConfigValueType::Decimal,
            options: Vec::new(),
        },
    );

    push_runtime_config_entry(
        &mut entries,
        RuntimeConfigEntryDraft {
            section: "polymarket",
            field: "account_id",
            label: "Polymarket account id",
            env_name: "POLYEDGE_POLYMARKET__ACCOUNT_ID",
            value: self.polymarket.account_id.clone(),
            default_value: defaults.polymarket.account_id.clone(),
            value_type: RuntimeConfigValueType::Text,
            options: Vec::new(),
        },
    );
    push_runtime_config_entry(
        &mut entries,
        RuntimeConfigEntryDraft {
            section: "polymarket",
            field: "chain_id",
            label: "Polymarket chain id",
            env_name: "POLYEDGE_POLYMARKET__CHAIN_ID",
            value: self.polymarket.chain_id.to_string(),
            default_value: defaults.polymarket.chain_id.to_string(),
            value_type: RuntimeConfigValueType::Integer,
            options: Vec::new(),
        },
    );
    push_runtime_config_entry(
        &mut entries,
        RuntimeConfigEntryDraft {
            section: "polymarket",
            field: "signature_type",
            label: "Polymarket signature type",
            env_name: "POLYEDGE_POLYMARKET__SIGNATURE_TYPE",
            value: self.polymarket.signature_type.as_str().to_string(),
            default_value: defaults.polymarket.signature_type.as_str().to_string(),
            value_type: RuntimeConfigValueType::Enum,
            options: vec![
                "eoa".to_string(),
                "proxy".to_string(),
                "gnosis_safe".to_string(),
                "poly_1271".to_string(),
            ],
        },
    );
    push_runtime_config_entry(
        &mut entries,
        RuntimeConfigEntryDraft {
            section: "polymarket",
            field: "funder",
            label: "Polymarket funder",
            env_name: "POLYEDGE_POLYMARKET__FUNDER",
            value: optional_config_string(self.polymarket.funder.as_deref()),
            default_value: optional_config_string(defaults.polymarket.funder.as_deref()),
            value_type: RuntimeConfigValueType::Text,
            options: Vec::new(),
        },
    );
    push_runtime_config_entry(
        &mut entries,
        RuntimeConfigEntryDraft {
            section: "polymarket",
            field: "clob_host",
            label: "Polymarket CLOB host",
            env_name: "POLYEDGE_POLYMARKET__CLOB_HOST",
            value: self.polymarket.clob_host.clone(),
            default_value: defaults.polymarket.clob_host.clone(),
            value_type: RuntimeConfigValueType::Url,
            options: Vec::new(),
        },
    );
    push_runtime_config_entry(
        &mut entries,
        RuntimeConfigEntryDraft {
            section: "polymarket",
            field: "ws_host",
            label: "Polymarket websocket host",
            env_name: "POLYEDGE_POLYMARKET__WS_HOST",
            value: self.polymarket.ws_host.clone(),
            default_value: defaults.polymarket.ws_host.clone(),
            value_type: RuntimeConfigValueType::Url,
            options: Vec::new(),
        },
    );
    push_runtime_config_entry(
        &mut entries,
        RuntimeConfigEntryDraft {
            section: "polymarket",
            field: "gamma_host",
            label: "Polymarket Gamma host",
            env_name: "POLYEDGE_POLYMARKET__GAMMA_HOST",
            value: self.polymarket.gamma_host.clone(),
            default_value: defaults.polymarket.gamma_host.clone(),
            value_type: RuntimeConfigValueType::Url,
            options: Vec::new(),
        },
    );
    push_runtime_config_entry(
        &mut entries,
        RuntimeConfigEntryDraft {
            section: "polymarket",
            field: "data_api_host",
            label: "Polymarket data API host",
            env_name: "POLYEDGE_POLYMARKET__DATA_API_HOST",
            value: self.polymarket.data_api_host.clone(),
            default_value: defaults.polymarket.data_api_host.clone(),
            value_type: RuntimeConfigValueType::Url,
            options: Vec::new(),
        },
    );
    push_runtime_config_entry(
        &mut entries,
        RuntimeConfigEntryDraft {
            section: "polymarket",
            field: "order_status_poll_limit",
            label: "Order status poll limit",
            env_name: "POLYEDGE_POLYMARKET__ORDER_STATUS_POLL_LIMIT",
            value: self.polymarket.order_status_poll_limit.to_string(),
            default_value: defaults.polymarket.order_status_poll_limit.to_string(),
            value_type: RuntimeConfigValueType::Integer,
            options: Vec::new(),
        },
    );
    push_runtime_config_entry(
        &mut entries,
        RuntimeConfigEntryDraft {
            section: "polymarket",
            field: "fill_poll_limit",
            label: "Fill poll limit",
            env_name: "POLYEDGE_POLYMARKET__FILL_POLL_LIMIT",
            value: self.polymarket.fill_poll_limit.to_string(),
            default_value: defaults.polymarket.fill_poll_limit.to_string(),
            value_type: RuntimeConfigValueType::Integer,
            options: Vec::new(),
        },
    );
    push_runtime_config_entry(
        &mut entries,
        RuntimeConfigEntryDraft {
            section: "polymarket",
            field: "ws_max_instruments",
            label: "Websocket instrument limit",
            env_name: "POLYEDGE_POLYMARKET__WS_MAX_INSTRUMENTS",
            value: self.polymarket.ws_max_instruments.to_string(),
            default_value: defaults.polymarket.ws_max_instruments.to_string(),
            value_type: RuntimeConfigValueType::Integer,
            options: Vec::new(),
        },
    );
    push_runtime_config_entry(
        &mut entries,
        RuntimeConfigEntryDraft {
            section: "polymarket",
            field: "ws_idle_warn_secs",
            label: "Websocket idle warn seconds",
            env_name: "POLYEDGE_POLYMARKET__WS_IDLE_WARN_SECS",
            value: self.polymarket.ws_idle_warn_secs.to_string(),
            default_value: defaults.polymarket.ws_idle_warn_secs.to_string(),
            value_type: RuntimeConfigValueType::Integer,
            options: Vec::new(),
        },
    );
    push_runtime_config_entry(
        &mut entries,
        RuntimeConfigEntryDraft {
            section: "polymarket",
            field: "ws_stale_after_secs",
            label: "Websocket stale seconds",
            env_name: "POLYEDGE_POLYMARKET__WS_STALE_AFTER_SECS",
            value: self.polymarket.ws_stale_after_secs.to_string(),
            default_value: defaults.polymarket.ws_stale_after_secs.to_string(),
            value_type: RuntimeConfigValueType::Integer,
            options: Vec::new(),
        },
    );

    push_runtime_config_entry(
        &mut entries,
        RuntimeConfigEntryDraft {
            section: "arbitrage",
            field: "enabled",
            label: "Arbitrage enabled",
            env_name: "POLYEDGE_ARBITRAGE__ENABLED",
            value: self.arbitrage.enabled.to_string(),
            default_value: defaults.arbitrage.enabled.to_string(),
            value_type: RuntimeConfigValueType::Boolean,
            options: Vec::new(),
        },
    );
    push_runtime_config_entry(
        &mut entries,
        RuntimeConfigEntryDraft {
            section: "arbitrage",
            field: "poll_interval_secs",
            label: "Arbitrage poll interval seconds",
            env_name: "POLYEDGE_ARBITRAGE__POLL_INTERVAL_SECS",
            value: self.arbitrage.poll_interval_secs.to_string(),
            default_value: defaults.arbitrage.poll_interval_secs.to_string(),
            value_type: RuntimeConfigValueType::Integer,
            options: Vec::new(),
        },
    );
    push_runtime_config_entry(
        &mut entries,
        RuntimeConfigEntryDraft {
            section: "arbitrage",
            field: "scan_limit",
            label: "Arbitrage scan limit",
            env_name: "POLYEDGE_ARBITRAGE__SCAN_LIMIT",
            value: self.arbitrage.scan_limit.to_string(),
            default_value: defaults.arbitrage.scan_limit.to_string(),
            value_type: RuntimeConfigValueType::Integer,
            options: Vec::new(),
        },
    );
    push_runtime_config_entry(
        &mut entries,
        RuntimeConfigEntryDraft {
            section: "arbitrage",
            field: "scanner_version",
            label: "Arbitrage scanner version",
            env_name: "POLYEDGE_ARBITRAGE__SCANNER_VERSION",
            value: self.arbitrage.scanner_version.clone(),
            default_value: defaults.arbitrage.scanner_version.clone(),
            value_type: RuntimeConfigValueType::Text,
            options: Vec::new(),
        },
    );
    push_runtime_config_entry(
        &mut entries,
        RuntimeConfigEntryDraft {
            section: "arbitrage",
            field: "book_source",
            label: "Arbitrage book source",
            env_name: "POLYEDGE_ARBITRAGE__BOOK_SOURCE",
            value: self.arbitrage.book_source.clone(),
            default_value: defaults.arbitrage.book_source.clone(),
            value_type: RuntimeConfigValueType::Enum,
            options: vec!["market_snapshot".to_string(), "polymarket".to_string()],
        },
    );
    push_runtime_config_entry(
        &mut entries,
        RuntimeConfigEntryDraft {
            section: "arbitrage",
            field: "analysis_lookback_hours",
            label: "Arbitrage analysis lookback hours",
            env_name: "POLYEDGE_ARBITRAGE__ANALYSIS_LOOKBACK_HOURS",
            value: self.arbitrage.analysis_lookback_hours.to_string(),
            default_value: defaults.arbitrage.analysis_lookback_hours.to_string(),
            value_type: RuntimeConfigValueType::Integer,
            options: Vec::new(),
        },
    );
    push_runtime_config_entry(
        &mut entries,
        RuntimeConfigEntryDraft {
            section: "arbitrage",
            field: "max_book_age_ms",
            label: "Max book age milliseconds",
            env_name: "POLYEDGE_ARBITRAGE__MAX_BOOK_AGE_MS",
            value: self.arbitrage.max_book_age_ms.to_string(),
            default_value: defaults.arbitrage.max_book_age_ms.to_string(),
            value_type: RuntimeConfigValueType::Integer,
            options: Vec::new(),
        },
    );
    push_runtime_config_entry(
        &mut entries,
        RuntimeConfigEntryDraft {
            section: "arbitrage",
            field: "opportunity_ttl_secs",
            label: "Opportunity TTL seconds",
            env_name: "POLYEDGE_ARBITRAGE__OPPORTUNITY_TTL_SECS",
            value: self.arbitrage.opportunity_ttl_secs.to_string(),
            default_value: defaults.arbitrage.opportunity_ttl_secs.to_string(),
            value_type: RuntimeConfigValueType::Integer,
            options: Vec::new(),
        },
    );
    push_runtime_config_entry(
        &mut entries,
        RuntimeConfigEntryDraft {
            section: "arbitrage",
            field: "event_retention_hours",
            label: "Arbitrage event retention hours",
            env_name: "POLYEDGE_ARBITRAGE__EVENT_RETENTION_HOURS",
            value: self.arbitrage.event_retention_hours.to_string(),
            default_value: defaults.arbitrage.event_retention_hours.to_string(),
            value_type: RuntimeConfigValueType::Integer,
            options: Vec::new(),
        },
    );
    push_runtime_config_entry(
        &mut entries,
        RuntimeConfigEntryDraft {
            section: "arbitrage",
            field: "min_gross_edge",
            label: "Minimum gross edge",
            env_name: "POLYEDGE_ARBITRAGE__MIN_GROSS_EDGE",
            value: self.arbitrage.min_gross_edge.to_string(),
            default_value: defaults.arbitrage.min_gross_edge.to_string(),
            value_type: RuntimeConfigValueType::Decimal,
            options: Vec::new(),
        },
    );
    push_runtime_config_entry(
        &mut entries,
        RuntimeConfigEntryDraft {
            section: "arbitrage",
            field: "min_capacity",
            label: "Minimum capacity",
            env_name: "POLYEDGE_ARBITRAGE__MIN_CAPACITY",
            value: self.arbitrage.min_capacity.to_string(),
            default_value: defaults.arbitrage.min_capacity.to_string(),
            value_type: RuntimeConfigValueType::Decimal,
            options: Vec::new(),
        },
    );
    push_runtime_config_entry(
        &mut entries,
        RuntimeConfigEntryDraft {
            section: "arbitrage",
            field: "fee_buffer",
            label: "Fee buffer",
            env_name: "POLYEDGE_ARBITRAGE__FEE_BUFFER",
            value: self.arbitrage.fee_buffer.to_string(),
            default_value: defaults.arbitrage.fee_buffer.to_string(),
            value_type: RuntimeConfigValueType::Decimal,
            options: Vec::new(),
        },
    );
    push_runtime_config_entry(
        &mut entries,
        RuntimeConfigEntryDraft {
            section: "arbitrage",
            field: "slippage_buffer",
            label: "Slippage buffer",
            env_name: "POLYEDGE_ARBITRAGE__SLIPPAGE_BUFFER",
            value: self.arbitrage.slippage_buffer.to_string(),
            default_value: defaults.arbitrage.slippage_buffer.to_string(),
            value_type: RuntimeConfigValueType::Decimal,
            options: Vec::new(),
        },
    );

    push_runtime_config_entry(
        &mut entries,
        RuntimeConfigEntryDraft {
            section: "rewards",
            field: "enabled",
            label: "Rewards polling enabled",
            env_name: "POLYEDGE_REWARDS__ENABLED",
            value: self.rewards.enabled.to_string(),
            default_value: defaults.rewards.enabled.to_string(),
            value_type: RuntimeConfigValueType::Boolean,
            options: Vec::new(),
        },
    );
    push_runtime_config_entry(
        &mut entries,
        RuntimeConfigEntryDraft {
            section: "rewards",
            field: "poll_interval_secs",
            label: "Rewards poll interval seconds",
            env_name: "POLYEDGE_REWARDS__POLL_INTERVAL_SECS",
            value: self.rewards.poll_interval_secs.to_string(),
            default_value: defaults.rewards.poll_interval_secs.to_string(),
            value_type: RuntimeConfigValueType::Integer,
            options: Vec::new(),
        },
    );

    push_runtime_config_entry(
        &mut entries,
        RuntimeConfigEntryDraft {
            section: "news",
            field: "enabled",
            label: "News ingestion enabled",
            env_name: "POLYEDGE_NEWS__ENABLED",
            value: self.news.enabled.to_string(),
            default_value: defaults.news.enabled.to_string(),
            value_type: RuntimeConfigValueType::Boolean,
            options: Vec::new(),
        },
    );
    push_runtime_config_entry(
        &mut entries,
        RuntimeConfigEntryDraft {
            section: "news",
            field: "poll_interval_secs",
            label: "News poll interval seconds",
            env_name: "POLYEDGE_NEWS__POLL_INTERVAL_SECS",
            value: self.news.poll_interval_secs.to_string(),
            default_value: defaults.news.poll_interval_secs.to_string(),
            value_type: RuntimeConfigValueType::Integer,
            options: Vec::new(),
        },
    );
    push_runtime_config_entry(
        &mut entries,
        RuntimeConfigEntryDraft {
            section: "news",
            field: "request_timeout_secs",
            label: "News request timeout seconds",
            env_name: "POLYEDGE_NEWS__REQUEST_TIMEOUT_SECS",
            value: self.news.request_timeout_secs.to_string(),
            default_value: defaults.news.request_timeout_secs.to_string(),
            value_type: RuntimeConfigValueType::Integer,
            options: Vec::new(),
        },
    );
    push_runtime_config_entry(
        &mut entries,
        RuntimeConfigEntryDraft {
            section: "news",
            field: "max_items_per_source",
            label: "News max items per source",
            env_name: "POLYEDGE_NEWS__MAX_ITEMS_PER_SOURCE",
            value: self.news.max_items_per_source.to_string(),
            default_value: defaults.news.max_items_per_source.to_string(),
            value_type: RuntimeConfigValueType::Integer,
            options: Vec::new(),
        },
    );
    push_runtime_config_entry(
        &mut entries,
        RuntimeConfigEntryDraft {
            section: "news",
            field: "sources_json",
            label: "News sources JSON",
            env_name: "POLYEDGE_NEWS__SOURCES_JSON",
            value: news_sources_json(&self.news.sources),
            default_value: news_sources_json(&defaults.news.sources),
            value_type: RuntimeConfigValueType::Json,
            options: Vec::new(),
        },
    );

    push_worker_runtime_config_entries(&mut entries, self, &defaults);

    push_runtime_config_entry(
        &mut entries,
        RuntimeConfigEntryDraft {
            section: "orderbook_stream",
            field: "max_tokens",
            label: "Orderbook stream max tokens",
            env_name: "POLYEDGE_ORDERBOOK_STREAM__MAX_TOKENS",
            value: self.orderbook_stream.max_tokens.to_string(),
            default_value: defaults.orderbook_stream.max_tokens.to_string(),
            value_type: RuntimeConfigValueType::Integer,
            options: Vec::new(),
        },
    );
    push_runtime_config_entry(
        &mut entries,
        RuntimeConfigEntryDraft {
            section: "orderbook_stream",
            field: "reward_candidate_token_cap",
            label: "Orderbook reward candidate token cap",
            env_name: "POLYEDGE_ORDERBOOK_STREAM__REWARD_CANDIDATE_TOKEN_CAP",
            value: self
                .orderbook_stream
                .reward_candidate_token_cap
                .to_string(),
            default_value: defaults
                .orderbook_stream
                .reward_candidate_token_cap
                .to_string(),
            value_type: RuntimeConfigValueType::Integer,
            options: Vec::new(),
        },
    );
    push_runtime_config_entry(
        &mut entries,
        RuntimeConfigEntryDraft {
            section: "orderbook_stream",
            field: "max_levels_per_side",
            label: "Orderbook max levels per side",
            env_name: "POLYEDGE_ORDERBOOK_STREAM__MAX_LEVELS_PER_SIDE",
            value: self.orderbook_stream.max_levels_per_side.to_string(),
            default_value: defaults.orderbook_stream.max_levels_per_side.to_string(),
            value_type: RuntimeConfigValueType::Integer,
            options: Vec::new(),
        },
    );
    push_runtime_config_entry(
        &mut entries,
        RuntimeConfigEntryDraft {
            section: "orderbook_stream",
            field: "ws_chunk_size",
            label: "Orderbook WS chunk size",
            env_name: "POLYEDGE_ORDERBOOK_STREAM__WS_CHUNK_SIZE",
            value: self.orderbook_stream.ws_chunk_size.to_string(),
            default_value: defaults.orderbook_stream.ws_chunk_size.to_string(),
            value_type: RuntimeConfigValueType::Integer,
            options: Vec::new(),
        },
    );
    push_runtime_config_entry(
        &mut entries,
        RuntimeConfigEntryDraft {
            section: "orderbook_stream",
            field: "poll_reconcile_interval_secs",
            label: "Orderbook stream poll reconcile interval seconds",
            env_name: "POLYEDGE_ORDERBOOK_STREAM__POLL_RECONCILE_INTERVAL_SECS",
            value: self.orderbook_stream.poll_reconcile_interval_secs.to_string(),
            default_value: defaults.orderbook_stream.poll_reconcile_interval_secs.to_string(),
            value_type: RuntimeConfigValueType::Integer,
            options: Vec::new(),
        },
    );
    push_runtime_config_entry(
        &mut entries,
        RuntimeConfigEntryDraft {
            section: "orderbook_stream",
            field: "stale_threshold_ms",
            label: "Orderbook stream stale threshold milliseconds",
            env_name: "POLYEDGE_ORDERBOOK_STREAM__STALE_THRESHOLD_MS",
            value: self.orderbook_stream.stale_threshold_ms.to_string(),
            default_value: defaults.orderbook_stream.stale_threshold_ms.to_string(),
            value_type: RuntimeConfigValueType::Integer,
            options: Vec::new(),
        },
    );
    push_runtime_config_entry(
        &mut entries,
        RuntimeConfigEntryDraft {
            section: "orderbook_stream",
            field: "book_ttl_ms",
            label: "Orderbook in-memory book TTL (ms)",
            env_name: "POLYEDGE_ORDERBOOK_STREAM__BOOK_TTL_MS",
            value: self.orderbook_stream.book_ttl_ms.to_string(),
            default_value: defaults.orderbook_stream.book_ttl_ms.to_string(),
            value_type: RuntimeConfigValueType::Integer,
            options: Vec::new(),
        },
    );
    push_runtime_config_entry(
        &mut entries,
        RuntimeConfigEntryDraft {
            section: "orderbook_stream",
            field: "token_refresh_interval_secs",
            label: "Orderbook token refresh interval (secs)",
            env_name: "POLYEDGE_ORDERBOOK_STREAM__TOKEN_REFRESH_INTERVAL_SECS",
            value: self.orderbook_stream.token_refresh_interval_secs.to_string(),
            default_value: defaults.orderbook_stream.token_refresh_interval_secs.to_string(),
            value_type: RuntimeConfigValueType::Integer,
            options: Vec::new(),
        },
    );
    push_runtime_config_entry(
        &mut entries,
        RuntimeConfigEntryDraft {
            section: "orderbook_stream",
            field: "restart_interval_secs",
            label: "Orderbook stream restart interval seconds",
            env_name: "POLYEDGE_ORDERBOOK_STREAM__RESTART_INTERVAL_SECS",
            value: self.orderbook_stream.restart_interval_secs.to_string(),
            default_value: defaults.orderbook_stream.restart_interval_secs.to_string(),
            value_type: RuntimeConfigValueType::Integer,
            options: Vec::new(),
        },
    );
    push_runtime_config_entry(
        &mut entries,
        RuntimeConfigEntryDraft {
            section: "orderbook_stream",
            field: "orderbook_ws_incremental_reconcile",
            label: "Orderbook WS incremental reconcile (keep connections alive)",
            env_name: "POLYEDGE_ORDERBOOK_STREAM__WS_INCREMENTAL_RECONCILE",
            value: self
                .orderbook_stream
                .orderbook_ws_incremental_reconcile
                .to_string(),
            default_value: defaults
                .orderbook_stream
                .orderbook_ws_incremental_reconcile
                .to_string(),
            value_type: RuntimeConfigValueType::Boolean,
            options: Vec::new(),
        },
    );
    push_runtime_config_entry(
        &mut entries,
        RuntimeConfigEntryDraft {
            section: "orderbook_stream",
            field: "orderbook_full_resync_interval_secs",
            label: "Orderbook WS full resync interval seconds (0 = off)",
            env_name: "POLYEDGE_ORDERBOOK_STREAM__FULL_RESYNC_INTERVAL_SECS",
            value: self
                .orderbook_stream
                .orderbook_full_resync_interval_secs
                .to_string(),
            default_value: defaults
                .orderbook_stream
                .orderbook_full_resync_interval_secs
                .to_string(),
            value_type: RuntimeConfigValueType::Integer,
            options: Vec::new(),
        },
    );

    entries
}

#[must_use]
pub fn runtime_config_value_map(&self) -> BTreeMap<String, String> {
    self.runtime_config_entries()
        .into_iter()
        .map(|entry| (entry.key, entry.value))
        .collect()
}

}
