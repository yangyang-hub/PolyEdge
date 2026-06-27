// Smart Money Intelligence persistence. The initial implementation supports
// durable candidates, profiles, scores, source trades and read-only signal lists.

include!("smart_money/in_memory.rs");
include!("smart_money/postgres_rows.rs");
include!("smart_money/postgres_config.rs");
include!("smart_money/postgres.rs");
