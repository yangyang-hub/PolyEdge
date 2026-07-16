pub mod api;
pub mod config;
pub mod error;
pub mod execution;
pub mod orderbook;
pub mod secrets;
pub mod state;
pub mod store;
pub mod wallet_crypto;

use axum::Router;
use state::AppState;

pub fn app(state: AppState) -> Router {
    api::router(state)
}
