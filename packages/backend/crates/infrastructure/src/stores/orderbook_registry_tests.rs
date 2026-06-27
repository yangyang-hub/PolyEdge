#[cfg(test)]
mod orderbook_registry_tests {
    use super::*;

    #[tokio::test]
    async fn register_tokens_atomically_replaces_source_order() {
        let registry = InMemoryOrderbookSubscriptionRegistry::new();
        registry
            .register_tokens("rewards_candidates", &["1".to_string(), "2".to_string()])
            .await
            .expect("register initial tokens");
        registry
            .register_tokens("rewards_candidates", &["3".to_string(), "3".to_string()])
            .await
            .expect("replace tokens");

        assert_eq!(registry.list_all_tokens().await, vec!["3".to_string()]);
        assert_eq!(registry.source_count().await, 1);
    }

    #[tokio::test]
    async fn empty_source_replacement_clears_stale_tokens() {
        let registry = InMemoryOrderbookSubscriptionRegistry::new();
        registry
            .register_tokens("rewards_eligible", &["1".to_string()])
            .await
            .expect("register eligible token");
        registry
            .register_tokens("rewards_eligible", &[])
            .await
            .expect("clear eligible tokens");

        assert!(registry.list_all_tokens().await.is_empty());
        assert_eq!(registry.source_count().await, 0);
    }

    #[tokio::test]
    async fn list_all_tokens_preserves_live_priority_and_deduplicates() {
        let registry = InMemoryOrderbookSubscriptionRegistry::new();
        registry
            .register_tokens("copytrade", &["5".to_string()])
            .await
            .expect("register copytrade");
        registry
            .register_tokens("rewards_candidates", &["4".to_string(), "5".to_string()])
            .await
            .expect("register candidates");
        registry
            .register_tokens("legacy_source", &["4".to_string(), "6".to_string()])
            .await
            .expect("register legacy source");
        registry
            .register_tokens("rewards_ai_provider", &["3".to_string(), "7".to_string()])
            .await
            .expect("register AI provider");
        registry
            .register_tokens("rewards_eligible", &["3".to_string(), "4".to_string()])
            .await
            .expect("register eligible rewards");
        registry
            .register_tokens("exec_orders", &["2".to_string(), "3".to_string()])
            .await
            .expect("register execution");
        registry
            .register_tokens("rewards_active", &["1".to_string(), "2".to_string()])
            .await
            .expect("register active rewards");

        assert_eq!(
            registry.list_all_tokens().await,
            vec![
                "1".to_string(),
                "2".to_string(),
                "3".to_string(),
                "4".to_string(),
                "7".to_string(),
                "5".to_string(),
                "6".to_string()
            ]
        );
    }

    #[tokio::test]
    async fn source_tokens_and_change_notifications_are_isolated() {
        let registry = InMemoryOrderbookSubscriptionRegistry::new();
        let mut changes = registry
            .subscribe_changes()
            .expect("local registry exposes changes");

        registry
            .register_tokens("rewards_ai_provider", &["7".to_string(), "8".to_string()])
            .await
            .expect("register provider source");

        assert!(changes.changed().await.is_ok());
        assert_eq!(
            registry.list_source_tokens("rewards_ai_provider").await,
            vec!["7".to_string(), "8".to_string()]
        );
        assert!(
            registry
                .list_source_tokens("rewards_eligible")
                .await
                .is_empty()
        );
    }
}
