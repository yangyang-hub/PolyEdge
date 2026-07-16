use crate::error::{Result, ServerError};
use polyedge_contracts::{
    CreateExecutionBatchRequest, CreateMarketStrategyRequest, CreateWalletAccountRequest,
    ExecutionBatchData, ManualTradingListQuery, MarketStrategyData, UpdateMarketStrategyRequest,
    UpdateWalletAccountRequest, WalletAccountData,
};
use polyedge_domain::{
    ExecutionBatch, ManagedMarket, ManagedMarketOutcome, ManagedOrder, ManagedPosition,
    MarketRewardTerms, MarketStrategy, QuoteOutcome, QuotePricingMode, StrategyQuoteSlot,
    StrategyVersion, StrategyWalletTarget, WalletAccount, WalletAccountState, WalletAccountStatus,
    WalletCredentialRef, WalletExecutionJob, WalletRiskPolicy,
};
use rust_decimal::Decimal;
use sqlx::{PgPool, Postgres, Row, Transaction, postgres::PgPoolOptions};
use std::{collections::HashMap, str::FromStr};
use time::OffsetDateTime;

pub enum IdempotencyBegin {
    Started { owner_token: String },
    Replay(serde_json::Value),
}

mod batches;
mod execution;
mod helpers;
mod order_reconciliation;
mod positions;
mod strategies;
mod trading;
mod wallets;

use batches::job_from_row;
use helpers::*;
use positions::replace_wallet_positions;
use strategies::slot_from_row;
use trading::managed_order_from_row;
use wallets::insert_audit;

#[derive(Clone)]
pub struct PostgresStore {
    pool: PgPool,
}

impl PostgresStore {
    pub async fn connect(database_url: &str, max_connections: u32) -> Result<Self> {
        let pool = PgPoolOptions::new()
            .max_connections(max_connections)
            .connect(database_url)
            .await?;
        Ok(Self { pool })
    }

    #[must_use]
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    pub async fn migrate(&self) -> Result<()> {
        sqlx::migrate!("../migrations_v2")
            .run(&self.pool)
            .await
            .map_err(|error| ServerError::Internal(format!("database migration failed: {error}")))
    }

    pub async fn ping(&self) -> bool {
        sqlx::query_scalar::<_, i64>("SELECT 1::bigint")
            .fetch_one(&self.pool)
            .await
            .is_ok()
    }
}
