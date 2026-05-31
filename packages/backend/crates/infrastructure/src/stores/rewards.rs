// Reward-bot persistence. An in-memory implementation backs tests and the no-database
// local path; a Postgres implementation backs shared, durable state. Both implement
// `RewardBotStore` and are split by backend; the row mappers and SQL helpers they share
// live in the parent `stores` module.

include!("rewards/in_memory.rs");
include!("rewards/postgres_control_commands.rs");
include!("rewards/postgres_writes.rs");
include!("rewards/postgres.rs");
