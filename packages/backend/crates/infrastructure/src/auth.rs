//! Internal authentication: EdDSA JWT verification, Axum route extractors, and a
//! local-dev shortcut. Shared imports and the core `AuthContext` live here; the
//! rest is split by responsibility and inlined with `include!` (sub-files share
//! this module's imports, so they declare no `use` of their own).

use crate::{
    http::{HttpError, new_trace_id},
    runtime::AppState,
    settings::AuthSettings,
};
use axum::{
    extract::{Request, State},
    middleware::Next,
    response::Response,
};
use base64::{Engine, engine::general_purpose};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use polyedge_domain::{AppError, Result, StepUpScope, UserRole};
use serde::Deserialize;
use std::{collections::HashMap, collections::HashSet};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

#[derive(Debug, Clone, Copy)]
pub enum RequestKind {
    Read,
    Write,
}

#[derive(Debug, Clone)]
pub struct IdempotencyKey(pub String);

#[derive(Debug, Clone)]
pub struct AuthContext {
    pub user_id: String,
    pub session_id: String,
    pub roles: Vec<UserRole>,
    pub request_id: String,
    pub step_up_verified: bool,
    pub step_up_scopes: Vec<StepUpScope>,
    pub step_up_until: Option<OffsetDateTime>,
    pub ip: Option<String>,
    pub user_agent: Option<String>,
}

impl AuthContext {
    pub fn ensure_any_role(&self, accepted_roles: &[UserRole]) -> Result<()> {
        if self.roles.iter().any(|role| accepted_roles.contains(role)) {
            return Ok(());
        }

        Err(AppError::forbidden(
            "AUTH_ROLE_FORBIDDEN",
            "authenticated actor does not have a permitted role for this route",
        ))
    }

    pub fn ensure_scope(&self, required_scope: StepUpScope, now: OffsetDateTime) -> Result<()> {
        if !self.step_up_verified {
            return Err(AppError::forbidden(
                "AUTH_STEP_UP_REQUIRED",
                "step-up verification is required for this action",
            ));
        }

        if !self.step_up_scopes.contains(&required_scope) {
            return Err(AppError::forbidden(
                "AUTH_STEP_UP_SCOPE_MISSING",
                "required step-up scope is missing",
            ));
        }

        let Some(step_up_until) = self.step_up_until else {
            return Err(AppError::forbidden(
                "AUTH_STEP_UP_EXPIRED",
                "step-up verification is missing an expiry timestamp",
            ));
        };

        if step_up_until < now {
            return Err(AppError::forbidden(
                "AUTH_STEP_UP_EXPIRED",
                "step-up verification is no longer valid",
            ));
        }

        Ok(())
    }
}

include!("auth/jwt.rs");
include!("auth/verifier.rs");
include!("auth/authenticate.rs");
include!("auth/extractors.rs");
include!("auth/dev.rs");
include!("auth/tests.rs");
