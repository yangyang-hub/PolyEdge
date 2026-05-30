# Agent Guidelines

## Data Fetching Architecture

### Single Source of Truth: Database + Redis Cache

ALL external API data MUST be fetched by background workers and stored in the database
or Redis cache. Strategies, pages, and API handlers MUST read from these stores — NEVER
fetch directly from external APIs (Polymarket Gamma, CLOB, etc.).

### Market Data Pipeline

| Data | Worker | Source | Store | Interval |
|------|--------|--------|-------|----------|
| General markets | `sync-markets` | Gamma API `/markets/keyset` | `markets` table (Postgres) | 5 min |
| Reward markets | `sync-markets` | CLOB API `/rewards/markets/current` | `reward_markets` table (Postgres) | 5 min |
| Order books | `orderbook-stream` | CLOB WebSocket + `/book` poll | `ob:{token_id}` (Redis) | WS real-time + 30s poll reconcile |

All three are written by workers. All consumers read from the store, never from the API.

### Why This Architecture Exists

Previously the rewards bot fetched market data directly from Polymarket's CLOB API
every 60 seconds. The enrichment step (fetching `/markets/{condition_id}` for token
data) failed at scale due to rate limiting, causing only ~50 of 500+ markets to survive
the `tokens >= 2` filter. Centralizing API fetching in the sync worker with proper
retries solves this and ensures consistent data across all consumers.

### Anti-patterns to Avoid

- ❌ Calling Polymarket APIs directly from API handlers or strategy code
- ❌ Fetching market metadata (questions, tokens, slugs) from external APIs at request time
- ❌ Creating new connector calls outside the worker sync pipeline
- ❌ Reading market data from Polymarket when it exists in the database
- ❌ Fetching order books directly from CLOB when they exist in the Redis cache
- ❌ Duplicating data fetching logic across workers, API handlers, and strategies

### Key Files

| File | Role |
|------|------|
| `apps/worker/src/worker/market_sync.rs` | Sync worker — fetches markets from Polymarket, writes to Postgres |
| `apps/worker/src/worker/orderbook_stream.rs` | Orderbook stream — WS + poll, writes to Redis cache |
| `apps/worker/src/worker/rewards.rs` | Rewards bot — reads markets from Postgres, order books from Redis |
| `apps/api/src/handlers/reward_inputs.rs` | API handler — reads markets from Postgres, order books from Redis |
| `crates/application/src/rewards/service.rs` | RewardBotService — `upsert_reward_markets`, `list_active_reward_markets` |
| `crates/application/src/orderbook_cache.rs` | OrderbookCache trait — `get_book`, `set_book`, `set_books` |
| `crates/infrastructure/src/stores/orderbook_cache.rs` | Redis + in-memory OrderbookCache implementations |
