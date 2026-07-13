# Agent Guidelines

最后更新：2026-07-13

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
| Reward event windows | `polyedge-orderbook` Gamma market sync loops | Gamma `gameStartTime` / `events[].startTime` explicit schedules only | `reward_market_event_windows` + `reward_event_window_source_versions` | with each full/priority Gamma snapshot |
| Reward markets | `polyedge-orderbook` rewards catalog sync loop | CLOB API `/rewards/markets/current` | `reward_markets` table | after each run, default 5 min sleep |
| Order books | `polyedge-orderbook` service | CLOB WebSocket + `/books` batch poll, fallback `/book` | service-local `InMemoryOrderbookCache` | WS real-time + 10s reconcile |
| Reward price-history candles | `polyedge-orderbook` service | CLOB API `/prices-history` | `reward_market_candles` table | low-frequency rate-limited sync |
| Rewards account/order state | `polyedge-worker` rewards loop | authenticated CLOB, Data API fallback, Polygon RPC | rewards Postgres tables | live poll loop cadence |
| Reward fair-value estimates | `polyedge-worker` rewards loop | orderbook service books + local book history | `reward_fair_values` / `reward_fair_value_history` | each rewards live tick |

Orderbook subscriptions are owned by the standalone `polyedge-orderbook` service. It maintains WS + poll streams, in-memory orderbook cache, `OrderbookSubscriptionRegistry`, HTTP read/register/ingest APIs, and the internal `/orderbook/stream` feed. Workers and API handlers use `OrderbookHttpClient` and `OrderbookStreamClient`; they must not fetch CLOB orderbooks directly when orderbook service cache is available.

Registry source priority is fixed as `rewards_active`, `exec_orders`, `rewards_eligible`, `rewards_candidates`. Total subscribed tokens are capped by `POLYEDGE_ORDERBOOK_STREAM__MAX_TOKENS`; candidate prewarm is additionally capped by `POLYEDGE_ORDERBOOK_STREAM__REWARD_CANDIDATE_TOKEN_CAP`. AI/info-risk provider refresh no longer reads or temporarily subscribes to live orderbooks.

Orderbook cache-only reads remain unauthenticated, but positive `refresh_if_stale_ms` batches and the internal `/orderbook/stream` require the shared orderbook write token. Positive refresh requests are capped at 100 tokens and enter a bounded P0-P3 scheduler with a 2-second queue deadline, 8-second upstream deadline, weighted fairness and identical-token-set single-flight; deferred/failed refresh still returns existing cache plus a refresh summary. Internal stream connections are capped and slow sends time out.

Orderbook WS readers use a bounded non-blocking queue with per-token/price coalescing so cache pressure cannot block SDK heartbeat handling. Poll confirmation is version fenced: stale divergent snapshots beyond a 2-second safety window do not advance `confirmed_at`, and queue/drop/divergence/freshness metrics are exposed by `/orderbook/stats`. Authenticated ingest accepts only recent, non-crossed books with unique valid positive levels, and service time exclusively owns `confirmed_at`.

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
| `packages/backend/order/src/market_sync.rs` | Gamma full/priority sync, rewards catalog sync, explicit scheduled-event source snapshots |
| `packages/backend/order/src/candle_history.rs` | Rewards `/prices-history` sync into 5m source candles |
| `packages/backend/order/src/http_api.rs` | Orderbook read/batch/stats/register/ingest/internal stream APIs |
| `packages/backend/order/src/refresh_scheduler.rs` | Bounded priority CLOB refresh scheduler with deadlines, fairness and single-flight fan-out |
| `packages/backend/crates/application/src/orderbook_cache.rs` | Cached orderbook and stream event models |
| `packages/backend/crates/application/src/orderbook_registry.rs` | Multi-source token subscription registry trait |
| `packages/backend/crates/infrastructure/src/stores/orderbook_cache.rs` | In-memory orderbook cache with TTL and depth trimming |
| `packages/backend/crates/infrastructure/src/stores/orderbook_registry.rs` | In-memory registry with ordered source replacement and deterministic priority aggregation |
| `packages/backend/crates/connectors/src/polymarket/gamma.rs` + `gamma/scheduled_events.rs` | Gamma connector; separates lifecycle/deadline metadata from explicit schedules and emits stable multi-event keys |
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
| `packages/backend/crates/application/src/rewards/replay.rs` + `replay_v2.rs` | V1/V2/V3 deterministic replay; V3 is current capture and retains compact history/delta/hash encoding |
| `packages/backend/crates/application/src/rewards/market_selection.rs` | Maker market selection priority score and quote plan ordering |
| `packages/backend/crates/application/src/rewards/event_window.rs` + `event_window_source_models.rs` | Multi-event/source-priority event-window gate and source snapshot contract |
| `packages/backend/crates/application/src/rewards/ai_advisory_payload.rs` | Advisory payload and hourly candle aggregation |
| `packages/backend/crates/application/src/rewards/provider_prefilter.rs` | Pre-provider hard gate |
| `packages/backend/crates/application/src/rewards/run_ledger_models.rs` | Rewards strategy run, decision, action and order transition ledger models |
| `packages/backend/crates/application/src/rewards/action_planner.rs` | Planned strategy action proposal builder for orders and merge intents |
| `packages/backend/crates/infrastructure/src/stores/rewards.rs` + `rewards/*` | Rewards in-memory/Postgres persistence |
| `packages/backend/crates/infrastructure/src/stores/rewards/postgres_event_windows.rs` | Source-scoped event-window snapshot replace, advisory lock, tombstones and per-condition version/hash high-water |
| `packages/backend/crates/infrastructure/src/stores/rewards/postgres_run_ledger.rs` | Rewards strategy run ledger Postgres writes and queries |
| `packages/backend/apps/worker/src/worker/rewards.rs` | Rewards live tick and command execution |
| `packages/backend/apps/worker/src/worker/rewards/*` | Live sync, account sync, order submission/cancel/risk, provider refresh and event-cancel worker |
| `packages/backend/apps/worker/src/worker/rewards/replay_capture.rs` | Bounded asynchronous Replay V3 capture writer and shutdown drain |
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
| `packages/backend/migrations/0003_reward_event_window_semantics.sql` | Event-window semantic upgrade and `reward_event_window_source_versions` forward migration |

## 当前状态

- Frontend routes: `dashboard / markets / events / rewards / rewards/fair-value / funding / settings`.
- Frontend uses the real Rust API only; no mock-data mode.
- Frontend uses system font stacks and has no build-time Google Fonts dependency, so production builds do not require public font-network access.
- Rewards frontend protects unsaved config on page exit, debounces server-side table search, ignores stale snapshot responses, and rate-limits strategy-ledger pagination. Run, live-trading enable, automatic merge enable and reset show an explicit risk summary and use dedicated step-up scopes; cancel/reset operator notes are persisted through the control API. Fair-value summary statistics are explicitly scoped to the loaded page.
- Backend API routes cover markets, events, news, evidences, orders, trades, pricing, rewards bot, rewards strategy run ledger reads, funding, system, connector callback and orderbook reads.
- Rewards config/run/cancel/reset and Funding writes use the shared leased idempotency store and replay the first complete API response for the same key/payload. Expired `started` records can be owner-fenced and reclaimed; failures persist their error code. Requests accept an optional single-line `operator_note` (maximum 500 characters); control/Funding notes are persisted in command or audit reason and successful/accepted writes are recorded in `audit_logs`.
- Rewards dangerous operations use least-privilege step-up scopes: `rewards_run_once`, `rewards_live_trading_enable`, `rewards_merge_auto_execute`, and `rewards_state_reset`. A config payload that submits either dangerous boolean as `true` always requires its scope, including idempotent replay; cancel-all remains a protective console-write action without extra step-up.
- API CORS uses `POLYEDGE_CORS__ALLOWED_ORIGINS` exact origins. Wildcards, URL paths/query, and an empty production allowlist are rejected; same-origin calls remain available without CORS.
- Database schema uses a frozen clean-deploy baseline plus forward fixes: `0001_initial_schema.sql` must never be edited after release, `0002_reward_fair_value_history_identity.sql` upgrades fair-value history identity, `0003_reward_event_window_semantics.sql` upgrades event-window semantics/source versions, and `packages/backend/init.sql` represents the latest complete schema. `0003` is not rolling-compatible with old event-window producers, so deployment must stop every old producer process before migration and restart only the new runtime afterward. Historical migrations for removed modules remain gone.
- Runtime mode defaults to `live_auto`; old mock mode is removed.
- A newly initialized Postgres deployment loads `RewardBotConfig::production_live_drill_defaults()`: trading remains disabled and concentrates the uncalibrated drill in 1 market / 4 open orders, with a $20 per-market budget sized to cover common two-sided rewards minimums, $12 per-outcome and $20 global potential-exposure caps, a 5-second book freshness limit, $10k/$5k liquidity/volume gates, $150 opportunity exit depth, $50 live minimum depth and a 1-cent effective-edge minimum. The generic in-memory/test `Default` remains a permissive calculation baseline.
- `polyedge-orderbook` owns market sync, rewards catalog sync, price-history candle sync, orderbook WS/poll cache and registry.
- Orderbook WS fan-out enforces a default maximum of 8 Polymarket market-channel connections even when an older runtime config still requests 100-token chunks; the effective chunk size and reconnect policy are logged at stream startup.
- `polyedge-worker` supports database maintenance, news ingest/promotion, rewards live bot, rewards info-risk scan, execution drain, paper reconciliation, Polymarket order/fill/user-event workers, and orderbook token registration.
- Rewards bot is live-only. Standard maker quotes start at `quote_bid_rank` (default buy-one) and search through `quote_max_bid_rank` for the first post-only price preserving trading edge. Admission and capital priority use raw/effective edge after uncertainty and optional AI edge buffer; `reward_adjusted_edge_cents` is display/audit only. LP economics are capped at 10% of base quality and enter `selection_score` only through an explicit 10% reward-density term, after edge/exit/stability safety. `selection_score` remains the final capital priority; base `score` is secondary.
- Fair value now blends YES/NO midpoint parity with top-of-book microprice imbalance and history, and includes both dynamic market uncertainty and AI edge buffer in the final edge gate. The estimate is condition-scoped: Standard and BalancedMerge profiles share one estimate but calculate profile-specific decisions/edges; inconsistent cross-profile YES/NO token mappings fail closed. If an upstream event window already removed all quote legs, the decision is `not_evaluated`, not a fair-value failure: it does not add a fair-value blocker/selection penalty or write `fair_value_passed=false`. Persistence defensively normalizes latest rows by condition and makes history idempotent on `(condition_id, source, observed_at)`. If the first nominal rank fails the robust estimate, the planner searches deeper configured ranks before blocking.
- Live sizing uses `maker_market_budget_usd`, wallet availability, provider multiplier, per-outcome inventory headroom and global potential exposure. Inventory skew reduces the already-loaded outcome and favors the complementary outcome; resting BUY notional counts against `max_global_position_usd` because concurrent fills are possible. Rewards minimum size never overrides these risk budgets.
- AI advisory is a slow structural-risk reviewer and returns only `allow/reduce/stop_new` plus bounded size/edge modifiers. Info-risk returns evidence actions including directional cancellation; `directional_risk` means the resting-BUY outcome exposed to adverse selection, not the predicted winner. Cancellation requires confidence plus recent attributable sources, and breaking-news cancel requires two independent sources. Both providers exclude live price, side, rank, account and inventory context. Their confidence thresholds are `RewardBotConfig` fields, not environment variables.
- Provider-returned evidence is untrusted by default. A cancel action is downgraded to stop-new unless counted sources were promoted by a code-owned evidence verification pipeline; LLM-reported URLs/timestamps alone never cancel resting orders. Provider prompts isolate market text as untrusted data and reject embedded instructions.
- Stop-new and cancel are distinct. An ineligible/stop-new plan does not automatically cancel a safe resting order. Emergency book/fair-value/event risks cancel immediately; adverse downward repricing uses a short confirmation and bypasses competitive throttles; competitive upward repricing uses confirmation, cooldown and per-tick limits. Fills never trigger blanket sibling cancellation; the complementary BUY remains subject to its own edge/inventory/risk checks.
- Post-fill maker exits target cost basis/markup, while `maker_max_exit_loss_cents` defines a separate controlled flatten risk floor. Adaptive reselection and submitted-exit cancel-replace keep their existing durable reconciliation safeguards. BalancedMerge remains an independent fixed-rank profile. Merge intents enter a fail-closed `broadcasting` fence before chain submission; broadcasting rows without a persisted tx hash are never automatically replayed, and completed intents no longer reserve future paired inventory.
- Order lifecycle and BUY last-look resolve quote plans by `(condition_id, strategy_profile)`, while condition-scoped provider refresh evaluates every coexisting profile; standard and BalancedMerge plans cannot overwrite each other in live risk paths.
- Full tick records a strategy run/decision/action/order transition ledger and enqueues a bounded Replay V3 fixture without awaiting serialization or persistence. V3 retains V2's compact decision-window top-of-book history, final-state deltas and normalized expected-plan hashes while freezing the expanded event/fair-value decision model; readers remain compatible with V1, V2 and missing-version-as-V1 fixtures. A capacity-2 single-consumer writer runs expected hashing, sensitive-field scanning, canonical JSON/SHA and database persistence off the live tick, drops on backpressure and drains for at most 5 seconds on shutdown. Pre-provider, post-provider and final snapshot transforms remain centralized in `RewardDecisionEngine`; `RewardActionPlanner` writes `planned -> executing` with the same idempotency key before live side effects, and a failed executing write prevents the side effect. Fast reconcile/orderbook-event paths lazily create action-only runs only when they perform an external action. A Postgres-only durable action executor starts with the rewards poll loop, shares its account advisory lock, renews account-scoped leases and uses owner-fenced terminal writes. It handles idempotent merge-intent creation, validated cancel/cancel-replace, first-attempt PlaceBuy, match-first exit SELL and read-only reconciliation of execute-merge actions that already have a persisted Polygon tx hash. BUY/SELL always query venue orders first; no-match BUY must pass a fresh full last-look plus a current `RiskService` kill-switch read, while no-match SELL rechecks current inventory, notional and maker/flatten book semantics. Risk reads, ambiguous lookups and unknown submissions fail closed; recovered BUY execution is not replayed. Chain merge broadcasting remains in the fresh synchronous tick behind the broadcasting fence; the executor only queries receipts and never broadcasts/rebroadcasts a merge without a persisted hash.
- Opportunity metrics use the tick's injected timestamp, so live evaluation and replay share identical history cutoffs. Primary/fallback provider cache selection uses the newest evaluation time rather than TTL expiry.
- Event-window hard gating only accepts explicit discrete schedules with exact start provenance. Gamma `startDate`/`startDateIso` and `endDate`/`endDateIso` are lifecycle/resolution metadata and never become a hard gate; only `gameStartTime` and `events[].startTime` produce Gamma scheduled-event candidates. A condition may carry multiple stable `event_key` values. The application selects the highest-priority source per event key (`manual` can override or withdraw Gamma), then applies the most restrictive event across keys.
- Gamma event windows are persisted as source snapshots, not incremental rows. Each `coverage[]` entry declares `condition_id` plus its upstream `source_updated_at`; missing keys are tombstoned, Postgres serializes a source with a transaction advisory lock, and the per-condition `(producer_version, source_updated_at, observed_at)` high-water plus SHA-256 rejects stale or conflicting snapshots. `0003` quarantines legacy Gamma rows and makes all legacy rows without role/precision/provenance fail closed until a new producer republishes them.
- Orderbook event cancellation is condition-scoped: an update to either YES or NO refreshes the paired books, recalculates fair value for every coexisting profile and checks all resting BUY orders for that condition. Production live-drill book freshness is 5 seconds; catalog liquidity and 24h volume are both required, and reviewed explicit Gamma schedules participate from Medium confidence.
- Live orderbook validation failures are re-evaluated on every full rewards tick and persist for only 60 seconds for fast-path suppression/audit; they are never inherited into newly built plans, preventing stale 12-hour exclusions and cross-profile contamination between standard and BalancedMerge plans sharing a condition.
- Rewards worker prioritizes active order/position tokens, caps each full-tick book fetch at `MAX_TOKENS`, and limits each positive orderbook HTTP refresh request to 100 tokens while leaving cache-only reads large. Remote refresh failure preserves worker-local cache and does not restart the rewards poll runtime; all live actions still fail closed on `confirmed_at`. The orderbook service schedules P0 live action, P1 HTTP, P2 active poll and P3 candidate prewarm through one bounded weighted queue, with identical token sets coalesced across queued/in-flight requests. Registration logs raw/dedup/truncated coverage and warns when eligible plans yield no tokens or candidate prewarm is disabled.
- Execution-order registration uses a dedicated distinct-active-market query instead of the 200-row console order limit, so older submitted/open/partially-filled orders retain orderbook coverage.
- Reward price-history sync keeps first-backfill completion per token and batch-upserts each response, so an early failed cycle does not permanently leave later tokens with only the incremental window and large histories do not issue one SQL statement per point.
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
cargo run -p polyedge-worker -- run-database-maintenance-once
cargo run -p polyedge-worker -- scan-rewards-once
cargo run -p polyedge-worker -- poll-reward-bot
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
- The same token is required for worker internal stream connections and on-demand stale refresh requests; plain cache reads do not require it.
- `POLYEDGE_ORDERBOOK_STREAM__WS_CHUNK_SIZE` / `WS_MAX_CONNECTIONS` default to `500` / `8`; configure them in orderbook env, keeping the connection budget low enough to avoid upstream handshake rate limits.
- `POLYEDGE_ORDERBOOK_STREAM__REWARD_CANDIDATE_TOKEN_CAP` is explicitly `50` in the API deployment template. Setting it to `0` disables candidate prewarm and emits a worker warning; active/exec/eligible sources keep higher registry priority.
- Rewards live worker requires `POLYEDGE_REWARDS__ENABLED=true`, `POLYEDGE_WORKER__POLL_REWARD_BOT=true`, Postgres, orderbook service, and complete Polymarket credentials for real orders.
- `POLYEDGE_WORKER__POLL_REWARD_INFO_RISKS=true` is only needed for the standalone async info-risk scan path when AI advisory is not driving combined provider refresh.
- AI/info-risk action confidence thresholds and maker risk budgets are edited through Rewards config (`ai_action_min_confidence`, `info_risk_min_confidence`, `maker_market_budget_usd`); the removed `*_MIN_CONFIDENCE_BPS`, `per_market_usd`, `quote_size_usd` and `cancel_on_fill` settings are not supported.
- Deployment uses `deploy/.env.api.example`, `deploy/.env.orderbook.example`, and `deploy/.env.front.example`. Private keys and provider keys belong only in API env, not front or orderbook env.
- `POLYEDGE_CORS__ALLOWED_ORIGINS` is a comma-separated API browser allowlist and must include the exact production frontend origin. CORS is not authentication. The current static frontend has no login/session/Bearer-token acquisition path, so the deployment template keeps `POLYEDGE_AUTH__DISABLED=true` for usability and requires a VPN/private-network ACL or trusted reverse proxy boundary. Production additionally requires `POLYEDGE_AUTH__ALLOW_INSECURE_PRIVATE_DEPLOY=true` or refuses to start.
- When `POLYEDGE_AUTH__DISABLED=false`, production requires non-empty Ed25519 `POLYEDGE_AUTH__KEYS_JSON`. `POLYEDGE_AUTH__STEP_UP_CODE` is local-dev-only; production step-up scopes and expiry come from short-lived JWT claims issued by a trusted identity gateway. Do not place signing keys or long-lived JWTs in frontend public env/bundles.
- Docker Compose runs `polyedge-api` with embedded worker runtime, `polyedge-orderbook`, and `polyedge-front`; there is no separate worker service.
- Default production debugging endpoints remain: Frontend Rewards `http://192.168.31.5:33002/rewards`, API `http://100.87.45.72:38001`, Orderbook `http://100.87.45.72:38002`.

## Known Gaps

- Production-grade real session/auth UX is not complete; local/internal deployments commonly run with auth disabled.
- External Polymarket private tasks require real credentials, a funded account, small-size drills and an ops runbook before production use.
- Console order views cover PolyEdge-managed orders; account-wide visibility for unrelated external open orders is not complete.
- Deterministic Rewards replay covers stored decision inputs and expected-plan comparison, but does not yet simulate fill risk, exit cost or cancellation churn.
- Deposit Wallet lifecycle automation is incomplete: relayer wallet creation, pUSD wrapping/funding and approval batching remain external operational steps.
- Old arbitrage tables/migrations remain where still part of current baseline, but the active app no longer exposes old radar/signals/risk console flows.
- Several backend rewards/orderbook files and the frontend rewards config panel exceed the repository's documented physical-file hard limits; the current debt inventory is tracked in the package-level `AGENTS.md` files.
