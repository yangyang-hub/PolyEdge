// Copy-trading persistence. An in-memory implementation backs tests and the
// no-database local path; a Postgres implementation backs shared, durable state.
// Both implement `CopyTradeStore` and are split by backend.

include!("copytrade/in_memory.rs");
include!("copytrade/postgres_rows.rs");
include!("copytrade/postgres_control_commands.rs");
include!("copytrade/postgres.rs");
