use super::*;
use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use base64::{Engine, engine::general_purpose};
use ed25519_dalek::{Signer, SigningKey};
use polyedge_application::{
    ArbitrageAnalysisRunView, ArbitrageScanView, ArbitrageValidationConfig, AuthenticatedActor,
    MarkExecutionSubmittedCommand, MarketBookSnapshotView, NewsIngestSourceCommand,
    NewsIngestionItem, Paginated, SubmitExecutionStoreCommand, build_arbitrage_analysis,
    demo_fixture_bundle,
};
use polyedge_domain::{Edge, Probability, Quantity, StepUpScope, SystemMode, UserRole};
use polyedge_infrastructure::{AppState, AuthKeySettings, Runtime, Settings};
use serde::Serialize;
use tower::util::ServiceExt;
use uuid::Uuid;

#[derive(Serialize)]
struct TestHeader<'a> {
    alg: &'a str,
    kid: &'a str,
    typ: &'a str,
}

#[derive(Serialize)]
struct TestClaims {
    iss: String,
    aud: String,
    sub: String,
    iat: i64,
    nbf: i64,
    exp: i64,
    jti: String,
    session_id: String,
    roles: Vec<UserRole>,
    auth_time: i64,
    request_id: String,
    step_up_verified: bool,
    step_up_scope: Vec<polyedge_domain::StepUpScope>,
    step_up_until: Option<i64>,
}

#[test]
fn sse_message_filter_keeps_new_resource_versions_only() {
    let mut emitted_ids = HashSet::new();
    let mut emitted_id_order = VecDeque::new();
    let first_batch = filter_new_sse_messages(
        vec![
            SseMessage {
                id: "signals:sig_1:1".to_string(),
                event: "signal.updated",
                data: json!({ "signal_id": "sig_1", "version": 1 }),
            },
            SseMessage {
                id: "signals:sig_1:1".to_string(),
                event: "signal.updated",
                data: json!({ "signal_id": "sig_1", "version": 1 }),
            },
            SseMessage {
                id: "signals:sig_2:1".to_string(),
                event: "signal.created",
                data: json!({ "signal_id": "sig_2", "version": 1 }),
            },
        ],
        &mut emitted_ids,
        &mut emitted_id_order,
    );

    assert_eq!(
        first_batch
            .iter()
            .map(|message| message.id.as_str())
            .collect::<Vec<_>>(),
        vec!["signals:sig_1:1", "signals:sig_2:1"]
    );

    let second_batch = filter_new_sse_messages(
        vec![
            SseMessage {
                id: "signals:sig_1:1".to_string(),
                event: "signal.updated",
                data: json!({ "signal_id": "sig_1", "version": 1 }),
            },
            SseMessage {
                id: "signals:sig_1:2".to_string(),
                event: "signal.updated",
                data: json!({ "signal_id": "sig_1", "version": 2 }),
            },
        ],
        &mut emitted_ids,
        &mut emitted_id_order,
    );

    assert_eq!(
        second_batch
            .iter()
            .map(|message| message.id.as_str())
            .collect::<Vec<_>>(),
        vec!["signals:sig_1:2"]
    );
}

#[test]
fn sse_message_filter_bounds_seen_id_cache() {
    let mut emitted_ids = HashSet::new();
    let mut emitted_id_order = VecDeque::new();
    let messages = (0..=MAX_STREAM_EMITTED_IDS)
        .map(|index| SseMessage {
            id: format!("signals:sig_{index}:1"),
            event: "signal.updated",
            data: json!({ "signal_id": format!("sig_{index}"), "version": 1 }),
        })
        .collect::<Vec<_>>();

    let filtered = filter_new_sse_messages(messages, &mut emitted_ids, &mut emitted_id_order);

    assert_eq!(filtered.len(), MAX_STREAM_EMITTED_IDS + 1);
    assert_eq!(emitted_ids.len(), MAX_STREAM_EMITTED_IDS);
    assert!(!emitted_ids.contains("signals:sig_0:1"));
    assert!(emitted_ids.contains(&format!("signals:sig_{MAX_STREAM_EMITTED_IDS}:1")));
}

fn issue_token_with(
    signing_key: &SigningKey,
    kid: &str,
    request_id: &str,
    roles: Vec<UserRole>,
    step_up_scope: Vec<StepUpScope>,
) -> String {
    let now = time::OffsetDateTime::now_utc().unix_timestamp();
    let header = serde_json::to_vec(&TestHeader {
        alg: "EdDSA",
        kid,
        typ: "JWT",
    })
    .expect("serialize header");
    let claims = serde_json::to_vec(&TestClaims {
        iss: "polyedge-nextjs".to_string(),
        aud: "polyedge-rust-api".to_string(),
        sub: "usr_123".to_string(),
        iat: now,
        nbf: now,
        exp: now + 20,
        jti: format!("jit_{}", Uuid::now_v7()),
        session_id: "sess_123".to_string(),
        roles,
        auth_time: now - 30,
        request_id: request_id.to_string(),
        step_up_verified: true,
        step_up_scope,
        step_up_until: Some(now + 120),
    })
    .expect("serialize claims");
    let header_b64 = general_purpose::URL_SAFE_NO_PAD.encode(header);
    let claims_b64 = general_purpose::URL_SAFE_NO_PAD.encode(claims);
    let message = format!("{header_b64}.{claims_b64}");
    let signature = signing_key.sign(message.as_bytes());
    let signature_b64 = general_purpose::URL_SAFE_NO_PAD.encode(signature.to_bytes());
    format!("{message}.{signature_b64}")
}

fn issue_token(signing_key: &SigningKey, kid: &str, request_id: &str) -> String {
    issue_token_with(
        signing_key,
        kid,
        request_id,
        vec![UserRole::RiskAdmin],
        vec![StepUpScope::SystemModeSwitch],
    )
}

fn test_actor(request_id: &str) -> AuthenticatedActor {
    AuthenticatedActor {
        user_id: "usr_123".to_string(),
        session_id: "sess_123".to_string(),
        roles: vec![UserRole::RiskAdmin],
        request_id: request_id.to_string(),
        ip: None,
        user_agent: Some("api-tests".to_string()),
    }
}

struct TestExecutionSubmission {
    execution_request_id: String,
}

async fn submit_execution_for_test(
    state: &AppState,
    signal_id: &str,
    connector_name: &str,
) -> TestExecutionSubmission {
    let risk_state = state
        .risk_service
        .read_state()
        .await
        .expect("read risk state");
    let result = state
        .market_event_service
        .submit_execution_request(SubmitExecutionStoreCommand {
            signal_id: signal_id.to_string(),
            expected_signal_version: Some(9),
            limit_price: Probability::new("0.48".parse().expect("decimal")).expect("probability"),
            quantity: Quantity::new("25".parse().expect("decimal")).expect("quantity"),
            connector_name: connector_name.to_string(),
            reason: "queue manual execution request for connector callback flow".to_string(),
            requested_by_user_id: "test-user".to_string(),
            trace_id: format!("trc_{}", Uuid::now_v7()),
            mode: risk_state.mode,
            risk_state_version: risk_state.version,
        })
        .await
        .expect("submit execution request");

    TestExecutionSubmission {
        execution_request_id: result.execution_request.id,
    }
}

async fn dispatch_execution(
    state: &AppState,
    execution_request_id: &str,
    account_id: &str,
    external_order_id: &str,
) {
    let request_id = format!("req_dispatch_{}", Uuid::now_v7());
    state
        .execution_service
        .mark_execution_submitted(MarkExecutionSubmittedCommand {
            execution_request_id: execution_request_id.to_string(),
            account_id: account_id.to_string(),
            external_order_id: external_order_id.to_string(),
            request_id: request_id.clone(),
            trace_id: format!("trc_{}", Uuid::now_v7()),
            actor: test_actor(&request_id),
        })
        .await
        .expect("dispatch execution");
}

include!("tests/basic_routes.rs");
include!("tests/arbitrage.rs");
include!("tests/event_news.rs");
include!("tests/risk_execution.rs");
include!("tests/callbacks.rs");
include!("tests/mode_signal.rs");
