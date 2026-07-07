#[cfg(test)]
mod tests {
    use super::{PolymarketSignatureType, Settings, environment_source, probability};
    use std::collections::HashMap;

    #[test]
    fn settings_defaults_match_runtime_defaults() {
        let settings = Settings::from_config(config::Config::builder().build().expect("config"))
            .expect("settings");

        assert_eq!(settings.server.host, "0.0.0.0");
        assert_eq!(settings.server.port, 38001);
        assert_eq!(settings.runtime.environment, "local");
        assert_eq!(
            settings.runtime.initial_mode,
            polyedge_domain::SystemMode::LiveAuto
        );
        assert!(settings.polymarket.account_id.is_empty());
        assert!(settings.news.enabled);
        assert_eq!(settings.news.poll_interval_secs, 60);
        assert_eq!(settings.news.request_timeout_secs, 10);
        assert_eq!(settings.news.max_items_per_source, 50);
        assert_eq!(settings.news.sources.len(), 8);
        let expected_news_sources = [
            (
                "fed_press",
                "official",
                "https://www.federalreserve.gov/feeds/press_all.xml",
                "0.98",
            ),
            (
                "sec_press",
                "official",
                "https://www.sec.gov/news/pressreleases.rss",
                "0.96",
            ),
            (
                "nasa_news",
                "official",
                "https://www.nasa.gov/news-release/feed/",
                "0.95",
            ),
            (
                "bbc_world",
                "news",
                "https://feeds.bbci.co.uk/news/world/rss.xml",
                "0.85",
            ),
            (
                "npr_news",
                "news",
                "https://feeds.npr.org/1001/rss.xml",
                "0.84",
            ),
            (
                "coindesk",
                "news",
                "https://www.coindesk.com/arc/outboundfeeds/rss",
                "0.80",
            ),
            (
                "cointelegraph",
                "news",
                "https://cointelegraph.com/rss",
                "0.74",
            ),
            ("decrypt", "news", "https://decrypt.co/feed", "0.74"),
        ];
        for (source, (id, source_type, url, reliability)) in settings
            .news
            .sources
            .iter()
            .zip(expected_news_sources.iter().copied())
        {
            assert_eq!(source.id, id);
            assert_eq!(source.source_type, source_type);
            assert_eq!(source.url, url);
            assert_eq!(source.reliability, probability(reliability));
            assert!(source.enabled);
        }
        assert!(!settings.rewards.enabled);
        assert_eq!(settings.rewards.poll_interval_secs, 60);
        assert_eq!(
            settings.rewards.ai_openai_base_url,
            "https://api.openai.com/v1"
        );
        assert_eq!(settings.rewards.ai_model, "gpt-4.1-mini");
        assert_eq!(settings.rewards.info_risk_max_markets_per_cycle, 50);
        assert!(settings.worker.poll_news);
        assert!(settings.worker.promote_news_events);
        assert!(!settings.worker.poll_reward_bot);
        assert!(!settings.worker.drain_execution_queue);
        assert!(!settings.worker.poll_paper_order_statuses);
        assert!(!settings.worker.reconcile_paper_fills);
        assert!(!settings.worker.poll_polymarket_order_statuses);
        assert!(!settings.worker.reconcile_polymarket_fills);
        assert!(!settings.worker.consume_polymarket_user_events);
        assert!(settings.worker.consume_orderbook_stream);
        assert!(settings.worker.database_maintenance);
        assert_eq!(settings.worker.database_maintenance_interval_secs, 3_600);
        assert_eq!(settings.worker.news_promotion_interval_secs, 60);
        assert_eq!(settings.worker.execution_drain_interval_secs, 5);
        assert_eq!(settings.worker.order_status_poll_interval_secs, 15);
        assert_eq!(settings.worker.fill_reconciliation_interval_secs, 15);
        assert_eq!(
            settings.worker.polymarket_user_event_restart_interval_secs,
            5
        );
        assert_eq!(settings.worker.task_limit, 100);
        assert!(settings.postgres.url.is_none());
        assert_eq!(settings.postgres.max_connections, 20);
        assert!(settings.redis.url.is_none());
        assert_eq!(settings.orderbook_stream.max_tokens, 3_000);
        assert_eq!(settings.orderbook_stream.reward_candidate_token_cap, 50);
        assert_eq!(settings.orderbook_stream.ws_chunk_size, 100);
        assert_eq!(settings.orderbook_stream.max_levels_per_side, 100);
        assert_eq!(settings.orderbook_stream.poll_reconcile_interval_secs, 10);
        assert_eq!(settings.orderbook_stream.stale_threshold_ms, 15_000);
        assert_eq!(settings.orderbook_stream.book_ttl_ms, 300_000);
        assert_eq!(settings.orderbook_stream.token_refresh_interval_secs, 60);
        assert_eq!(settings.orderbook_stream.restart_interval_secs, 5);
        assert!(settings.orderbook_stream.orderbook_ws_incremental_reconcile);
        assert_eq!(
            settings
                .orderbook_stream
                .orderbook_full_resync_interval_secs,
            0
        );
        assert!(settings.orderbook_stream.reward_candle_history_enabled);
        assert_eq!(
            settings
                .orderbook_stream
                .reward_candle_history_sync_interval_secs,
            300
        );
        assert_eq!(
            settings
                .orderbook_stream
                .reward_candle_history_request_delay_ms,
            500
        );
        assert_eq!(
            settings
                .orderbook_stream
                .reward_candle_history_max_tokens_per_cycle,
            600
        );
        assert_eq!(
            settings
                .orderbook_stream
                .reward_candle_history_backfill_secs,
            7_200
        );
        assert_eq!(
            settings
                .orderbook_stream
                .reward_candle_history_incremental_secs,
            900
        );
        assert!(settings.orderbook.write_token.is_none());
        assert_eq!(
            settings.auth.force_reauth_after.as_deref(),
            Some("2026-01-01T00:00:00Z")
        );
        assert!(!settings.auth.disabled);
    }

    #[test]
    fn settings_can_be_loaded_from_environment_variables() {
        let source = environment_source().source(Some(HashMap::from([
            ("POLYEDGE_SERVER__PORT".to_string(), "9090".to_string()),
            (
                "POLYEDGE_POSTGRES__URL".to_string(),
                "postgres://postgres:postgres@localhost:5432/polyedge".to_string(),
            ),
            (
                "POLYEDGE_POSTGRES__MAX_CONNECTIONS".to_string(),
                "32".to_string(),
            ),
            (
                "POLYEDGE_RUNTIME__ENVIRONMENT".to_string(),
                "staging".to_string(),
            ),
            (
                "POLYEDGE_RUNTIME__INITIAL_MODE".to_string(),
                "live_auto".to_string(),
            ),
            (
                "POLYEDGE_RISK__INITIAL_KILL_SWITCH".to_string(),
                "true".to_string(),
            ),
            (
                "POLYEDGE_POLYMARKET__ACCOUNT_ID".to_string(),
                "acct_poly".to_string(),
            ),
            (
                "POLYEDGE_POLYMARKET__SIGNATURE_TYPE".to_string(),
                "poly_1271".to_string(),
            ),
            (
                "POLYEDGE_POLYMARKET__POLYGON_RPC_URL".to_string(),
                "https://polygon.example/rpc".to_string(),
            ),
            ("POLYEDGE_REWARDS__ENABLED".to_string(), "true".to_string()),
            (
                "POLYEDGE_REWARDS__POLL_INTERVAL_SECS".to_string(),
                "45".to_string(),
            ),
            (
                "POLYEDGE_REWARDS__AI_OPENAI_API_KEY".to_string(),
                "openai-compatible-test-key".to_string(),
            ),
            (
                "POLYEDGE_REWARDS__AI_OPENAI_BASE_URL".to_string(),
                "https://glm.example/api/paas/v4".to_string(),
            ),
            (
                "POLYEDGE_REWARDS__AI_MODEL".to_string(),
                "glm-test-model".to_string(),
            ),
            ("POLYEDGE_WORKER__POLL_NEWS".to_string(), "true".to_string()),
            (
                "POLYEDGE_WORKER__PROMOTE_NEWS_EVENTS".to_string(),
                "true".to_string(),
            ),
            (
                "POLYEDGE_WORKER__POLL_REWARD_BOT".to_string(),
                "true".to_string(),
            ),
            (
                "POLYEDGE_WORKER__DRAIN_EXECUTION_QUEUE".to_string(),
                "true".to_string(),
            ),
            (
                "POLYEDGE_WORKER__POLL_PAPER_ORDER_STATUSES".to_string(),
                "true".to_string(),
            ),
            (
                "POLYEDGE_WORKER__RECONCILE_PAPER_FILLS".to_string(),
                "true".to_string(),
            ),
            (
                "POLYEDGE_WORKER__POLL_POLYMARKET_ORDER_STATUSES".to_string(),
                "true".to_string(),
            ),
            (
                "POLYEDGE_WORKER__RECONCILE_POLYMARKET_FILLS".to_string(),
                "true".to_string(),
            ),
            (
                "POLYEDGE_WORKER__CONSUME_POLYMARKET_USER_EVENTS".to_string(),
                "true".to_string(),
            ),
            (
                "POLYEDGE_WORKER__CONSUME_ORDERBOOK_STREAM".to_string(),
                "true".to_string(),
            ),
            (
                "POLYEDGE_WORKER__NEWS_PROMOTION_INTERVAL_SECS".to_string(),
                "30".to_string(),
            ),
            (
                "POLYEDGE_WORKER__EXECUTION_DRAIN_INTERVAL_SECS".to_string(),
                "6".to_string(),
            ),
            (
                "POLYEDGE_WORKER__ORDER_STATUS_POLL_INTERVAL_SECS".to_string(),
                "20".to_string(),
            ),
            (
                "POLYEDGE_WORKER__FILL_RECONCILIATION_INTERVAL_SECS".to_string(),
                "25".to_string(),
            ),
            (
                "POLYEDGE_WORKER__POLYMARKET_USER_EVENT_RESTART_INTERVAL_SECS".to_string(),
                "10".to_string(),
            ),
            ("POLYEDGE_WORKER__TASK_LIMIT".to_string(), "33".to_string()),
            (
                "POLYEDGE_ORDERBOOK_STREAM__MAX_TOKENS".to_string(),
                "100".to_string(),
            ),
            (
                "POLYEDGE_ORDERBOOK_STREAM__REWARD_CANDIDATE_TOKEN_CAP".to_string(),
                "7".to_string(),
            ),
            (
                "POLYEDGE_ORDERBOOK_STREAM__WS_CHUNK_SIZE".to_string(),
                "25".to_string(),
            ),
            (
                "POLYEDGE_ORDERBOOK_STREAM__MAX_LEVELS_PER_SIDE".to_string(),
                "12".to_string(),
            ),
            (
                "POLYEDGE_ORDERBOOK_STREAM__POLL_RECONCILE_INTERVAL_SECS".to_string(),
                "15".to_string(),
            ),
            (
                "POLYEDGE_ORDERBOOK_STREAM__REWARD_CANDLE_HISTORY_ENABLED".to_string(),
                "false".to_string(),
            ),
            (
                "POLYEDGE_ORDERBOOK_STREAM__REWARD_CANDLE_HISTORY_SYNC_INTERVAL_SECS".to_string(),
                "180".to_string(),
            ),
            (
                "POLYEDGE_ORDERBOOK_STREAM__REWARD_CANDLE_HISTORY_REQUEST_DELAY_MS".to_string(),
                "750".to_string(),
            ),
            (
                "POLYEDGE_ORDERBOOK_STREAM__REWARD_CANDLE_HISTORY_MAX_TOKENS_PER_CYCLE".to_string(),
                "55".to_string(),
            ),
            (
                "POLYEDGE_ORDERBOOK_STREAM__REWARD_CANDLE_HISTORY_BACKFILL_SECS".to_string(),
                "3600".to_string(),
            ),
            (
                "POLYEDGE_ORDERBOOK_STREAM__REWARD_CANDLE_HISTORY_INCREMENTAL_SECS".to_string(),
                "600".to_string(),
            ),
            (
                "POLYEDGE_ORDERBOOK__WRITE_TOKEN".to_string(),
                "orderbook-test-token".to_string(),
            ),
            (
                "POLYEDGE_POLYMARKET__PRIVATE_KEY".to_string(),
                "".to_string(),
            ),
            (
                "POLYEDGE_AUTH__REVOKED_SESSIONS".to_string(),
                "sess_alpha,sess_beta".to_string(),
            ),
            ("POLYEDGE_AUTH__DISABLED".to_string(), "true".to_string()),
            ("POLYEDGE_AUTH__KEYS_JSON".to_string(), "[]".to_string()),
        ])));

        let settings = Settings::load_from_environment(
            source,
            Some(
                r#"[{"kid":"local-dev","public_key_base64":"AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA="}]"#
                    .to_string(),
            ),
            Some(
                r#"[{"id":"sec_feed","source_type":"official","url":"https://example.com/rss","reliability":"0.95","enabled":true}]"#
                    .to_string(),
            ),
        )
        .expect("settings");

        assert_eq!(settings.server.port, 9090);
        assert_eq!(
            settings.postgres.url.as_deref(),
            Some("postgres://postgres:postgres@localhost:5432/polyedge"),
        );
        assert_eq!(settings.postgres.max_connections, 32);
        assert_eq!(settings.runtime.environment, "staging");
        assert_eq!(
            settings.runtime.initial_mode,
            polyedge_domain::SystemMode::LiveAuto
        );
        assert!(settings.risk.initial_kill_switch);
        assert_eq!(settings.polymarket.account_id, "acct_poly");
        assert_eq!(
            settings.polymarket.signature_type,
            PolymarketSignatureType::Poly1271
        );
        assert_eq!(
            settings.polymarket.polygon_rpc_url,
            "https://polygon.example/rpc"
        );
        assert!(settings.polymarket.private_key.is_none());
        assert!(settings.rewards.enabled);
        assert_eq!(settings.rewards.poll_interval_secs, 45);
        assert_eq!(
            settings.rewards.ai_openai_api_key.as_deref(),
            Some("openai-compatible-test-key")
        );
        assert_eq!(
            settings.rewards.ai_openai_base_url,
            "https://glm.example/api/paas/v4"
        );
        assert_eq!(settings.rewards.ai_model, "glm-test-model");
        assert!(settings.worker.poll_news);
        assert!(settings.worker.promote_news_events);
        assert!(settings.worker.poll_reward_bot);
        assert!(settings.worker.drain_execution_queue);
        assert!(settings.worker.poll_paper_order_statuses);
        assert!(settings.worker.reconcile_paper_fills);
        assert!(settings.worker.poll_polymarket_order_statuses);
        assert!(settings.worker.reconcile_polymarket_fills);
        assert!(settings.worker.consume_polymarket_user_events);
        assert!(settings.worker.consume_orderbook_stream);
        assert_eq!(settings.worker.news_promotion_interval_secs, 30);
        assert_eq!(settings.worker.execution_drain_interval_secs, 6);
        assert_eq!(settings.worker.order_status_poll_interval_secs, 20);
        assert_eq!(settings.worker.fill_reconciliation_interval_secs, 25);
        assert_eq!(
            settings.worker.polymarket_user_event_restart_interval_secs,
            10
        );
        assert_eq!(settings.worker.task_limit, 33);
        assert_eq!(settings.orderbook_stream.max_tokens, 100);
        assert_eq!(settings.orderbook_stream.reward_candidate_token_cap, 7);
        assert_eq!(settings.orderbook_stream.ws_chunk_size, 25);
        assert_eq!(settings.orderbook_stream.max_levels_per_side, 12);
        assert_eq!(settings.orderbook_stream.poll_reconcile_interval_secs, 15);
        assert!(!settings.orderbook_stream.reward_candle_history_enabled);
        assert_eq!(
            settings
                .orderbook_stream
                .reward_candle_history_sync_interval_secs,
            180
        );
        assert_eq!(
            settings
                .orderbook_stream
                .reward_candle_history_request_delay_ms,
            750
        );
        assert_eq!(
            settings
                .orderbook_stream
                .reward_candle_history_max_tokens_per_cycle,
            55
        );
        assert_eq!(
            settings
                .orderbook_stream
                .reward_candle_history_backfill_secs,
            3600
        );
        assert_eq!(
            settings
                .orderbook_stream
                .reward_candle_history_incremental_secs,
            600
        );
        assert_eq!(
            settings.orderbook.write_token.as_deref(),
            Some("orderbook-test-token")
        );
        assert_eq!(
            settings.auth.revoked_sessions,
            vec!["sess_alpha".to_string(), "sess_beta".to_string()],
        );
        assert!(settings.auth.disabled);
        assert_eq!(settings.auth.keys.len(), 1);
        assert_eq!(settings.auth.keys[0].kid, "local-dev");
        assert_eq!(settings.news.sources.len(), 1);
        assert_eq!(settings.news.sources[0].id, "sec_feed");
        assert_eq!(settings.news.sources[0].source_type, "official");
        assert_eq!(settings.news.sources[0].url, "https://example.com/rss");
        assert!(settings.news.sources[0].enabled);
    }

    #[test]
    fn runtime_config_values_override_runtime_settings() {
        let mut settings = Settings::default();
        settings
            .apply_runtime_config_values(std::collections::BTreeMap::from([
                (
                    "polymarket.account_id".to_string(),
                    "acct_runtime".to_string(),
                ),
                (
                    "polymarket.signature_type".to_string(),
                    "deposit_wallet".to_string(),
                ),
                ("worker.poll_news".to_string(), "true".to_string()),
                (
                    "news.sources_json".to_string(),
                    r#"[{"id":"sec","source_type":"official","url":"https://example.com/rss","reliability":"0.9","enabled":true}]"#
                        .to_string(),
                ),
            ]))
            .expect("runtime config values");

        assert_eq!(settings.polymarket.account_id, "acct_runtime");
        assert_eq!(
            settings.polymarket.signature_type,
            PolymarketSignatureType::Poly1271
        );
        assert!(settings.worker.poll_news);
        assert_eq!(settings.news.sources.len(), 1);
        assert_eq!(settings.news.sources[0].id, "sec");
    }

    #[test]
    fn runtime_config_rejects_unknown_keys() {
        let values =
            std::collections::BTreeMap::from([("server.port".to_string(), "38002".to_string())]);

        let error = Settings::validate_runtime_config_keys(&values).expect_err("unknown key");

        assert_eq!(error.code(), "CONFIG_RUNTIME_KEY_UNSUPPORTED");
    }
}
