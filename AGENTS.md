# Agent Guidelines

最后更新：2026-06-24

## 维护规则

- **模块文档优先**：修改任何模块前，必须先查阅 `doc/modules/` 下对应的模块文档（索引见 [doc/modules/README.md](./doc/modules/README.md)）；修改后必须同步更新对应文档（顶部日期、关键文件、数据结构、当前状态）。
- 任何改变行为、路由、命令、环境变量、部署方式、依赖、集成状态或已知缺口的改动，都要同步更新本文件。
- 不要把设计文档里的目标能力写成已实现能力。
- 如果本文件、README、页面文案冲突，以本文件为仓库状态快照优先修正。

## 数据获取架构（编码时必须遵守）

### Single Source of Truth: Database + In-Memory Cache

ALL external API data MUST be fetched by background workers and stored in the database
or in-memory cache. Strategies, pages, and API handlers MUST read from these stores — NEVER
fetch directly from external APIs (Polymarket Gamma, CLOB, etc.).

### Market Data Pipeline

| Data | Producer | Source | Store | Interval |
|------|--------|--------|-------|----------|
| General markets | `polyedge-orderbook` Gamma market sync loops | Gamma API `/markets` + priority `/markets?condition_ids=...` | `markets` table (Postgres) | full fixed cadence + priority dynamic cadence |
| Reward markets | `polyedge-orderbook` rewards catalog sync loop | CLOB API `/rewards/markets/current` | `reward_markets` table (Postgres) | after each run, default 5 min sleep |
| Order books | `polyedge-orderbook` 服务 | CLOB WebSocket + `/books` batch poll（回退 `/book`） | `InMemoryOrderbookCache`（orderbook 服务进程内，TTL 5 分钟） | WS real-time + 10s full reconcile |
| Reward price-history candles | `polyedge-orderbook` 服务 | CLOB API `/prices-history`（5m fidelity） | `reward_market_candles` table (Postgres) | low-frequency rate-limited history sync, default 5 min cadence |

Orderbook 订阅由独立的 `polyedge-orderbook` 服务管理。该服务始终运行 WS + poll stream，维护进程内缓存和 `OrderbookSubscriptionRegistry`，暴露 HTTP API（`GET /orderbook/{token_id}`、`POST /orderbook/batch`、`GET /orderbook/stats`、`POST /orderbook/register` 等）和内部 WS 推送接口（`GET /orderbook/stream`）。Worker 和 API 通过 `OrderbookHttpClient`（HTTP 调用 orderbook 服务）读取盘口数据；普通 batch 只读缓存，`get_books_with_max_age()` 会在 `POST /orderbook/batch` 请求体携带 `refresh_if_stale_ms`，orderbook 服务仅对缺失或 `confirmed_at` 超过该年龄的 token 同步 CLOB `/books` 刷新后再返回缓存结果，刷新失败则记录 warn 并返回现有缓存由调用方 fail closed。rewards worker 长期 poll loop 还会通过 `OrderbookStreamClient` 连接内部 WS，维护 worker 本地盘口 cache 并用活跃 token 更新唤醒 fast reconcile；内部 WS 连接建立最多等待 5 秒，worker 在约 3 个 poll reconcile 周期无消息后会主动重连并重新 HTTP bootstrap。Worker 通过携带 `POLYEDGE_ORDERBOOK__WRITE_TOKEN` 注册 token。`/orderbook/register` 会原子替换对应 source 当前有序 token 集合，空集合会删除该 source，避免 DELETE/POST 空窗、陈旧来源残留和同一 source 单调增长；worker 周期注册任务会对成功空集合做防抖，`rewards_active`/`exec_orders` 连续 2 轮为空、`rewards_eligible`/`rewards_candidates` 连续 3 轮为空才清远端 source，查询失败或即时 active 刷新读到空集合会保留上一版。HTTP registry 最多保留 32 个 source，in-memory registry 在写锁内再次原子校验上限；`/orderbook/stats` 返回真实 cache 条目数、registry 来源数和 registry 去重 token 总数。聚合优先级固定为 `rewards_active`、`exec_orders`、`rewards_eligible`、`rewards_ai_provider`、`rewards_low_competition_probe`、`rewards_candidates`、`copytrade`；总量受 `POLYEDGE_ORDERBOOK_STREAM__MAX_TOKENS` 限制，`rewards_low_competition_probe` 由 rewards full tick 按最多 10 个低竞争候选市场一批预热 gate 前盘口，批次达到样本要求、候选失效或 5 分钟超时后轮转；`rewards_candidates` 预热来源还受 `POLYEDGE_ORDERBOOK_STREAM__REWARD_CANDIDATE_TOKEN_CAP` 限制（默认 50）；Polymarket WS 订阅按 `POLYEDGE_ORDERBOOK_STREAM__WS_CHUNK_SIZE` 分片成多条连接（默认 100 token/连接），降低高消息量下 SDK broadcast lag 风险；chunk 内 SDK stream reader 会先快速 drain `book`/`price_change` 事件，再交给缓存写入循环处理，减少慢写入阻塞 SDK broadcast receiver；chunk 退出或被取消时会成对释放 SDK market subscription；stream refresh 在聚合 token 成员集合变化后短暂 debounce 并再次确认：默认（`POLYEDGE_ORDERBOOK_STREAM__WS_INCREMENTAL_RECONCILE=true`）WS 连接常驻，只对 diff 做增量 subscribe/unsubscribe（先 subscribe 新集合再 unsubscribe 旧集合，SDK 按资产 refcount 只发新增/退订帧，保持共享 Market 通道始终非空），只有 reader 结束（连接死亡）才重建那一个 chunk，其余连接不受影响；`WS_INCREMENTAL_RECONCILE=false` 回退到成员变化即整体重建 WS 的旧行为。单纯顺序变化只更新 poll reconciler 的共享列表，不触发任何重订/重连。register/ingest/delete 写接口要求共享写 token，未配置时写接口关闭；HTTP ingest 会先校验整批盘口，再批量写入并传播缓存错误。WS 同时消费完整 `book` 快照和 `price_change` 增量；所有缓存写入会先把 bids 按价格降序、asks 按价格升序排序，再保留每侧最多 `POLYEDGE_ORDERBOOK_STREAM__MAX_LEVELS_PER_SIDE` 档深度（默认 100），并拒绝旧 `observed_at` 覆盖更新盘口。每次 WS snapshot、WS price_change、poll reconcile、按需 batch refresh 或 HTTP ingest 成功写入缓存后，orderbook 服务都会广播携带单调 sequence、reason 和 `CachedOrderBook` 的 `OrderbookStreamEvent`；慢消费者需断线后重新 HTTP bootstrap。rewards midpoint candle 不再由这些高频 cache 更新派生，改由 orderbook 服务独立低频限速调用 CLOB `/prices-history` 写库，避免本地 candle 队列在高频 price_change 下打满。poll reconciler 默认每 10 秒优先刷新 stale token，随后刷新其余注册 token，使用 CLOB `/books` 批量接口并在失败或遗漏时回退 `/book`，以修复未被发现的 WS 增量丢失；stale threshold 小于等于 0 时只关闭年龄 stale 优先级。
Orderbook 进程内缓存只让未过期条目拒绝旧 `observed_at` 覆盖盘口内容，但会合并更新的 `confirmed_at` 作为最近确认时间；已过期条目不会阻挡后续 poll/ingest 恢复。`observed_at` 表示盘口内容版本，`confirmed_at` 表示 orderbook 服务最近通过 WS/poll/ingest 确认该盘口仍可用；rewards 新挂单、撤单和远程刷新 stale 判断使用 `confirmed_at`，安静市场只要最近被 poll 确认过就不会因内容版本长期不变被误判过期。rewards worker 本地盘口 cache 的 TTL 按本地接收/写入时间计算，不用上游 `observed_at` 延长缓存寿命。

市场和奖励市场由 orderbook 服务同步写入 Postgres，盘口数据由 orderbook 服务流式写入进程内缓存；rewards token 的 5 分钟 midpoint K 线由 orderbook 服务从 CLOB `/prices-history` 低频限速同步写入 `reward_market_candles`，不包含真实成交量。price-history 行会把 provider price 同时写入 close、`best_bid_close` 和 `best_ask_close`，`spread_cents_close=0`，`sample_count` 代表同 bucket 内持久化的 provider history 点数量。所有消费者从数据库或 orderbook 服务读取，不直接调用外部 API。

### Why This Architecture Exists

Previously the rewards bot fetched market data directly from Polymarket's CLOB API
every 60 seconds. The enrichment step (fetching `/markets/{condition_id}` for token
data) failed at scale due to rate limiting, causing only ~50 of 500+ markets to survive
the `tokens >= 2` filter. Centralizing API fetching in the designated sync producer
with proper retries solves this and ensures consistent data across all consumers.
The designated sync producer is now the standalone `polyedge-orderbook` service.

### Anti-patterns to Avoid

- ❌ Calling Polymarket APIs directly from API handlers or strategy code
- ❌ Fetching market metadata (questions, tokens, slugs) from external APIs at request time
- ❌ Creating new connector calls outside the designated worker/orderbook sync pipeline
- ❌ Reading market data from Polymarket when it exists in the database
- ❌ Fetching order books directly from CLOB when they exist in the in-memory cache
- ❌ Duplicating data fetching logic across workers, API handlers, and strategies

### Key Data Files

| File | Role |
|------|------|
| `packages/backend/apps/worker/src/worker/market_sync.rs` | 市场同步 CLI 兼容入口；daemon 同步已迁移到 orderbook 服务 |
| `packages/backend/apps/worker/src/worker/orderbook_stream.rs` | Orderbook stream — 仅保留 CLI 子命令兼容，核心逻辑已迁移到 polyedge-orderbook 服务 |
| `packages/orderbook/src/main.rs` | 独立 orderbook 服务入口 — HTTP server、Gamma full/priority sync、rewards catalog sync、WS stream + token 注册 |
| `packages/orderbook/src/market_sync.rs` | Orderbook market sync — Gamma full sync、priority condition sync、rewards catalog sync |
| `packages/orderbook/src/candle_history.rs` | Rewards candle history sync — 限速调用 CLOB `/prices-history` 写入 5m price-history candles |
| `packages/orderbook/src/http_api.rs` | Orderbook HTTP/API — read/batch/stats/register/ingest、内部 WS stream、写 token 校验、最优档排序 |
| `packages/orderbook/src/updates.rs` | Orderbook update broadcaster — 为 WS/poll/ingest 缓存更新分配 sequence、推送内部 WS |
| `packages/backend/crates/common/src/lib.rs` | 后端二进制共享进程外壳 helper — bind address、TCP listener、Ctrl-C/SIGTERM shutdown |
| `packages/backend/crates/connectors/src/polymarket/gamma.rs` | Gamma markets connector — `/markets` offset 分页、condition_ids 批量查询、market id 去重 |
| `packages/backend/crates/connectors/src/polymarket/chain.rs` | Polygon chain connector — 读取资金钱包链上 pUSD ERC20 余额 |
| `packages/backend/crates/connectors/src/polymarket/live.rs` + `live/raw.rs` | Polymarket live connector — CLOB V2 认证、heartbeat、收益查询 raw fallback、余额/订单/下单/撤单 |
| `packages/backend/crates/connectors/src/polymarket/live/trade_reconciliation.rs` | Polymarket live order-specific fill 与订单终态对账 helper |
| `packages/backend/crates/connectors/src/news.rs` | RSS/Atom 新闻 connector — 抓取 feed、解析 item/entry、标准化 raw news item |
| `packages/backend/crates/connectors/src/rewards.rs` + `rewards/orderbooks.rs` + `rewards/price_history.rs` | Rewards catalog connector + CLOB `/books` batch poll, `/book` fallback and `/prices-history` |
| `packages/backend/crates/connectors/src/orderbook.rs` | Orderbook service client — HTTP batch/register/ingest + internal WS stream client |
| `packages/backend/crates/connectors/src/openai_compat.rs` | OpenAI-compatible provider helper — root base URL 自动补 `/v1`，Bearer + `api-key` 认证头兼容，provider 文本响应候选 JSON 提取 |
| `packages/backend/crates/connectors/src/reward_ai.rs` | Rewards AI advisory connector — OpenAI Responses/Chat Completions and Anthropic Messages |
| `packages/backend/crates/connectors/src/reward_info_risk.rs` | Rewards info-risk connector — OpenAI/Anthropic structured risk assessment, optional OpenAI Responses web search, single/batch parsing |
| `packages/backend/crates/infrastructure/src/settings/defaults.rs` | 后端默认配置 — 包含未设置 `POLYEDGE_NEWS__SOURCES_JSON` 时的默认新闻源列表 |
| `packages/backend/apps/worker/src/worker/rewards.rs` | Rewards bot — executes live strategy ticks and queued run/cancel/reset commands |
| `packages/backend/apps/worker/src/worker/service_info_risk.rs` | Worker runtime hook for async rewards info-risk scans |
| `packages/backend/apps/worker/src/worker/orderbook_registration.rs` | Worker orderbook token registration — 周期注册 active/eligible/candidate token，并在 rewards 新买单落库后即时刷新 `rewards_active` |
| `packages/backend/apps/worker/src/worker/rewards/provider_advisory.rs` | Rewards AI advisory cache gate, candidate ordering, provider connector/permit helpers |
| `packages/backend/apps/worker/src/worker/rewards/provider_refresh_batch.rs` | Rewards AI advisory / info-risk main refresh batch helper — 按配置批量请求、逐项保存、漏项/错配回退单市场 |
| `packages/backend/apps/worker/src/worker/rewards/provider_refresh_orderbook.rs` | Rewards provider refresh temporary orderbook source helper — 临时订阅 AI 所需盘口、allow 后提升 eligible source |
| `packages/backend/apps/worker/src/worker/rewards/low_competition_probe.rs` | Rewards low-competition probe helper — gate 前按最多 10 个市场一批注册 `rewards_low_competition_probe` 预热盘口和历史样本 |
| `packages/backend/apps/worker/src/worker/rewards/provider_refresh.rs` | Rewards AI advisory / info-risk provider refresh — 开放订单/持仓优先，普通候选与低竞争通过 gate 的 pre-AI 市场按约 2:1 混排，每个临时盘口批次内先补 AI advisory 再补信息风险 |
| `packages/backend/apps/worker/src/worker/rewards/info_risk.rs` | Rewards info-risk async scan loop, provider cache lookup/write, quote-plan risk application |
| `packages/api/src/handlers/rewards.rs` | Rewards API — reads snapshots/config and enqueues worker control commands |
| `packages/backend/crates/application/src/rewards/service.rs` | RewardBotService — reward markets, snapshots, live order lifecycle, control command queue, in-process command wake channel |
| `packages/backend/crates/application/src/rewards/service_cache.rs` | RewardBotService cached reads — events, fills, open_order_count, positions, heartbeat, event log helper |
| `packages/backend/crates/application/src/rewards/service_snapshot.rs` | RewardBotService snapshot aggregation — orders/plans pagination and low-competition report |
| `packages/backend/crates/application/src/rewards/runtime_models.rs` | Rewards runtime models — account/position/order/fill/event/report/snapshot types |
| `packages/backend/crates/application/src/rewards/quote_selection_models.rs` | Rewards quote/selection/AI advisory enums — double/auto、observe/enforce、provider/request format |
| `packages/backend/crates/application/src/rewards/ai_advisory_models.rs` | Rewards AI advisory request/decision/cache models and guarded plan enforcement |
| `packages/backend/crates/application/src/rewards/info_risk_models.rs` | Rewards info-risk request/decision/cache models and guarded plan filtering |
| `packages/backend/crates/application/src/rewards/config_impl.rs` | Rewards config defaults、normalization、candidate filter and patch application |
| `packages/backend/crates/application/src/rewards/planner.rs` | Rewards deterministic quote planner — 静态事件风险过滤、首单 info-risk/quarantine gate、quote plan 构建 |
| `packages/backend/crates/application/src/rewards/low_competition.rs` | Rewards low-competition sleeve metrics/gate — competition notional、reward/100/day、exit depth/slippage、book stability |
| `packages/backend/crates/application/src/rewards/planner_selection.rs` | Rewards deterministic quote selection — dominant single-side recommendation, book concentration metrics, preferred category bonus |
| `packages/backend/crates/application/src/rewards/planner_live.rs` | Rewards live quote materializer — live orderbook rank/spread/auto metrics before wallet-balance placement gate |
| `packages/backend/crates/application/src/rewards/low_competition_report.rs` | Rewards low-competition observations and shadow report aggregation |
| `packages/backend/crates/application/src/rewards/pagination.rs` | Rewards order pagination query and response metadata |
| `packages/backend/crates/application/src/arbitrage/service.rs` | Arbitrage service/store port — 扫描生命周期、机会/验证、事件与扫描历史 retention |
| `packages/backend/crates/application/src/maintenance.rs` | Database maintenance service/store port — 集中定义数据库历史/缓存/队列表 retention cutoffs 与清理统计 |
| `packages/backend/crates/infrastructure/src/catalog/postgres/arbitrage.rs` | Arbitrage Postgres store — 盘口快照/机会持久化和旧 scan 分批清理 |
| `packages/backend/crates/infrastructure/src/stores/maintenance.rs` | Database maintenance Postgres/no-op store — 按表分批删除 raw events、缓存、candles、队列、copytrade/outbox/audit 历史 |
| `packages/backend/crates/infrastructure/src/stores/rewards/postgres_low_competition.rs` | Rewards low-competition observation persistence and recent-window query SQL |
| `packages/backend/crates/infrastructure/src/stores/rewards/postgres_candles.rs` | Rewards price-history candle upsert and recent-candle query SQL |
| `packages/front/src/features/rewards/components/rewards-low-competition-report.tsx` | Rewards low-competition shadow report panel |
| `packages/backend/apps/worker/src/worker/rewards/live_sync.rs` | Rewards live managed-order trade/status sync |
| `packages/backend/apps/worker/src/worker/rewards/account_sync.rs` | Rewards external balance, CLOB open-order snapshot/adoption, complete position snapshot sync, and detected-inventory original-price sell intents |
| `packages/backend/apps/worker/src/worker/rewards/live_orders.rs` | Rewards live cancel/fill and post-fill exit/flatten intents |
| `packages/backend/apps/worker/src/worker/rewards/live_submission.rs` | Rewards live single-order submit and submission markers |
| `packages/backend/apps/worker/src/worker/rewards/live_pending.rs` | Rewards durable intent submit/recovery workflow |
| `packages/backend/apps/worker/src/worker/rewards/live_orderbook_risk.rs` | Rewards live orderbook risk helpers — 新挂单 stale 余量、近期 BUY stale-only 撤单 grace、等待原因 |
| `packages/backend/apps/worker/src/worker/rewards/live_requote.rs` | Rewards live reprice guard — drift 稳定确认、订单冷却、单轮漂移撤单限速 |
| `packages/backend/apps/worker/src/worker/rewards/live_risk.rs` | Rewards live placement/cancel risk checks |
| `packages/backend/apps/worker/src/worker/rewards/orderbook_events.rs` | Rewards orderbook event consumer — 内部 WS、本地盘口 cache、HTTP bootstrap、活跃 token wake |
| `packages/backend/apps/worker/src/worker/rewards/polling.rs` | Rewards live poll loop, book fetch, event-driven fast reconcile, external sync throttling, 5-day history pruning, in-process book history, command wake subscription, background managed-orderbook-cache pre-warm task (`run_reward_managed_orderbook_cache_prewarm`) |
| `packages/backend/apps/worker/src/worker/database_maintenance.rs` | Worker database maintenance task — 定期执行数据库 retention 清理并输出逐表统计 |
| `packages/backend/apps/worker/src/worker/copytrade.rs` | Copytrade worker — wallet tracking, source trade detection, and queued analyze commands |
| `packages/api/src/handlers/copytrade.rs` | Copytrade API — reads snapshots/config and enqueues worker control commands |
| `packages/backend/crates/application/src/copytrade/service.rs` | CopyTradeService — copytrade config, wallet tracking, source trade detection, and control command queue |
| `packages/backend/crates/application/src/orderbook_cache.rs` | OrderbookCache trait and stream event models — `CachedOrderBook`、`OrderbookStreamEvent` |
| `packages/backend/crates/application/src/orderbook_registry.rs` | OrderbookSubscriptionRegistry trait — 多来源 token 订阅注册与来源统计 |
| `packages/backend/crates/infrastructure/src/stores/orderbook_cache.rs` | InMemoryOrderbookCache（TTL + 定期清理 + 每侧盘口深度裁剪）；保留 Redis 实现 |
| `packages/backend/crates/infrastructure/src/stores/orderbook_registry.rs` | InMemoryOrderbookSubscriptionRegistry — 来源有序 token 原子替换、确定性优先级聚合、来源与去重总数统计 |
| `packages/backend/crates/infrastructure/src/stores/helpers/reward_config.rs` | Rewards key-value config helper — `RewardBotConfig` 与 `reward_bot_config` 读写映射 |
| `packages/backend/crates/infrastructure/src/stores/rewards/postgres_market_methods.rs` | Rewards Postgres candidate query — 市场质量硬过滤、综合排序、row mapping |
| `packages/front/src/features/rewards/components/rewards-low-competition-config.tsx` | Rewards frontend low-competition sleeve config panel |
| `packages/front/src/features/rewards/components/rewards-low-competition-summary.tsx` | Rewards frontend low-competition quote-plan metrics summary |
| `packages/backend/migrations/0022_reward_bot_control_commands.sql` | Rewards API-to-worker command queue table |
| `packages/backend/migrations/0023_copytrade_control_commands.sql` | Copytrade API-to-worker command queue table |
| `packages/backend/migrations/0024_reward_markets_active_index.sql` | Reward market active/daily-rate query index |
| `packages/backend/migrations/0025_markets_active_volume_index.sql` | Open/tradable market 24h-volume query index |
| `packages/backend/migrations/0026_reward_control_running_lease_index.sql` | Rewards running control command lease query index |
| `packages/backend/migrations/0028_reward_positions_external_inventory.sql` | Allow complete external rewards account inventory outside the reward catalog |
| `packages/backend/migrations/0030_rewards_snapshot_indexes.sql` | Indexes for reward_fills and reward_positions snapshot queries |
| `packages/backend/migrations/0031_worker_query_indexes.sql` | Indexes for worker orders, raw_events event_time, and copytrade source_trades queries |
| `packages/backend/migrations/0032_reward_worker_heartbeats.sql` | Rewards worker heartbeat used by snapshot running status |
| `packages/backend/migrations/0033_reward_candidate_filter.sql` | Rewards candidate filter config |
| `packages/backend/migrations/0034_reward_account_external_buy_notional.sql` | Rewards account external buy notional snapshot |
| `packages/backend/migrations/0035_auto_cancel_not_found_orders.sql` | Historical rewards managed-order repair |
| `packages/backend/migrations/0036_restore_not_found_reconciliation.sql` | Restore incorrectly auto-cancelled 404 orders for trade reconciliation |
| `packages/backend/migrations/0037_reward_market_quality.sql` | Gamma market liquidity/end-time/freshness fields, rewards quality index, unsafe stale-cancel repair |
| `packages/backend/migrations/0038_reward_market_advisories.sql` | Rewards AI advisory cache table keyed by provider/request_format/model/input_hash |
| `packages/backend/migrations/0039_reward_market_info_risks.sql` | Rewards info-risk cache table keyed by provider/request_format/model/input_hash |
| `packages/backend/migrations/0040_markets_quality_index_no_synced_at.sql` | Rewards market quality index excludes high-churn `markets.synced_at` |
| `packages/backend/migrations/0041_market_asset_id_lookup_indexes.sql` | Market yes/no asset id indexes for orderbook priority token-to-condition lookup |
| `packages/backend/migrations/0042_reward_order_strategy_bucket.sql` | Rewards managed order `strategy_bucket` for standard vs low-competition bucket tracking |
| `packages/backend/migrations/0043_reward_low_competition_observations.sql` | Rewards low-competition cross-cycle observation table for shadow reports |
| `packages/backend/migrations/0044_reward_market_candles.sql` | Rewards 5m price-history candle table for AI advisory |
| `packages/backend/migrations/0045_reward_control_command_dedupe.sql` | Rewards control command pending/running dedupe partial unique indexes |
| `packages/backend/init.sql` | Complete empty-database initialization script generated from migrations 0001–0045 |

## 仓库结构

- `doc/`：系统设计、API 契约、鉴权、存储、前后端计划等文档。
- `packages/front/`：`Next.js 16 + React 19 + Tailwind v4 + shadcn/ui` 控制台前端。前端代码规范（目录结构、数据层、文件行数上限、公共代码提取）见 [packages/front/AGENTS.md](./packages/front/AGENTS.md)，写或改前端代码前必须遵守。
- `packages/Cargo.toml`：Rust workspace 根。
- `packages/api/`：`polyedge-api` 服务 crate（HTTP API + 内嵌 worker runtime）。
- `packages/orderbook/`：`polyedge-orderbook` 服务 crate（市场同步、盘口 WS/poll、盘口 HTTP API）。
- `packages/backend/`：后端共享 crates、`worker / replay` apps、迁移和初始化 SQL；包含 `application / common / connectors / contracts / domain / infrastructure` crates。后端代码规范（分层架构、`include!` 模块化、文件行数上限、公共代码提取、测试组织）见 [packages/backend/AGENTS.md](./packages/backend/AGENTS.md)，写或改后端 Rust 代码前必须遵守。
- `deploy/`：Docker Compose 部署模板和环境变量示例；当前 Compose 服务为 `polyedge-api`（内嵌 worker runtime）、`polyedge-orderbook` 和 `polyedge-front`。
- `scripts/`：构建、部署、冒烟脚本。
- `bin/`：部署镜像复制的预构建后端二进制。

## 当前状态

- 仓库已经不是纯文档仓库：前端控制台、Rust API、worker、迁移、配置和 Docker 部署入口都已具备。
- 前端控制台已有 `dashboard / markets / events / radar / rewards / copy-trading / wallet-analysis / signals / positions / risk / settings` 页面；`/replay` 和未落地的 approvals 页面不再作为前端入口暴露。
- 前端控制台导航在桌面端使用左侧折叠 sidebar，在移动端通过顶栏菜单按钮打开左侧抽屉；两者共享同一份导航项配置，并使用原生链接跳转以适配静态导出部署。
- 前端数据层统一走 `src/lib/api/*`（读取按领域文件 `markets.ts` / `signals.ts` / `risk.ts`… 基于 `base.ts`，写操作通过 `actions.ts` barrel 暴露、实现按领域拆在 `actions/`），页面装配在 `src/features/*/loaders` 和 `src/features/*/components`。`src/server/` 目前是空目录（历史遗留）。
- 前端仅支持中文，文案走 `@/lib/i18n/dictionaries` 字典导入。
- 前端不再提供 mock 数据模式；`NEXT_PUBLIC_POLYEDGE_API_BASE_URL` 必须指向 Rust 后端，读写都走真实 `/api/v1/...`。
- 当前控制台会话只保留 `off`，不是生产级真实会话。
- 默认生产排查环境：Frontend Rewards 工作台 `http://192.168.31.5:33002/rewards`，API 服务 `http://100.87.45.72:38001`，Orderbook 服务 `http://100.87.45.72:38002`；除非用户明确指定其他环境，后续线上问题排查默认使用这组地址。前端静态构建的 `NEXT_PUBLIC_POLYEDGE_API_BASE_URL` 应指向该 API 地址。
- 后端 API 已覆盖 markets、events、news、evidences、signals、orders、trades、positions、pricing、arbitrage、rewards bot、risk、system、connector callback 和 orderbook（`GET /api/v1/orderbook/{token_id}`）等主路径；risk 控制台快照只按当前 positions 涉及的 market id 批量读取市场分类信息，不再通过 markets 列表接口全量扫描市场表。
- 后端默认 tracing filter 在未设置 `RUST_LOG` 时包含 `polyedge_worker=info`，因此 `polyedge-api` 内嵌 worker runtime 的 info/warn 日志会出现在 API 服务日志中；显式设置 `RUST_LOG` 会覆盖默认 filter。
- 新闻采集当前支持 RSS/Atom XML feed；未配置 `POLYEDGE_NEWS__SOURCES_JSON` 时，代码默认新闻源为 `fed_press`、`sec_press`、`nasa_news`、`bbc_world`、`npr_news`、`coindesk`、`cointelegraph`、`decrypt`；部署模板 `deploy/.env.api.example` 也显式写入同一默认源列表，环境变量或 runtime config 可覆盖整个 sources 列表。
- `polyedge-worker` 支持 database maintenance、news ingest、news promotion、arbitrage radar、rewards bot live 策略、copytrade 钱包跟踪/分析、execution drain、paper reconciliation、Polymarket order/fill/user-event、orderbook token 注册任务。市场同步和 orderbook 订阅已迁移到独立 `polyedge-orderbook` 服务；orderbook 服务启动时先暴露 HTTP `/healthz`，再后台执行独立的 Gamma full sync、Gamma priority sync、rewards catalog sync 与 rewards candle history sync 循环，避免外部 Polymarket API 延迟阻塞容器健康检查，也避免较慢的 rewards 详情补全阻塞 Gamma `markets.synced_at` 刷新；Gamma full sync 使用 `/markets` offset 分页并按 market id 去重，写入时跳过同版本同内容行，并只在 `synced_at` 超过 rewards 新鲜度窗口约三分之二时刷新安静市场；Gamma full/priority 写入 `markets` 时在 orderbook 进程内串行化，并通过 Postgres `lock_timeout`/`statement_timeout` 避免长时间锁等待堆积；Gamma priority sync 会强制刷新已注册 token 映射到的 condition、活跃 rewards 订单/持仓、最终 eligible 或 pre-AI deterministic eligible quote plans 和放宽新鲜度后的 rewards 候选 condition，并用 active rewards catalog 的高奖励市场补足剩余 priority 额度作为恢复种子，最多 500 个 condition，刷新间隔由 rewards `max_market_data_age_minutes` 动态推导（约为窗口三分之一，30-300 秒）；Gamma 单次 full sync 有 60-240 秒超时，priority sync 最长 120 秒超时，rewards 单次同步有 45 分钟超时，rewards 空目录或详情补全后仍不完整时保留上一版目录；reward catalog upsert 先写入当前快照、再只停用缺失 active 市场，避免每轮全量 active=false/true 写放大；candle history sync 默认每 300 秒最多处理 600 个 active reward token，按 token 至少间隔 500ms 请求 CLOB `/prices-history`，首次 backfill 2 小时、后续增量 15 分钟，遇到 429/认证错误/超时/常见 5xx/解码失败会停止本轮以避免继续压外部 API；orderbook WS + poll stream 遵守 `POLYEDGE_ORDERBOOK_STREAM__RESTART_INTERVAL_SECS`，按 `POLYEDGE_ORDERBOOK_STREAM__WS_CHUNK_SIZE` 分片消费 `book` + `price_change`（默认 100 token/连接），chunk 内 SDK stream reader 与缓存写入解耦以减少 broadcast lag，chunk 退出或被取消时会成对释放 SDK market subscription，registry 变更会实时唤醒 token refresh，但仅在订阅 token 成员真实增删并经过短暂 debounce 后仍变化时，默认增量模式（`WS_INCREMENTAL_RECONCILE=true`）对 diff 做 subscribe/unsubscribe 并保持 WS 连接存活、不整体重建，registry 聚合顺序抖动不会触发重订/重连；默认每 10 秒对全部注册 token 做批量快照恢复，poll 写入保留 CLOB timestamp 为 `observed_at`、用本地接收时间刷新 `confirmed_at` freshness，内部写接口要求 `POLYEDGE_ORDERBOOK__WRITE_TOKEN`，缓存统一排序后裁剪最优档位并拒绝旧盘口内容覆盖；Gamma、CLOB rewards、order book 和 price-history 解码失败会在错误中携带最多 300 字节的转义响应体 preview，便于区分 HTML/截断/结构漂移响应；`/orderbook/stream` 会把 WS、poll 和 ingest 的规范化缓存更新推送给内部消费者，`/orderbook/stream?source=...` 可按 registry source 过滤返回事件（底层 Polymarket WS 仍按聚合 token 统一订阅）。
- 套利雷达是只读链路：发现、记录、校验、分析和展示已具备，但不会创建 execution request 或订单；每轮扫描完成后按 `arbitrage.event_retention_hours` 清理旧 `arbitrage_events` 和旧 `arbitrage_scans`，`market_book_snapshots`、`arbitrage_opportunities` 与 validations 通过外键级联清理，避免盘口快照无限膨胀。
- Rewards bot 仅支持 `live` 实盘模式（`execution_mode` 字段已移除，旧配置键读取时忽略）。它只使用 `reward_markets` 表作为奖励市场来源，并关联 `markets` 表硬过滤非 open/tradable、高歧义、低流动性、低 24h 成交量、临近结算、Gamma 价差过宽、同步数据过期或异常超前以及 FDV/launch/token/official-result 等高事件跳变风险市场；候选按奖励、流动性、成交量、剩余时长和 rewards spread（CLOB 原始单位即 cents）的综合质量分优先，命中 `preferred_categories` 的 Gamma 分类只增加排序分，不绕过硬过滤。只有唯一且明确的 YES/NO token 才进入盘口订阅和规划。长期 rewards poll loop 通过 `OrderbookStreamClient` 消费 orderbook 服务内部 `/orderbook/stream`，维护 worker 本地盘口 cache；启动、重连、缺失 token、本地盘口超过 `stale_book_ms` 或接近新挂单 freshness headroom 时用 `OrderbookHttpClient` batch API bootstrap/refresh，full tick 读取候选和活跃订单/持仓盘口，并在 AI/info-risk cache gate、订单/账户同步和 live action 盘口刷新后先 materialize quote readiness，再统一保存最终 quote plan 快照；`rewards_eligible` source 由周期注册任务统一注册全部最终 eligible quote plan token，AI/info-risk gate 前已 deterministic eligible 且保存在 `orderbook_token_ids` 的 token 改由 `rewards_ai_provider` 临时 source 按最多 10 个市场一批订阅，批次切换会取消上一批；低竞争 gate 前候选由 `rewards_low_competition_probe` source 按最多 10 个市场一批预热盘口和历史样本；provider 返回非 `avoid` advisory 后即时合并到 `rewards_eligible`，因此 `reward_candidate_token_cap=0` 只会关闭候选预热，不会阻止最终 eligible 市场、AI provider 临时批次或低竞争 probe 批次按需订阅盘口；周期注册任务对空集合做防抖，active/exec 连续 2 轮、eligible/candidates 连续 3 轮成功为空才清远端 source；新买单 intent 持久化后会立即刷新 `rewards_active` source，避免刚落库实盘单等待下一次周期注册，若即时刷新读到空集合则保留上一版 source 等周期任务确认；fast reconcile 可由活跃 token 盘口更新直接唤醒，`reconcile_interval_sec` 和 `POLYEDGE_REWARDS__POLL_INTERVAL_SECS` 仍作为兜底。`RewardOrderbookRuntime` 额外 spawn 一个独立后台 task（`run_reward_managed_orderbook_cache_prewarm`，默认每 5 秒），用 `refresh_reward_managed_orderbook_cache` 对活跃订单/持仓 + eligible quote plan + 候选 token 做本地盘口 cache 预热（复用 `fetch_cached_reward_books`，仅 HTTP 拉取本地 age 接近 placement 新鲜度阈值的 token），让盘口少有变化的安静市场在两次 full tick 之间也保持新鲜；该 task 独立于 poll loop、不阻塞 fast reconcile、不持有 advisory lease，poll loop 结束时随 runtime drop 一起 abort。worker 默认生成 YES/NO post-only 双边买单计划；`rewards_min_size` 是份额数量要求，报价腿会先对齐到 CLOB 成本精度，并满足 Polymarket 1 美元最小名义金额，避免提交缩量后失去奖励资格或被 venue 最小名义金额拒单。新报价价格由 `quote_bid_rank=1|2|3` 明确选择买一/买二/买三（按不同买价计档，默认买一），但 quote plan 构建阶段不再因为目标档位缺失、目标价超出 rewards spread、auto 单边盘口指标、实际盘口价格预算、`per_market_usd` 或 `quote_size_usd` 而淘汰市场；准备挂单时才用当前 orderbook materialize 真实报价腿并验证目标档位、rewards spread、touch ask、安全边际、盘口集中度/退出深度和实际 size/notional，随后由 live placement 用实际钱包余额做同 condition 准入。live placement 缺少、空、过期或接近 stale 边界的盘口时不下单、不写 12 小时 skip，而是先对本地接近新挂单 freshness headroom 的 token 做 orderbook HTTP batch refresh，再保持 quote plan eligible 并等待 orderbook 订阅/缓存返回；配置为 `quote_mode=auto` + `selection_mode=enforce` 且启用 dominant single-side 时，双边目标档位、rewards spread、touch ask 或安全边际不通过会先尝试通过同一校验的单腿；没有可行单腿或其他 live 盘口验证不通过时才不下单，并把 quote plan 标记 `live_skip_until` / `live_skip_reason`，标记默认 12 小时后失效以便奖励范围或盘口变化后重新评估；开放订单目标价相对最新目标档位漂移超过 `requote_drift_cents` 时只进入受 `requote_drift_confirm_sec` 历史同向确认、`requote_drift_cooldown_sec` 订单冷却和 `requote_drift_max_cancels_per_cycle` 单轮限速保护的换价撤单候选，避免盘口档位抖动导致全量撤空后再重挂；旧 `quote_edge_cents` 配置键读取时忽略。`quote_mode=double` + `selection_mode=observe` 是默认行为；配置为 `quote_mode=auto` + `selection_mode=enforce` 且启用 dominant single-side 后，planner 只根据一边倒概率区间生成初步 `double` / `single_yes` / `single_no` / `none` 计划，退出深度、top1/top3 深度占比、HHI 以及双边不可行时的单腿回退都在 live materializer 中使用当前盘口验证。`observe` 只在 quote plan 记录推荐模式和 `book_metrics`。AI advisory 可选启用：live tick 只读取 `reward_market_advisories` 缓存并 fail closed，不等待外部 provider；worker 用单实例后台 market provider refresh 保留开放订单/持仓最高优先级，将剩余普通 eligible/candidate 与低竞争通过 gate 的 pre-AI 市场按约 2:1 混排，并用 `rewards_ai_provider` 临时 orderbook source 按最多 10 个市场一批获取 AI 所需盘口，下一批取消上一批；每个临时盘口批次内先补 AI advisory 再补 info-risk，`ai_advisory_batch_size` / `info_risk_batch_size` 分别控制主 refresh 单次 provider 请求包含的市场数（默认 1，最大 12），批量响应按 condition 拆分保存，漏项、错配或整体解析失败会回退单市场，provider 过载则停止本轮剩余请求；缓存未命中时分别通过 `RewardAiAdvisoryConnector` / `RewardInfoRiskConnector` 调用 OpenAI Responses、OpenAI Chat Completions 或 Anthropic Messages 并写入缓存，供后续 tick 使用；AI 开启后新增挂单必须先通过 provider 过滤；缺少未过期 advisory、provider 配置缺失、模型为空或请求失败仍会把原本 eligible 的计划改为不可挂并覆盖保存 quote plan 快照；AI `avoid` 硬拦，AI `watch`、低置信度 `allow/watch` 或非 avoid 的 `quote_mode=none` 会回退 deterministic 计划并继续进入 live 盘口、资金和订单风控；provider confidence 会在 connector 解析时钳制到 `0..=1`。高置信度 `allow` 决策可在 `selection_mode=enforce` 且 `quote_mode=auto` 时把已 eligible 的 auto 双边计划收窄为单腿，但不会绕过市场质量、盘口和风控硬过滤。信息风险可选启用：AI advisory 启用时由同一个 market provider refresh 按 condition 同步推进，独立 info-risk worker 不再连续请求全量 provider；AI advisory 未启用时，独立 worker 任务仍按开放订单、持仓、eligible quote plan、候选市场顺序，用 active reward market / quote plan / account payload 构建 query/input hash，先读写 `reward_market_info_risks` 缓存，缓存未命中时通过 `RewardInfoRiskConnector` 调用 OpenAI/Anthropic；OpenAI Responses 可选启用 web search tool，provider confidence 同样会钳制到 `0..=1`。live tick 只读取缓存，不等待外部搜索；`info_risk_mode=enforce` 时缺少未过期风险缓存会 fail closed，已有 `critical`、官方结果、`resolution_imminent=true` 或配置为 `low/medium` 避免等级时命中的风险会在置信度达到环境变量阈值后把计划置为不可挂；普通 `high` 风险以及仅 `risk_type=imminent_resolution` 但 `resolution_imminent=false` 的结果只保留为信息提示，继续走 live 盘口、资金和订单风控。worker 使用 `LivePolymarketConnector` 提交 post-only GTC token 买单和 post-only maker SELL 退出，并撤销本系统托管订单；rewards poll loop 全程持有 Postgres advisory lease，只有 leader 维护 5 秒 CLOB heartbeat id 链并执行命令/tick/reconcile，单次 heartbeat 请求 4 秒超时。新建 quote intent 与已落库待提交 BUY 在提交前都会复用 live 撤单风控（计划仍 eligible、报价漂移、min depth、bid rank、depth drop、fill velocity、mass cancel、kill switch 等），风险不通过的本地 intent 会在提交前取消；已有 external order id 的近期 BUY 只在单纯 stale 盘口风险下短暂延迟撤单，缺盘口/空盘口和资格、深度、kill switch 等硬风险仍立即撤单；价格漂移只在 reprice guard 确认后按单轮上限撤单。confirmed fill 按 external trade id + external order id 幂等入账，买入 fill 与退出 intent 同事务落库；退出 floor 用 intent price 与当前持仓 `avg_price` 的较高值，提交前不使用 midpoint 或页面“当前价”降价；SELL 退出按 floor 提交 post-only maker 单，提交前会按当前 token 持仓裁剪 size，无持仓 stale exit 会关闭，当前买一穿过 floor 时按 30 秒退避等待可 resting 的盘口；明确 post-only 退出拒单使用有界退避并在达到最大拒绝次数后停止盲目重试；提交前低于 Polymarket 1 美元最小名义金额的退出单会进入短 reason 的 dust deferred 状态，每 300 秒重新评估但不重复拼接历史原因。单订单查询返回 404 时，worker 会按 token 和下单时间窗口查询认证账户 trades，并按 external order id 精确补账，不会把 404 直接标记为 cancelled；仍无法确认时保持 critical 对账锁，暂停新增买单但继续同步、撤单和卖出退出，后续成功查询会自动解除锁；若该 404 锁超过 5 分钟且仍没有 CLOB/Data API 成交证据，worker 会把本地订单标记为 cancelled 以释放开放挂单计数。提交结果未知订单现在会在恢复查询确认 CLOB 无对应 open 订单并经过 `LIVE_SUBMISSION_UNKNOWN_CLOSE_AFTER_SECS`（默认 600 秒）宽限后自动本地关闭以释放对账锁（与 404 锁一致，不再需要手动改库）；取消结果未知订单仍不会仅因本地超时释放对账锁。每轮还会读取 CLOB open orders snapshot：普通已提交 open-like BUY 若不在外部开放订单列表且无提交未知、404、pending cancel、post-only violation 等对账锁，会本地标记为 cancelled 释放开放挂单计数；该反查和账户开放 buy notional 观测不受 confirmed fill 保护期影响。成交后 sibling cancel 只撤同 condition 对侧 buy，不撤 sell exit；同 token 存在未完成卖出退出时暂停新增买单。full tick 和 fast reconcile 会先同步 managed orders；本轮有新增 confirmed fill，或数据库最新 confirmed fill 距今不足 120 秒时，只保留本地 balance/positions，等待 CLOB/Data API 最终一致性追平后再同步完整外部账户快照。外部账户同步的资金钱包地址优先使用 `FUNDER`，未配置时使用 `ACCOUNT_ID`；CLOB balance 为 0 或失败但链上 pUSD 余额大于 0 时，worker 用链上 pUSD 回填账户 snapshot，并清零遗留 `reserved_usd`。成功 positions 快照原子替换该账户全部持仓，失败时保留上一版。即使 `enabled=false` 且没有开放订单，worker 仍会尝试刷新外部账户状态。worker 按账户写入数据库 heartbeat，API snapshot 仅在配置启用且 heartbeat 不超过 2 分钟时返回 `running=true`；`status.error` 只由当前开放订单的活跃对账锁推导，不会把短暂 `awaiting final reconciliation` 当作错误，也不会被历史 critical event 永久污染。API 不直接请求 Polymarket，`orders` 与 `orders_page` 都描述本地 managed orders。`RewardBotService` 内部缓存 config、account、positions、最新 200 条 events、最新 200 条 fills、open_order_count 和 heartbeat，API 与内嵌 worker runtime 共享实例时直接从内存读取这些热状态，缓存为空时回退数据库；控制命令入队通过 in-process command_wake channel 立即唤醒 worker poll loop。账户范围外开放订单明细和奖励结算对账仍是缺口。
- Rewards quote plan snapshot 会持久化 `pre_ai_eligible` 和 `orderbook_token_ids`；AI/info-risk gate 前的 deterministic eligible token 不再长期纳入 `rewards_eligible` source，而是由后台 provider refresh 使用 `rewards_ai_provider` 临时 source 按最多 10 个市场一批订阅盘口，批次切换会取消上一批；provider refresh 保留开放订单/持仓最高优先级，剩余普通候选与低竞争通过 gate 的 pre-AI 市场按约 2:1 混排；provider 返回非 `avoid` advisory 后再即时合并到 `rewards_eligible` source，最终下单仍由 live 盘口、资金和订单风控决定。
- Rewards quote plan 额外持久化 `quote_readiness=ready_to_quote|waiting_orderbook|provider_pending|blocked`，把“策略 eligible、应继续订阅”的候选语义与“已有真实报价腿、可立即进入 live 下单检查”的可报价语义拆开；API snapshot 的 `status.ready_quote_markets` 只统计 `ready_to_quote`，同时返回 `waiting_orderbook_markets`、`provider_pending_markets` 和 `blocker_counts`，供前端解释可挂市场与实际可报价数量差异以及主要拦截原因。缺盘口/过期盘口的计划仍可保持 `eligible=true` 以进入 `rewards_eligible` 订阅；前端顶部“可挂市场”和可挂比例使用 `status.eligible_markets`，提示行再展示 ready/waiting/provider 拆分，“主要拦截”卡片展示 AI pending/低置信度/watch/avoid、信息风险、低竞争、资金不足、live 盘口验证等阻断结构，避免盘口 freshness 或 provider gate 抖动把通过最终过滤的市场数量显示成 0 且难以解释。
- Rewards 首次买入报价现在有可配置入场缓冲：当 `info_risk_enabled=true` 且 `info_risk_mode=enforce` 时，`require_info_risk_before_first_quote=true` 会要求新 condition 先命中未过期信息风险缓存，`first_quote_quarantine_sec` 会要求新 condition 至少观察指定秒数（默认 300）后才允许首次 live BUY；已有 open-like 订单或持仓的 condition 跳过该首单 gate，以便继续撤单、退出和管理库存。planner 的静态事件风险关键词也扩展到 appointed/confirmed/certified、drop out/withdraw/resign/step down/removed 以及 scheduled/deadline/market-close 类市场。
- Rewards full tick 在 AI/info-risk gate 完成后会用本轮内存 quote plan 立即替换 `rewards_eligible` orderbook source，周期注册任务仍作为兜底；这样新 eligible token 不必等下一次注册周期，后续订单/账户同步期间 orderbook 服务即可开始 WS/poll 预热，最终快照仍在 live action 盘口刷新并 materialize `quote_readiness` 后保存。
- Rewards 成交后退出策略由 `post_fill_strategy` 配置决定，不再由 AI advisory 的 `exit_policy` 覆盖；`ExitAtMarkup` 以被吃买单原价加 `exit_markup_cents` 为卖价基准并向上取整到 0.01 tick，默认加价为 0；页面“持有并续挂”对应 `hold_and_requote`，也会按被吃买单原价持久化 SELL 退出 floor，之后继续正常报价。SELL 退出按 floor 提交 post-only maker SELL，不再用 FAK taker；提交前会按当前 token 持仓裁剪 size，无持仓 stale exit 会关闭，当前买一穿过 floor 时按 30 秒退避等待可 resting 的盘口，不使用 midpoint 或页面“当前价”降价。
- Rewards 外部 positions 快照检测到已有库存且没有同 token open-like SELL 时，会按持仓 `avg_price` 向上对齐 tick 后补原价 SELL intent；后续按 post-only maker 规则挂出，避免已有库存无人接管退出且避免用低于持仓均价的买一价或 midpoint 卖出。
- Rewards `quote_bid_rank` 对细 tick 盘口不是纯第 N 个 0.001 价位：上条所称买二/买三在细 tick 下会从买一回退 `rank-1` 个 0.01 价格步长，再选择不高于目标价的当前买盘档位；粗 tick 盘口仍按不同买价的买一/买二/买三选择。
- Rewards CLOB open-order snapshot 会先收养未归属但 token 可唯一映射到 active reward market 的开放 BUY 为 managed order；如果同 external id 的本地 BUY 已被关成非 open，但 CLOB 仍 open，会重开原本 managed order。SELL、非 rewards 市场和无法唯一映射 token 的外部开放订单明细，以及奖励结算对账仍是缺口。
- Rewards 低竞争市场 sleeve v2 已实现并默认关闭（见 [doc/rewards-low-competition-sleeve-plan.md](./doc/rewards-low-competition-sleeve-plan.md)）：使用独立 `standard` / `low_competition` candidate profile，不全局降低主策略流动性/成交量硬门槛；`observe` 只写入 `strategy_bucket`、低竞争指标和 observation，不实盘下单；`enforce` 需要竞争资金、预估 reward/100/day、退出深度、盘口历史样本和 midpoint 稳定性达标，并要求 AI advisory 开启且 info-risk 为 enforce，之后仍由既有 AI/info-risk cache gate、live materializer、kill switch、订单/库存/账户外部 BUY notional 风控 fail closed；低竞争 gate 前会先用 `rewards_low_competition_probe` source 按最多 10 个市场一批预热盘口和历史样本；通过低竞争 gate 的 pre-AI 市场会与普通 eligible/candidate 市场按约 2:1 混排进入 provider refresh 的每批 10 个市场临时盘口订阅和 AI 分析。缺 fresh midpoint、盘口指标或历史样本时会保留 quote legs 和 `orderbook_token_ids` 并标记 data unavailable。低竞争 managed order 会持久化 `strategy_bucket=low_competition`；worker 会用 gate 前固化的计划 notional 持久化 `reward_low_competition_observations`，API snapshot 返回最近 24 小时 shadow report 和保守小额 enforce 建议，但不会自动切换配置；live placement 会把低竞争 open-like 订单限制在全局 `max_open_orders` 的约 30% 内（全局允许时至少 1 单）。
- Rewards 成交对账除 404 fallback 外，也会在关联 trade 按 ID 查询失败时按 token/time 扫描认证账户 trades 并按 external order id 精确匹配；认证 CLOB 明确返回 matched size、但 trade 响应仍无法解码时，worker 仅在 Data API 钱包交易的 token/BUY/price/time/累计 size 与唯一 managed order 全部严格匹配后补账。若外部账户和持仓快照已覆盖该成交，只补订单、fill 和退出 intent，不重复扣现金或叠加持仓。任何单笔订单的全部回退失败都只隔离当前订单，不再阻断其余订单对账、账户持仓同步或 stale 清理。
- Rewards fast reconcile 的重型外部同步受独立节流保护；如上一条状态快照描述 fast reconcile 会同步订单/账户，实际执行时托管订单状态、CLOB open-order snapshot、managed scoring、账户级 rewards earnings 和 balance/positions snapshot 分别按最小间隔执行，不会因活跃盘口事件每秒全量打外部 API。
- Rewards AI advisory / info-risk provider refresh 受 `POLYEDGE_REWARDS__INFO_RISK_MAX_MARKETS_PER_CYCLE` 每轮 condition cap 控制（默认 50，0 表示本轮不发 provider 请求）；full tick 会先记录 AI 过滤前的 deterministic eligible condition 集合，新一轮 quote plan 构建时只继承上一版未过期且 provider/request_format/model 匹配的 advisory，不会因缺少 advisory 在 prepare 阶段提前 fail closed；live tick 只读取已有 advisory cache 并立即 gate，仍缺少 advisory、provider 配置缺失、模型为空或请求失败的原本 eligible 计划会 fail closed；AI `avoid` 也会硬拦，但 AI `watch`、低置信度 `allow/watch` 或非 avoid 的 `quote_mode=none` 会回退 deterministic 计划继续进入 live 盘口、资金和订单风控；quote plan 快照只在 AI/info-risk gate 全部完成后统一保存。后台 market provider refresh 用进程内 `AtomicBool` 保证同一进程最多一个任务在跑；候选 condition 保留开放订单、持仓最高优先级，剩余普通 eligible/candidate 市场与低竞争通过 gate 的 pre-AI quote plan 按约 2:1 混排，并用 `rewards_ai_provider` 临时 orderbook source 按最多 10 个市场一批获取 AI 所需盘口，下一批取消上一批，结束后清空；每个临时盘口批次内先按稳定 cache-key payload 构建 AI `input_hash` 并查询缓存，缓存未命中且该市场所有报价 token 盘口都已发布（非空 bids 与 asks）时才请求 provider；请求 payload 包含账户、订单、持仓、当前 orderbook top levels、最近 24 根 5m price-history candles 和 candle summary，但 cache key 只纳入市场身份/问题、奖励参数、计划 quote mode、相关策略配置和 candle summary，不纳入账户/订单/持仓或即时盘口档位。`ai_advisory_batch_size` 控制 AI advisory 批量请求市场数，批量响应按 condition 拆分保存，漏项/错配回退单市场；盘口缺失/为空的市场本轮跳过 AI advisory 请求且不写缓存，等 orderbook 订阅/缓存返回盘口后再评估，避免在缓存键不含即时盘口的设计下被一条空 watch/avoid 长期卡住整个 TTL（与 live placement 缺盘口即等待订阅数据的模式一致）；命中或保存的 advisory 会挂到本轮内存 plan，随后再按 `info_risk_batch_size` 批量或逐市场构建 info-risk `input_hash`、查询缓存并在缓存未命中时请求 provider。advisory cache key `schema_version` 已升到 5，使从 orderbook-derived candles 切换到 price-history candles 前的 advisory 失效，并按新 payload 重新评估。provider 成功后只写入 `reward_market_advisories` / `reward_market_info_risks` 缓存，供后续 tick 使用，不再用旧 cycle 增量覆盖完整 quote plan 快照。live cache gate 会写入包含 pre_ai_eligible_plans/ai_existing_advisories/ai_request_candidates/ai_pending_plans/cache_hits/skipped_missing_market/applied 的 info 日志；后台 provider refresh 会分别写入 AI 与 info-risk 的 candidates/cache_hits/requested/saved/failures/skipped_missing_market 汇总（AI 侧额外含 skipped_missing_book）和逐个 requesting/saved 进度。Rewards config 的 AI provider wire value 使用 `openai|anthropic`，request format 使用 `openai_responses|openai_chat_completions|anthropic_messages`；后端兼容读取旧 `open_ai*` 拼写但序列化始终输出 `openai*`。OpenAI-compatible provider 的 base URL 可配置为根地址或 `/v1` 地址，connector 会统一请求 `/v1/...` 并同时携带 Bearer 与 `api-key` 认证头；MiMo provider 使用 `openai_chat_completions`，不使用未实现的 Responses endpoint；Chat Completions 请求使用 MiMo 官方兼容的 `max_completion_tokens`，AI advisory/info-risk 分别给 4096/6144 completion token 预算，降低 reasoning 模型耗尽预算导致最终 `content` 为空的概率；AI advisory/info-risk 请求温度固定为 0，prompt 要求单个合法 JSON 对象，解析层会从 provider 文本中扫描 markdown fence、解释文字、JSON 字符串或数组包装里的候选对象，并且只有通过现有必填字段与枚举校验的对象才会保存，无法提取时 warning 会携带短 preview。AI provider 单次请求默认超时为 180 秒，可通过 `POLYEDGE_REWARDS__AI_REQUEST_TIMEOUT_SECS` 覆盖；AI advisory 和 info-risk 共用进程内 `Semaphore(1)`，同一 worker/API 进程内任意时刻只允许一个 AI provider HTTP 请求在飞。API 内嵌 worker 启动会记录 rewards poll loop 是否启用、AI key 是否配置、模型名和 interval；每轮 full tick 会记录 markets/books/plans/pre_ai_eligible_plans/eligible/open_orders/positions 以及 AI/info-risk 配置。AI advisory 启用时由 market provider refresh 与 AI 按 condition 同步推进，独立 info-risk poll task 会跳过 provider 请求；AI advisory 未启用时，独立 info-risk task 仍按开放订单、持仓、eligible quote plan、候选市场顺序覆盖候选但同样受每轮 cap 限制。provider HTTP 传输失败，或明确返回限流、认证失败、服务端不可用（HTTP 429/401/403/5xx / `system_cpu_overloaded` / overloaded）时，worker 会停止本轮剩余 provider 请求以避免继续压垮 provider，并保留既有缓存/过滤语义。
- Rewards AI advisory 新增 orderbook 事件驱动批量通道（默认关闭 `POLYEDGE_REWARDS__AI_ADVISORY_EVENT_DRIVEN_ENABLED=false`，与 full-tick provider refresh 并存而非替代）：rewards orderbook 本地 cache 在某 condition 全部报价 token 首次都有真实 bids/asks 时入队 condition_id（就绪检测直接判 `CachedOrderBook` 非空，不构建 HashMap、热路径零额外分配，并用 `token_to_condition` 反向索引 + `notified_ready` 去重）；常驻 batch worker（随 orderbook runtime 一起 spawn/drop）攒满 `POLYEDGE_REWARDS__AI_ADVISORY_BATCH_SIZE`（默认 8，clamp `[1,12]`）个或等待 `POLYEDGE_REWARDS__AI_ADVISORY_BATCH_TIMEOUT_SECS`（默认 8）后，用 `current_live_cycle_state` 轻量 cycle + 候选/活跃 market 并集构建 markets_by_condition，对每个 condition 做 pre_ai_eligible 过滤、advisory cache miss 去重和盘口就绪复检，再单次 `RewardAiAdvisoryConnector::advise_batch` 评估一批（OpenAI Responses/Chat/Anthropic 各有批量变体，prompt 要求返回 `{"advisories":[{condition_id,...}]}` 数组并按 condition_id 匹配，漏项/拼错/多余被丢弃，batch size=1 时兼容单 object 返回），解析整体失败或模型漏掉部分 condition 时逐个回退到单市场 `advise`（路径 B，仍共享 `Semaphore(1)` 和 cache miss 去重）；批量保存 advisory 后对每个通过过滤的 condition 串行推进 info-risk（复用 `refresh_reward_info_risk_for_condition`），完成后清除这些 condition 的就绪标记以便 advisory TTL 过期后盘口再次变化时重新触发。两条通道不共享进程级 `REWARD_MARKET_PROVIDER_REFRESH_RUNNING`（tick refresh 专用互斥），只靠 advisory cache miss 去重 + `Semaphore(1)` 序列化 + 幂等 save 保证重叠时最多浪费一次重复调用；tick refresh 仍作全量兜底（覆盖 event 漏触发或 TTL 续期），event-driven 只是把盘口就绪到评估的延迟从最多一个轮询周期降到秒级。avoid 或 provider pending 的自动退订仍由 plan `eligible=false` 持久化 + 周期 orderbook token 注册任务在下一轮自然完成，无需新增退订端点。
- Rewards info-risk provider payload 会携带 `evaluation_time_utc` 和 imminent 判定策略；Chat Completions、Anthropic Messages 和 Responses prompt 都要求按该 UTC 时间判断 `resolution_imminent`，避免 provider 使用训练日期或过期上下文把远期/历史事件误判为临近结算。info-risk cache key `schema_version=3` 会让未携带该评估时间策略的旧缓存失效并重新评估，但 `evaluation_time_utc` 不进入稳定 cache key，避免每轮刷新造成缓存雪崩。
- Data API 最终成交回退也覆盖单订单已返回 404 的场景，包括认证账户 trade 扫描报错和扫描成功但没有精确 external order id 成交两种结果；此时必须额外满足：钱包交易累计量恰好等于本地订单剩余量，且完整外部持仓快照已覆盖该数量；否则先保持人工对账锁，若 404 锁超过 5 分钟仍无成交证据则本地标记为 cancelled。Rewards snapshot 的 `status.open_orders` 只统计已有 `external_order_id` 的 open-like managed orders，本地尚未提交的 planned/exit intent 不再显示为 Polymarket 开放挂单。
- Rewards worker 通过认证 CLOB raw HTTP `GET /rewards/user/total?sponsored=true` 同步 UTC 当日账户级 maker rewards 聚合值到 `account.reward_earned_usd`，以对齐 Polymarket `/rewards` 页面顶部 Daily Rewards 的 native+sponsored 口径；当聚合端点为空、为 0 或不可用时，会回退分页读取 `GET /rewards/user` native 明细并合并 `sponsored=true` sponsored-only 明细，按 `earnings * asset_rate` 求和；SDK 解码失败时会使用同一 L2 签名的 raw HTTP fallback，宽容解析带 trailing input 的 JSON 响应。前端只读取数据库/API snapshot，不直连 Polymarket。
- Rewards live 会在提交旧 intent 前先执行当前盘口/资格撤单检查；任一提交结果未知、待最终对账或外部订单 404 会暂停全部新增买单，但继续同步、撤单和卖出退出；外部订单 404 锁超过 5 分钟且仍无成交证据时会自动本地关闭。提交结果未知时，开放订单严格匹配失败会先保持对账锁；当恢复查询确认 CLOB 无对应 open 订单并经过 `LIVE_SUBMISSION_UNKNOWN_CLOSE_AFTER_SECS`（默认 600 秒）宽限后，worker 会把该订单本地标记为 cancelled 以释放锁（与 404 锁一致），不再需要手动改库。CLOB `post_order` 只要返回订单 ID 就保留为 accepted 供后续成交/状态对账，包含 `unmatched` / `canceled` / 未知状态；HTTP 4xx 明确拒单会标记当前 intent 为 error，只有网络中断、5xx 或成功响应缺少订单 ID 才进入提交结果未知锁。managed order 的后续 upsert 会同步更新实际提交数量；SELL intent price 保留非亏损退出 floor；post-only exit 被取消后的重试保留退出 floor，并在后续按 maker 规则重试。订单 scoring 观测只推进 `last_scored_at`，不修改业务状态 `updated_at`；reconciliation 锁订单跳过 scoring 查询，避免周期性观测掩盖真实业务状态年龄。
- Polymarket connector 已迁移到 CLOB V2 Rust crate：`packages/Cargo.toml` 保留 dependency key `polymarket-client-sdk`，实际指向 `polymarket_client_sdk_v2`；live CLOB 签名类型支持 `eoa`、`proxy`、`gnosis_safe`、`poly_1271`，其中 `poly_1271` 用于已有 Deposit Wallet（`FUNDER` 填 deposit wallet 地址），下单前会调用 CLOB balance allowance update；已支持 collateral balance 查询、Polygon pUSD ERC20 余额读取、开放订单全量分页、heartbeat raw 续链/`heartbeat_id:null` 重建 fallback 和 rewards earnings raw JSON fallback。Rewards 账户同步优先把 `FUNDER` 作为资金钱包地址，CLOB balance 为 0 或失败但链上 pUSD 大于 0 时用链上余额回填 snapshot；下单价格当前收敛到最多 2 位小数，同一 trade 内重复 maker entry 会聚合后入账。
- Rewards CLOB heartbeat 失败或超时后会清空本地 heartbeat id，并按 5-60 秒退避重建链；连续失败首条和每 6 次记录 warn，其余降为 debug，恢复时记录 info。
- 聪明钱跟单（copy-trading）已精简为只读跟踪+分析子系统：跟踪多个 Polymarket 钱包地址（`TrackedWallet`）、通过 Polymarket Data API（`data-api.polymarket.com`，通过 `PolymarketDataApiConnector`）检测钱包新成交、钱包分析统计（胜率/ROI/成交量）、`Analyze` 与钱包管理前端 UI。模拟引擎（模拟资金账本、仓位、订单、PnL）已移除，跟单不会下单。前端不再展示模拟账户、订单、持仓、Run、Cancel 或 Reset，只保留启停跟踪、钱包管理、Analyze、源成交和事件日志。未处理 source trades 按时间排序并记录。API 服务不执行 copytrade 跟单循环或钱包分析，前端 Analyze 只会写入数据库控制命令，由 worker 领取执行；`POLYEDGE_COPYTRADE__ENABLED=true` 启用 worker 轮询。
- Polymarket 运行时不再提供 mock mode；市场列表走 Gamma 实时数据，私有订单/成交任务需要真实凭证、真实账户、小额演练和运维 runbook。
- 数据库自动清理已分两层接入：套利扫描每轮按 `arbitrage.event_retention_hours` 删除旧 scan 并级联清 `market_book_snapshots`；通用 `database-maintenance` worker 默认生产开启，按 retention 分批清理 raw events、过期 AI/info-risk cache、`reward_market_candles`、控制命令、copytrade 历史、outbox/external dedup、LLM calls、audit logs 和 mode transitions，避免这些表无限膨胀。删除只释放可复用空间；如需把已膨胀文件还给操作系统，仍需运维执行 vacuum/repack 类操作。
- 数据库迁移目前到 `0045_reward_control_command_dedupe.sql`；`packages/backend/init.sql` 是按 0001–0045 合并的空库完整初始化脚本，运行时仍保留 `packages/backend/migrations/` 给 `sqlx` 校验和增量迁移使用。

## 主要缺口

- 生产级真实会话体系未完成；当前前端只保留 `off` 模式。
- 内部 JWT 签名 helper 已有代码路径，但当前不会从 `off` 签发可信令牌。
- 前端已移除 SSE 实时流机制，页面数据通过 REST API 加载；Rewards 工作台会额外每 10 秒静默刷新当前 snapshot，以反映 worker 写入的 AI advisory、信息风险、订单和账户状态；静默自动刷新遇到短暂网络失败时保留现有页面状态且不弹出“操作失败”，用户主动操作/筛选触发的失败仍会反馈。
- 新闻源可以抓取、去重、提升为 events/evidences，但尚未自动生成 signals。
- Rewards live maker 已接入真实 post-only 买单提交、撤单、本系统托管订单成交与计分同步、CLOB open-order 反查、可映射 active rewards BUY 收养/重开、成交后现金/库存/PnL 更新、sibling leg 撤单和 exit/flatten sell 下单；worker 在 managed order 同步后刷新账户开放买单总 notional 观测，并在新增买单准入时把未归属到本系统 managed order 的外部 BUY notional 从可用资金中保守扣除；confirmed fill 保护期外会刷新 CLOB 余额、资金钱包链上 pUSD 回退和 Data API 完整持仓快照，API 只从数据库读取且不再需要 Polymarket 凭证。仍未完成 SELL、非 rewards 市场、无法唯一映射 token 的账户范围外开放订单明细同步或奖励结算对账。实盘策略仍应沿用“本系统未成交 maker 买单不硬锁全局 pUSD、成交后才更新现金/库存并撤超额挂单；未知外部 BUY 保守占用可用资金”的资金模型。
- Rewards 低竞争市场 sleeve 已有 observe/enforce v2、独立小额度配置、指标 gate、跨周期 observation 和 shadow report；仍缺自动启用/自动切换 enforce。
- Polymarket live 链路已具备 CLOB V2 SDK、认证、token buy/sell 下单和撤单能力，并可配置已有 Deposit Wallet 的 `poly_1271` 签名；仍未实现 relayer 建钱包、pUSD 入金/approval 等 Deposit Wallet 生命周期管理，且仍需真实资金链路小额验证。

## 运行命令

前端：

```bash
cd packages/front
yarn dev
yarn lint
yarn build
```

后端：

```bash
cd packages
cargo check --workspace
cargo test --workspace
cargo run -p polyedge-api
cargo run -p polyedge-worker
cargo run -p polyedge-orderbook
```

常用 worker 子命令：

```bash
cargo run -p polyedge-worker -- ingest-news-once
cargo run -p polyedge-worker -- run-database-maintenance-once
cargo run -p polyedge-worker -- poll-news
cargo run -p polyedge-worker -- promote-news-events
cargo run -p polyedge-worker -- scan-arbitrage-once
cargo run -p polyedge-worker -- poll-arbitrage-radar
cargo run -p polyedge-worker -- analyze-arbitrage-opportunities
cargo run -p polyedge-worker -- scan-rewards-once
cargo run -p polyedge-worker -- poll-reward-bot
cargo run -p polyedge-worker -- scan-reward-info-risks-once
cargo run -p polyedge-worker -- poll-reward-info-risks
cargo run -p polyedge-worker -- drain-execution-queue
cargo run -p polyedge-worker -- poll-polymarket-order-statuses
cargo run -p polyedge-worker -- reconcile-polymarket-fills
cargo run -p polyedge-worker -- consume-polymarket-user-events
cargo run -p polyedge-worker -- scan-copytrade-once
cargo run -p polyedge-worker -- poll-copytrade
cargo run -p polyedge-worker -- analyze-wallets-once
```

套利雷达冒烟：

```bash
./scripts/smoke-arbitrage-radar.sh
```

## 配置要点

- 后端默认监听 `0.0.0.0:38001`。
- 默认 runtime mode 是 `live_auto`。
- Polymarket connector 没有 mock mode；未配置真实账户/私钥时，不要开启 Polymarket 私有订单、成交或用户 websocket worker 任务。
- `POLYEDGE_POLYMARKET__SIGNATURE_TYPE` 可选 `eoa`、`proxy`、`gnosis_safe`、`poly_1271`；新 Deposit Wallet 使用 `poly_1271`，并将 `POLYEDGE_POLYMARKET__FUNDER` 设置为 deposit wallet 地址。
- `POLYEDGE_POLYMARKET__POLYGON_RPC_URL` 默认 `https://polygon-bor-rpc.publicnode.com`；Rewards worker 用它读取资金钱包链上 pUSD 余额，生产环境可替换为自有或有 SLA 的 Polygon RPC。
- `POLYEDGE_WORKER__DATABASE_MAINTENANCE` 控制 API 内嵌 worker 的数据库维护循环；部署模板默认 `true` 且 `POLYEDGE_WORKER__DATABASE_MAINTENANCE_INTERVAL_SECS=3600`，本地 `packages/backend/.env.example` 默认关闭。
- 部署模板默认开启 news ingestion 的子系统/worker 开关，默认关闭 arbitrage radar 及新闻提升为 events/evidences。
- `POLYEDGE_NEWS__SOURCES_JSON` 未配置时使用代码默认 RSS/Atom 源列表；`deploy/.env.api.example` 已显式写入当前默认源列表，设置该变量会覆盖整个列表。新闻采集在部署模板中默认启用（`POLYEDGE_NEWS__ENABLED=true`、`POLYEDGE_WORKER__POLL_NEWS=true`），新闻提升为 events 仍需 `POLYEDGE_WORKER__PROMOTE_NEWS_EVENTS=true`。
- 默认 rewards bot worker 是 disabled；前端 `/rewards` 的 Run / Cancel / Reset 只会入队命令，且同账户同动作已有 pending/running 命令时会合并重复请求；worker 需要同时设置 `POLYEDGE_REWARDS__ENABLED=true` 和 `POLYEDGE_WORKER__POLL_REWARD_BOT=true` 才会领取并执行。`ai_advisory_enabled=true` 时，信息风险 provider 刷新由 rewards full tick 的 market provider refresh 按 condition 与 AI advisory 同步推进；未启用 AI advisory 但要独立异步扫描信息风险时，才需要额外设置 `POLYEDGE_WORKER__POLL_REWARD_INFO_RISKS=true` 并在 `/rewards` 配置中启用 `info_risk_enabled`。要产生新挂单和 live post-only 下单，还需要配置真实 Polymarket 凭证并确保 `polyedge-orderbook` 服务正在运行并同步了 reward 市场数据。
- 部署侧环境变量已精简为三个服务级模板：`deploy/.env.api.example`、`deploy/.env.orderbook.example`、`deploy/.env.front.example`。`deploy/.env.api.example` 同时包含 API、内嵌 worker runtime、Polymarket live/Deposit Wallet（`poly_1271`）和 rewards AI/信息风险可选凭证示例；新闻采集默认开启，其他后台 worker 循环默认关闭。高级轮询/阈值调参优先使用 Settings/runtime_config 或代码默认值。私钥和 AI provider key 只放 `deploy/.env.api`；front/orderbook 不持有 Polymarket 凭证，余额和持仓由 worker 同步到数据库后 API 从数据库读取。
- Rewards bot 的 `max_markets=0` 或 `max_open_orders=0` 表示不再新挂单；`quote_size_usd=0` 不再禁用报价，报价腿按 rewards 最小份额、Polymarket 1 美元最小名义金额和实际钱包余额准入处理。前端 `/rewards` 不再展示或提交 `per_market_usd`、`quote_size_usd`、`low_competition_per_market_usd` 额度字段，这些字段只保留后端兼容读取/序列化。
- Rewards bot 的 `quote_bid_rank` 仅允许 `1`、`2`、`3`，默认 `1`；粗 tick 盘口按不同买价挂在买一、买二、买三，细 tick 盘口会从买一回退 `rank-1` 个 0.01 价格步长后选择不高于目标价的当前买盘档位，避免 0.001 tick 下买三只退两个细档。该检查只在 live placement 准备挂单时基于当前 orderbook 执行，不在 quote plan 构建阶段提前过滤候选；缺少、过期或接近 stale 边界的盘口会保持等待订阅数据返回，auto/enforce/dominant 下双边缺档可回退到目标档位存在且通过校验的单腿，否则非 transient 验证失败才写入 12 小时 `live_skip_until`/`live_skip_reason`。
- Rewards bot 的 `max_spread_cents` 限制为 `0.1..=99`；超过概率价格有效范围的输入会归一化为 99。
- Rewards bot 市场质量硬门槛默认是：`min_market_liquidity_usd=1000`、`min_market_volume_24h_usd=1000`、`min_hours_to_end=48`、`max_market_spread_cents=10`、`max_market_data_age_minutes=15`；通过门槛后再按奖励、流动性、成交量、剩余时长和奖励 spread 综合排序。`max_market_data_age_minutes` 同时驱动 orderbook Gamma priority sync 间隔，窗口越小，已注册/活跃/rewards 候选市场刷新越频繁，避免仅因全量 Gamma 目录慢而触发新鲜度撤单。
- Rewards bot 盘口选择默认 `quote_mode=double`、`selection_mode=observe`、`dominant_single_side_enabled=false`，保持 YES/NO 双边计划。启用 auto/enforce 后，planner 阶段只用 `dominant_min_probability` / `dominant_max_probability` 生成初步单边/双边/跳过模式；需要当前盘口的 `dominant_min_exit_depth_usd`、`max_top1_depth_share`、`max_top3_depth_share` 和 `max_book_hhi` 在 live placement materialize 阶段验证。双边目标档位、rewards spread、touch ask 或安全边际不满足时，会优先回退到仍满足这些 live 校验的单腿；两腿都不可行才跳过。`preferred_categories` 默认偏好 `politics,elections,geopolitics`，只作为排序加分。AI advisory 配置包含 `ai_advisory_enabled`、`ai_provider=openai|anthropic`、`ai_request_format=openai_responses|openai_chat_completions|anthropic_messages`、TTL 和 `ai_advisory_batch_size`；信息风险配置包含 `info_risk_enabled`、`info_risk_mode=observe|enforce`、`info_risk_avoid_level=low|medium|high|critical|unknown`、TTL、`info_risk_batch_size`、`require_info_risk_before_first_quote` 和 `first_quote_quarantine_sec`。API key/base URL/model/timeout/最低置信度来自 worker 环境变量（如 `POLYEDGE_REWARDS__AI_OPENAI_API_KEY`、`POLYEDGE_REWARDS__AI_ANTHROPIC_API_KEY`、`POLYEDGE_REWARDS__AI_MODEL`、`POLYEDGE_REWARDS__AI_MIN_CONFIDENCE_BPS=5500`、`POLYEDGE_REWARDS__INFO_RISK_MIN_CONFIDENCE_BPS=7000`、`POLYEDGE_REWARDS__INFO_RISK_WEB_SEARCH_ENABLED=false`、`POLYEDGE_REWARDS__INFO_RISK_MAX_MARKETS_PER_CYCLE=50`），不会进入前端或 API snapshot；AI advisory 每轮最大市场数环境变量已移除，信息风险每轮最大市场数环境变量现在作为 AI/info-risk provider 每轮 condition cap，默认 50，0 表示本轮不发 provider 请求。
- Rewards bot 本系统未成交 post-only maker 买单不在本地按全局 notional 硬锁资金；不同 condition 可复用同一资金池，但同一 condition 的已有 managed BUY 剩余 notional 与待补 YES/NO 腿必须合计不超过最近同步的 `available_usd` 扣除未归属外部 BUY notional 后的余额，否则整组不挂。CLOB open-order snapshot 会把可映射 active rewards BUY 收养/重开为 managed order；SELL、非 rewards 市场和无法唯一映射 token 的外部开放订单明细仍缺失，当前其余未知开放 BUY 只同步账户级 `external_buy_notional` 并用 `external_buy_notional - managed_external_buy_notional` 作为保守未知占用；`stale_book_ms` 默认 45000，配置归一化下限为 5000ms，不再允许生产配置把盘口年龄检查降到 0；worker 会对本地缺失、过期或超过新挂单 freshness headroom 的盘口用 orderbook 服务 HTTP batch 兜底刷新，新挂单要求盘口距离 stale 边界仍有余量；full tick 在 AI/info-risk gate、订单同步和账户同步之后，进入撤单/待提交 intent/新挂单前会对当前 open-like 订单与 eligible quote plan token 再做一次本地 cache / HTTP batch 刷新，避免 tick 前半段 I/O 耗时让初读盘口在 placement 阶段变旧；近期已提交 BUY 的单纯 stale 撤单会短暂 grace，缺盘口/空盘口和其他硬风险不延迟。
- Rewards live placement freshness headroom 默认保留 10 秒 stale 余量（短 stale 窗口保留半窗）；默认 `stale_book_ms=45000` 时，新挂单接受 `confirmed_at` age 约 35 秒以内的盘口，worker 本地 HTTP 预刷新阈值约 25 秒；worker 远程 batch 读取会把该预刷新阈值传给 orderbook 服务，服务端缓存也超过该年龄时会同步 CLOB `/books` 刷新，避免 orderbook 10 秒 poll 与 full tick I/O 把可挂市场长期卡在等待盘口。
- Rewards quote plan 的 midpoint/materialize 与 live 风控统一使用 `RewardOrderBook.confirmed_at` 判断盘口新鲜度，`observed_at` 只表示盘口内容版本和历史样本时间；安静市场只要 orderbook poll/WS 最近确认过，就不会因内容时间戳不变被误判为缺 fresh midpoint。
- Rewards bot 对外部订单 404 会先保持对账锁；若超过 5 分钟仍无 CLOB/Data API 成交证据，则将本地订单标记为 `cancelled`，使其不再计入开放挂单。普通已提交 open-like BUY 若在 CLOB open orders snapshot 中缺失且无活跃对账锁，也会本地标记为 `cancelled`。提交结果未知订单现在会在恢复查询确认 CLOB 无对应 open 订单后经过 `LIVE_SUBMISSION_UNKNOWN_CLOSE_AFTER_SECS`（默认 600 秒）宽限自动本地关闭（与 404 锁一致），不再永久卡死；取消结果未知订单仍不会仅因本地等待超时 force-cancel。旧 `auto_cancel_stale_minutes` 配置键读取时忽略。
- Rewards fast reconcile 可被活跃 token 盘口事件直接唤醒，不再做固定 1 秒合并，但重型外部同步独立节流：托管订单状态最小 5 秒间隔，CLOB open-order snapshot 最小 15 秒间隔，managed scoring 按 `min_scoring_check_sec` 且归一化下限 15 秒，账户级 rewards earnings 与 balance/positions snapshot 最小 60 秒间隔；full tick 或 `run_once` 完整同步后会刷新这些节流时间戳。post-only violation 的 cancel rejected/unknown 会按最小 15 秒间隔重试，cancel accepted 但超过 30 秒仍未完成最终对账时会再次尝试撤单。
- Rewards poll loop 持有 live advisory lease 后会先尝试一次历史清理，之后每 5 天清理一次 5 天前的终态 managed orders（`cancelled`/`filled`/`error`）、`reward_risk_events` 和 `reward_low_competition_observations`，避免订单和事件表无限膨胀；清理不会删除 `planned`/`open`/`exit_pending`、fills、positions 或 account state，因此不影响当前挂单、持仓和成交账本。
- Rewards bot 不再用 `per_market_usd`、`quote_size_usd` 或 `low_competition_per_market_usd` 限制报价腿构造；live materializer 只满足报价腿按 CLOB 成本精度向上对齐后的 `rewards_min_size` 和 Polymarket 1 美元最小名义金额。新增报价是否可挂由实际钱包余额决定：同一 condition 已有 managed BUY 剩余 notional 与待补 YES/NO 腿总 notional 必须不超过最近同步的 `available_usd` 扣除未归属外部 BUY notional 后的余额；若待补最低 rewards size 腿已经放不下，live placement 会把该 quote plan 标记为不可挂并写入 funding reason，等待后续余额或开放订单同步后重新评估。
- `POLYEDGE_ORDERBOOK_STREAM__MAX_TOKENS` 默认 3000；调高会增加 orderbook WS/poll 内存占用，调低会减少 rewards 盘口覆盖。每个 source（活跃 rewards、execution、最终 eligible rewards、AI provider 临时批次、低竞争 probe 批次、其余候选 token）独立注册全量 token，由聚合层按固定优先级 `rewards_active > exec_orders > rewards_eligible > rewards_ai_provider > rewards_low_competition_probe > rewards_candidates > copytrade` 跨 source 去重并 take 上限截断；`rewards_eligible` 由周期任务注册全部最终 eligible quote plan token，pre-AI provider 候选通过 `rewards_ai_provider` 每批最多 10 个市场临时订阅，批次切换会取消上一批，且 provider refresh 会先保留开放订单/持仓，再将普通候选与低竞争通过 gate 的 pre-AI 市场按约 2:1 混排；rewards 新买单 intent 持久化后会即时刷新 `rewards_active` source。`POLYEDGE_ORDERBOOK_STREAM__REWARD_CANDIDATE_TOKEN_CAP` 默认 50，只限制 rewards 候选预热 source，不影响活跃订单、持仓、execution、最终 eligible 或 AI provider 临时批次；设为 0 可关闭候选预热以快速降带宽，但这些 eligible/provider/low-competition-probe 批次仍会按需订阅。`POLYEDGE_ORDERBOOK_STREAM__WS_CHUNK_SIZE` 默认 100，用于控制每条 Polymarket WS 连接承载的 token 数；调低会减少单连接消息压力、增加连接数，调高则相反。`POLYEDGE_ORDERBOOK_STREAM__POLL_RECONCILE_INTERVAL_SECS` 默认 10；调低会更快修复 WS 缺口但增加 CLOB `/books` 压力，调高可能让 rewards live placement 因盘口超过默认约 35 秒 placement 窗口而等待。`POLYEDGE_ORDERBOOK_STREAM__MAX_LEVELS_PER_SIDE` 默认 100，用于限制进程内缓存和 HTTP ingest 每个 token 的 bids/asks 保留深度；写入时先排序再裁剪，保留最优档位。poll 每周期会刷新全部注册 token；`POLYEDGE_ORDERBOOK_STREAM__STALE_THRESHOLD_MS=0` 只关闭年龄 stale 优先级。`POLYEDGE_ORDERBOOK_STREAM__WS_INCREMENTAL_RECONCILE`（默认 true）控制 token 集合成员变化时是保持 WS 连接做增量 subscribe/unsubscribe（true）还是整体重建连接（false，回滚用）；`POLYEDGE_ORDERBOOK_STREAM__FULL_RESYNC_INTERVAL_SECS`（默认 0=关）大于 0 时按周期强制 teardown+rebuild 全部 WS 连接，作为增量状态漂移的应急兜底（默认依赖 poll reconciler 与 SDK reconnect 全量重订阅兜底）。
- 默认跟单 worker 是 disabled；前端 `/copy-trading` 只提供启停跟踪配置、钱包管理和 Analyze 命令入队，不再暴露 Run / Cancel / Reset。worker 需要设置 `POLYEDGE_COPYTRADE__ENABLED=true` + `POLYEDGE_WORKER__POLL_COPYTRADE=true` 才会持续扫描源成交；`POLYEDGE_WORKER__ANALYZE_WALLETS=true` 仍用于独立钱包分析循环，前端 Analyze 命令也需要 worker 领取后才会更新分析统计。
- `POLYEDGE_POSTGRES__URL` / `POLYEDGE_REDIS__URL` 为空时，本地可能走内存路径，无法验证多进程共享状态和持久化 outbox。
- Postgres rewards live worker 在整个 poll loop 生命周期持有 advisory lease，因此 `POLYEDGE_POSTGRES__MAX_CONNECTIONS` 必须至少为 2（默认 20）。生产环境必须运行持续 poll loop 维持 CLOB heartbeat；`scan-rewards-once` 或有限 `max_cycles` 只适合诊断，进程结束后不能继续守护已提交订单。
- `POLYEDGE_ORDERBOOK__SERVICE_URL` 的代码默认值是 `http://localhost:38002`，只适用于宿主机直接运行；Docker Compose 同项目部署必须在 `deploy/.env.api` 使用 `http://polyedge-orderbook:38002`，跨服务器部署时使用 orderbook 服务器的实际地址（默认生产排查地址为 `http://100.87.45.72:38002`）。worker 会用同一地址转换为 `ws(s)://.../orderbook/stream` 连接内部盘口推送。Compose 不会再覆盖 `.env.api` 中的值。`POLYEDGE_ORDERBOOK__WRITE_TOKEN` 是 orderbook/API 内嵌 worker 部署必填共享密钥，分别放在 `deploy/.env.orderbook` 与 `deploy/.env.api` 且值必须一致，不放入 front 环境；`OrderbookHttpClient` 使用 5 秒连接超时和 30 秒请求超时，`OrderbookStreamClient` 建立内部 WS 连接最多等待 5 秒。
- `POLYEDGE_ARBITRAGE__BOOK_SOURCE=polymarket` 会请求真实 Polymarket CLOB `/book`；live 冒烟必须使用真实 Polymarket refs。
- Docker 部署中没有单独的 `polyedge-worker` service；`polyedge-api` 只加载 `.env.api` 并在同一进程内启动 worker runtime。部署模板默认启用新闻采集，其他后台循环仍显式设为 `false`；需要运行套利、rewards、copytrade 或新闻提升时必须在 `deploy/.env.api` 显式设为 `true`。市场同步和 orderbook 订阅由独立 `polyedge-orderbook` 服务管理，不需要在 worker 中启用。

## Docker 部署

后端镜像从 `bin/` 目录复制预构建二进制；服务器部署不编译 Rust。构建机/CI 先执行：

```bash
./scripts/build-backend-bin.sh
git add bin/polyedge-api bin/polyedge-orderbook
```

跨服务器部署时只需构建目标服务器需要的二进制，例如 orderbook 服务器只需 `polyedge-orderbook`；只设置 `POLYEDGE_BACKEND_BINARY` 时构建脚本会自动选择同名 Cargo package：

```bash
POLYEDGE_BACKEND_BINARY=polyedge-orderbook ./scripts/build-backend-bin.sh
```

服务器部署入口：

```bash
cp deploy/.env.api.example deploy/.env.api
cp deploy/.env.orderbook.example deploy/.env.orderbook
cp deploy/.env.front.example deploy/.env.front
# 在 .env.api 和 .env.orderbook 填入外部 PostgreSQL URL，并设置相同的 POLYEDGE_ORDERBOOK__WRITE_TOKEN
# 在 .env.api 设置 POLYEDGE_ORDERBOOK__SERVICE_URL；在 .env.front 设置 NEXT_PUBLIC_POLYEDGE_API_BASE_URL
# Polymarket live / Deposit Wallet / AI provider 示例在 deploy/.env.api.example 内
# 同 Compose 项目使用 http://polyedge-orderbook:38002；跨服务器设置实际地址
./scripts/deploy.sh all
```

`deploy/docker-compose.yml` 编排（各服务无启动依赖，可独立部署在不同服务器）：

- `polyedge-orderbook`（独立 orderbook 服务，WS + poll + HTTP API，使用 `deploy/orderbook.Dockerfile`）
- `polyedge-api`（内嵌 worker runtime，通过 `POLYEDGE_ORDERBOOK__SERVICE_URL` 读取盘口，使用 `deploy/api.Dockerfile`，加载 `.env.api`）
- `polyedge-front`

`scripts/deploy.sh` 每个服务独立部署，互不依赖：

- 不传参数或 `auto`：拉取最新代码，per-service 检测二进制 hash 变化，只 rebuild 变化的镜像并 restart 变化或未运行的服务。
- `all`：重建所有可用镜像并重启所有可用服务。
- `api`（或 `worker`）：重建 api 镜像并重启 API（`worker` 是兼容别名）。
- `orderbook`（或 `ob`）：重建 orderbook 镜像并重启 orderbook 服务。
- `front`：只重建前端镜像并重启前端。
- 支持组合，例如 `api front` 或 `api,orderbook`。
- `POLYEDGE_SKIP_SERVICES=orderbook` 排除特定服务，适合同一服务器只部署部分服务的场景。

部署脚本默认使用 `/tmp/polyedge-deploy.lock` 防止 cron/CI 重叠执行，默认 `COMPOSE_PARALLEL_LIMIT=1` 串行构建镜像。Auto 模式 per-service 独立检测：api、orderbook、front 各自独立镜像；`worker` 只是 api 目标兼容别名，因为 worker runtime 内嵌在 `polyedge-api` 中。容器未运行但 hash 未变时直接启动已有镜像。前端 `yarn build` 前会读取 `deploy/.env.front` 并把 `NEXT_PUBLIC_*` 写入静态 bundle，build 前会清理旧 `.next/` 和 `out/`，build 后会给 HTML 中的 `/_next/static/*.js/css` 引用追加 front hash query；前端 Nginx 对 HTML 与 `/_next/static/` 使用 `Cache-Control: no-cache, must-revalidate`，避免静态导出 chunk 文件名复用导致浏览器长期运行旧工作台代码。Compose 构建上下文已收窄：后端只上传 `bin/`，前端只上传 `packages/front/`，避免扫描本地 `packages/target`、`node_modules`、`.next` 等大目录。跨服务器部署时每台服务器只需本地存在的二进制，脚本只检查目标服务所需的文件。
旧的 `packages/backend/Dockerfile` 仅作为仓库根 context 兼容模板保留，当前只复制默认构建脚本产出的 `bin/polyedge-api` 与 `bin/polyedge-orderbook`；Compose 部署不使用它，仍使用 `deploy/api.Dockerfile` 和 `deploy/orderbook.Dockerfile`。

## 关键入口

前端：

- `packages/front/src/lib/api/base.ts`
- `packages/front/src/lib/api/actions.ts` + `actions/`
- `packages/front/src/lib/api/copytrade.ts`
- `packages/front/src/proxy.ts`
- `packages/front/src/lib/i18n/*`
- `packages/front/src/features/radar/*`
- `packages/front/src/features/copytrade/*`

后端：

- `packages/api/src/lib.rs`
- `packages/api/src/handlers/rewards.rs`
- `packages/api/src/handlers/copytrade.rs`
- `packages/orderbook/src/main.rs`
- `packages/backend/apps/worker/src/worker/rewards.rs`
- `packages/backend/apps/worker/src/worker/rewards/account_sync.rs`
- `packages/backend/apps/worker/src/worker/copytrade.rs`
- `packages/backend/crates/application/src/rewards/service.rs`
- `packages/backend/crates/application/src/rewards/pagination.rs`
- `packages/backend/crates/application/src/copytrade.rs`
- `packages/backend/crates/application/src/copytrade/service.rs`
- `packages/backend/crates/connectors/src/polymarket/data_api.rs`
- `packages/backend/crates/connectors/src/polymarket/live.rs` / `live/raw.rs` — `LivePolymarketConnector`：认证、下单、撤单、查询余额、挂单、heartbeat 和 rewards earnings raw fallback
- `packages/backend/crates/connectors/src/polymarket/models.rs` — Polymarket connector 类型定义（`PolymarketOpenOrder`、`PolymarketTokenOrderSide` 等）
- `packages/backend/crates/connectors/src/orderbook.rs`
- `packages/backend/crates/connectors/src/rewards.rs`
- `packages/backend/crates/infrastructure/src/stores/copytrade.rs`
- `packages/backend/crates/infrastructure/src/settings.rs`
- `packages/backend/crates/application/src/orderbook_registry.rs`
- `packages/backend/crates/infrastructure/src/stores/orderbook_registry.rs`
- `packages/backend/migrations/0028_reward_positions_external_inventory.sql`
- `packages/backend/migrations/0030_rewards_snapshot_indexes.sql`
- `packages/backend/migrations/0031_worker_query_indexes.sql`
- `packages/backend/migrations/0032_reward_worker_heartbeats.sql`

部署：

- `deploy/orderbook.Dockerfile`
- `deploy/api.Dockerfile`
- `packages/front/Dockerfile`
- `deploy/docker-compose.yml`
- `deploy/.env.api.example`
- `deploy/.env.orderbook.example`
- `deploy/.env.front.example`
- `scripts/deploy.sh`
- `scripts/build-backend-bin.sh`

## 更新检查

改代码后至少检查：

- 是否新增、删除或重命名页面、API、worker 子命令、迁移或部署服务。
- 是否修改环境变量、默认端口、运行模式、鉴权方式或依赖。
- 是否改变前后端贯通状态、Polymarket live 状态或部署命令。
- 顶部日期是否需要更新。
