#![allow(
    clippy::collapsible_if,
    clippy::derivable_impls,
    clippy::new_without_default,
    clippy::type_complexity
)]

pub mod auth;
pub mod catalog;
pub mod http;
pub mod runtime;
pub mod settings;
pub mod stores;
pub mod telemetry;

pub use auth::{
    AuthContext, IdempotencyKey, InternalTokenVerifier, RequestKind, require_connector_write_auth,
    require_console_read_auth, require_console_write_auth, require_mode_write_auth,
};
pub use http::{HttpError, hash_json, new_trace_id, request_id_from_headers};
pub use runtime::{AppState, PostgresAdvisoryLease, Runtime, RuntimeDependencies};
pub use settings::{
    AuthKeySettings, AuthSettings, NewsSettings, NewsSourceSettings, PolymarketSignatureType,
    RewardsSettings, RuntimeSettings, ServerSettings, Settings, WorkerSettings,
};
