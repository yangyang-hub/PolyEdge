#[cfg(test)]
mod consistency_tests {
    use super::*;

    fn idempotency_request(request_id: &str) -> IdempotencyRequest {
        IdempotencyRequest {
            scope: "test".to_string(),
            idempotency_key: "intent-1".to_string(),
            request_hash: "hash-1".to_string(),
            request_id: request_id.to_string(),
            actor_user_id: Some("operator".to_string()),
            actor_session_id: Some("session".to_string()),
            resource_type: Some("test".to_string()),
            resource_id: Some("resource-1".to_string()),
        }
    }

    #[tokio::test]
    async fn failed_idempotency_request_can_retry_with_same_payload() {
        let store = InMemoryIdempotencyStore::new();
        let first = idempotency_request("request-1");
        assert!(matches!(
            store.begin(&first).await.expect("begin first request"),
            IdempotencyBegin::Started
        ));
        store.fail(&first, "TEST_FAILED").await.expect("fail request");

        let retry = idempotency_request("request-2");
        assert!(matches!(
            store.begin(&retry).await.expect("retry failed request"),
            IdempotencyBegin::Started
        ));
        store
            .complete(&retry, r#"{"ok":true}"#)
            .await
            .expect("complete retry");
        assert!(matches!(
            store.begin(&retry).await.expect("replay completed request"),
            IdempotencyBegin::Replay(response) if response == r#"{"ok":true}"#
        ));
    }

    #[tokio::test]
    async fn expired_idempotency_lease_is_reclaimed_and_old_owner_is_fenced() {
        let store = InMemoryIdempotencyStore::new();
        let first = idempotency_request("request-1");
        store.begin(&first).await.expect("begin first request");
        store
            .records
            .lock()
            .await
            .get_mut(&(first.scope.clone(), first.idempotency_key.clone()))
            .expect("stored request")
            .lease_expires_at = Some(OffsetDateTime::now_utc() - Duration::seconds(1));

        let replacement = idempotency_request("request-2");
        assert!(matches!(
            store.begin(&replacement).await.expect("reclaim request"),
            IdempotencyBegin::Started
        ));
        assert!(store.complete(&first, r#"{"stale":true}"#).await.is_err());
        store
            .complete(&replacement, r#"{"ok":true}"#)
            .await
            .expect("current owner completes request");
    }

    #[tokio::test]
    async fn expired_external_event_lease_is_reclaimed_and_old_owner_is_fenced() {
        let store = InMemoryExternalEventStore::new();
        assert_eq!(
            store
                .begin("connector", "event-1", "payload-1", "trace-old")
                .await
                .expect("begin event"),
            ExternalEventBegin::New
        );
        store
            .records
            .lock()
            .await
            .get_mut(&("connector".to_string(), "event-1".to_string()))
            .expect("stored event")
            .lease_expires_at = OffsetDateTime::now_utc() - Duration::seconds(1);

        assert_eq!(
            store
                .begin("connector", "event-1", "payload-1", "trace-new")
                .await
                .expect("reclaim event"),
            ExternalEventBegin::New
        );
        assert!(
            store
                .mark_processed("connector", "event-1", "trace-old")
                .await
                .is_err()
        );
        store
            .mark_processed("connector", "event-1", "trace-new")
            .await
            .expect("current event owner completes");
        assert_eq!(
            store
                .begin("connector", "event-1", "payload-1", "trace-replay")
                .await
                .expect("replay event"),
            ExternalEventBegin::Replay
        );
    }
}
