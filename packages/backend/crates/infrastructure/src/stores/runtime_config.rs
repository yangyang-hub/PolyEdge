#[async_trait]
pub trait RuntimeConfigStore: Send + Sync {
    async fn load_values(&self) -> Result<BTreeMap<String, String>>;
    async fn save_values(&self, values: &BTreeMap<String, String>) -> Result<()>;
}

pub struct InMemoryRuntimeConfigStore {
    values: RwLock<BTreeMap<String, String>>,
}

impl InMemoryRuntimeConfigStore {
    #[must_use]
    pub fn new(defaults: BTreeMap<String, String>) -> Self {
        Self {
            values: RwLock::new(defaults),
        }
    }
}

#[async_trait]
impl RuntimeConfigStore for InMemoryRuntimeConfigStore {
    async fn load_values(&self) -> Result<BTreeMap<String, String>> {
        Ok(self.values.read().await.clone())
    }

    async fn save_values(&self, values: &BTreeMap<String, String>) -> Result<()> {
        self.values.write().await.extend(values.clone());
        Ok(())
    }
}

pub struct PostgresRuntimeConfigStore {
    pool: PgPool,
}

impl PostgresRuntimeConfigStore {
    #[must_use]
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn bootstrap(&self, defaults: &BTreeMap<String, String>) -> Result<()> {
        let mut transaction = self.pool.begin().await.map_err(|error| {
            db_error(
                "POSTGRES_TRANSACTION_BEGIN_FAILED",
                format!("failed to begin runtime config bootstrap transaction: {error}"),
            )
        })?;

        for (key, value) in defaults {
            sqlx::query(
                r#"
                INSERT INTO runtime_config (key, value, updated_at)
                VALUES ($1, $2, now())
                ON CONFLICT (key) DO UPDATE
                SET value = EXCLUDED.value,
                    updated_at = now()
                WHERE runtime_config.value = ''
                "#,
            )
            .bind(key)
            .bind(value)
            .execute(&mut *transaction)
            .await
            .map_err(|error| {
                db_error(
                    "POSTGRES_UPSERT_FAILED",
                    format!("failed to bootstrap runtime config: {error}"),
                )
            })?;
        }

        transaction.commit().await.map_err(|error| {
            db_error(
                "POSTGRES_TRANSACTION_COMMIT_FAILED",
                format!("failed to commit runtime config bootstrap transaction: {error}"),
            )
        })?;
        Ok(())
    }
}

#[async_trait]
impl RuntimeConfigStore for PostgresRuntimeConfigStore {
    async fn load_values(&self) -> Result<BTreeMap<String, String>> {
        let rows = sqlx::query(
            r#"
            SELECT key, value
            FROM runtime_config
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_QUERY_FAILED",
                format!("failed to query runtime config: {error}"),
            )
        })?;

        let mut values = BTreeMap::new();
        for row in rows {
            let key: String = row.try_get("key").map_err(postgres_decode_error)?;
            let value: String = row.try_get("value").map_err(postgres_decode_error)?;
            values.insert(key, value);
        }
        Ok(values)
    }

    async fn save_values(&self, values: &BTreeMap<String, String>) -> Result<()> {
        let mut transaction = self.pool.begin().await.map_err(|error| {
            db_error(
                "POSTGRES_TRANSACTION_BEGIN_FAILED",
                format!("failed to begin runtime config transaction: {error}"),
            )
        })?;

        for (key, value) in values {
            sqlx::query(
                r#"
                INSERT INTO runtime_config (key, value, updated_at)
                VALUES ($1, $2, now())
                ON CONFLICT (key) DO UPDATE
                SET value = EXCLUDED.value,
                    updated_at = now()
                "#,
            )
            .bind(key)
            .bind(value)
            .execute(&mut *transaction)
            .await
            .map_err(|error| {
                db_error(
                    "POSTGRES_UPSERT_FAILED",
                    format!("failed to upsert runtime config: {error}"),
                )
            })?;
        }

        transaction.commit().await.map_err(|error| {
            db_error(
                "POSTGRES_TRANSACTION_COMMIT_FAILED",
                format!("failed to commit runtime config transaction: {error}"),
            )
        })?;
        Ok(())
    }
}
