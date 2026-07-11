# Agent Guidelines

最后更新：2026-07-11

## 维护规则

- **模块文档优先**：修改任何模块前，必须先查阅 `doc/modules/` 下对应的模块文档（索引见 [doc/modules/README.md](./doc/modules/README.md)）；修改后必须同步更新对应文档（顶部日期、关键文件、数据结构、当前状态）。
- 任何改变行为、路由、命令、环境变量、部署方式、依赖、集成状态或已知缺口的改动，都要同步更新本文件。
- 不要把设计文档里的目标能力写成已实现能力。
- 如果本文件、README、页面文案冲突，以本文件为仓库状态快照优先修正。

## 当前聚焦

PolyEdge 当前产品焦点是 Polymarket 做市商策略，核心路径为 rewards market maker 的 live 报价、fair-value 定价、风控、成交后退出和 BalancedMerge 合并。市场数据、事件/新闻和 Funding 保留为做市策略支撑能力；历史钱包类和独立研究模块已从前端路由、API、worker、application service、infrastructure store、DTO、数据库 schema 和模块文档中移除；新部署按当前 schema 重新初始化，不兼容旧表。

## 数据获取架构（编码时必须遵守）

### Single Source of Truth: Database + In-Memory Cache

ALL external API data MUST be fetched by background workers and stored in the database
or in-memory cache. Strategies, pages, and API handlers MUST read from these stores, never fetch directly from external APIs at request time.

### Market Data Pipeline

| Data | Producer | Source | Store | Interval |
|------|----------|--------|-------|----------|
| General markets | `polyedge-orderbook` Gamma market sync loops | Gamma API `/markets` + priority `/markets?condition_ids=...` | `markets` table | full fixed cadence + priority dynamic cadence |
| Reward markets | `polyedge-orderbook` rewards catalog sync loop | CLOB API `/rewards/markets/current` | `reward_markets` table | after each run, default 5 min sleep |
| Order books | `polyedge-orderbook` service | CLOB WebSocket + `/books` batch poll, fallback `/book` | service-local `InMemoryOrderbookCache` | WS real-time + 10s reconcile |
| Reward price-history candles | `polyedge-orderbook` service | CLOB API `/prices-history` | `reward_market_candles` table | low-frequency rate-limited sync |
| Rewards account/order state | `polyedge-worker` rewards loop | authenticated CLOB, Data API fallback, Polygon RPC | rewards Postgres tables | live poll loop cadence |
| Reward fair-value estimates | `polyedge-worker` rewards loop | orderbook service books + local book history | `reward_fair_values` / `reward_fair_value_history` | each rewards live tick |

Orderbook subscriptions are owned by the standalone `polyedge-orderbook` service. It maintains WS + poll streams, in-memory orderbook cache, `OrderbookSubscriptionRegistry`, HTTP read/register/ingest APIs, and the internal `/orderbook/stream` feed. Workers and API handlers use `OrderbookHttpClient` and `OrderbookStreamClient`; they must not fetch CLOB orderbooks directly when orderbook service cache is available.

Registry source priority is fixed as `rewards_active`, `exec_orders`, `rewards_eligible`, `rewards_candidates`. Total subscribed tokens are capped by `POLYEDGE_ORDERBOOK_STREAM__MAX_TOKENS`; candidate prewarm is additionally capped by `POLYEDGE_ORDERBOOK_STREAM__REWARD_CANDIDATE_TOKEN_CAP`. AI/info-risk provider refresh no longer reads or temporarily subscribes to live orderbooks.

Polymarket market-channel WS uses a target chunk size plus a hard connection budget: `POLYEDGE_ORDERBOOK_STREAM__WS_CHUNK_SIZE` defaults to 500 and `POLYEDGE_ORDERBOOK_STREAM__WS_MAX_CONNECTIONS` defaults to 8. The service automatically enlarges the effective chunk size when required, staggers chunk startup by 500ms, and configures SDK reconnect backoff to 30-120 seconds to avoid Cloudflare 429/1015 reconnect storms. Poll reconcile `/books` batches are spaced by 100ms.

`observed_at` means orderbook content version, while `confirmed_at` means the service recently confirmed the book via WS/poll/ingest. Rewards placement and cancellation freshness checks use `confirmed_at` so quiet markets are not incorrectly treated as stale.

### Anti-Patterns To Avoid

- Calling Polymarket APIs directly from API handlers or strategy code.
- Fetching market metadata from external APIs at request time.
- Creating connector calls outside the designated worker/orderbook sync pipeline.
- Reading market data from Polymarket when it exists in Postgres.
- Fetching orderbooks directly from CLOB when they exist in the orderbook service cache.
- Duplicating data fetching logic across workers, API handlers, and strategies.

## Key Files

| File | Role |
|------|------|
| `packages/backend/order/src/main.rs` | Standalone orderbook service entrypoint: HTTP server, Gamma sync, rewards catalog sync, WS/poll stream, token registry |
| `packages/backend/order/src/market_sync.rs` | Gamma full/priority sync, rewards catalog sync, event-window candidates from Gamma dates |
| `packages/backend/order/src/candle_history.rs` | Rewards `/prices-history` sync into 5m source candles |
| `packages/backend/order/src/http_api.rs` | Orderbook read/batch/stats/register/ingest/internal stream APIs |
| `packages/backend/crates/application/src/orderbook_cache.rs` | Cached orderbook and stream event models |
| `packages/backend/crates/application/src/orderbook_registry.rs` | Multi-source token subscription registry trait |
| `packages/backend/crates/infrastructure/src/stores/orderbook_cache.rs` | In-memory orderbook cache with TTL and depth trimming |
| `packages/backend/crates/infrastructure/src/stores/orderbook_registry.rs` | In-memory registry with ordered source replacement and deterministic priority aggregation |
| `packages/backend/crates/connectors/src/polymarket/gamma.rs` | Gamma connector |
| `packages/backend/crates/connectors/src/polymarket/live.rs` + `live/raw.rs` | Authenticated CLOB connector, live orders, balances, rewards earnings fallback |
| `packages/backend/crates/connectors/src/polymarket/data_api.rs` | Public Data API connector used only for rewards trade/position reconciliation fallback |
| `packages/backend/crates/connectors/src/rewards.rs` + `rewards/*` | Rewards catalog, orderbook batch/fallback and price-history connectors |
| `packages/backend/crates/connectors/src/orderbook.rs` | Orderbook service HTTP/internal WS client |
| `packages/backend/crates/connectors/src/reward_provider.rs` | Combined rewards provider connector for AI advisory and info-risk |
| `packages/backend/api/src/lib.rs` | API routes: markets/events/news/evidences/orders/trades/pricing/rewards/funding/system/orderbook |
| `packages/backend/api/src/handlers/rewards.rs` | Rewards snapshot/config/control API and strategy run ledger read APIs |
| `packages/backend/api/src/handlers/funding.rs` | Funding API for backend-signed Polygon bridge deposits |
| `packages/backend/crates/application/src/rewards/service.rs` | RewardBotService and command queue wake channel |
| `packages/backend/crates/application/src/rewards/config_impl.rs` | Rewards defaults, normalization and config patch application |
| `packages/backend/crates/application/src/rewards/engine.rs` | RewardDecisionEngine pure decision transforms and tick outcome model |
| `packages/backend/crates/application/src/rewards/strategy_input.rs` | `RewardStrategyInput` serializable tick input snapshot and `RewardLiveCycle::from_strategy_input` bridge |
| `packages/backend/crates/application/src/rewards/planner.rs` | Deterministic rewards quote planner |
| `packages/backend/crates/application/src/rewards/planner_live.rs` | Live orderbook quote materializer |
| `packages/backend/crates/application/src/rewards/opportunity_metrics.rs` | Unified opportunity scoring |
| `packages/backend/crates/application/src/rewards/fair_value.rs` | Market-implied fair-value estimate and quote edge gate |
| `packages/backend/crates/application/src/rewards/market_selection.rs` | Maker market selection priority score and quote plan ordering |
| `packages/backend/crates/application/src/rewards/event_window.rs` | Event-window risk gate |
| `packages/backend/crates/application/src/rewards/ai_advisory_payload.rs` | Advisory payload and hourly candle aggregation |
| `packages/backend/crates/application/src/rewards/provider_prefilter.rs` | Pre-provider hard gate |
| `packages/backend/crates/application/src/rewards/run_ledger_models.rs` | Rewards strategy run, decision, action and order transition ledger models |
| `packages/backend/crates/application/src/rewards/action_planner.rs` | Planned strategy action proposal builder for orders and merge intents |
| `packages/backend/crates/infrastructure/src/stores/rewards.rs` + `rewards/*` | Rewards in-memory/Postgres persistence |
| `packages/backend/crates/infrastructure/src/stores/rewards/postgres_run_ledger.rs` | Rewards strategy run ledger Postgres writes and queries |
| `packages/backend/apps/worker/src/worker/rewards.rs` | Rewards live tick and command execution |
| `packages/backend/apps/worker/src/worker/rewards/*` | Live sync, account sync, order submission/cancel/risk, provider refresh and event-cancel worker |
| `packages/backend/apps/worker/src/worker/orderbook_registration.rs` | Active/eligible/candidate token registration |
| `packages/backend/apps/worker/src/worker/service.rs` | Embedded worker runtime task wiring |
| `packages/backend/apps/worker/src/worker/database_maintenance.rs` | Database retention cleanup worker |
| `packages/backend/crates/application/src/maintenance.rs` | Database maintenance cutoffs/report models |
| `packages/backend/crates/infrastructure/src/stores/maintenance.rs` | Postgres/no-op maintenance store |
| `packages/front/src/app/(console)/rewards/page.tsx` | Rewards console route |
| `packages/front/src/app/(console)/rewards/fair-value/page.tsx` | Fair-value workbench route |
| `packages/front/src/features/rewards/components/rewards-config-panel.tsx` | Rewards config UI |
| `packages/front/src/features/rewards/components/rewards-fair-value-workbench.tsx` | Fair-value estimate/edge audit UI |
| `packages/front/src/features/rewards/components/rewards-run-ledger-panel.tsx` | Strategy run ledger audit UI |
| `packages/front/src/features/rewards/components/rewards-opportunity-config.tsx` | Opportunity scoring config UI |
| `packages/front/src/features/rewards/components/rewards-advanced-config.tsx` | Book selection, AI advisory, info-risk and event-window config UI |
| `packages/front/src/features/rewards/components/rewards-tables.tsx` | Rewards quote plan/order/position/fill/event tables |
| `packages/front/src/app/layout.tsx` + `app/globals.css` | Frontend root providers and offline-safe system font theme |
| `packages/front/src/app/(console)/funding/page.tsx` | Funding route |
| `packages/front/src/lib/api/rewards.ts` + `actions/rewards.ts` | Rewards frontend data/actions |
| `packages/front/src/lib/contracts/dto/rewards.ts` | Rewards frontend DTOs |
| `packages/backend/init.sql` | Empty-database initialization snapshot generated from current migrations |

## 当前状态

- Frontend routes: `dashboard / markets / events / rewards / rewards/fair-value / funding / settings`.
- Frontend uses the real Rust API only; no mock-data mode.
- Frontend uses system font stacks and has no build-time Google Fonts dependency, so production builds do not require public font-network access.
- Backend API routes cover markets, events, news, evidences, orders, trades, pricing, rewards bot, rewards strategy run ledger reads, funding, system, connector callback and orderbook reads.
- Database schema is currently a single clean-deploy baseline: `packages/backend/init.sql` and `packages/backend/migrations/0001_initial_schema.sql`. Historical incremental migrations for removed modules are gone; new deployments initialize from the current schema baseline.
- Runtime mode defaults to `live_auto`; old mock mode is removed.
- A newly initialized Postgres deployment loads `RewardBotConfig::production_live_drill_defaults()`: trading remains disabled, with 2 markets / 6 open orders, $10 per-market budget, $10 per-position and $25 global exposure caps, 1-cent effective-edge minimum, stricter liquidity/stability/event windows, one competitive cancel per cycle, and enabled structural depth-shock guards. The generic in-memory/test `Default` remains a permissive calculation baseline.
- `polyedge-orderbook` owns market sync, rewards catalog sync, price-history candle sync, orderbook WS/poll cache and registry.
- Orderbook WS fan-out enforces a default maximum of 8 Polymarket market-channel connections even when an older runtime config still requests 100-token chunks; the effective chunk size and reconnect policy are logged at stream startup.
- `polyedge-worker` supports database maintenance, news ingest/promotion, rewards live bot, rewards info-risk scan, execution drain, paper reconciliation, Polymarket order/fill/user-event workers, and orderbook token registration.
- Rewards bot is live-only. Standard maker quotes start at `quote_bid_rank` (default buy-one) and search through `quote_max_bid_rank` for the first post-only price preserving trading edge. Admission and capital priority use raw/effective edge after uncertainty and optional AI edge buffer; `reward_adjusted_edge_cents` is display/audit only. LP economics are capped at 10% of base quality and enter `selection_score` only through an explicit 10% reward-density term, after edge/exit/stability safety. `selection_score` remains the final capital priority; base `score` is secondary.
- Live sizing uses `maker_market_budget_usd`, wallet availability, provider multiplier, per-outcome inventory headroom and global potential exposure. Inventory skew reduces the already-loaded outcome and favors the complementary outcome; resting BUY notional counts against `max_global_position_usd` because concurrent fills are possible. Rewards minimum size never overrides these risk budgets.
- AI advisory is a slow structural-risk reviewer and returns only `allow/reduce/stop_new` plus bounded size/edge modifiers. Info-risk returns evidence actions including directional cancellation; `directional_risk` means the resting-BUY outcome exposed to adverse selection, not the predicted winner. Cancellation requires confidence plus recent attributable sources, and breaking-news cancel requires two independent sources. Both providers exclude live price, side, rank, account and inventory context. Their confidence thresholds are `RewardBotConfig` fields, not environment variables.
- Stop-new and cancel are distinct. An ineligible/stop-new plan does not automatically cancel a safe resting order. Emergency book/fair-value/event risks cancel immediately; adverse downward repricing uses a short confirmation and bypasses competitive throttles; competitive upward repricing uses confirmation, cooldown and per-tick limits. Fills never trigger blanket sibling cancellation; the complementary BUY remains subject to its own edge/inventory/risk checks.
- Post-fill maker exits target cost basis/markup, while `maker_max_exit_loss_cents` defines a separate controlled flatten risk floor. Adaptive reselection and submitted-exit cancel-replace keep their existing durable reconciliation safeguards. BalancedMerge remains an independent fixed-rank profile.
- Order lifecycle and BUY last-look resolve quote plans by `(condition_id, strategy_profile)`, while condition-scoped provider refresh evaluates every coexisting profile; standard and BalancedMerge plans cannot overwrite each other in live risk paths.
- Full tick records a strategy run/decision/action/order transition ledger and automatically stores a bounded, sensitive-field-checked replay fixture. Pre-provider, post-provider and final snapshot transforms remain centralized in `RewardDecisionEngine`; `RewardActionPlanner` writes `planned -> executing` with the same idempotency key before live side effects, and a failed executing write prevents the side effect. Fast reconcile/orderbook-event paths lazily create action-only runs only when they perform an external action. A Postgres-only durable action executor starts with the rewards poll loop, shares its account advisory lock, renews account-scoped leases and uses owner-fenced terminal writes. It handles idempotent merge-intent creation, validated cancel/cancel-replace, first-attempt PlaceBuy, match-first exit SELL and read-only reconciliation of execute-merge actions that already have a persisted Polygon tx hash. BUY/SELL always query venue orders first; no-match BUY must pass a fresh full last-look, while no-match SELL rechecks current inventory, notional and maker/flatten book semantics. Ambiguous/failed lookup and unknown submission fail closed; recovered BUY execution is not replayed. Chain merge broadcasting remains in the fresh synchronous tick; the executor only queries receipts, fences intent resolution by intent id + tx hash, and never broadcasts/rebroadcasts a merge without a persisted hash.
- Live orderbook validation failures are re-evaluated on every full rewards tick and persist for only 60 seconds for fast-path suppression/audit; they are never inherited into newly built plans, preventing stale 12-hour exclusions and cross-profile contamination between standard and BalancedMerge plans sharing a condition.
- Rewards worker prioritizes active order/position tokens, caps each full-tick book fetch at `MAX_TOKENS`, and limits each orderbook HTTP refresh request to 500 tokens. The orderbook service serializes background poll and HTTP on-demand CLOB `/books` batches through one shared gate, rechecks staleness after waiting, and spaces 100-token upstream batches by 100ms.
- LLM calls for rewards combined provider are recorded in `llm_calls(task_type=reward_provider)`. Provider cache hits do not count as external calls.
- Database maintenance prunes raw events, expired AI/info-risk caches, reward candles, fair-value history, strategy run ledger, order transitions, completed/failed control commands, outbox/external dedup, LLM calls, audit logs and mode transitions. It preserves current rewards orders, fills, positions and account state.

## Commands

Backend:

```bash
cd packages/backend
cargo fmt --all
cargo check --workspace --tests
cargo test --workspace
cargo run -p polyedge-api
cargo run -p polyedge-orderbook
cargo run -p polyedge-worker
```

Common worker commands:

```bash
cargo run -p polyedge-worker -- ingest-news-once
cargo run -p polyedge-worker -- poll-news
cargo run -p polyedge-worker -- promote-news-events
cargo run -p polyedge-worker -- scan-rewards-once
cargo run -p polyedge-worker -- poll-reward-bot
cargo run -p polyedge-worker -- poll-reward-action-executor
cargo run -p polyedge-worker -- poll-reward-action-executor
cargo run -p polyedge-worker -- scan-reward-info-risks-once
cargo run -p polyedge-worker -- poll-reward-info-risks
cargo run -p polyedge-worker -- drain-execution-queue
cargo run -p polyedge-worker -- poll-polymarket-order-statuses
cargo run -p polyedge-worker -- reconcile-polymarket-fills
cargo run -p polyedge-worker -- consume-polymarket-user-events
cargo run -p polyedge-replay -- --run-id <RUN_ID>
cargo run -p polyedge-replay -- --fixture <FIXTURE.json>
cargo run -p polyedge-replay -- --stored-run-id <RUN_ID>
```

Frontend:

```bash
cd packages/front
yarn install
yarn dev
yarn lint
yarn build
```

## Configuration Notes

- Backend API listens on `0.0.0.0:38001` by default.
- Orderbook listens on `0.0.0.0:38002` by default.
- `POLYEDGE_ORDERBOOK__SERVICE_URL` must point API/worker to the orderbook service. In Docker Compose use `http://polyedge-orderbook:38002`.
- `POLYEDGE_ORDERBOOK__WRITE_TOKEN` is required for worker token registration and must match orderbook service env.
- `POLYEDGE_ORDERBOOK_STREAM__WS_CHUNK_SIZE` / `WS_MAX_CONNECTIONS` default to `500` / `8`; configure them in orderbook env, keeping the connection budget low enough to avoid upstream handshake rate limits.
- Rewards live worker requires `POLYEDGE_REWARDS__ENABLED=true`, `POLYEDGE_WORKER__POLL_REWARD_BOT=true`, Postgres, orderbook service, and complete Polymarket credentials for real orders.
- `POLYEDGE_WORKER__POLL_REWARD_INFO_RISKS=true` is only needed for the standalone async info-risk scan path when AI advisory is not driving combined provider refresh.
- AI/info-risk action confidence thresholds and maker risk budgets are edited through Rewards config (`ai_action_min_confidence`, `info_risk_min_confidence`, `maker_market_budget_usd`); the removed `*_MIN_CONFIDENCE_BPS`, `per_market_usd`, `quote_size_usd` and `cancel_on_fill` settings are not supported.
- Deployment uses `deploy/.env.api.example`, `deploy/.env.orderbook.example`, and `deploy/.env.front.example`. Private keys and provider keys belong only in API env, not front or orderbook env.
- Docker Compose runs `polyedge-api` with embedded worker runtime, `polyedge-orderbook`, and `polyedge-front`; there is no separate worker service.
- Default production debugging endpoints remain: Frontend Rewards `http://192.168.31.5:33002/rewards`, API `http://100.87.45.72:38001`, Orderbook `http://100.87.45.72:38002`.

## Known Gaps

- Production-grade real session/auth UX is not complete; local/internal deployments commonly run with auth disabled.
- External Polymarket private tasks require real credentials, a funded account, small-size drills and an ops runbook before production use.
- Old arbitrage tables/migrations remain where still part of current baseline, but the active app no longer exposes old radar/signals/risk console flows.
