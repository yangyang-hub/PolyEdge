# Agent Guidelines

最后更新：2026-06-17

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
| Order books | `polyedge-orderbook` 服务 | CLOB WebSocket + `/books` batch poll（回退 `/book`） | `InMemoryOrderbookCache`（orderbook 服务进程内，TTL 5 分钟） | WS real-time + 30s full reconcile |

Orderbook 订阅由独立的 `polyedge-orderbook` 服务管理。该服务始终运行 WS + poll stream，维护进程内缓存和 `OrderbookSubscriptionRegistry`，暴露 HTTP API（`GET /orderbook/{token_id}`、`POST /orderbook/batch`、`GET /orderbook/stats`、`POST /orderbook/register` 等）和内部 WS 推送接口（`GET /orderbook/stream`）。Worker 和 API 通过 `OrderbookHttpClient`（HTTP 调用 orderbook 服务）读取盘口数据，rewards worker 长期 poll loop 还会通过 `OrderbookStreamClient` 连接内部 WS，维护 worker 本地盘口 cache 并用活跃 token 更新唤醒 fast reconcile；内部 WS 连接建立最多等待 5 秒，worker 在约 3 个 poll reconcile 周期无消息后会主动重连并重新 HTTP bootstrap。Worker 通过携带 `POLYEDGE_ORDERBOOK__WRITE_TOKEN` 注册 token。`/orderbook/register` 会原子替换对应 source 当前有序 token 集合，空集合会删除该 source，避免 DELETE/POST 空窗、陈旧来源残留和同一 source 单调增长；HTTP registry 最多保留 32 个 source，in-memory registry 在写锁内再次原子校验上限；`/orderbook/stats` 返回真实 cache 条目数、registry 来源数和 registry 去重 token 总数。聚合优先级固定为 `rewards_active`、`exec_orders`、`rewards_eligible`、`rewards_candidates`、`copytrade`；总量受 `POLYEDGE_ORDERBOOK_STREAM__MAX_TOKENS` 限制；stream refresh 只在聚合 token 成员集合变化时重建 Polymarket WS 订阅，单纯顺序变化只更新 poll reconciler 的共享列表，不触发 WS 重连。register/ingest/delete 写接口要求共享写 token，未配置时写接口关闭；HTTP ingest 会先校验整批盘口，再批量写入并传播缓存错误。WS 同时消费完整 `book` 快照和 `price_change` 增量；所有缓存写入会先把 bids 按价格降序、asks 按价格升序排序，再保留每侧最多 `POLYEDGE_ORDERBOOK_STREAM__MAX_LEVELS_PER_SIDE` 档深度（默认 100），并拒绝旧 `observed_at` 覆盖更新盘口。每次 WS snapshot、WS price_change、poll reconcile 或 HTTP ingest 成功写入缓存后，orderbook 服务都会广播携带单调 sequence、reason 和 `CachedOrderBook` 的 `OrderbookStreamEvent`；慢消费者需断线后重新 HTTP bootstrap。poll reconciler 每个周期优先刷新 stale token，随后刷新其余注册 token，使用 CLOB `/books` 批量接口并在失败或遗漏时回退 `/book`，以修复未被发现的 WS 增量丢失；stale threshold 小于等于 0 时只关闭年龄 stale 优先级。

市场和奖励市场由 orderbook 服务同步写入 Postgres，盘口数据由 orderbook 服务流式写入进程内缓存。所有消费者从数据库或 orderbook 服务读取，不直接调用外部 API。

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
| `apps/worker/src/worker/market_sync.rs` | 市场同步 CLI 兼容入口；daemon 同步已迁移到 orderbook 服务 |
| `apps/worker/src/worker/orderbook_stream.rs` | Orderbook stream — 仅保留 CLI 子命令兼容，核心逻辑已迁移到 polyedge-orderbook 服务 |
| `apps/orderbook/src/main.rs` | 独立 orderbook 服务入口 — HTTP server、Gamma full/priority sync、rewards catalog sync、WS stream + token 注册 |
| `apps/orderbook/src/market_sync.rs` | Orderbook market sync — Gamma full sync、priority condition sync、rewards catalog sync |
| `apps/orderbook/src/http_api.rs` | Orderbook HTTP/API — read/batch/stats/register/ingest、内部 WS stream、写 token 校验、最优档排序 |
| `apps/orderbook/src/updates.rs` | Orderbook update broadcaster — 为 WS/poll/ingest 缓存更新分配 sequence 并推送内部 WS |
| `crates/connectors/src/polymarket/gamma.rs` | Gamma markets connector — `/markets` offset 分页、condition_ids 批量查询、market id 去重 |
| `crates/connectors/src/polymarket/chain.rs` | Polygon chain connector — 读取资金钱包链上 pUSD ERC20 余额 |
| `crates/connectors/src/polymarket/live.rs` + `live/raw.rs` | Polymarket live connector — CLOB V2 认证、heartbeat、收益查询 raw fallback、余额/订单/下单/撤单 |
| `crates/connectors/src/polymarket/live/trade_reconciliation.rs` | Polymarket live order-specific fill 与订单终态对账 helper |
| `crates/connectors/src/news.rs` | RSS/Atom 新闻 connector — 抓取 feed、解析 item/entry、标准化 raw news item |
| `crates/connectors/src/rewards.rs` + `rewards/orderbooks.rs` | Rewards catalog connector + CLOB `/books` batch poll and `/book` fallback |
| `crates/connectors/src/orderbook.rs` | Orderbook service client — HTTP batch/register/ingest + internal WS stream client |
| `crates/connectors/src/openai_compat.rs` | OpenAI-compatible provider helper — root base URL 自动补 `/v1`，Bearer + `api-key` 认证头兼容 |
| `crates/connectors/src/reward_ai.rs` | Rewards AI advisory connector — OpenAI Responses/Chat Completions and Anthropic Messages |
| `crates/connectors/src/reward_info_risk.rs` | Rewards info-risk connector — OpenAI/Anthropic structured risk assessment, optional OpenAI Responses web search |
| `crates/infrastructure/src/settings/defaults.rs` | 后端默认配置 — 包含未设置 `POLYEDGE_NEWS__SOURCES_JSON` 时的默认新闻源列表 |
| `apps/worker/src/worker/rewards.rs` | Rewards bot — executes live strategy ticks and queued run/cancel/reset commands |
| `apps/worker/src/worker/service_info_risk.rs` | Worker runtime hook for async rewards info-risk scans |
| `apps/worker/src/worker/rewards/info_risk.rs` | Rewards info-risk async scan loop, provider cache lookup/write, quote-plan risk application |
| `apps/api/src/handlers/rewards.rs` | Rewards API — reads snapshots/config and enqueues worker control commands |
| `crates/application/src/rewards/service.rs` | RewardBotService — reward markets, snapshots, live order lifecycle, control command queue, in-process command wake channel |
| `crates/application/src/rewards/service_cache.rs` | RewardBotService cached reads — events, fills, open_order_count, positions, heartbeat, event log helper |
| `crates/application/src/rewards/runtime_models.rs` | Rewards runtime models — account/position/order/fill/event/report/snapshot types |
| `crates/application/src/rewards/quote_selection_models.rs` | Rewards quote/selection/AI advisory enums — double/auto、observe/enforce、provider/request format |
| `crates/application/src/rewards/ai_advisory_models.rs` | Rewards AI advisory request/decision/cache models and guarded plan enforcement |
| `crates/application/src/rewards/info_risk_models.rs` | Rewards info-risk request/decision/cache models and guarded plan filtering |
| `crates/application/src/rewards/config_impl.rs` | Rewards config defaults、normalization、candidate filter and patch application |
| `crates/application/src/rewards/planner_selection.rs` | Rewards deterministic quote selection — dominant single-side recommendation, book concentration metrics, preferred category bonus |
| `crates/application/src/rewards/pagination.rs` | Rewards order pagination query and response metadata |
| `apps/worker/src/worker/rewards/live_sync.rs` | Rewards live managed-order trade/status sync |
| `apps/worker/src/worker/rewards/account_sync.rs` | Rewards external balance, CLOB open-order snapshot, and complete position snapshot sync |
| `apps/worker/src/worker/rewards/live_orders.rs` | Rewards live cancel/fill and post-fill exit/flatten intents |
| `apps/worker/src/worker/rewards/live_submission.rs` | Rewards live single-order submit and submission markers |
| `apps/worker/src/worker/rewards/live_pending.rs` | Rewards durable intent submit/recovery workflow |
| `apps/worker/src/worker/rewards/live_risk.rs` | Rewards live placement/cancel risk checks |
| `apps/worker/src/worker/rewards/orderbook_events.rs` | Rewards orderbook event consumer — 内部 WS、本地盘口 cache、HTTP bootstrap、活跃 token wake |
| `apps/worker/src/worker/rewards/polling.rs` | Rewards live poll loop, book fetch, event-driven fast reconcile, in-process book history, command wake subscription |
| `apps/worker/src/worker/copytrade.rs` | Copytrade worker — wallet tracking, source trade detection, and queued analyze commands |
| `apps/api/src/handlers/copytrade.rs` | Copytrade API — reads snapshots/config and enqueues worker control commands |
| `crates/application/src/copytrade/service.rs` | CopyTradeService — copytrade config, wallet tracking, source trade detection, and control command queue |
| `crates/application/src/orderbook_cache.rs` | OrderbookCache trait and stream event models — `CachedOrderBook`、`OrderbookStreamEvent` |
| `crates/application/src/orderbook_registry.rs` | OrderbookSubscriptionRegistry trait — 多来源 token 订阅注册与来源统计 |
| `crates/infrastructure/src/stores/orderbook_cache.rs` | InMemoryOrderbookCache（TTL + 定期清理 + 每侧盘口深度裁剪）；保留 Redis 实现 |
| `crates/infrastructure/src/stores/orderbook_registry.rs` | InMemoryOrderbookSubscriptionRegistry — 来源有序 token 原子替换、确定性优先级聚合、来源与去重总数统计 |
| `crates/infrastructure/src/stores/rewards/postgres_market_methods.rs` | Rewards Postgres candidate query — 市场质量硬过滤、综合排序、row mapping |
| `migrations/0022_reward_bot_control_commands.sql` | Rewards API-to-worker command queue table |
| `migrations/0023_copytrade_control_commands.sql` | Copytrade API-to-worker command queue table |
| `migrations/0024_reward_markets_active_index.sql` | Reward market active/daily-rate query index |
| `migrations/0025_markets_active_volume_index.sql` | Open/tradable market 24h-volume query index |
| `migrations/0026_reward_control_running_lease_index.sql` | Rewards running control command lease query index |
| `migrations/0028_reward_positions_external_inventory.sql` | Allow complete external rewards account inventory outside the reward catalog |
| `migrations/0030_rewards_snapshot_indexes.sql` | Indexes for reward_fills and reward_positions snapshot queries |
| `migrations/0031_worker_query_indexes.sql` | Indexes for worker orders, raw_events event_time, and copytrade source_trades queries |
| `migrations/0032_reward_worker_heartbeats.sql` | Rewards worker heartbeat used by snapshot running status |
| `migrations/0033_reward_candidate_filter.sql` | Rewards candidate filter config |
| `migrations/0034_reward_account_external_buy_notional.sql` | Rewards account external buy notional snapshot |
| `migrations/0035_auto_cancel_not_found_orders.sql` | Historical rewards managed-order repair |
| `migrations/0036_restore_not_found_reconciliation.sql` | Restore incorrectly auto-cancelled 404 orders for trade reconciliation |
| `migrations/0037_reward_market_quality.sql` | Gamma market liquidity/end-time/freshness fields, rewards quality index, unsafe stale-cancel repair |
| `migrations/0038_reward_market_advisories.sql` | Rewards AI advisory cache table keyed by provider/request_format/model/input_hash |
| `migrations/0039_reward_market_info_risks.sql` | Rewards info-risk cache table keyed by provider/request_format/model/input_hash |

## 仓库结构

- `doc/`：系统设计、API 契约、鉴权、存储、前后端计划等文档。
- `packages/front/`：`Next.js 16 + React 19 + Tailwind v4 + shadcn/ui` 控制台前端。前端代码规范（目录结构、数据层、文件行数上限、公共代码提取）见 [packages/front/AGENTS.md](./packages/front/AGENTS.md)，写或改前端代码前必须遵守。
- `packages/backend/`：Rust workspace，包含 `api / worker / orderbook / replay` apps，以及 `application / connectors / contracts / domain / infrastructure` crates。后端代码规范（分层架构、`include!` 模块化、文件行数上限、公共代码提取、测试组织）见 [packages/backend/AGENTS.md](./packages/backend/AGENTS.md)，写或改后端 Rust 代码前必须遵守。
- `deploy/`：Docker Compose 部署模板和环境变量示例；当前 Compose 服务为 `polyedge-api`（内嵌 worker runtime）、`polyedge-orderbook` 和 `polyedge-front`。
- `scripts/`：构建、部署、冒烟脚本。
- `bin/`：部署镜像复制的预构建后端二进制。

## 当前状态

- 仓库已经不是纯文档仓库：前端控制台、Rust API、worker、迁移、配置和 Docker 部署入口都已具备。
- 前端控制台已有 `dashboard / markets / events / radar / rewards / copy-trading / wallet-analysis / signals / positions / risk / settings` 页面；`/replay` 和未落地的 approvals 页面不再作为前端入口暴露。
- 前端数据层统一走 `src/lib/api/*`（读取按领域文件 `markets.ts` / `signals.ts` / `risk.ts`… 基于 `base.ts`，写操作走 `actions.ts`），页面装配在 `src/features/*/loaders` 和 `src/features/*/components`。`src/server/` 目前是空目录（历史遗留）。
- 前端仅支持中文，文案走 `@/lib/i18n/dictionaries` 字典导入。
- 前端不再提供 mock 数据模式；`NEXT_PUBLIC_POLYEDGE_API_BASE_URL` 必须指向 Rust 后端，读写都走真实 `/api/v1/...`。
- 当前控制台会话只保留 `off`，不是生产级真实会话。
- 后端 API 已覆盖 markets、events、news、evidences、signals、orders、trades、positions、pricing、arbitrage、rewards bot、risk、system、connector callback 和 orderbook（`GET /api/v1/orderbook/{token_id}`）等主路径。
- 后端默认 tracing filter 在未设置 `RUST_LOG` 时包含 `polyedge_worker=info`，因此 `polyedge-api` 内嵌 worker runtime 的 info/warn 日志会出现在 API 服务日志中；显式设置 `RUST_LOG` 会覆盖默认 filter。
- 新闻采集当前支持 RSS/Atom XML feed；未配置 `POLYEDGE_NEWS__SOURCES_JSON` 时，代码默认新闻源为 `fed_press`、`sec_press`、`nasa_news`、`bbc_world`、`npr_news`、`coindesk`、`cointelegraph`、`decrypt`；部署模板 `deploy/.env.api.example` 也显式写入同一默认源列表，环境变量或 runtime config 可覆盖整个 sources 列表。
- `polyedge-worker` 支持 news ingest、news promotion、arbitrage radar、rewards bot live 策略、copytrade 钱包跟踪/分析、execution drain、paper reconciliation、Polymarket order/fill/user-event、orderbook token 注册任务。市场同步和 orderbook 订阅已迁移到独立 `polyedge-orderbook` 服务；orderbook 服务启动时先暴露 HTTP `/healthz`，再后台执行独立的 Gamma full sync、Gamma priority sync 与 rewards catalog sync 循环，避免外部 Polymarket API 延迟阻塞容器健康检查，也避免较慢的 rewards 详情补全阻塞 Gamma `markets.synced_at` 刷新；Gamma full sync 使用 `/markets` offset 分页并按 market id 去重，Gamma priority sync 会优先刷新已注册 token 映射到的 condition、活跃 rewards 订单/持仓、eligible quote plans 和放宽新鲜度后的 rewards 候选 condition，最多 500 个 condition，刷新间隔由 rewards `max_market_data_age_minutes` 动态推导（约为窗口三分之一，30-300 秒）；Gamma 单次 full sync 有 60-240 秒超时，priority sync 最长 120 秒超时，rewards 单次同步有 45 分钟超时，rewards 空目录或详情补全后仍不完整时保留上一版目录；orderbook WS + poll stream 遵守 `POLYEDGE_ORDERBOOK_STREAM__RESTART_INTERVAL_SECS`，消费 `book` + `price_change`，token refresh 仅在订阅 token 成员真实增删时重建 WS，registry 聚合顺序抖动不会触发重连；每个 poll 周期对全部注册 token 做批量快照恢复，poll 使用 CLOB 服务端时间戳且同时间戳 WS 优先，内部写接口要求 `POLYEDGE_ORDERBOOK__WRITE_TOKEN`，缓存统一排序后裁剪最优档位并拒绝旧快照覆盖；`/orderbook/stream` 会把 WS、poll 和 ingest 的规范化缓存更新推送给内部消费者。
- 套利雷达是只读链路：发现、记录、校验、分析和展示已具备，但不会创建 execution request 或订单。
- Rewards bot 仅支持 `live` 实盘模式（`execution_mode` 字段已移除，旧配置键读取时忽略）。它只使用 `reward_markets` 表作为奖励市场来源，并关联 `markets` 表硬过滤非 open/tradable、高歧义、低流动性、低 24h 成交量、临近结算、Gamma 价差过宽、同步数据过期或异常超前以及 FDV/launch/token/official-result 等高事件跳变风险市场；候选按奖励、流动性、成交量、剩余时长和 rewards spread（CLOB 原始单位即 cents）的综合质量分优先，命中 `preferred_categories` 的 Gamma 分类只增加排序分，不绕过硬过滤。只有唯一且明确的 YES/NO token 才进入盘口订阅和规划。长期 rewards poll loop 通过 `OrderbookStreamClient` 消费 orderbook 服务内部 `/orderbook/stream`，维护 worker 本地盘口 cache；启动、重连和缺失 token 时用 `OrderbookHttpClient` batch API bootstrap，full tick 读取候选和活跃订单/持仓盘口，fast reconcile 可由活跃 token 盘口更新唤醒且至少 1 秒合并一次，`reconcile_interval_sec` 和 `POLYEDGE_REWARDS__POLL_INTERVAL_SECS` 仍作为兜底。worker 默认生成 YES/NO post-only 双边买单计划；`rewards_min_size` 会先对齐到 CLOB 成本精度，避免提交缩量后失去奖励资格。新报价价格由 `quote_bid_rank=1|2|3` 明确选择买一/买二/买三（按不同买价计档，默认买一）；任一腿缺少目标档位或目标价格超出有效 rewards spread 时该市场本轮不挂单，旧 `quote_edge_cents` 配置键读取时忽略。`quote_mode=double` + `selection_mode=observe` 是默认行为；配置为 `quote_mode=auto` + `selection_mode=enforce` 且启用 dominant single-side 后，planner 会根据一边倒概率区间、退出深度、top1/top3 深度占比和 HHI 生成 `single_yes` / `single_no` / `none` 计划。`observe` 只在 quote plan 记录推荐模式和 `book_metrics`。AI advisory 可选启用：worker 在 full tick 中按开放订单、持仓、初步 eligible quote plan 的顺序用 DB/orderbook/planner/account payload 构建 input hash，先读写 `reward_market_advisories` 缓存，缓存未命中时通过 `RewardAiAdvisoryConnector` 调用 OpenAI Responses、OpenAI Chat Completions 或 Anthropic Messages；AI 开启后新增挂单必须先通过 provider 过滤，缺少未过期 advisory、provider 配置缺失、模型为空、请求失败、低于置信度阈值、`watch/avoid` 或 `quote_mode=none` 都会把原本 eligible 的计划改为不可挂并覆盖保存 quote plan 快照；provider confidence 会在 connector 解析时钳制到 `0..=1`。只有高置信度 `allow` 决策才会放行新增挂单；`selection_mode=enforce` 且 `quote_mode=auto` 时，AI 还能把已 eligible 的 auto 双边计划收窄为单腿，但不会绕过市场质量、盘口和风控硬过滤。信息风险可选启用：独立 worker 任务按开放订单、持仓、eligible quote plan、候选市场顺序，用 active reward market / quote plan / account payload 构建 query/input hash，先读写 `reward_market_info_risks` 缓存，缓存未命中时通过 `RewardInfoRiskConnector` 调用 OpenAI/Anthropic；OpenAI Responses 可选启用 web search tool，provider confidence 同样会钳制到 `0..=1`。live tick 只读取缓存，不等待外部搜索；`info_risk_mode=enforce` 时缺少未过期风险缓存会 fail closed，已有高风险、临近结算或官方结果风险在置信度达到环境变量阈值时也会把计划置为不可挂并触发既有买单撤单路径。worker 使用 `LivePolymarketConnector` 提交 post-only GTC token 买单、FAK flatten 卖单并撤销本系统托管订单；rewards poll loop 全程持有 Postgres advisory lease，只有 leader 维护 5 秒 CLOB heartbeat id 链并执行命令/tick/reconcile，单次 heartbeat 请求 4 秒超时。新建 quote intent 与已落库待提交 BUY 在提交前都会复用 live 撤单风控（计划仍 eligible、报价漂移、min depth、bid rank、depth drop、fill velocity、mass cancel、kill switch 等），风险不通过的本地 intent 会在提交前取消。confirmed fill 按 external trade id + external order id 幂等入账，买入 fill 与退出 intent 同事务落库；明确退出拒单使用有界退避并在达到最大拒绝次数后停止自动重试，提交前低于 Polymarket 1 美元最小名义金额的退出单会进入短 reason 的 dust deferred 状态，每 300 秒重新评估但不重复拼接历史原因，FAK flatten 重试刷新盘口买一价时保留既有退避计数。单订单查询返回 404 时，worker 会按 token 和下单时间窗口查询认证账户 trades，并按 external order id 精确补账，不会把 404 直接标记为 cancelled；仍无法确认时保持 critical 对账锁，暂停新增买单但继续同步、撤单和卖出退出，后续成功查询会自动解除锁；若该 404 锁超过 5 分钟且仍没有 CLOB/Data API 成交证据，worker 会把本地订单标记为 cancelled 以释放开放挂单计数。提交结果未知或取消结果未知订单不会仅因本地超时而释放对账锁。每轮还会读取 CLOB open orders snapshot：普通已提交 open-like BUY 若不在外部开放订单列表且无提交未知、404、pending cancel、post-only violation 等对账锁，会本地标记为 cancelled 释放开放挂单计数；该反查和账户开放 buy notional 观测不受 confirmed fill 保护期影响。成交后 sibling cancel 只撤同 condition 对侧 buy，不撤 sell exit；同 token 存在未完成卖出退出时暂停新增买单。full tick 和 fast reconcile 会先同步 managed orders；本轮有新增 confirmed fill，或数据库最新 confirmed fill 距今不足 120 秒时，只保留本地 balance/positions，等待 CLOB/Data API 最终一致性追平后再同步完整外部账户快照。外部账户同步的资金钱包地址优先使用 `FUNDER`，未配置时使用 `ACCOUNT_ID`；CLOB balance 为 0 或失败但链上 pUSD 余额大于 0 时，worker 用链上 pUSD 回填账户 snapshot，并清零遗留 `reserved_usd`。成功 positions 快照原子替换该账户全部持仓，失败时保留上一版。即使 `enabled=false` 且没有开放订单，worker 仍会尝试刷新外部账户状态。worker 按账户写入数据库 heartbeat，API snapshot 仅在配置启用且 heartbeat 不超过 2 分钟时返回 `running=true`；`status.error` 只由当前开放订单的活跃对账锁推导，不会被历史 critical event 永久污染。API 不直接请求 Polymarket，`orders` 与 `orders_page` 都描述本地 managed orders。`RewardBotService` 内部缓存 config、account、positions、最新 200 条 events、最新 200 条 fills、open_order_count 和 heartbeat，API 与内嵌 worker runtime 共享实例时直接从内存读取这些热状态，缓存为空时回退数据库；控制命令入队通过 in-process command_wake channel 立即唤醒 worker poll loop。账户范围外开放订单明细和奖励结算对账仍是缺口。
- Rewards 成交对账除 404 fallback 外，也会在关联 trade 按 ID 查询失败时按 token/time 扫描认证账户 trades 并按 external order id 精确匹配；认证 CLOB 明确返回 matched size、但 trade 响应仍无法解码时，worker 仅在 Data API 钱包交易的 token/BUY/price/time/累计 size 与唯一 managed order 全部严格匹配后补账。若外部账户和持仓快照已覆盖该成交，只补订单、fill 和退出 intent，不重复扣现金或叠加持仓。任何单笔订单的全部回退失败都只隔离当前订单，不再阻断其余订单对账、账户持仓同步或 stale 清理。
- Rewards AI advisory 已不再按每轮最大市场数截断；full tick 会按开放订单、持仓、初步 eligible quote plan 的顺序覆盖对应 quote plan，缓存命中或 provider 成功保存后会立即把该 condition 的 advisory 增量写入完整 quote plan 快照，并写入包含 candidates/cache_hits/requested/saved/failures/skipped_missing_market/applied 的 info 汇总日志。Rewards config 的 AI provider wire value 使用 `openai|anthropic`，request format 使用 `openai_responses|openai_chat_completions|anthropic_messages`；后端兼容读取旧 `open_ai*` 拼写但序列化始终输出 `openai*`。OpenAI-compatible provider 的 base URL 可配置为根地址或 `/v1` 地址，connector 会统一请求 `/v1/...` 并同时携带 Bearer 与 `api-key` 认证头；MiMo provider 使用 `openai_chat_completions`，不使用未实现的 Responses endpoint。AI provider 单次请求默认超时为 180 秒，可通过 `POLYEDGE_REWARDS__AI_REQUEST_TIMEOUT_SECS` 覆盖；AI advisory 和 info-risk 共用进程内 `Semaphore(1)`，同一 worker/API 进程内任意时刻只允许一个 AI provider HTTP 请求在飞。AI advisory 每轮最大市场数环境变量已移除。API 内嵌 worker 启动会记录 rewards poll loop 是否启用、AI key 是否配置、模型名和 interval；每轮 full tick 会记录 markets/books/plans/eligible/open_orders/positions 以及 AI/info-risk 配置。信息风险异步扫描同样不再按每轮最大市场数截断，会按开放订单、持仓、eligible quote plan、候选市场顺序覆盖全量候选；API 内嵌 runtime 中 info-risk 首轮延迟一个 info-risk interval，避免启动时抢占 AI advisory 的 provider 通道；AI advisory 与 info-risk 会记录逐个 provider 请求的 requesting/saved 进度、汇总和 provider 失败日志；provider 明确返回过载（HTTP 503 / `system_cpu_overloaded` / overloaded）时，worker 会停止本轮剩余 provider 请求以避免继续压垮 provider，并保留既有缓存/过滤语义。旧 `POLYEDGE_REWARDS__INFO_RISK_MAX_MARKETS_PER_CYCLE` 保留为兼容读取但不再限制扫描范围。
- Data API 最终成交回退也覆盖单订单已返回 404 的场景，包括认证账户 trade 扫描报错和扫描成功但没有精确 external order id 成交两种结果；此时必须额外满足：钱包交易累计量恰好等于本地订单剩余量，且完整外部持仓快照已覆盖该数量；否则先保持人工对账锁，若 404 锁超过 5 分钟仍无成交证据则本地标记为 cancelled。Rewards snapshot 的 `status.open_orders` 只统计已有 `external_order_id` 的 open-like managed orders，本地尚未提交的 planned/exit intent 不再显示为 Polymarket 开放挂单。
- Rewards worker 通过认证 CLOB raw HTTP `GET /rewards/user/total?sponsored=true` 同步 UTC 当日账户级 maker rewards 聚合值到 `account.reward_earned_usd`，以对齐 Polymarket `/rewards` 页面顶部 Daily Rewards 的 native+sponsored 口径；当聚合端点为空、为 0 或不可用时，会回退分页读取 `GET /rewards/user` native 明细并合并 `sponsored=true` sponsored-only 明细，按 `earnings * asset_rate` 求和；SDK 解码失败时会使用同一 L2 签名的 raw HTTP fallback，宽容解析带 trailing input 的 JSON 响应。前端只读取数据库/API snapshot，不直连 Polymarket。
- Rewards live 会在提交旧 intent 前先执行当前盘口/资格撤单检查；任一提交结果未知、待最终对账或外部订单 404 会暂停全部新增买单，但继续同步、撤单和卖出退出；外部订单 404 锁超过 5 分钟且仍无成交证据时会自动本地关闭。提交结果未知时，开放订单严格匹配失败也会继续保持人工对账锁，不会自动取消。CLOB `post_order` 只要返回订单 ID 就保留为 accepted 供后续成交/状态对账，包含 `unmatched` / `canceled` / 未知状态；HTTP 4xx 明确拒单会标记当前 intent 为 error，只有网络中断、5xx 或成功响应缺少订单 ID 才进入提交结果未知锁。managed order 的后续 upsert 会同步更新实际提交价格和数量，post-only exit 被取消后的重试仍保持 post-only。订单 scoring 观测只推进 `last_scored_at`，不修改业务状态 `updated_at`；reconciliation 锁订单跳过 scoring 查询，避免周期性观测掩盖真实业务状态年龄。
- Polymarket connector 已迁移到 CLOB V2 Rust crate：`packages/backend/Cargo.toml` 保留 dependency key `polymarket-client-sdk`，实际指向 `polymarket_client_sdk_v2`；live CLOB 签名类型支持 `eoa`、`proxy`、`gnosis_safe`、`poly_1271`，其中 `poly_1271` 用于已有 Deposit Wallet（`FUNDER` 填 deposit wallet 地址），下单前会调用 CLOB balance allowance update；已支持 collateral balance 查询、Polygon pUSD ERC20 余额读取、开放订单全量分页、heartbeat raw 续链/`heartbeat_id:null` 重建 fallback 和 rewards earnings raw JSON fallback。Rewards 账户同步优先把 `FUNDER` 作为资金钱包地址，CLOB balance 为 0 或失败但链上 pUSD 大于 0 时用链上余额回填 snapshot；下单价格当前收敛到最多 2 位小数，同一 trade 内重复 maker entry 会聚合后入账。
- Rewards CLOB heartbeat 失败或超时后会清空本地 heartbeat id，并按 5-60 秒退避重建链；连续失败首条和每 6 次记录 warn，其余降为 debug，恢复时记录 info。
- 聪明钱跟单（copy-trading）已精简为只读跟踪+分析子系统：跟踪多个 Polymarket 钱包地址（`TrackedWallet`）、通过 Polymarket Data API（`data-api.polymarket.com`，通过 `PolymarketDataApiConnector`）检测钱包新成交、钱包分析统计（胜率/ROI/成交量）、`Analyze` 与钱包管理前端 UI。模拟引擎（模拟资金账本、仓位、订单、PnL）已移除，跟单不会下单。前端不再展示模拟账户、订单、持仓、Run、Cancel 或 Reset，只保留启停跟踪、钱包管理、Analyze、源成交和事件日志。未处理 source trades 按时间排序并记录。API 服务不执行 copytrade 跟单循环或钱包分析，前端 Analyze 只会写入数据库控制命令，由 worker 领取执行；`POLYEDGE_COPYTRADE__ENABLED=true` 启用 worker 轮询。
- Polymarket 运行时不再提供 mock mode；市场列表走 Gamma 实时数据，私有订单/成交任务需要真实凭证、真实账户、小额演练和运维 runbook。
- 数据库迁移目前到 `0039_reward_market_info_risks.sql`。

## 主要缺口

- 生产级真实会话体系未完成；当前前端只保留 `off` 模式。
- 内部 JWT 签名 helper 已有代码路径，但当前不会从 `off` 签发可信令牌。
- 前端已移除 SSE 实时流机制，所有页面数据通过 REST API 初始加载。
- 新闻源可以抓取、去重、提升为 events/evidences，但尚未自动生成 signals。
- Rewards live maker 已接入真实 post-only 买单提交、撤单、本系统托管订单成交与计分同步、CLOB open-order 反查、成交后现金/库存/PnL 更新、sibling leg 撤单和 exit/flatten sell 下单；worker 在 managed order 同步后刷新账户开放买单总 notional 观测，并在新增买单准入时把未归属到本系统 managed order 的外部 BUY notional 从可用资金中保守扣除；confirmed fill 保护期外会刷新 CLOB 余额、资金钱包链上 pUSD 回退和 Data API 完整持仓快照，API 只从数据库读取且不再需要 Polymarket 凭证。仍未完成账户范围外开放订单明细同步或奖励结算对账。实盘策略仍应沿用“本系统未成交 maker 买单不硬锁全局 pUSD、成交后才更新现金/库存并撤超额挂单；未知外部 BUY 保守占用可用资金”的资金模型。
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
cd packages/backend
cargo check --workspace
cargo test --workspace
cargo run -p polyedge-api
cargo run -p polyedge-worker
cargo run -p polyedge-orderbook
```

常用 worker 子命令：

```bash
cargo run -p polyedge-worker -- ingest-news-once
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
- 部署模板默认开启 news ingestion 的子系统/worker 开关，默认关闭 arbitrage radar 及新闻提升为 events/evidences。
- `POLYEDGE_NEWS__SOURCES_JSON` 未配置时使用代码默认 RSS/Atom 源列表；`deploy/.env.api.example` 已显式写入当前默认源列表，设置该变量会覆盖整个列表。新闻采集在部署模板中默认启用（`POLYEDGE_NEWS__ENABLED=true`、`POLYEDGE_WORKER__POLL_NEWS=true`），新闻提升为 events 仍需 `POLYEDGE_WORKER__PROMOTE_NEWS_EVENTS=true`。
- 默认 rewards bot worker 是 disabled；前端 `/rewards` 的 Run / Cancel / Reset 只会入队命令，worker 需要同时设置 `POLYEDGE_REWARDS__ENABLED=true` 和 `POLYEDGE_WORKER__POLL_REWARD_BOT=true` 才会领取并执行。信息风险异步扫描还需要设置 `POLYEDGE_WORKER__POLL_REWARD_INFO_RISKS=true` 并在 `/rewards` 配置中启用 `info_risk_enabled`。要产生新挂单和 live post-only 下单，还需要配置真实 Polymarket 凭证并确保 `polyedge-orderbook` 服务正在运行并同步了 reward 市场数据。
- 部署侧环境变量已精简为三个服务级模板：`deploy/.env.api.example`、`deploy/.env.orderbook.example`、`deploy/.env.front.example`。`deploy/.env.api.example` 同时包含 API、内嵌 worker runtime、Polymarket live/Deposit Wallet（`poly_1271`）和 rewards AI/信息风险可选凭证示例；新闻采集默认开启，其他后台 worker 循环默认关闭。高级轮询/阈值调参优先使用 Settings/runtime_config 或代码默认值。私钥和 AI provider key 只放 `deploy/.env.api`；front/orderbook 不持有 Polymarket 凭证，余额和持仓由 worker 同步到数据库后 API 从数据库读取。
- Rewards bot 的 `max_markets=0`、`max_open_orders=0` 或 `quote_size_usd=0` 都表示不再新挂单；不是无限制。
- Rewards bot 的 `quote_bid_rank` 仅允许 `1`、`2`、`3`，分别表示按盘口不同买价挂在买一、买二、买三，默认 `1`；所选档位不是相对中间价偏移，目标档位不足时不会回退到其他价格。
- Rewards bot 的 `max_spread_cents` 限制为 `0.1..=99`；超过概率价格有效范围的输入会归一化为 99。
- Rewards bot 市场质量硬门槛默认是：`min_market_liquidity_usd=1000`、`min_market_volume_24h_usd=1000`、`min_hours_to_end=48`、`max_market_spread_cents=10`、`max_market_data_age_minutes=15`；通过门槛后再按奖励、流动性、成交量、剩余时长和奖励 spread 综合排序。`max_market_data_age_minutes` 同时驱动 orderbook Gamma priority sync 间隔，窗口越小，已注册/活跃/rewards 候选市场刷新越频繁，避免仅因全量 Gamma 目录慢而触发新鲜度撤单。
- Rewards bot 盘口选择默认 `quote_mode=double`、`selection_mode=observe`、`dominant_single_side_enabled=false`，保持 YES/NO 双边计划。启用 auto/enforce 后，dominant 单边推荐受 `dominant_min_probability`、`dominant_max_probability`、`dominant_min_exit_depth_usd`、`max_top1_depth_share`、`max_top3_depth_share` 和 `max_book_hhi` 限制；`preferred_categories` 默认偏好 `politics,elections,geopolitics`，只作为排序加分。AI advisory 配置包含 `ai_advisory_enabled`、`ai_provider=openai|anthropic`、`ai_request_format=openai_responses|openai_chat_completions|anthropic_messages` 和 TTL；信息风险配置包含 `info_risk_enabled`、`info_risk_mode=observe|enforce`、`info_risk_avoid_level=low|medium|high|critical|unknown` 和 TTL。API key/base URL/model/timeout/最低置信度来自 worker 环境变量（如 `POLYEDGE_REWARDS__AI_OPENAI_API_KEY`、`POLYEDGE_REWARDS__AI_ANTHROPIC_API_KEY`、`POLYEDGE_REWARDS__AI_MODEL`、`POLYEDGE_REWARDS__AI_MIN_CONFIDENCE_BPS=6500`、`POLYEDGE_REWARDS__INFO_RISK_MIN_CONFIDENCE_BPS=7000`、`POLYEDGE_REWARDS__INFO_RISK_WEB_SEARCH_ENABLED=false`），不会进入前端或 API snapshot；AI advisory 每轮最大市场数环境变量已移除，旧信息风险每轮最大市场数环境变量读取兼容但不再限制扫描范围。
- Rewards bot 本系统未成交 post-only maker 买单不在本地按全局 notional 硬锁资金；不同 condition 可复用同一资金池，但同一 condition 的已有 managed BUY 剩余 notional 与待补 YES/NO 腿必须合计不超过最近同步的 `available_usd` 扣除未归属外部 BUY notional 后的余额，否则整组不挂。账户范围外开放订单明细仍缺失，当前只同步账户级 `external_buy_notional` 并用 `external_buy_notional - managed_external_buy_notional` 作为保守未知占用；`stale_book_ms` 默认 45000，`stale_book_ms=0` 只关闭盘口年龄检查，仍要求盘口存在且非空，开放 live 订单缺盘口会被撤单。
- Rewards bot 对外部订单 404 会先保持对账锁；若超过 5 分钟仍无 CLOB/Data API 成交证据，则将本地订单标记为 `cancelled`，使其不再计入开放挂单。普通已提交 open-like BUY 若在 CLOB open orders snapshot 中缺失且无活跃对账锁，也会本地标记为 `cancelled`。提交结果未知或取消结果未知订单仍不会仅因本地等待超时 force-cancel。旧 `auto_cancel_stale_minutes` 配置键读取时忽略。
- Rewards bot 的 `per_market_usd` 是 YES + NO 两腿合计预算：报价计划先满足两腿按 CLOB 成本精度向上对齐后的 `rewards_min_size`，再按单腿目标 notional 缺口分配剩余额度，不再固定均分预算而误拒绝价格不对称市场。
- `POLYEDGE_ORDERBOOK_STREAM__MAX_TOKENS` 默认 3000；调高会增加 orderbook WS/poll 内存占用，调低会减少 rewards 候选盘口覆盖。订阅预算依次分配给活跃 rewards、execution、当前 eligible rewards 和其余候选 token。`POLYEDGE_ORDERBOOK_STREAM__MAX_LEVELS_PER_SIDE` 默认 100，用于限制进程内缓存和 HTTP ingest 每个 token 的 bids/asks 保留深度；写入时先排序再裁剪，保留最优档位。poll 每周期会刷新全部注册 token；`POLYEDGE_ORDERBOOK_STREAM__STALE_THRESHOLD_MS=0` 只关闭年龄 stale 优先级。
- 默认跟单 worker 是 disabled；前端 `/copy-trading` 只提供启停跟踪配置、钱包管理和 Analyze 命令入队，不再暴露 Run / Cancel / Reset。worker 需要设置 `POLYEDGE_COPYTRADE__ENABLED=true` + `POLYEDGE_WORKER__POLL_COPYTRADE=true` 才会持续扫描源成交；`POLYEDGE_WORKER__ANALYZE_WALLETS=true` 仍用于独立钱包分析循环，前端 Analyze 命令也需要 worker 领取后才会更新分析统计。
- `POLYEDGE_POSTGRES__URL` / `POLYEDGE_REDIS__URL` 为空时，本地可能走内存路径，无法验证多进程共享状态和持久化 outbox。
- Postgres rewards live worker 在整个 poll loop 生命周期持有 advisory lease，因此 `POLYEDGE_POSTGRES__MAX_CONNECTIONS` 必须至少为 2（默认 20）。生产环境必须运行持续 poll loop 维持 CLOB heartbeat；`scan-rewards-once` 或有限 `max_cycles` 只适合诊断，进程结束后不能继续守护已提交订单。
- `POLYEDGE_ORDERBOOK__SERVICE_URL` 的代码默认值是 `http://localhost:38002`，只适用于宿主机直接运行；Docker Compose 同项目部署必须在 `deploy/.env.api` 使用 `http://polyedge-orderbook:38002`，跨服务器部署时使用 orderbook 服务器的实际地址（如 `http://192.168.31.10:38002`）。worker 会用同一地址转换为 `ws(s)://.../orderbook/stream` 连接内部盘口推送。Compose 不会再覆盖 `.env.api` 中的值。`POLYEDGE_ORDERBOOK__WRITE_TOKEN` 是 orderbook/API 内嵌 worker 部署必填共享密钥，分别放在 `deploy/.env.orderbook` 与 `deploy/.env.api` 且值必须一致，不放入 front 环境；`OrderbookHttpClient` 使用 5 秒连接超时和 30 秒请求超时，`OrderbookStreamClient` 建立内部 WS 连接最多等待 5 秒。
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

部署脚本默认使用 `/tmp/polyedge-deploy.lock` 防止 cron/CI 重叠执行，默认 `COMPOSE_PARALLEL_LIMIT=1` 串行构建镜像。Auto 模式 per-service 独立检测：api、orderbook、front 各自独立镜像；`worker` 只是 api 目标兼容别名，因为 worker runtime 内嵌在 `polyedge-api` 中。容器未运行但 hash 未变时直接启动已有镜像。前端 `yarn build` 前会读取 `deploy/.env.front` 并把 `NEXT_PUBLIC_*` 写入静态 bundle。Compose 构建上下文已收窄：后端只上传 `bin/`，前端只上传 `packages/front/`，避免扫描本地 `packages/backend/target`、`node_modules`、`.next` 等大目录。跨服务器部署时每台服务器只需本地存在的二进制，脚本只检查目标服务所需的文件。

## 关键入口

前端：

- `packages/front/src/lib/api/base.ts`
- `packages/front/src/lib/api/actions.ts`
- `packages/front/src/lib/api/copytrade.ts`
- `packages/front/src/proxy.ts`
- `packages/front/src/lib/i18n/*`
- `packages/front/src/features/radar/*`
- `packages/front/src/features/copytrade/*`

后端：

- `packages/backend/apps/api/src/lib.rs`
- `packages/backend/apps/api/src/handlers/rewards.rs`
- `packages/backend/apps/api/src/handlers/copytrade.rs`
- `packages/backend/apps/orderbook/src/main.rs`
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
