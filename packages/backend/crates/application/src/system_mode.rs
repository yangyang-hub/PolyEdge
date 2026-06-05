use async_trait::async_trait;
use polyedge_domain::{AppError, AuditResult, Result, StepUpScope, SystemMode, UserRole};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModeSnapshot {
    pub mode: SystemMode,
    pub environment: String,
    pub version: i64,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthenticatedActor {
    pub user_id: String,
    pub session_id: String,
    pub roles: Vec<UserRole>,
    pub request_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ip: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_agent: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModeTransitionCommand {
    pub to_mode: SystemMode,
    pub reason: String,
    pub request_id: String,
    pub trace_id: String,
    pub idempotency_key: String,
    pub request_hash: String,
    pub actor: AuthenticatedActor,
    pub required_scope: StepUpScope,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemModeTransitionReceipt {
    pub snapshot: ModeSnapshot,
    pub replayed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditLogEntry {
    #[serde(with = "time::serde::rfc3339")]
    pub occurred_at: OffsetDateTime,
    pub request_id: String,
    pub trace_id: String,
    pub actor: AuthenticatedActor,
    pub action: String,
    pub resource_type: String,
    pub resource_id: String,
    pub reason: String,
    pub result: AuditResult,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_code: Option<String>,
}

#[derive(Debug, Clone)]
pub enum IdempotencyBegin {
    Started,
    Replay(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdempotencyRequest {
    pub scope: String,
    pub idempotency_key: String,
    pub request_hash: String,
    pub request_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actor_user_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actor_session_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource_id: Option<String>,
}

#[async_trait]
pub trait ModeStateStore: Send + Sync {
    async fn current(&self) -> Result<ModeSnapshot>;

    async fn transition(&self, command: &ModeTransitionCommand) -> Result<ModeSnapshot>;
}

#[async_trait]
pub trait IdempotencyStore: Send + Sync {
    async fn begin(&self, request: &IdempotencyRequest) -> Result<IdempotencyBegin>;

    async fn complete(&self, request: &IdempotencyRequest, response_json: &str) -> Result<()>;

    async fn fail(&self, request: &IdempotencyRequest, error_code: &str) -> Result<()>;
}

#[async_trait]
pub trait AuditLogSink: Send + Sync {
    async fn append(&self, entry: AuditLogEntry) -> Result<()>;
}

pub struct SystemModeService {
    mode_store: std::sync::Arc<dyn ModeStateStore>,
    idempotency_store: std::sync::Arc<dyn IdempotencyStore>,
    audit_log_sink: std::sync::Arc<dyn AuditLogSink>,
}

fn validate_transition_target(_mode: SystemMode) -> Result<()> {
    Ok(())
}

impl SystemModeService {
    pub fn new(
        mode_store: std::sync::Arc<dyn ModeStateStore>,
        idempotency_store: std::sync::Arc<dyn IdempotencyStore>,
        audit_log_sink: std::sync::Arc<dyn AuditLogSink>,
    ) -> Self {
        Self {
            mode_store,
            idempotency_store,
            audit_log_sink,
        }
    }

    pub async fn read_mode(&self) -> Result<ModeSnapshot> {
        self.mode_store.current().await
    }

    pub async fn transition_mode_without_idempotency(
        &self,
        command: ModeTransitionCommand,
    ) -> Result<ModeSnapshot> {
        validate_transition_target(command.to_mode)?;

        let current_snapshot = self.mode_store.current().await?;
        if current_snapshot.mode == command.to_mode {
            return Err(AppError::invalid_input(
                "SYSTEM_MODE_ALREADY_SET",
                "requested system mode is already active",
            ));
        }

        self.mode_store.transition(&command).await
    }

    pub async fn transition_mode(
        &self,
        command: ModeTransitionCommand,
    ) -> Result<SystemModeTransitionReceipt> {
        validate_transition_target(command.to_mode)?;

        const IDEMPOTENCY_SCOPE: &str = "system.mode.switch";
        let idempotency_request = IdempotencyRequest {
            scope: IDEMPOTENCY_SCOPE.to_string(),
            idempotency_key: command.idempotency_key.clone(),
            request_hash: command.request_hash.clone(),
            request_id: command.request_id.clone(),
            actor_user_id: Some(command.actor.user_id.clone()),
            actor_session_id: Some(command.actor.session_id.clone()),
            resource_type: Some("system_mode".to_string()),
            resource_id: Some("global".to_string()),
        };

        match self.idempotency_store.begin(&idempotency_request).await? {
            IdempotencyBegin::Replay(response_json) => {
                let mut receipt: SystemModeTransitionReceipt = serde_json::from_str(&response_json)
                    .map_err(|error| {
                        AppError::internal(
                            "SYSTEM_MODE_REPLAY_DESERIALIZE_FAILED",
                            format!("failed to deserialize idempotent response: {error}"),
                        )
                    })?;
                receipt.replayed = true;

                return Ok(receipt);
            }
            IdempotencyBegin::Started => {}
        }

        let current_snapshot = self.mode_store.current().await?;
        if current_snapshot.mode == command.to_mode {
            self.idempotency_store
                .fail(&idempotency_request, "SYSTEM_MODE_ALREADY_SET")
                .await?;

            return Err(AppError::invalid_input(
                "SYSTEM_MODE_ALREADY_SET",
                "requested system mode is already active",
            ));
        }

        let snapshot = match self.mode_store.transition(&command).await {
            Ok(snapshot) => snapshot,
            Err(error) => {
                self.idempotency_store
                    .fail(&idempotency_request, error.code())
                    .await?;
                return Err(error);
            }
        };

        self.audit_log_sink
            .append(AuditLogEntry {
                occurred_at: OffsetDateTime::now_utc(),
                request_id: command.request_id.clone(),
                trace_id: command.trace_id.clone(),
                actor: command.actor.clone(),
                action: "system.mode.switch".to_string(),
                resource_type: "system_mode".to_string(),
                resource_id: "global".to_string(),
                reason: command.reason.clone(),
                result: AuditResult::Succeeded,
                error_code: None,
            })
            .await?;

        let receipt = SystemModeTransitionReceipt {
            snapshot,
            replayed: false,
        };

        let response_json = serde_json::to_string(&receipt).map_err(|error| {
            AppError::internal(
                "SYSTEM_MODE_RECEIPT_SERIALIZE_FAILED",
                format!("failed to serialize mode transition receipt: {error}"),
            )
        })?;

        self.idempotency_store
            .complete(&idempotency_request, &response_json)
            .await?;

        Ok(receipt)
    }
}
