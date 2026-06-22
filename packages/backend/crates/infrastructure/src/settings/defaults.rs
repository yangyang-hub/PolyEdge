// `Default` implementations for every settings struct (production-safe baseline values).

impl Default for ServerSettings {
    fn default() -> Self {
        Self {
            host: "0.0.0.0".to_string(),
            port: 38001,
        }
    }
}

impl Default for DatabaseSettings {
    fn default() -> Self {
        Self {
            url: None,
            max_connections: 20,
        }
    }
}

impl Default for RuntimeSettings {
    fn default() -> Self {
        Self {
            environment: "local".to_string(),
            initial_mode: SystemMode::LiveAuto,
        }
    }
}

impl Default for RiskSettings {
    fn default() -> Self {
        Self {
            exposure_reference_nav: usd_amount("100.00"),
            initial_daily_pnl: signed_usd_amount("0.00"),
            initial_gross_exposure: exposure_ratio("0"),
            initial_net_exposure: exposure_ratio("0"),
            initial_open_alerts: 0,
            initial_kill_switch: false,
            min_signal_confidence: probability("0.55"),
            min_edge_to_execute: probability("0.03"),
            max_open_alerts: 3,
            max_daily_loss: usd_amount("5000.00"),
            max_gross_exposure: exposure_ratio("0.50"),
            max_net_exposure: exposure_ratio("0.30"),
        }
    }
}

impl Default for PolymarketSignatureType {
    fn default() -> Self {
        Self::Eoa
    }
}

impl Default for PolymarketSettings {
    fn default() -> Self {
        Self {
            account_id: String::new(),
            chain_id: 137,
            signature_type: PolymarketSignatureType::Eoa,
            funder: None,
            private_key: None,
            api_key: None,
            api_secret: None,
            api_passphrase: None,
            clob_host: "https://clob.polymarket.com".to_string(),
            ws_host: "wss://ws-subscriptions-clob.polymarket.com/ws/market".to_string(),
            gamma_host: "https://gamma-api.polymarket.com".to_string(),
            data_api_host: "https://data-api.polymarket.com".to_string(),
            polygon_rpc_url: "https://polygon-bor-rpc.publicnode.com".to_string(),
            order_status_poll_limit: 100,
            fill_poll_limit: 100,
            ws_max_instruments: 100,
            ws_idle_warn_secs: 15,
            ws_stale_after_secs: 60,
        }
    }
}

impl Default for ArbitrageSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            poll_interval_secs: 5,
            scan_limit: 100,
            scanner_version: "v1".to_string(),
            book_source: "market_snapshot".to_string(),
            analysis_lookback_hours: 24,
            max_book_age_ms: 10_000,
            opportunity_ttl_secs: 60,
            event_retention_hours: 24,
            min_gross_edge: edge("0.005"),
            min_capacity: quantity("1"),
            fee_buffer: edge("0.005"),
            slippage_buffer: edge("0.005"),
        }
    }
}

impl Default for RewardsSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            poll_interval_secs: 60,
            ai_openai_api_key: None,
            ai_anthropic_api_key: None,
            ai_openai_base_url: "https://api.openai.com/v1".to_string(),
            ai_anthropic_base_url: "https://api.anthropic.com".to_string(),
            ai_model: "gpt-4.1-mini".to_string(),
            ai_min_confidence_bps: 6500,
            ai_request_timeout_secs: 180,
            ai_advisory_batch_size: 8,
            ai_advisory_batch_timeout_secs: 8,
            ai_advisory_event_driven_enabled: false,
            info_risk_interval_secs: 300,
            info_risk_max_markets_per_cycle: 50,
            info_risk_min_confidence_bps: 7000,
            info_risk_web_search_enabled: false,
        }
    }
}

impl Default for NewsSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            poll_interval_secs: 60,
            request_timeout_secs: 10,
            max_items_per_source: 50,
            sources: default_news_sources(),
        }
    }
}

impl Default for NewsSourceSettings {
    fn default() -> Self {
        Self {
            id: String::new(),
            source_type: "news".to_string(),
            url: String::new(),
            reliability: probability("0.50"),
            enabled: true,
        }
    }
}

fn default_news_sources() -> Vec<NewsSourceSettings> {
    vec![
        news_source(
            "fed_press",
            "official",
            "https://www.federalreserve.gov/feeds/press_all.xml",
            "0.98",
        ),
        news_source(
            "sec_press",
            "official",
            "https://www.sec.gov/news/pressreleases.rss",
            "0.96",
        ),
        news_source(
            "nasa_news",
            "official",
            "https://www.nasa.gov/news-release/feed/",
            "0.95",
        ),
        news_source(
            "bbc_world",
            "news",
            "https://feeds.bbci.co.uk/news/world/rss.xml",
            "0.85",
        ),
        news_source(
            "npr_news",
            "news",
            "https://feeds.npr.org/1001/rss.xml",
            "0.84",
        ),
        news_source(
            "coindesk",
            "news",
            "https://www.coindesk.com/arc/outboundfeeds/rss",
            "0.80",
        ),
        news_source(
            "cointelegraph",
            "news",
            "https://cointelegraph.com/rss",
            "0.74",
        ),
        news_source("decrypt", "news", "https://decrypt.co/feed", "0.74"),
    ]
}

fn news_source(id: &str, source_type: &str, url: &str, reliability: &str) -> NewsSourceSettings {
    NewsSourceSettings {
        id: id.to_string(),
        source_type: source_type.to_string(),
        url: url.to_string(),
        reliability: probability(reliability),
        enabled: true,
    }
}

impl Default for WorkerSettings {
    fn default() -> Self {
        Self {
            poll_news: true,
            promote_news_events: true,
            poll_arbitrage_radar: true,
            analyze_arbitrage_opportunities: true,
            poll_reward_bot: false,
            poll_reward_info_risks: false,
            drain_execution_queue: true,
            poll_paper_order_statuses: true,
            reconcile_paper_fills: true,
            poll_polymarket_order_statuses: true,
            reconcile_polymarket_fills: true,
            consume_polymarket_user_events: true,
            poll_market_sync: true,
            consume_orderbook_stream: true,
            poll_copytrade: true,
            analyze_wallets: true,
            recompute_signals: true,
            news_promotion_interval_secs: 60,
            signal_recompute_interval_secs: 120,
            arbitrage_analysis_interval_secs: 300,
            execution_drain_interval_secs: 5,
            order_status_poll_interval_secs: 15,
            fill_reconciliation_interval_secs: 15,
            polymarket_user_event_restart_interval_secs: 5,
            market_sync_interval_secs: 300,
            task_limit: 100,
        }
    }
}

impl Default for OrderbookStreamSettings {
    fn default() -> Self {
        Self {
            max_tokens: 3_000,
            reward_candidate_token_cap: 50,
            ws_chunk_size: 100,
            max_levels_per_side: 100,
            poll_reconcile_interval_secs: 60,
            stale_threshold_ms: 15_000,
            book_ttl_ms: 300_000, // 5 minutes
            token_refresh_interval_secs: 60,
            restart_interval_secs: 5,
            orderbook_ws_incremental_reconcile: true,
            orderbook_full_resync_interval_secs: 0,
            reward_candle_history_enabled: true,
            reward_candle_history_sync_interval_secs: 300,
            reward_candle_history_request_delay_ms: 500,
            reward_candle_history_max_tokens_per_cycle: 600,
            reward_candle_history_backfill_secs: 7_200,
            reward_candle_history_incremental_secs: 900,
        }
    }
}

impl Default for OrderbookServiceSettings {
    fn default() -> Self {
        Self {
            port: 38002,
            service_url: "http://localhost:38002".to_string(),
            write_token: None,
        }
    }
}

impl Default for CopyTradeSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            poll_interval_secs: 60,
            analysis_interval_secs: 300,
            wallet_activity_limit: 50,
        }
    }
}

impl Default for AuthSettings {
    fn default() -> Self {
        Self {
            disabled: false,
            issuer: "polyedge-nextjs".to_string(),
            audience: "polyedge-rust-api".to_string(),
            clock_skew_secs: 30,
            max_query_ttl_secs: 60,
            max_write_ttl_secs: 30,
            max_step_up_window_secs: 600,
            step_up_code: String::new(),
            revoked_sessions: Vec::new(),
            force_reauth_after: Some("2026-01-01T00:00:00Z".to_string()),
            keys: Vec::new(),
        }
    }
}
