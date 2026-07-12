use sha2::{Digest, Sha256};

const FROZEN_INITIAL_SCHEMA_SHA256: &str =
    "e004e2fcd79d7c8ffa1260323ac0ac29bf22e4f98845ccc7f685734f25e2e4c8";
static TEST_MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("../../migrations");

#[test]
fn released_initial_migration_checksum_is_frozen() {
    let migration = include_bytes!("../../../migrations/0001_initial_schema.sql");
    let checksum = format!("{:x}", Sha256::digest(migration));

    assert_eq!(checksum, FROZEN_INITIAL_SCHEMA_SHA256);
}

#[test]
fn embedded_migrations_include_the_forward_schema_fix() {
    let versions = TEST_MIGRATOR
        .migrations
        .iter()
        .map(|migration| migration.version)
        .collect::<Vec<_>>();

    assert_eq!(versions, vec![1, 2]);
}
