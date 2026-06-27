use super::*;

mod market_event;
mod news;

pub struct PostgresMarketEventStore {
    pool: PgPool,
}

impl PostgresMarketEventStore {
    #[must_use]
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}
