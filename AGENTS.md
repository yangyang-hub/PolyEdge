# Agent Guidelines

最后更新：2026-06-28

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
| Reward price-history candles | `polyedge-orderbook` 服务 | CLOB API `/prices-history`（5m source fidelity） | `reward_market_candles` table (Postgres) | low-frequency rate-limited history sync, default 5 min cadence |

Orderbook 订阅由独立的 `polyedge-orderbook` 服务管理。该服务始终运行 WS + poll stream，维护进程内缓存和 `OrderbookSubscriptionRegistry`，暴露 HTTP API（`GET /orderbook/{token_id}`、`POST /orderbook/batch`、`GET /orderbook/stats`、`POST /orderbook/register` 等）和内部 WS 推送接口（`GET /orderbook/stream`）。Worker 和 API 通过 `OrderbookHttpClient`（HTTP 调用 orderbook 服务）读取盘口数据；普通 batch 只读缓存，`get_books_with_max_age()` 会在 `POST /orderbook/batch` 请求体携带 `refresh_if_stale_ms`，orderbook 服务仅对缺失或 `confirmed_at` 超过该年龄的 token 同步 CLOB `/books` 刷新后再返回缓存结果，刷新失败则记录 warn 并返回现有缓存由调用方 fail closed。rewards worker 长期 poll loop 还会通过 `OrderbookStreamClient` 连接内部 WS，维护 worker 本地盘口 cache，并把活跃 token 更新同时用于唤醒普通 fast reconcile 和投递到独立 hard-risk cancel worker；内部 WS 连接建立最多等待 5 秒，worker 在约 3 个 poll reconcile 周期无消息后会主动重连并重新 HTTP bootstrap。Worker 通过携带 `POLYEDGE_ORDERBOOK__WRITE_TOKEN` 注册 token。`/orderbook/register` 会原子替换对应 source 当前有序 token 集合，空集合会删除该 source，避免 DELETE/POST 空窗、陈旧来源残留和同一 source 单调增长；worker 周期注册任务会对成功空集合做防抖，`rewards_active`/`exec_orders` 连续 2 轮为空、`rewards_eligible`/`rewards_candidates` 连续 3 轮为空才清远端 source，查询失败或即时 active 刷新读到空集合会保留上一版。HTTP registry 最多保留 32 个 source，in-memory registry 在写锁内再次原子校验上限；`/orderbook/stats` 返回真实 cache 条目数、registry 来源数和 registry 去重 token 总数。聚合优先级固定为 `rewards_active`、`exec_orders`、`rewards_eligible`、`rewards_ai_provider`、`rewards_candidates`、`copytrade`；总量受 `POLYEDGE_ORDERBOOK_STREAM__MAX_TOKENS` 限制；`rewards_candidates` 预热来源还受 `POLYEDGE_ORDERBOOK_STREAM__REWARD_CANDIDATE_TOKEN_CAP` 限制（默认 50）；Polymarket WS 订阅按 `POLYEDGE_ORDERBOOK_STREAM__WS_CHUNK_SIZE` 分片成多条连接（默认 100 token/连接），降低高消息量下 SDK broadcast lag 风险；chunk 内 SDK stream reader 会先快速 drain `book`/`price_change` 事件，再交给缓存写入循环处理，减少慢写入阻塞 SDK broadcast receiver；chunk 退出或被取消时会成对释放 SDK market subscription；stream refresh 在聚合 token 成员集合变化后短暂 debounce 并再次确认：默认（`POLYEDGE_ORDERBOOK_STREAM__WS_INCREMENTAL_RECONCILE=true`）WS 连接常驻，只对 diff 做增量 subscribe/unsubscribe（先 subscribe 新集合再 unsubscribe 旧集合，SDK 按资产 refcount 只发新增/退订帧，保持共享 Market 通道始终非空），只有 reader 结束（连接死亡）才重建那一个 chunk，其余连接不受影响；`WS_INCREMENTAL_RECONCILE=false` 回退到成员变化即整体重建 WS 的旧行为。单纯顺序变化只更新 poll reconciler 的共享列表，不触发任何重订/重连。register/ingest/delete 写接口要求共享写 token，未配置时写接口关闭；HTTP ingest 会先校验整批盘口，再批量写入并传播缓存错误。WS 同时消费完整 `book` 快照和 `price_change` 增量；所有缓存写入会先把 bids 按价格降序、asks 按价格升序排序，再保留每侧最多 `POLYEDGE_ORDERBOOK_STREAM__MAX_LEVELS_PER_SIDE` 档深度（默认 100），并拒绝旧 `observed_at` 覆盖更新盘口。每次 WS snapshot、WS price_change、poll reconcile、按需 batch refresh 或 HTTP ingest 成功写入缓存后，orderbook 服务都会广播携带单调 sequence、reason 和 `CachedOrderBook` 的 `OrderbookStreamEvent`；慢消费者需断线后重新 HTTP bootstrap。rewards midpoint candle 不再由这些高频 cache 更新派生，改由 orderbook 服务独立低频限速调用 CLOB `/prices-history` 写库，避免本地 candle 队列在高频 price_change 下打满。poll reconciler 默认每 10 秒优先刷新 stale token，随后刷新其余注册 token，使用 CLOB `/books` 批量接口并在失败或遗漏时回退 `/book`，以修复未被发现的 WS 增量丢失；stale threshold 小于等于 0 时只关闭年龄 stale 优先级。
Orderbook 进程内缓存只让未过期条目拒绝旧 `observed_at` 覆盖盘口内容，但会合并更新的 `confirmed_at` 作为最近确认时间；已过期条目不会阻挡后续 poll/ingest 恢复。`observed_at` 表示盘口内容版本，`confirmed_at` 表示 orderbook 服务最近通过 WS/poll/ingest 确认该盘口仍可用；rewards 新挂单、撤单和远程刷新 stale 判断使用 `confirmed_at`，安静市场只要最近被 poll 确认过就不会因内容版本长期不变被误判过期。rewards worker 本地盘口 cache 的 TTL 按本地接收/写入时间计算，不用上游 `observed_at` 延长缓存寿命。

市场和奖励市场由 orderbook 服务同步写入 Postgres，盘口数据由 orderbook 服务流式写入进程内缓存；rewards token 的 5 分钟 midpoint source K 线由 orderbook 服务从 CLOB `/prices-history` 低频限速同步写入 `reward_market_candles`，不包含真实成交量；AI advisory 在 application 层把这些 source candles 聚合为最多 24 根 1h candles，并且 cache key 只包含已完成小时级摘要。price-history 行会把 provider price 同时写入 close、`best_bid_close` 和 `best_ask_close`，`spread_cents_close=0`，`sample_count` 代表同 bucket 内持久化的 provider history 点数量。所有消费者从数据库或 orderbook 服务读取，不直接调用外部 API。

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
| `packages/backend/order/src/main.rs` | 独立 orderbook 服务入口 — HTTP server、Gamma full/priority sync、rewards catalog sync、WS stream + token 注册 |
| `packages/backend/order/src/market_sync.rs` | Orderbook market sync — Gamma full sync、priority condition sync、rewards catalog sync、Gamma 日期候选写入 rewards event windows |
| `packages/backend/order/src/candle_history.rs` | Rewards candle history sync — 限速调用 CLOB `/prices-history` 写入 5m price-history source candles |
| `packages/backend/order/src/http_api.rs` | Orderbook HTTP/API — read/batch/stats/register/ingest、内部 WS stream、写 token 校验、最优档排序 |
| `packages/backend/order/src/updates.rs` | Orderbook update broadcaster — 为 WS/poll/ingest 缓存更新分配 sequence、推送内部 WS |
| `packages/backend/crates/common/src/lib.rs` | 后端二进制共享进程外壳 helper — bind address、TCP listener、Ctrl-C/SIGTERM shutdown |
| `packages/backend/crates/connectors/src/polymarket/gamma.rs` | Gamma markets connector — `/markets` offset 分页、condition_ids 批量查询、market id 去重、market/event 日期与 `hasReviewedDates` 解析 |
| `packages/backend/crates/connectors/src/polymarket/chain.rs` | Polygon chain connector — 读取资金钱包链上 pUSD ERC20 余额；通过 Polymarket Bridge 生成入金地址并广播后端资金钱包 USDC/USDT 转账 |
| `packages/backend/crates/connectors/src/polymarket/live.rs` + `live/raw.rs` | Polymarket live connector — CLOB V2 认证、heartbeat、收益查询 raw fallback、余额/订单/下单/撤单 |
| `packages/backend/crates/connectors/src/polymarket/live/trade_reconciliation.rs` | Polymarket live order-specific fill 与订单终态对账 helper |
| `packages/backend/crates/connectors/src/news.rs` | RSS/Atom 新闻 connector — 抓取 feed、解析 item/entry、标准化 raw news item |
| `packages/backend/crates/connectors/src/rewards.rs` + `rewards/orderbooks.rs` + `rewards/price_history.rs` | Rewards catalog connector + CLOB `/books` batch poll, `/book` fallback and `/prices-history` |
| `packages/backend/crates/connectors/src/orderbook.rs` | Orderbook service client — HTTP batch/register/ingest + internal WS stream client |
| `packages/backend/crates/connectors/src/llm_provider.rs` | LLM provider global single-flight gate — Rewards AI advisory、Rewards info-risk 和 Smart Money signal advisory 的实际外部大模型 HTTP 调用共享同一进程内 semaphore |
| `packages/backend/crates/connectors/src/openai_compat.rs` | OpenAI-compatible provider helper — root base URL 自动补 `/v1`、已带 `/vN` 的 provider base 保持原样，Bearer + `api-key` 认证头兼容，模型名包含 GLM/DeepSeek 时的 Chat Completions JSON mode 差异，provider 文本响应候选 JSON 提取 |
| `packages/backend/crates/connectors/src/reward_ai.rs` | Rewards AI advisory connector — OpenAI Responses、OpenAI-compatible Chat Completions（含 GLM/DeepSeek 模型名特例）和 Anthropic Messages，解析 `allow_quote` + conservative `strategy_hint` |
| `packages/backend/crates/connectors/src/reward_ai_tests.rs` | Rewards AI advisory connector tests — 二值 `allow_quote`、`strategy_hint` 解析、旧 `suitability` 响应 fail-closed 兼容与 GLM/DeepSeek Chat Completions mock 请求 |
| `packages/backend/crates/connectors/src/reward_info_risk.rs` | Rewards info-risk connector — OpenAI-compatible/Anthropic structured risk assessment（含 GLM/DeepSeek 模型名特例）, optional OpenAI Responses web search, single/batch parsing |
| `packages/backend/crates/connectors/src/reward_info_risk_tests.rs` | Rewards info-risk connector tests — 二值 `allow_quote`、旧响应兼容与 DeepSeek Chat Completions mock 请求 |
| `packages/backend/crates/connectors/src/smart_signal_advisory.rs` | Smart Money signal advisory connector — OpenAI Responses、OpenAI-compatible Chat Completions（含 GLM/DeepSeek 模型名特例）和 Anthropic Messages，解析 `allow/observe/reject` 三态建议 |
| `packages/backend/crates/connectors/src/test_http.rs` | Connectors test HTTP helper — 本地捕获 provider endpoint、header 与 JSON 请求体 |
| `packages/backend/crates/infrastructure/src/settings/defaults.rs` | 后端默认配置 — 包含未设置 `POLYEDGE_NEWS__SOURCES_JSON` 时的默认新闻源列表 |
| `packages/backend/apps/worker/src/worker/rewards.rs` | Rewards bot — executes live strategy ticks and queued run/cancel/reset commands |
| `packages/backend/apps/worker/src/worker/service_info_risk.rs` | Worker runtime hook for async rewards info-risk scans |
| `packages/backend/apps/worker/src/worker/orderbook_registration.rs` | Worker orderbook token registration — 周期注册 active/eligible/candidate token，并在 rewards 新买单落库后即时刷新 `rewards_active` |
| `packages/backend/apps/worker/src/worker/rewards/provider_advisory.rs` | Rewards AI advisory cache gate, pre-LLM candidate hard filter, provider connector/permit helpers and LLM call recording helper |
| `packages/backend/apps/worker/src/worker/rewards/provider_fallback.rs` | Rewards LLM provider fallback — 解析可选第二个独立 provider/model 接口、主接口任意失败时用同一请求重试备用接口（`advise_with_fallback`/`assess_with_fallback`）、合并 overload 判定、primary/fallback 双 tuple 缓存读取 |
| `packages/backend/apps/worker/src/worker/rewards/provider_refresh_batch.rs` | Rewards AI advisory / info-risk main refresh batch helper — 按配置批量请求、逐项保存、记录实际 provider 调用，AI batch 失败或漏项/错配后的单市场回退同样受本轮 provider request cap 约束 |
| `packages/backend/apps/worker/src/worker/rewards/provider_refresh_orderbook.rs` | Rewards provider refresh temporary orderbook source helper — 临时订阅 AI 所需盘口、allow 后提升 eligible source |
| `packages/backend/apps/worker/src/worker/rewards/provider_refresh.rs` | Rewards AI advisory / info-risk provider refresh — full tick 应用已有 provider cache gate 后分别启动 AI advisory 与 info-risk 后台 task，各自开放订单/持仓优先，其余按统一 standard 候选顺序处理并跳过未到刷新窗口的缓存命中；后台 task 按 rewards poll interval 派生 wall-clock timeout，超时释放单实例锁；AI task 使用 `rewards_ai_provider` 临时盘口批次，info-risk task 独立补缓存；实际 provider 调用写入 `llm_calls` |
| `packages/backend/apps/worker/src/worker/rewards/provider_content_filter.rs` | Rewards AI advisory / info-risk provider 内容过滤处理 — 识别 GLM/OpenAI-compatible `contentFilter` / `1301` 等不可重试输入拒绝并写入 fail-closed 缓存 |
| `packages/backend/apps/worker/src/worker/rewards/provider_refresh_candidates.rs` | Rewards provider refresh candidate ordering — 开放订单/持仓优先，其余按统一 standard 候选顺序处理 |
| `packages/backend/apps/worker/src/worker/rewards/info_risk.rs` | Rewards info-risk async scan loop, provider cache lookup/write, pre-LLM candidate hard filter, quote-plan risk application, provider call recording |
| `packages/backend/api/src/handlers/rewards.rs` | Rewards API — reads snapshots/config and enqueues worker control commands |
| `packages/backend/crates/application/src/rewards/service.rs` | RewardBotService — reward markets, snapshots, live order lifecycle, control command queue, in-process command wake channel |
| `packages/backend/crates/application/src/rewards/service_cache.rs` | RewardBotService cached reads — events, fills, external_open_order_count, positions, heartbeat, event log helper |
| `packages/backend/crates/application/src/rewards/service_snapshot.rs` | RewardBotService snapshot aggregation — orders/plans pagination、legacy `low_competition_report=null` and daily LLM usage stats |
| `packages/backend/crates/application/src/rewards/runtime_models.rs` | Rewards runtime models — account/position/order/fill/event/report/snapshot and LLM usage types |
| `packages/backend/crates/application/src/rewards/quote_selection_models.rs` | Rewards quote/selection/AI advisory enums — double/auto、observe/enforce、provider/request format |
| `packages/backend/crates/application/src/rewards/event_window.rs` | Rewards event-window gate — effective event time assessment、StopNewQuotes/CancelOpenBuys/InEventWindow/PostEventCooldown 状态机和 quote-plan 应用 helper |
| `packages/backend/crates/application/src/rewards/ai_advisory_models.rs` | Rewards AI advisory request/decision/cache models, strategy hint parsing and guarded plan enforcement |
| `packages/backend/crates/application/src/rewards/ai_advisory_payload.rs` | Rewards AI advisory payload helpers — 当前盘口定价上下文、TTL policy 和 1h candle 聚合 |
| `packages/backend/crates/application/src/rewards/info_risk_models.rs` | Rewards info-risk request/decision/cache models and guarded plan filtering |
| `packages/backend/crates/application/src/rewards/provider_prefilter.rs` | Rewards provider pre-LLM hard gate — 保留开放订单/持仓，过滤未通过资格的无敞口计划；legacy low-competition bucket 按 standard 处理 |
| `packages/backend/crates/application/src/rewards/config_impl.rs` | Rewards config defaults、normalization、candidate filter and patch application |
| `packages/backend/crates/application/src/rewards/planner.rs` | Rewards deterministic quote planner — 静态事件风险过滤、首单 info-risk/quarantine gate、quote plan 构建 |
| `packages/backend/crates/application/src/rewards/opportunity_metrics.rs` | Rewards unified opportunity metrics — 竞争度、奖励密度、资金占比、退出能力、盘口稳定性和 score adjustment |
| `packages/backend/crates/application/src/rewards/planner_selection.rs` | Rewards deterministic quote selection — dominant single-side recommendation, book concentration metrics, preferred category bonus |
| `packages/backend/crates/application/src/rewards/planner_live.rs` | Rewards live quote materializer — live orderbook rank/spread/auto metrics and AI-hinted conservative rank before wallet-balance placement gate |
| `packages/backend/crates/application/src/rewards/pagination.rs` | Rewards order pagination query and response metadata |
| `packages/backend/crates/application/src/maintenance.rs` | Database maintenance service/store port — 集中定义数据库历史/缓存/队列表 retention cutoffs 与清理统计 |
| `packages/backend/crates/infrastructure/src/stores/maintenance.rs` | Database maintenance Postgres/no-op store — 按表分批删除 raw events、缓存、candles、队列、copytrade/outbox/audit 历史 |
| `packages/backend/crates/infrastructure/src/stores/rewards/postgres.rs` | Rewards Postgres store — 完整 rewards 持久化、`llm_calls` 记录和每日调用统计 |
| `packages/backend/crates/infrastructure/src/stores/rewards/postgres_event_windows.rs` | Rewards event-window Postgres store — upsert source candidates and list effective event windows by confidence/source priority |
| `packages/backend/crates/infrastructure/src/stores/rewards/postgres_low_competition.rs` | Rewards legacy low-competition observation persistence and recent-window query SQL；当前统一机会评分运行路径不再写入新的独立 observation |
| `packages/backend/crates/infrastructure/src/stores/rewards/postgres_candles.rs` | Rewards price-history candle upsert and recent-candle query SQL |
| `packages/front/src/features/rewards/components/rewards-overview-cards.tsx` | Rewards overview cards — live status, daily LLM usage, command center and summary metrics |
| `packages/front/src/features/rewards/components/rewards-opportunity-config.tsx` | Rewards unified opportunity metrics config panel — 竞争/奖励/资金占比/退出/稳定性阈值和权重 |
| `packages/front/src/features/rewards/components/rewards-opportunity-summary.tsx` | Rewards quote plan opportunity metrics summary — 机会分、score adjustment、竞争倍数、100U 日奖、退出深度和样本数 |
| `packages/front/src/features/rewards/components/rewards-advanced-config.tsx` | Rewards advanced config — 盘口选择、AI advisory strategy hint、信息风险和事件窗口配置 |
| `packages/front/src/features/rewards/components/rewards-tables.tsx` | Rewards tables — quote plan/订单/持仓/成交/事件展示，AI/info-risk 决策展示为“允许挂单/不允许挂单”二值，并显示 AI direction/rank/notional hint |
| `packages/front/src/features/rewards/components/rewards-table-controls.tsx` | Rewards table controls — 搜索、筛选 tabs 和排序指示共享控件 |
| `packages/front/src/features/rewards/lib/rewards-helpers.ts` | Rewards frontend helpers — readiness labels、事件分类和 AI strategy hint metrics 解析 |
| `packages/front/src/app/(console)/funding/page.tsx` | Funding route — Polymarket 入金页面入口 |
| `packages/backend/api/src/handlers/funding.rs` | Funding API — 读取后端资金配置与资金钱包 USDC/USDT 链上余额、幂等校验、step-up 校验并委托 connector 执行 Polymarket Bridge 入金转账 |
| `packages/backend/crates/contracts/src/dto/funding.rs` | Funding DTO — status、token、transfer request/receipt 合约 |
| `packages/front/src/lib/api/funding.ts` + `packages/front/src/lib/api/actions/funding.ts` | Funding frontend data layer — 读取 funding status、提交后端签名转账 |
| `packages/front/src/features/funding/components/funding-workbench.tsx` | Funding workbench — 选择后端资金钱包 USDC/USDT 入金资产与金额，提交后端 Bridge 入金转账 |
| `packages/front/src/features/funding/lib/polygon-funding.ts` | Funding Polygon helpers — 金额最小单位转换、Polygonscan 链接和 token 说明映射 |
| `packages/front/src/app/(console)/high-probability/page.tsx` | High Probability route — 动态高概率定价研究页面入口 |
| `packages/front/src/features/high-probability/components/high-probability-workbench.tsx` | High Probability workbench — 只读展示研究配置、research report、baseline backtest、基础退出规则对比、bucket stats 和 observations，不触发交易 |
| `packages/front/src/features/high-probability/components/high-probability-backtest-history.tsx` | High Probability backtest history — 只读展示持久化 baseline 回测 run，并支持切换查看所选 run 的交易明细 |
| `packages/front/src/features/high-probability/loaders/high-probability-page-data.ts` | High Probability frontend loader — 通过前端数据层读取 `/api/v1/high-probability` snapshot、`/report`、`/backtests` 和 `/backtest-runs` |
| `packages/front/src/features/high-probability/lib/high-probability-formatters.ts` | High Probability frontend formatters — 概率、回撤、bucket 维度、report note、exit rule 和空值展示 helper |
| `packages/front/src/lib/api/high-probability.ts` + `packages/front/src/lib/contracts/dto/high-probability.ts` | High Probability frontend data layer — 只读读取 snapshot/config/bucket stats/report/backtest/exit-rule/backtest-runs DTO |
| `packages/front/src/lib/i18n/dictionaries/high-probability.ts` | High Probability frontend dictionary — 高概率研究页中文文案 |
| `packages/backend/apps/worker/src/worker/rewards/live_sync.rs` | Rewards live managed-order trade/status sync |
| `packages/backend/apps/worker/src/worker/rewards/account_sync.rs` | Rewards external balance, CLOB open-order snapshot/adoption, complete position snapshot sync, and detected-inventory original-price sell intents |
| `packages/backend/apps/worker/src/worker/rewards/live_orders.rs` | Rewards live cancel/fill, external-order cancel in-flight dedupe, and post-fill exit/flatten intents |
| `packages/backend/apps/worker/src/worker/rewards/live_submission.rs` | Rewards live single-order submit and submission markers |
| `packages/backend/apps/worker/src/worker/rewards/live_pending.rs` | Rewards durable intent submit/recovery workflow — BUY 提交前 last-look 只校验当前提交 token |
| `packages/backend/apps/worker/src/worker/rewards/live_orderbook_risk.rs` | Rewards live orderbook risk helpers — 新挂单 stale 余量、近期 BUY stale-only 撤单 grace、等待原因 |
| `packages/backend/apps/worker/src/worker/rewards/live_requote.rs` | Rewards live reprice guard — drift 稳定确认、订单冷却、单轮漂移撤单限速 |
| `packages/backend/apps/worker/src/worker/rewards/live_placement_limits.rs` | Rewards live placement funding/pre-provider precheck helpers — 同 condition BUY 预算、AI notional cap、最低 rewards size 缺口 |
| `packages/backend/apps/worker/src/worker/rewards/live_cancel.rs` | Rewards live cancel reason routing — 通用硬风控、depth/rank/history/requote 规则 |
| `packages/backend/apps/worker/src/worker/rewards/live_risk.rs` | Rewards live placement risk checks and shared risk helpers |
| `packages/backend/apps/worker/src/worker/rewards/orderbook_events.rs` | Rewards orderbook event consumer — 内部 WS、本地盘口 cache、HTTP bootstrap、活跃 token wake + cancel channel |
| `packages/backend/apps/worker/src/worker/rewards/event_cancel.rs` | Rewards orderbook event-driven hard-risk cancel worker — 独立消费活跃 token 更新并立即 cancel-only 撤单，使用统一 hard-risk/depth/rank/history gate |
| `packages/backend/apps/worker/src/worker/rewards/polling.rs` | Rewards live poll loop, book fetch, independent event-cancel worker wiring, fast reconcile, external sync throttling, 5-day history pruning, in-process book history, command wake subscription, background managed-orderbook-cache pre-warm task (`run_reward_managed_orderbook_cache_prewarm`) |
| `packages/backend/apps/worker/src/worker/database_maintenance.rs` | Worker database maintenance task — 定期执行数据库 retention 清理并输出逐表统计 |
| `packages/backend/apps/worker/src/worker/copytrade.rs` | Copytrade worker — wallet tracking, source trade detection, and queued analyze commands |
| `packages/backend/apps/worker/src/worker/smart_money.rs` | Smart Money Intelligence worker — 一次性/定时从 Polymarket Data API leaderboard 和 active copytrade tracked wallets seed 候选，再扫描候选钱包 activity/positions/closed positions/trades，写入画像、评分和源交易；随后读取 orderbook 服务缓存生成 deterministic observe/rejected 信号和 deterministic gate decision；开启 signal advisory 时构造 observe 信号 payload/input_hash、查缓存，并在 provider key 存在时保存三态 advisory；不执行 paper/live 交易 |
| `packages/backend/apps/worker/src/worker/smart_money/advisory.rs` | Smart Money signal advisory worker refresh — 选择近期 observe 信号，补齐源交易/profile/score 上下文，按 Smart Money 独立 provider/request-format/model 配置构造 advisory request/input_hash，统计缓存命中和待 provider 请求数量；有独立 provider key 时调用 `SmartSignalAdvisoryConnector` 保存 advisory 并记录 `llm_calls` |
| `packages/backend/apps/worker/src/worker/smart_money/profile.rs` | Smart Money wallet profile/trade helper — 从 Data API 近端样本构造钱包画像和去重源交易 |
| `packages/backend/apps/worker/src/worker/high_probability.rs` | High Probability Pricing worker — 导入本地 outcome JSON 标签，从本地 outcome 标签 + rewards candles 构建 first-touch 样本、刷新动态高概率策略 bucket stats、持久化 baseline backtest run/trade/退出规则摘要，并用 orderbook 服务缓存执行一次性只读 observe 扫描；不抓外部 API、不交易 |
| `packages/backend/api/src/handlers/copytrade.rs` | Copytrade API — reads snapshots/config and enqueues worker control commands |
| `packages/backend/api/src/handlers/smart_money.rs` | Smart Money API — foundation snapshot/config/candidate status；handler 只读写 SmartMoneyService，不抓 Polymarket/Data API/链上/LLM |
| `packages/backend/api/src/handlers/high_probability.rs` | High Probability Pricing API — research snapshot/config/bucket stats/report/backtests/backtest-runs；handler 只读写 HighProbabilityService，不抓外部 API、不执行交易 |
| `packages/backend/crates/application/src/copytrade/service.rs` | CopyTradeService — copytrade config, wallet tracking, source trade detection, and control command queue |
| `packages/backend/crates/application/src/smart_money.rs` + `smart_money/*` | SmartMoneyService/Store models — Smart Money config、候选钱包、候选状态更新、画像、确定性评分、源交易、确定性 signal gate、signal decision 审计、signal advisory 缓存、signal advisory request payload/input_hash builder、provider decision 模型、worker refresh 所需只读列表代理和 snapshot 基础模型 |
| `packages/backend/crates/application/src/high_probability.rs` + `high_probability/*` | HighProbabilityService/Store models — 动态高概率市场定价研究配置、样本、分桶统计、research report、baseline walk-forward backtest、基础退出规则对比、observe candidate/observation gate、持久化 backtest run/trade 和 observation 基础模型 |
| `packages/backend/crates/application/src/orderbook_cache.rs` | OrderbookCache trait and stream event models — `CachedOrderBook`、`OrderbookStreamEvent` |
| `packages/backend/crates/application/src/orderbook_registry.rs` | OrderbookSubscriptionRegistry trait — 多来源 token 订阅注册与来源统计 |
| `packages/backend/crates/infrastructure/src/stores/orderbook_cache.rs` | InMemoryOrderbookCache（TTL + 定期清理 + 每侧盘口深度裁剪）；保留 Redis 实现 |
| `packages/backend/crates/infrastructure/src/stores/orderbook_registry.rs` | InMemoryOrderbookSubscriptionRegistry — 来源有序 token 原子替换、确定性优先级聚合、来源与去重总数统计 |
| `packages/backend/crates/infrastructure/src/stores/helpers/reward_config.rs` | Rewards key-value config helper — `RewardBotConfig` 与 `reward_bot_config` 读写映射 |
| `packages/backend/crates/infrastructure/src/stores/rewards/postgres_market_methods.rs` | Rewards Postgres candidate query — 市场质量硬过滤、综合排序、row mapping |
| `packages/front/src/features/rewards/components/rewards-opportunity-config.tsx` | Rewards frontend unified opportunity metrics config panel |
| `packages/front/src/features/rewards/components/rewards-opportunity-summary.tsx` | Rewards frontend quote-plan opportunity metrics summary |
| `packages/front/src/features/copytrade/components/smart-money-signals-panel.tsx` | Smart Money frontend signal flow panel — 展示 observe/rejected 信号、源价格、当前价格、滑点和拒绝原因 |
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
| `packages/backend/migrations/0043_reward_low_competition_observations.sql` | Rewards legacy low-competition cross-cycle observation table |
| `packages/backend/migrations/0044_reward_market_candles.sql` | Rewards 5m price-history source candle table for AI advisory |
| `packages/backend/migrations/0045_reward_control_command_dedupe.sql` | Rewards control command pending/running dedupe partial unique indexes |
| `packages/backend/migrations/0046_reward_low_competition_competition_share.sql` | Rewards low-competition competition share and allocation observation fields |
| `packages/backend/migrations/0047_reward_low_competition_not_low_competition.sql` | Rewards low-competition `not_low_competition` early-exclusion label |
| `packages/backend/migrations/0048_reward_account_unmanaged_buy_notional.sql` | Rewards account snapshot-frozen unmanaged (non-managed) external buy notional consumed by funding precheck |
| `packages/backend/migrations/0049_smart_money_intelligence.sql` | Smart Money Intelligence foundation schema — config、candidate/profile/score/trade/signal/advisory/paper tables |
| `packages/backend/migrations/0050_high_probability_pricing_strategy.sql` | High Probability Pricing foundation schema — config、samples、bucket stats、observations |
| `packages/backend/migrations/0051_high_probability_market_outcomes.sql` | High Probability Pricing market outcome label schema |
| `packages/backend/migrations/0052_high_probability_backtests.sql` | High Probability Pricing baseline backtest persistence schema — backtest runs/trades |
| `packages/backend/migrations/0053_high_probability_backtest_exit_rules.sql` | High Probability Pricing baseline backtest exit-rule report schema — backtest run `exit_rule_reports` |
| `packages/backend/migrations/0054_reward_market_event_windows.sql` | Rewards event-window source candidate schema — `reward_market_event_windows` |
| `packages/backend/init.sql` | Complete empty-database initialization script generated from migrations 0001–0054 |

## 仓库结构

- `doc/`：系统设计、API 契约、鉴权、存储、前后端计划等文档。
- `packages/front/`：`Next.js 16 + React 19 + Tailwind v4 + shadcn/ui` 控制台前端。前端代码规范（目录结构、数据层、文件行数上限、公共代码提取）见 [packages/front/AGENTS.md](./packages/front/AGENTS.md)，写或改前端代码前必须遵守。
- `packages/backend/`：Rust 后端根目录；包含 `Cargo.toml`、`Cargo.lock`、`rust-toolchain.toml`、`api`、`order`、`worker / replay` apps、共享 crates、迁移和初始化 SQL。`api` 是 `polyedge-api` 服务 crate，`order` 是 `polyedge-orderbook` 服务 crate，共享 crates 包含 `application / common / connectors / contracts / domain / infrastructure`。后端代码规范（分层架构、`include!` 模块化、文件行数上限、公共代码提取、测试组织）见 [packages/backend/AGENTS.md](./packages/backend/AGENTS.md)，写或改后端 Rust 代码前必须遵守。
- `deploy/`：Docker Compose 部署模板和环境变量示例；当前 Compose 服务为 `polyedge-api`（内嵌 worker runtime）、`polyedge-orderbook` 和 `polyedge-front`。
- `scripts/`：构建、部署、冒烟脚本。
- `bin/`：部署镜像复制的预构建后端二进制。

## 当前状态

- 仓库已经不是纯文档仓库：前端控制台、Rust API、worker、迁移、配置和 Docker 部署入口都已具备。
- 前端控制台已有 `dashboard / markets / events / rewards / funding / high-probability / copy-trading / wallet-analysis / settings` 页面；`radar / signals / positions / risk` 旧入口已移除，`/replay` 和未落地的 approvals 页面不再作为前端入口暴露。
- 前端控制台导航在桌面端使用左侧折叠 sidebar，在移动端通过顶栏菜单按钮打开左侧抽屉；两者共享同一份导航项配置，并使用原生链接跳转以适配静态导出部署。
- 前端数据层统一走 `src/lib/api/*`（读取按领域文件 `markets.ts` / `rewards.ts` / `funding.ts` / `high-probability.ts`… 基于 `base.ts`，写操作通过 `actions.ts` barrel 暴露、实现按领域拆在 `actions/`），页面装配在 `src/features/*/loaders` 和 `src/features/*/components`。`src/server/` 目前是空目录（历史遗留）。
- 前端仅支持中文，文案走 `@/lib/i18n/dictionaries` 字典导入。
- 前端不再提供 mock 数据模式；`NEXT_PUBLIC_POLYEDGE_API_BASE_URL` 必须指向 Rust 后端，读写都走真实 `/api/v1/...`。
- 当前控制台会话只保留 `off`，不是生产级真实会话。
- 默认生产排查环境：Frontend Rewards 工作台 `http://192.168.31.5:33002/rewards`，API 服务 `http://100.87.45.72:38001`，Orderbook 服务 `http://100.87.45.72:38002`；除非用户明确指定其他环境，后续线上问题排查默认使用这组地址。前端静态构建的 `NEXT_PUBLIC_POLYEDGE_API_BASE_URL` 应指向该 API 地址。
- 后端 API 已覆盖 markets、events、news、evidences、orders、trades、pricing、rewards bot、funding、copytrade、smart money foundation、high probability pricing research foundation、wallet analysis、system、connector callback 和 orderbook（`GET /api/v1/orderbook/{token_id}`）等主路径；`/api/v1/signals`、`/api/v1/positions`、`/api/v1/risk/*` 和 `/api/v1/arbitrage/*` 旧控制台 API 已移除。
- Rewards snapshot 包含 `llm_usage`，按 UTC 日展示 AI advisory 与 info-risk 的实际外部 provider 调用次数、总次数和失败次数；前端 `/rewards` 顶部执行概览展示今日总调用和最近 7 天明细，统计不包含 provider 缓存命中。Rewards AI advisory、Rewards info-risk 和 Smart Money signal advisory 的实际外部大模型 HTTP 调用在 connectors 层共享单进程全局 semaphore，同一进程内任意时刻最多 1 个大模型请求在飞。
- Rewards AI advisory provider schema 当前要求 `allow_quote` 二值判断加 `strategy_hint`（方向、bid rank、condition notional cap）。当 `ai_strategy_hint_enabled=true`（默认）且置信度达到 `ai_strategy_hint_min_confidence=0.75` 时，live tick 会直接按 hint 收窄报价方向、把挂单挡位调得更保守或压低同 condition 新增 BUY 预算；hint 存在 `reward_market_advisories.metrics_json.strategy_hint`，不会新增 DB 列，也不能绕过市场质量、盘口、资金、库存、kill switch 或统一机会评分风控。
- Rewards 统一机会评分默认启用，当前基线为 10U 探针、100U 日奖最低 0.75、竞争倍数上限 4、账户/单市场占用警告 1500/500 bps、退出深度至少 60U 或计划名义额 2.5 倍、入场退出滑点 2c、坏成交恢复 3 天、30 分钟观察窗口至少 30 个盘口样本、中点波动 3c、top-of-book 跳变 8 次，评分权重为 reward/competition/exit/stability = 35/30/25/10。
- GLM `https://open.bigmodel.cn/api/coding/paas/v4` + `glm-4.7` 和 DeepSeek `https://api.deepseek.com` + `deepseek-v4-flash` 已用测试 key 对当前 Chat Completions 请求形态做过鉴权 smoke test（均 HTTP 200，返回可解析 JSON object 字符串）；测试 key 不写入仓库，正式运行仍只通过 worker 环境变量注入。
- 后端默认 tracing filter 在未设置 `RUST_LOG` 时包含 `polyedge_worker=info`，因此 `polyedge-api` 内嵌 worker runtime 的 info/warn 日志会出现在 API 服务日志中；显式设置 `RUST_LOG` 会覆盖默认 filter。
- 新闻采集当前支持 RSS/Atom XML feed；未配置 `POLYEDGE_NEWS__SOURCES_JSON` 时，代码默认新闻源为 `fed_press`、`sec_press`、`nasa_news`、`bbc_world`、`npr_news`、`coindesk`、`cointelegraph`、`decrypt`；部署模板 `deploy/.env.api.example` 也显式写入同一默认源列表，环境变量或 runtime config 可覆盖整个 sources 列表。
- `polyedge-worker` 支持 database maintenance、news ingest、news promotion、rewards bot live 策略、copytrade 钱包跟踪/分析、smart money 一次性/定时画像扫描、execution drain、paper reconciliation、Polymarket order/fill/user-event、orderbook token 注册任务；旧 signal recompute 和 arbitrage radar 循环已移除。市场同步和 orderbook 订阅已迁移到独立 `polyedge-orderbook` 服务；orderbook 服务启动时先暴露 HTTP `/healthz`，再后台执行独立的 Gamma full sync、Gamma priority sync、rewards catalog sync 与 rewards candle history sync 循环，避免外部 Polymarket API 延迟阻塞容器健康检查，也避免较慢的 rewards 详情补全阻塞 Gamma `markets.synced_at` 刷新；Gamma full sync 使用 `/markets` offset 分页并按 market id 去重，写入时跳过同版本同内容行，并只在 `synced_at` 超过 rewards 新鲜度窗口约三分之二时刷新安静市场；Gamma full/priority 写入 `markets` 时在 orderbook 进程内串行化，并通过 Postgres `lock_timeout`/`statement_timeout` 避免长时间锁等待堆积；Gamma full/priority sync 会从 market/event 日期和 `hasReviewedDates` 为 rewards condition 派生 `reward_market_event_windows` 候选，默认忽略未审核 Gamma 日期，已审核 Gamma 日期保存为 medium confidence 候选且默认不会达到 high hard-gate 阈值；Gamma priority sync 会强制刷新已注册 token 映射到的 condition、活跃 rewards 订单/持仓、最终 eligible 或 pre-AI deterministic eligible quote plans 和放宽新鲜度后的 rewards 候选 condition，并用 active rewards catalog 的高奖励市场补足剩余 priority 额度作为恢复种子，最多 500 个 condition，刷新间隔由 rewards `max_market_data_age_minutes` 动态推导（约为窗口三分之一，30-300 秒）；Gamma 单次 full sync 有 60-240 秒超时，priority sync 最长 120 秒超时，rewards 单次同步有 45 分钟超时，rewards 空目录或详情补全后仍不完整时保留上一版目录；reward catalog upsert 先写入当前快照、再只停用缺失 active 市场，避免每轮全量 active=false/true 写放大；candle history sync 默认每 300 秒最多处理 600 个 active reward token，按 token 至少间隔 500ms 请求 CLOB `/prices-history`，首次 backfill 2 小时、后续增量 15 分钟，遇到 429/认证错误/超时/常见 5xx/解码失败会停止本轮以避免继续压外部 API；orderbook WS + poll stream 遵守 `POLYEDGE_ORDERBOOK_STREAM__RESTART_INTERVAL_SECS`，按 `POLYEDGE_ORDERBOOK_STREAM__WS_CHUNK_SIZE` 分片消费 `book` + `price_change`（默认 100 token/连接），chunk 内 SDK stream reader 与缓存写入解耦以减少 broadcast lag，chunk 退出或被取消时会成对释放 SDK market subscription，registry 变更会实时唤醒 token refresh，但仅在订阅 token 成员真实增删并经过短暂 debounce 后仍变化时，默认增量模式（`WS_INCREMENTAL_RECONCILE=true`）对 diff 做 subscribe/unsubscribe 并保持 WS 连接存活、不整体重建，registry 聚合顺序抖动不会触发重订/重连；默认每 10 秒对全部注册 token 做批量快照恢复，poll 写入保留 CLOB timestamp 为 `observed_at`、用本地接收时间刷新 `confirmed_at` freshness，内部写接口要求 `POLYEDGE_ORDERBOOK__WRITE_TOKEN`，缓存统一排序后裁剪最优档位并拒绝旧盘口内容覆盖；Gamma、CLOB rewards、order book 和 price-history 解码失败会在错误中携带最多 300 字节的转义响应体 preview，便于区分 HTML/截断/结构漂移响应；`/orderbook/stream` 会把 WS、poll 和 ingest 的规范化缓存更新推送给内部消费者，`/orderbook/stream?source=...` 可按 registry source 过滤返回事件（底层 Polymarket WS 仍按聚合 token 统一订阅）。
- Rewards bot 仅支持 `live` 实盘模式（`execution_mode` 字段已移除，旧配置键读取时忽略）。它只使用 `reward_markets` 表作为奖励市场来源，并关联 `markets` 表硬过滤非 open/tradable、高歧义、Gamma liquidity 与 24h 成交量两个活跃度代理均低于阈值、临近结算、Gamma 价差过宽、同步数据过期或异常超前以及 FDV/launch/token/official-result 等高事件跳变风险市场；若只配置 liquidity 或 volume 单项阈值，则只检查该项，两项都为 0 时关闭该活跃度预筛；候选按奖励、流动性、成交量、剩余时长和 rewards spread（CLOB 原始单位即 cents）的综合质量分优先，命中 `preferred_categories` 的 Gamma 分类只增加排序分，不绕过硬过滤；随后统一 `opportunity_metrics` 会把竞争资金、奖励密度、退出能力、盘口稳定性和资金占用纳入 quote plan score 与 `min_market_score` 资格判断。只有唯一且明确的 YES/NO token 才进入盘口订阅和规划。长期 rewards poll loop 通过 `OrderbookStreamClient` 消费 orderbook 服务内部 `/orderbook/stream`，维护 worker 本地盘口 cache；启动、重连、缺失 token、本地盘口超过 `stale_book_ms` 或接近新挂单 freshness headroom 时用 `OrderbookHttpClient` batch API bootstrap/refresh，full tick 读取候选和活跃订单/持仓盘口，并在 AI/info-risk cache gate、订单/账户同步和 live action 盘口刷新后先 materialize quote readiness，再统一保存最终 quote plan 快照；`rewards_eligible` source 由周期注册任务统一注册全部最终 eligible quote plan token，AI/info-risk gate 前已 deterministic eligible 且保存在 `orderbook_token_ids` 的 token 改由 `rewards_ai_provider` 临时 source 按最多 10 个市场一批订阅，批次切换会取消上一批；provider 返回允许挂单 advisory 后即时合并到 `rewards_eligible`，因此 `reward_candidate_token_cap=0` 只会关闭候选预热，不会阻止最终 eligible 市场或 AI provider 临时批次按需订阅盘口；周期注册任务对空集合做防抖，active/exec 连续 2 轮、eligible/candidates 连续 3 轮成功为空才清远端 source；新买单 intent 持久化后会立即刷新 `rewards_active` source，避免刚落库实盘单等待下一次周期注册，若即时刷新读到空集合则保留上一版 source 等周期任务确认；活跃 token 盘口更新会直接投递到独立 `RewardEventCancelGuard` hard-risk cancel worker，并仍唤醒普通 fast reconcile，`reconcile_interval_sec` 和 `POLYEDGE_REWARDS__POLL_INTERVAL_SECS` 仍作为兜底。`RewardOrderbookRuntime` 额外 spawn 一个独立后台 task（`run_reward_managed_orderbook_cache_prewarm`，默认每 5 秒），用 `refresh_reward_managed_orderbook_cache` 对活跃订单/持仓 + eligible quote plan + 候选 token 做本地盘口 cache 预热（复用 `fetch_cached_reward_books`，仅 HTTP 拉取本地 age 接近 placement 新鲜度阈值的 token），让盘口少有变化的安静市场在两次 full tick 之间也保持新鲜；该 task 独立于 poll loop、不阻塞 fast reconcile、不持有 advisory lease，poll loop 结束时随 runtime drop 一起 abort。worker 默认生成 YES/NO post-only 双边买单计划；`rewards_min_size` 是份额数量要求，报价腿会先对齐到 CLOB 成本精度，并满足 Polymarket 1 美元最小名义金额，避免提交缩量后失去奖励资格或被 venue 最小名义金额拒单。新报价价格由 `quote_bid_rank=1|2|3` 明确选择买一/买二/买三（按不同买价计档，默认买一），但 quote plan 构建阶段不再因为目标档位缺失、目标价超出 rewards spread、auto 单边盘口指标、实际盘口价格预算、`per_market_usd` 或 `quote_size_usd` 而淘汰市场；准备挂单时才用当前 orderbook materialize 真实报价腿并验证目标档位、rewards spread、touch ask、安全边际、盘口集中度/退出深度和实际 size/notional，随后由 live placement 用实际钱包余额做同 condition 准入。live placement 缺少、空、过期或接近 stale 边界的盘口时不下单、不写 12 小时 skip，而是先对本地接近新挂单 freshness headroom 的 token 做 orderbook HTTP batch refresh，再保持 quote plan eligible 并等待 orderbook 订阅/缓存返回；配置为 `quote_mode=auto` + `selection_mode=enforce` 且启用 dominant single-side 时，双边目标档位、rewards spread、touch ask 或安全边际不通过会先尝试通过同一校验的单腿；没有可行单腿或其他 live 盘口验证不通过时才不下单，并把 quote plan 标记 `live_skip_until` / `live_skip_reason`，标记默认 12 小时后失效以便奖励范围或盘口变化后重新评估；开放订单目标价相对最新目标档位漂移超过 `requote_drift_cents` 时只进入受 `requote_drift_confirm_sec` 历史同向确认、`requote_drift_cooldown_sec` 订单冷却和 `requote_drift_max_cancels_per_cycle` 单轮限速保护的换价撤单候选，避免盘口档位抖动导致全量撤空后再重挂；旧 `quote_edge_cents` 配置键读取时忽略。`quote_mode=double` + `selection_mode=observe` 是默认行为；配置为 `quote_mode=auto` + `selection_mode=enforce` 且启用 dominant single-side 后，planner 只根据一边倒概率区间生成初步 `double` / `single_yes` / `single_no` / `none` 计划，退出深度、top1/top3 深度占比、HHI 以及双边不可行时的单腿回退都在 live materializer 中使用当前盘口验证。`observe` 只在 quote plan 记录推荐模式和 `book_metrics`。AI advisory 可选启用：live tick 只读取 `reward_market_advisories` 缓存并 fail closed，不等待外部 provider；worker 在 full tick 后启动独立 AI advisory provider refresh 和 info-risk provider refresh，两条队列各自保留开放订单/持仓最高优先级，其余按统一 standard 候选顺序处理；AI refresh 用 `rewards_ai_provider` 临时 orderbook source 按最多 10 个市场一批获取 AI 所需盘口，下一批取消上一批；`ai_advisory_batch_size` / `info_risk_batch_size` 分别控制对应 refresh 单次 provider 请求包含的市场数（默认 1，最大 12），批量响应按 condition 拆分保存，漏项、错配或整体解析失败会回退单市场，provider 过载则停止对应 task 本轮剩余请求；缓存未命中时分别通过 `RewardAiAdvisoryConnector` / `RewardInfoRiskConnector` 调用 OpenAI Responses、OpenAI-compatible Chat Completions（含 GLM/DeepSeek 模型名特例）或 Anthropic Messages 并写入缓存，供后续 tick 使用；AI 开启后新增挂单必须先通过 provider 过滤；缺少未过期 advisory、provider 配置缺失、模型为空或请求失败仍会把原本 eligible 的计划改为不可挂并覆盖保存 quote plan 快照；新 provider 输出 `allow_quote=true|false` 二值和 conservative `strategy_hint`，二值不允许会硬拦，二值允许保留 deterministic 计划继续进入 live 盘口、资金和订单风控；旧 `suitability` 响应按二值 fail-closed 兼容：仅 `allow` 视为允许并保留 deterministic 计划，`watch`/`avoid`/其它非 allow 值一律映射为 `avoid` 硬拦（旧 watch 放行行为已移除）；置信度达标的 `allow` 会直接应用 `strategy_hint`：方向只能收窄或跳过，bid rank 只能更保守，同 condition 新增 BUY 预算会被 cap 压低；低置信度 `allow` 在标准 sleeve 仍继续进入 live 盘口/资金风控；provider confidence 会在 connector 解析时钳制到 `0..=1`。高置信度 `allow` 决策可在 `selection_mode=enforce` 且 `quote_mode=auto` 时把已 eligible 的 auto 双边计划收窄为单腿，但不会绕过市场质量、盘口和风控硬过滤。信息风险可选启用：AI advisory 启用时由 full tick 的专用 info-risk provider refresh 推进，独立 info-risk worker 不再连续请求全量 provider；AI advisory 未启用时，独立 worker 任务仍按开放订单、持仓、eligible quote plan、候选市场顺序，用 active reward market / quote plan / account payload 构建 query/input hash，先读写 `reward_market_info_risks` 缓存，缓存未命中时通过 `RewardInfoRiskConnector` 调用 OpenAI-compatible/Anthropic；OpenAI Responses 可选启用 web search tool，provider confidence 同样会钳制到 `0..=1`。live tick 只读取缓存，不等待外部搜索；`info_risk_mode=enforce` 时缺少未过期风险缓存会 fail closed，已有 `critical`、官方结果、`resolution_imminent=true` 或配置为 `low/medium` 避免等级时命中的风险会在置信度达到环境变量阈值后把计划置为不可挂；普通 `high` 风险以及仅 `risk_type=imminent_resolution` 但 `resolution_imminent=false` 的结果只保留为信息提示，继续走 live 盘口、资金和订单风控。worker 使用 `LivePolymarketConnector` 提交 post-only GTC token 买单、post-only maker SELL 普通退出和非 post-only FAK/taker flatten，并撤销本系统托管订单；rewards poll loop 全程持有 Postgres advisory lease，只有 leader 维护 5 秒 CLOB heartbeat id 链并执行命令/tick/reconcile，单次 heartbeat 请求 4 秒超时。新建 quote intent 与已落库待提交 BUY 在提交前都会复用 live 撤单风控（计划仍 eligible、报价漂移、min depth、bid rank、depth drop、fill velocity、mass cancel、best ask touch、kill switch 等），并在真正 POST 前用 orderbook 服务做 1 秒 max-age last-look；风险不通过、盘口缺失或刷新失败的本地 intent 不会提交；已有 external order id 的近期 BUY 只在单纯 stale 盘口风险下短暂延迟撤单，缺盘口/空盘口和资格、深度、kill switch 等硬风险仍立即撤单；价格漂移只在 reprice guard 确认后按单轮上限撤单。confirmed fill 按 external trade id + external order id 幂等入账，买入 fill 与退出 intent 同事务落库；退出 floor 用 intent price 与当前持仓 `avg_price` 的较高值，提交前不使用 midpoint 或页面“当前价”降价；`ExitAtMarkup`、`HoldAndRequote` 和外部库存补退出按 floor 提交 post-only maker SELL，当前买一穿过 floor 时按 30 秒退避等待可 resting 的盘口；`FlattenImmediately` 只有在 best bid 不低于 floor 时用非 post-only FAK/taker SELL 按 best bid 尝试非亏损平仓，best bid 缺失或低于 floor 时保留 deferred exit 并按 30 秒退避；提交前会按当前 token 持仓裁剪 size，无持仓 stale exit 会关闭；明确 post-only 退出拒单使用有界退避并在达到最大拒绝次数后停止盲目重试；提交前低于 Polymarket 1 美元最小名义金额的退出单会进入短 reason 的 dust deferred 状态，每 300 秒重新评估但不重复拼接历史原因。单订单查询返回 404 时，worker 会按 token 和下单时间窗口查询认证账户 trades，并按 external order id 精确补账，不会把 404 直接标记为 cancelled；仍无法确认时保持 critical 对账锁，暂停新增买单但继续同步、撤单和卖出退出，后续成功查询会自动解除锁；若该 404 锁超过 5 分钟且仍没有 CLOB/Data API 成交证据，worker 会把本地订单标记为 cancelled 以释放开放挂单计数。提交结果未知订单现在会在恢复查询确认 CLOB 无对应 open 订单并经过 `LIVE_SUBMISSION_UNKNOWN_CLOSE_AFTER_SECS`（默认 600 秒）宽限后自动本地关闭以释放对账锁（与 404 锁一致），但若 positions 快照显示该 BUY token 在提交后出现库存则继续保留对账锁；取消结果未知订单仍不会仅因本地超时释放对账锁。所有 live 撤单请求共享进程内 in-flight 去重，同一 external order id 在一次 `cancel_order()` 未返回前不会被事件撤单 worker 和普通 reconcile 重复发送。每轮还会读取 CLOB open orders snapshot：普通已提交 open-like BUY 若不在外部开放订单列表且无提交未知、404、pending cancel、post-only violation 等对账锁，会本地标记为 cancelled 释放开放挂单计数；该反查和账户开放 buy notional 观测不受 confirmed fill 保护期影响。成交后 sibling cancel 只撤同 condition 对侧 buy，不撤 sell exit；同 token 存在未完成卖出退出时暂停新增买单。full tick 和 fast reconcile 会先同步 managed orders；本轮有新增 confirmed fill，或数据库最新 confirmed fill 距今不足 120 秒时，只保留本地 balance/positions，等待 CLOB/Data API 最终一致性追平后再同步完整外部账户快照。外部账户同步的资金钱包地址优先使用 `FUNDER`，未配置时使用 `ACCOUNT_ID`；CLOB balance 为 0 或失败但链上 pUSD 余额大于 0 时，worker 用链上 pUSD 回填账户 snapshot，并清零遗留 `reserved_usd`。成功 positions 快照原子替换该账户全部持仓，失败时保留上一版。即使 `enabled=false` 且没有开放订单，worker 仍会尝试刷新外部账户状态。worker 按账户写入数据库 heartbeat，API snapshot 仅在配置启用且 heartbeat 不超过 2 分钟时返回 `running=true`；`status.error` 只由当前开放订单的活跃对账锁推导，不会把短暂 `awaiting final reconciliation` 当作错误，也不会被历史 critical event 永久污染。API 不直接请求 Polymarket，`orders` 与 `orders_page` 都描述本地 managed orders。rewards snapshot 额外携带 `token_quotes`（按 `token_id` 索引的 best_bid/best_ask/mark_price 侧表）：每个返回 snapshot 的 rewards API handler 在响应前 best-effort 用 `state.orderbook_cache`（`OrderbookHttpClient` → orderbook 服务 `POST /orderbook/batch`）批量读取当前页 positions/orders 的 token 盘口注入该字段，供前端库存/订单表展示买一/卖一和持仓盈亏金额/百分比；orderbook 服务不可用、请求失败或某 token 无盘口时不阻断 snapshot，对应 token 缺失或字段为空，前端显示 `—`，持仓盈亏由前端按 `(mark-avg)*size + realized` 推导。`RewardBotService` 内部缓存 config、account、positions、最新 200 条 events、最新 200 条 fills、open_order_count 和 heartbeat，API 与内嵌 worker runtime 共享实例时直接从内存读取这些热状态，缓存为空时回退数据库；控制命令入队通过 in-process command_wake channel 立即唤醒 worker poll loop。账户范围外开放订单明细和奖励结算对账仍是缺口。
- Rewards quote plan snapshot 会持久化 `pre_ai_eligible` 和 `orderbook_token_ids`；AI/info-risk gate 前的 deterministic eligible token 不再长期纳入 `rewards_eligible` source，而是由后台 AI advisory refresh 使用 `rewards_ai_provider` 临时 source 按最多 10 个市场一批订阅盘口，批次切换会取消上一批；full tick 会在启动 provider refresh 前先对无开放订单/持仓的新 condition 执行 live funding precheck、标记 `pre_ai_eligible` 并应用已有 AI/info-risk cache gate，当前资金放不下最低 rewards size 待补腿的计划先写 funding reason 并退出普通 AI/info-risk 候选队列，已有未过期且未到刷新窗口的 provider 缓存命中不会反复占用本轮 refresh 名额；AI 与 info-risk refresh 队列各自保留开放订单/持仓最高优先级，其余按统一 standard 候选顺序处理；provider 返回允许挂单 advisory 后再即时合并到 `rewards_eligible` source，最终下单仍由 live 盘口、资金和订单风控决定。
- Rewards quote plan 额外持久化 `quote_readiness=ready_to_quote|waiting_orderbook|provider_pending|blocked`，把“策略 eligible、应继续订阅”的候选语义与“已有真实报价腿、可立即进入 live 下单检查”的可报价语义拆开；API snapshot 的 `status.ready_quote_markets` 只统计 `ready_to_quote`，同时返回 `waiting_orderbook_markets`、`provider_pending_markets` 和 `blocker_counts`，供前端解释可挂市场与实际可报价数量差异以及主要拦截原因。缺盘口/过期盘口的计划仍可保持 `eligible=true` 以进入 `rewards_eligible` 订阅；前端顶部把 `ready_quote_markets` 显示为“实时可报价”、把 `eligible_markets` 显示为“最终可挂”，关键指标条单独展示候选计划总量、已拦截计划、等待 provider、资金不足、live 盘口验证和 AI/信息风控拦截数量，避免盘口 freshness、provider gate、资金约束或 live validation 抖动被误读成 reward 市场池大幅变化。
- Rewards 首次买入报价现在有可配置入场缓冲：当 `info_risk_enabled=true` 且 `info_risk_mode=enforce` 时，`require_info_risk_before_first_quote=true` 会要求新 condition 先命中未过期信息风险缓存，`first_quote_quarantine_sec` 会要求新 condition 至少观察指定秒数（默认 600）后才允许首次 live BUY；已有 open-like 订单或持仓的 condition 跳过该首单 gate，以便继续撤单、退出和管理库存。planner 的静态事件风险关键词也扩展到 appointed/confirmed/certified、drop out/withdraw/resign/step down/removed 以及 scheduled/deadline/market-close 类市场。
- Rewards full tick 在 AI/info-risk gate 完成后会用本轮内存 quote plan 立即替换 `rewards_eligible` orderbook source，周期注册任务仍作为兜底；这样新 eligible token 不必等下一次注册周期，后续订单/账户同步期间 orderbook 服务即可开始 WS/poll 预热，最终快照仍在 live action 盘口刷新并 materialize `quote_readiness` 后保存。
- Rewards 成交后退出策略由 `post_fill_strategy` 配置决定，不再由 AI advisory 的 `exit_policy` 覆盖；`ExitAtMarkup` 以被吃买单原价加 `exit_markup_cents` 为卖价基准并向上取整到 0.01 tick，默认加价为 0；页面“持有并续挂”对应 `hold_and_requote`，也会按被吃买单原价持久化 SELL 退出 floor，之后继续正常报价。`ExitAtMarkup`、`HoldAndRequote` 和外部库存补退出按 floor 提交 post-only maker SELL；`FlattenImmediately` 在 best bid 不低于 floor 时走非 post-only FAK/taker SELL，否则按 30 秒退避等待非亏损 bid。SELL 提交前会按当前 token 持仓裁剪 size，无持仓 stale exit 会关闭，不使用 midpoint 或页面“当前价”降价。
- Rewards 外部 positions 快照检测到已有库存且没有同 token open-like SELL 时，会按持仓 `avg_price` 向上对齐 tick 后补原价 SELL intent；后续按 post-only maker 规则挂出，避免已有库存无人接管退出且避免用低于持仓均价的买一价或 midpoint 卖出。
- Rewards `quote_bid_rank` 对细 tick 盘口不是纯第 N 个 0.001 价位：上条所称买二/买三在细 tick 下会从买一回退 `rank-1` 个 0.01 价格步长，再选择不高于目标价的当前买盘档位；粗 tick 盘口仍按不同买价的买一/买二/买三选择。
- Rewards CLOB open-order snapshot 会先收养未归属但 token 可唯一映射到 active reward market 的开放 BUY 为 managed order；如果同 external id 的本地 BUY 已被关成非 open，但 CLOB 仍 open，会重开原本 managed order。SELL、非 rewards 市场和无法唯一映射 token 的外部开放订单明细，以及奖励结算对账仍是缺口。
- Rewards 低竞争市场 sleeve 已合并为统一机会评分：不再有独立 `standard` / `low_competition` candidate profile、`rewards_low_competition_probe` source、observe/enforce 模式、shadow report、低竞争 open-order 占比 cap、专用 last-look 或专用撤单 gate。`low_competition_*` 配置、`strategy_bucket=low_competition`、`low_competition_metrics` 和 `reward_low_competition_observations` 仅保留历史/API/DB 兼容；配置归一化强制 low-competition mode/off、独立市场/订单/全局占比为 0，前端保存时也会关闭旧字段。当前所有奖励市场统一进入 standard 候选流，并由 `opportunity_metrics` 综合竞争倍数、100U 日奖、账户/单市场资金占比、退出深度/滑点、坏成交恢复天数、盘口样本/波动/跳变和组件权重，通过 `score_adjustment` 影响 quote plan score 与 `min_market_score` 资格判断；full tick 会在 provider gate 前先应用机会评分影响候选资格，订单/账户同步与 live action 盘口刷新后再刷新指标，后置刷新只允许降级或更新展示、不把已被 provider/资金/盘口 gate 阻塞的计划重新放行；前端展示机会分、竞争倍数、100U 日奖、退出深度、样本数和警告数量。
- Rewards 成交对账除 404 fallback 外，也会在关联 trade 按 ID 查询失败时按 token/time 扫描认证账户 trades 并按 external order id 精确匹配；认证 CLOB 明确返回 matched size、但 trade 响应仍无法解码时，worker 仅在 Data API 钱包交易的 token/BUY/price/time/累计 size 与唯一 managed order 全部严格匹配后补账。若外部账户和持仓快照已覆盖该成交，只补订单、fill 和退出 intent，不重复扣现金或叠加持仓。任何单笔订单的全部回退失败都只隔离当前订单，不再阻断其余订单对账、账户持仓同步或 stale 清理。
- Rewards fast reconcile 的重型外部同步受独立节流保护；如上一条状态快照描述 fast reconcile 会同步订单/账户，实际执行时托管订单状态、CLOB open-order snapshot、managed scoring、账户级 rewards earnings 和 balance/positions snapshot 分别按最小间隔执行，不会因活跃盘口事件每秒全量打外部 API。
- Rewards AI advisory / info-risk provider refresh 受 `POLYEDGE_REWARDS__INFO_RISK_MAX_MARKETS_PER_CYCLE` 控制（默认 50，0 表示本轮不发 provider 请求）；AI advisory 按缓存过滤后的实际 provider request 数消耗该上限，batch 失败 fallback single 和漏项/错配 fallback single 也消耗同一上限，缓存命中不占名额，info-risk 仍按 selected condition cap 截断。full tick 会先执行 live funding precheck，再记录 AI 过滤前的 deterministic eligible condition 集合，新一轮 quote plan 构建时只继承上一版未过期且 provider/request_format/model 匹配的 advisory，不会因缺少 advisory 在 prepare 阶段提前 fail closed；live tick 只读取已有 advisory cache 并立即 gate，仍缺少 advisory、provider 配置缺失、模型为空或请求失败的原本 eligible 计划会 fail closed；新 provider 输出 `allow_quote=true|false` 二值和 conservative `strategy_hint`，二值不允许会硬拦，二值允许保留 deterministic 计划继续进入 live 盘口、资金和订单风控；旧 `suitability` 响应按二值 fail-closed 兼容：仅 `allow` 视为允许，`watch`/`avoid`/其它非 allow 值一律映射为 `avoid` 硬拦（旧 watch 放行行为已移除），置信度达标的 `allow` 会直接应用 `strategy_hint`：方向只能收窄或跳过，bid rank 只能更保守，同 condition 新增 BUY 预算会被 cap 压低；quote plan 快照只在 AI/info-risk gate 全部完成后统一保存。full tick 后台 provider refresh 拆成 AI advisory 与 info-risk 两个独立 Tokio task，分别用进程内 `AtomicBool` 保证各自单实例；每个后台 task 会按 rewards poll interval 派生整体 wall-clock timeout（默认 60 秒 poll 时最多 120 秒，范围 30-120 秒），超时会停止本轮 refresh、释放单实例锁，AI task 还会清空 `rewards_ai_provider` 临时 source，避免慢 provider 或批量请求超时让后续 full tick 长期只看到 `another AI refresh is running`；AI refresh 使用 live gate 前 deterministic quote plan 构建稳定 `input_hash`，避免缺缓存 fail-closed 后的 `quote_mode=None` 和空 legs 污染 provider 缓存键；两条队列各自保留开放订单、持仓最高优先级，其余市场按统一 standard 候选顺序处理，AI 与 info-risk 互不共享 selected condition 队列，避免 info-risk 候选挤占 AI 名额；AI task 用 `rewards_ai_provider` 临时 orderbook source 按最多 10 个市场一批获取 AI 所需盘口，下一批取消上一批，结束后清空；每个临时盘口批次内按稳定 cache-key payload 构建 AI `input_hash` 并查询缓存，缓存未命中且该市场所有报价 token 盘口都已发布（非空 bids 与 asks）时才请求 provider；请求 payload 包含账户、订单、持仓、当前 orderbook top levels、`pricing_context` 当前盘口定价合理性摘要和 `provider_cache_policy` TTL 策略，以及由最多 24 小时 5m price-history source candles 聚合出的最多 24 根 1h candles 和摘要，但 cache key 只纳入市场身份/问题、奖励参数、计划 quote mode、相关策略配置和已完成小时级 candle summary，不纳入账户/订单/持仓、即时盘口档位或当前小时内 5m source 更新。新保存的 AI advisory / info-risk provider cache 会按 condition/provider/request_format/model/input hash 等稳定键增加最多 TTL 20% 且最多 15 分钟的确定性正向 jitter；AI provider refresh、info-risk provider refresh、事件驱动 AI 批量通道和独立 info-risk worker 在缓存仍未过期但进入较小提前刷新窗口（`min(TTL/20, 60s)`）时会继续使用旧缓存并发起续期，避免大批记录同一秒失效造成 provider pending 集中爆发，同时避免 1 小时 TTL 在过期前 12 分钟就滚动重打；未进入提前刷新窗口的缓存命中不会占用本轮 provider 请求名额。`ai_advisory_batch_size` 控制 AI advisory 批量请求市场数，批量响应按 condition 拆分保存，漏项/错配回退单市场；盘口缺失/为空的市场本轮跳过 AI advisory 请求且不写缓存，等 orderbook 订阅/缓存返回盘口后再评估，避免在缓存键不含即时盘口的设计下被一条空盘口不允许挂单结果长期卡住整个 TTL（与 live placement 缺盘口即等待订阅数据的模式一致）；命中或保存的 advisory 会挂到 AI task 的本轮内存 plan。info-risk task 独立按 `info_risk_batch_size` 批量或逐市场构建 info-risk `input_hash`、查询缓存并在缓存未命中或进入提前刷新窗口时请求 provider，不再等待 AI 临时盘口批次。advisory cache key `schema_version` 已升到 10，使旧 watch 放行时期缓存、缺少 strategy hint 的二值缓存和旧低竞争策略 cache key 按新契约重新评估。provider 成功后只写入 `reward_market_advisories` / `reward_market_info_risks` 缓存，供后续 tick 使用，不再用旧 cycle 增量覆盖完整 quote plan 快照。live cache gate 会写入包含 pre_ai_eligible_plans/ai_existing_advisories/ai_request_candidates/ai_pending_plans/cache_hits/skipped_missing_market/applied 的 info 日志；后台 provider refresh 会分别写入 AI 与 info-risk 的 candidates/cache_hits/requested/saved/failures/skipped_missing_market 汇总（AI 侧额外含 skipped_missing_book）和逐个 requesting/saved 进度，超时会记录 `reward ... provider refresh timed out` warn。Rewards config 的 AI provider wire value 使用 `openai|anthropic`，request format 使用 `openai_responses|openai_chat_completions|anthropic_messages`；后端兼容读取旧 `open_ai*` 拼写但序列化始终输出 `openai*`。OpenAI-compatible provider 的 base URL 可配置为根地址或 `/v1` 地址，connector 会统一请求 `/v1/...` 并同时携带 Bearer 与 `api-key` 认证头；MiMo provider 使用 `openai_chat_completions`，不使用未实现的 Responses endpoint；Chat Completions 请求使用 MiMo 官方兼容的 `max_completion_tokens`，AI advisory/info-risk 分别给 4096/6144 completion token 预算，降低 reasoning 模型耗尽预算导致最终 `content` 为空的概率；AI advisory/info-risk 请求温度固定为 0，prompt 要求单个合法 JSON 对象，解析层会从 provider 文本中扫描 markdown fence、解释文字、JSON 字符串或数组包装里的候选对象，并且只有通过现有必填字段与枚举校验的对象才会保存，无法提取时 warning 会携带短 preview。AI provider 单次请求默认超时为 180 秒，可通过 `POLYEDGE_REWARDS__AI_REQUEST_TIMEOUT_SECS` 覆盖；AI advisory 和 info-risk 分别使用独立进程内 `Semaphore(1)`，同一 worker/API 进程内 AI 与 info-risk 各自单飞但可彼此并行。API 内嵌 worker 启动会记录 rewards poll loop 是否启用、AI key 是否配置、模型名和 interval；每轮 full tick 会记录 markets/books/plans/pre_ai_eligible_plans/eligible/open_orders/positions 以及 AI/info-risk 配置。AI advisory 启用时 full tick 同时启动专用 AI advisory provider refresh 和 info-risk provider refresh；独立 info-risk poll task 会跳过 provider 请求；AI advisory 未启用时，独立 info-risk task 仍按开放订单、持仓、eligible quote plan、候选市场顺序覆盖候选但同样受每轮 cap 限制。provider HTTP 传输失败，或明确返回限流、认证失败、服务端不可用（HTTP 429/401/403/5xx / `system_cpu_overloaded` / overloaded）时，对应 task 会停止本轮剩余 provider 请求以避免继续压垮 provider，并保留既有缓存/过滤语义。
- Rewards AI advisory / info-risk provider 明确内容过滤拒绝（如 GLM/OpenAI-compatible `contentFilter` 或 `1301`）会按同一 request/input hash 写入 fail-closed 缓存：AI advisory 保存 `avoid`，info-risk 保存 `critical`，直到 TTL 过期后才重试；若配置了 fallback，只有主备端点都返回内容过滤才写入该拒绝缓存，fallback 的临时网络/过载失败不会被固化。
- Rewards AI advisory / info-risk provider 请求前统一应用 pre-LLM 硬过滤：有开放订单或持仓的 condition 始终保留最高优先级；无敞口计划必须仍是 eligible 或 pre-AI eligible；legacy low-competition bucket 按 standard 候选处理，不再形成独立候选类型或观察队列。该过滤同时覆盖 full tick 后台 AI refresh、full tick info-risk refresh 和 AI 未启用时的独立 info-risk scanner，避免 market-only 候选批量变成大模型调用。
- Rewards AI advisory / info-risk 和 Smart Money signal advisory 的实际外部 provider HTTP 调用会写入 `llm_calls`，`task_type` 分别为 `reward_ai_advisory`、`reward_info_risk` 和 `smart_signal_advisory`；批量请求按一次外部调用计数，HTTP/解析失败会记录为失败调用，失败记录只影响统计和排查，不改变原有 fail-closed/缓存策略。
- Rewards live 撤单已回到统一路径：BUY/SELL 都共享通用硬风控、depth/rank/history/requote、best ask touch、kill switch、post-only/cancel retry 和对账锁检查；不再有低竞争专用撤单 gate、companion token 复核或独立宽松阈值。
- Rewards AI advisory 新增 orderbook 事件驱动批量通道（默认关闭 `POLYEDGE_REWARDS__AI_ADVISORY_EVENT_DRIVEN_ENABLED=false`，与 full-tick provider refresh 并存而非替代）：rewards orderbook 本地 cache 在某 condition 全部报价 token 首次都有真实 bids/asks 时入队 condition_id（就绪检测直接判 `CachedOrderBook` 非空，不构建 HashMap、热路径零额外分配，并用 `token_to_condition` 反向索引 + `notified_ready` 去重）；常驻 batch worker（随 orderbook runtime 一起 spawn/drop）攒满 `POLYEDGE_REWARDS__AI_ADVISORY_BATCH_SIZE`（默认 8，clamp `[1,12]`）个或等待 `POLYEDGE_REWARDS__AI_ADVISORY_BATCH_TIMEOUT_SECS`（默认 8）后，用 `current_live_cycle_state` 轻量 cycle + 候选/活跃 market 并集构建 markets_by_condition，对每个 condition 做 pre_ai_eligible 过滤、advisory cache miss 去重和盘口就绪复检，再单次 `RewardAiAdvisoryConnector::advise_batch` 评估一批（OpenAI Responses/Chat/Anthropic 各有批量变体，prompt 要求返回 `{"advisories":[{condition_id,...}]}` 数组并按 condition_id 匹配，漏项/拼错/多余被丢弃，batch size=1 时兼容单 object 返回），解析整体失败或模型漏掉部分 condition 时逐个回退到单市场 `advise`（路径 B，仍使用 AI advisory `Semaphore(1)` 和 cache miss 去重）；批量保存 advisory 后对每个通过过滤的 condition 串行推进 info-risk（复用 `refresh_reward_info_risk_for_condition`，使用独立 info-risk permit），完成后清除这些 condition 的就绪标记以便 advisory TTL 过期后盘口再次变化时重新触发。事件驱动通道不共享 full-tick AI/info-risk running flags，只靠 advisory cache miss 去重、AI advisory `Semaphore(1)` 和幂等 save 保证重叠时最多浪费一次重复调用；tick refresh 仍作全量兜底（覆盖 event 漏触发或 TTL 续期），event-driven 只是把盘口就绪到评估的延迟从最多一个轮询周期降到秒级。不允许挂单或 provider pending 的自动退订仍由 plan `eligible=false` 持久化 + 周期 orderbook token 注册任务在下一轮自然完成，无需新增退订端点。
- Rewards info-risk provider payload 会携带 `evaluation_time_utc` 和 imminent 判定策略；Chat Completions、Anthropic Messages 和 Responses prompt 都要求按该 UTC 时间判断 `resolution_imminent`，避免 provider 使用训练日期或过期上下文把远期/历史事件误判为临近结算。info-risk cache key `schema_version=5` 会让旧多状态、未携带 provider_cache_policy 或旧低竞争策略字段的缓存失效并重新评估，但 `evaluation_time_utc` 不进入稳定 cache key，避免每轮刷新造成缓存雪崩。
- Data API 最终成交回退也覆盖单订单已返回 404 的场景，包括认证账户 trade 扫描报错和扫描成功但没有精确 external order id 成交两种结果；此时必须额外满足：钱包交易累计量恰好等于本地订单剩余量，且完整外部持仓快照已覆盖该数量；否则先保持人工对账锁，若 404 锁超过 5 分钟仍无成交证据则本地标记为 cancelled。Rewards snapshot 的 `status.open_orders` 只统计已有非内部 `external_order_id`、仍是 open-like、本地剩余量为正且未处于提交未知、取消未知、404 人工对账或 `awaiting final reconciliation` 锁定的 managed orders；本地尚未提交的 planned/exit intent、已完全成交但状态尚未终态化的 open-like 行和已接受取消但等待最终对账的订单不再显示为当前 Polymarket 开放挂单。
- Worker 每次成功读取 CLOB 账户开放订单 snapshot 后，会把仍出现在该 snapshot 中且外部剩余量为正、状态非 filled/matched/cancelled/expired 的本系统 managed 外部订单数量写入 `RewardBotService` 热缓存；Rewards snapshot 的 `status.open_orders` 优先展示该观测值，冷启动或尚未成功同步时才回退到本地 store 计数，避免本地 stale open-like 或已完全成交订单短时间抬高顶部开放挂单数。
- Rewards worker 通过认证 CLOB raw HTTP `GET /rewards/user/total?sponsored=true` 同步 UTC 当日账户级 maker rewards 聚合值到 `account.reward_earned_usd`，以对齐 Polymarket `/rewards` 页面顶部 Daily Rewards 的 native+sponsored 口径；当聚合端点为空、为 0 或不可用时，会回退分页读取 `GET /rewards/user` native 明细并合并 `sponsored=true` sponsored-only 明细，按 `earnings * asset_rate` 求和；SDK 解码失败时会使用同一 L2 签名的 raw HTTP fallback，宽容解析带 trailing input 的 JSON 响应。前端只读取数据库/API snapshot，不直连 Polymarket。
- Rewards live 会在提交旧 intent 前先执行当前盘口/资格撤单检查；BUY intent 在真正 POST 前还会用 orderbook 服务做 1 秒 max-age last-look，若当前提交 token 盘口缺失、刷新失败或出现 best ask touch 等硬风险则不提交。任一提交结果未知、待最终对账或外部订单 404 会暂停全部新增买单，但继续同步、撤单和卖出退出；外部订单 404 锁超过 5 分钟且仍无成交证据时会自动本地关闭。提交结果未知时，开放订单严格匹配失败会先保持对账锁；当恢复查询确认 CLOB 无对应 open 订单并经过 `LIVE_SUBMISSION_UNKNOWN_CLOSE_AFTER_SECS`（默认 600 秒）宽限后，worker 会把该订单本地标记为 cancelled 以释放锁（与 404 锁一致），但若 positions 快照显示该 BUY token 在提交后出现库存则继续保留对账锁。CLOB `post_order` 只要返回订单 ID 就保留为 accepted 供后续成交/状态对账，包含 `unmatched` / `canceled` / 未知状态；HTTP 4xx 明确拒单会标记当前 intent 为 error，只有网络中断、5xx 或成功响应缺少订单 ID 才进入提交结果未知锁。managed order 的后续 upsert 会同步更新实际提交数量；SELL intent price 保留非亏损退出 floor；post-only exit 被取消后的重试保留退出 floor 并按 maker 规则重试，flatten replacement 保留退出 floor 并在后续按 best bid 非亏损 FAK 或继续等待。订单 scoring 观测只推进 `last_scored_at`，不修改业务状态 `updated_at`；reconciliation 锁订单跳过 scoring 查询，避免周期性观测掩盖真实业务状态年龄。
- Polymarket connector 已迁移到 CLOB V2 Rust crate：`packages/backend/Cargo.toml` 保留 dependency key `polymarket-client-sdk`，实际指向 `polymarket_client_sdk_v2`；live CLOB 签名类型支持 `eoa`、`proxy`、`gnosis_safe`、`poly_1271`，其中 `poly_1271` 用于已有 Deposit Wallet（`FUNDER` 填 deposit wallet 地址），下单前会调用 CLOB balance allowance update；已支持 collateral balance 查询、Polygon pUSD ERC20 余额读取、Funding 页面后端资金钱包 USDC/USDT ERC20 余额读取、开放订单全量分页、heartbeat raw 续链/`heartbeat_id:null` 重建 fallback 和 rewards earnings raw JSON fallback。Rewards 账户同步优先把 `FUNDER` 作为资金钱包地址，CLOB balance 为 0 或失败但链上 pUSD 大于 0 时用链上余额回填 snapshot；下单价格当前收敛到最多 2 位小数，同一 trade 内重复 maker entry 会聚合后入账。
- Rewards CLOB heartbeat 失败或超时后会清空本地 heartbeat id，并按 5-60 秒退避重建链；连续失败首条和每 6 次记录 warn，其余降为 debug，恢复时记录 info。
- 聪明钱跟单（copy-trading）已精简为只读跟踪+分析子系统：跟踪多个 Polymarket 钱包地址（`TrackedWallet`）、通过 Polymarket Data API（`data-api.polymarket.com`，通过 `PolymarketDataApiConnector`）检测钱包新成交、钱包分析统计（胜率/ROI/成交量）、`Analyze` 与钱包管理前端 UI。模拟引擎（模拟资金账本、仓位、订单、PnL）已移除，跟单不会下单。前端不再展示模拟账户、订单、持仓、Run、Cancel 或 Reset，只保留启停跟踪、钱包管理、Analyze、源成交和事件日志；同页还读取 Smart Money snapshot，支持保存 Smart Money foundation 配置，展示自动发现候选池、profile/score 摘要、recent_signals 信号流，并支持把候选设为 watch/tracked/blocked/rejected。未处理 source trades 按时间排序并记录。API 服务不执行 copytrade 跟单循环或钱包分析，前端 Analyze 只会写入数据库控制命令，由 worker 领取执行；`POLYEDGE_COPYTRADE__ENABLED=true` 启用 worker 轮询。
- Smart Money Intelligence 重构已开始落地 foundation：`/api/v1/smart-money` 读取 snapshot、`/api/v1/smart-money/config` 保存配置、`/api/v1/smart-money/candidates/status` 更新候选钱包状态（可按 wallet+source 或 wallet 全来源更新，未入库钱包创建 `manual` 来源记录）；数据库已有 smart money config、candidate/profile/score/trade/signal/advisory/paper 表，`SmartMoneyStore` 已接入 `smart_signal_advisories` 缓存读写并在 snapshot DTO 中返回最近 signal advisory，application 已能构造 signal advisory provider payload 和稳定 input_hash；`polyedge-worker scan-smart-money-once`、`poll-smart-money` 和可选 API 内嵌 runtime 定时任务会从 Polymarket Data API leaderboard 和 active copytrade tracked wallets 派生候选，再按 tracked/watch/candidate 状态扫描候选钱包，抓 Polymarket Data API activity/positions/closed positions/trades，保存近端样本画像、确定性评分和 activity 源交易；随后读取 orderbook 服务缓存，为尚未处理的源交易按年龄、盘口、方向价格、滑点和最优档深度生成 `observe` 或 `rejected` 信号，并写入 `stage=deterministic_gate` 的 signal decision 审计记录；开启 `signal_advisory_enabled` 时还会为近期 observe 信号补齐源交易/profile/score 上下文，按 Smart Money 配置中的 `signal_advisory_provider` / `signal_advisory_request_format` / `signal_advisory_model` 构造 advisory payload/input_hash 并检查缓存，provider key/base URL/timeout 来自 `POLYEDGE_SMART_MONEY__SIGNAL_ADVISORY_*` 环境变量，有 key 时调用 `SmartSignalAdvisoryConnector` 保存 `allow|observe|reject` 三态 advisory 并写入 `llm_calls(task_type=smart_signal_advisory)`，无 key 时只记录候选、缓存命中和待请求数量；任一来源已标记 `blocked` 或 `rejected` 的钱包不会被自动 seed 或扫描。`/copy-trading` 已提供 Smart Money 配置保存（含 signal advisory provider/request format/model）、候选池查看、候选状态操作和基础信号流/advisory 展示入口。定时任务默认关闭，需 `POLYEDGE_WORKER__POLL_SMART_MONEY=true` 且 Smart Money config `enabled=true`；若 Smart Money config disabled，worker 跳过外部 discovery/profile 请求，但仍可尝试处理已入库 source trades 的信号、decision 和 advisory refresh。已接入的是 leaderboard 种子发现、基础配置、候选管理、deterministic observe/rejected 信号生成、deterministic decision 审计、signal advisory 缓存层、signal advisory request builder、独立 provider 配置和 provider refresh；recent trades/链上完整 discovery、wallet advisory、纸面模拟和 guarded live execution 尚未实现，不能描述为当前可用能力。
- High Probability Pricing 动态高概率市场定价研究已开始落地 foundation：数据库已有 `high_probability_config`、`high_probability_market_outcomes`、`high_probability_samples`、`high_probability_bucket_stats`、`high_probability_backtest_runs`（含 `exit_rule_reports`）、`high_probability_backtest_trades`、`high_probability_observations`；`HighProbabilityService` 和 Postgres/in-memory store 已支持配置、本地 outcome 标签、rewards candle sample input、observe candidate 查询、样本、分桶统计、只读 research report、即时 baseline walk-forward backtest report、基础退出规则对比、observe gate、持久化 baseline backtest run/trade 和 observation；API 已提供只读 `/api/v1/high-probability`、`/api/v1/high-probability/config`、`/api/v1/high-probability/buckets`、`/api/v1/high-probability/report`、`/api/v1/high-probability/backtests`、`/api/v1/high-probability/backtest-runs` 和 `/api/v1/high-probability/backtest-runs/{run_id}/trades`；前端 `/high-probability` 只读研究页已接入 snapshot/config/bucket stats/report/backtest/backtest-runs DTO 和中文文案，展示研究配置、样本覆盖、加权胜率/期望、即时 baseline 回测、基础退出规则对比、持久化历史回测 run、所选 run 交易明细、最佳/最差 bucket、bucket stats 与 observations，不提供写操作或交易控制；`polyedge-worker import-high-probability-outcomes <path>` 可从本地 JSON 文件导入 outcome 标签（worker crate 显式引用 workspace `serde` 用于该本地 JSON 解析），`build-high-probability-samples-once [limit]` 会从本地 outcome 标签 + rewards candles 构建 first-touch 样本，`refresh-high-probability-buckets-once` 会从已入库已结算样本计算并替换当前模型版本 bucket stats，`run-high-probability-backtest-once` 会按当前配置运行并持久化 baseline 70/30 walk-forward 回测、退出规则摘要和交易明细，`observe-high-probability-once [limit]` 会读取活跃 rewards 最新 candle 候选 + orderbook 服务缓存并写入 `allow/reject/skip` observations；API 内嵌 worker runtime 也可通过 `POLYEDGE_WORKER__POLL_HIGH_PROBABILITY_OBSERVE=true` 按 `POLYEDGE_WORKER__HIGH_PROBABILITY_OBSERVE_INTERVAL_SECS`（默认 300 秒，runtime 下限 60 秒）自动执行同一只读 observe 流程，默认关闭。全市场 price-history/outcome 自动 producer、完整执行成本/多阶段退出回测、paper/live guarded execution 尚未实现，不能描述为当前可用能力。
- Polymarket 运行时不再提供 mock mode；市场列表走 Gamma 实时数据，私有订单/成交任务需要真实凭证、真实账户、小额演练和运维 runbook。
- 数据库自动清理由通用 `database-maintenance` worker 默认生产开启，按 retention 分批清理 raw events、过期 AI/info-risk cache、`reward_market_candles`、控制命令、copytrade 历史、outbox/external dedup、LLM calls、audit logs 和 mode transitions，避免这些表无限膨胀；旧 arbitrage 历史表随迁移保留但当前不会继续写入新 scan/opportunity。删除只释放可复用空间；如需把已膨胀文件还给操作系统，仍需运维执行 vacuum/repack 类操作。
- 数据库迁移目前到 `0054_reward_market_event_windows.sql`；`packages/backend/init.sql` 是按 0001–0054 合并的空库完整初始化脚本，运行时仍保留 `packages/backend/migrations/` 给 `sqlx` 校验和增量迁移使用。

## 主要缺口

- 生产级真实会话体系未完成；当前前端只保留 `off` 模式。
- 内部 JWT 签名 helper 已有代码路径，但当前不会从 `off` 签发可信令牌。
- `/funding` Polymarket 入金页使用后端配置的资金钱包和私钥发起真实 Polygon ERC-20 USDC/USDT 转账：后端以 `FUNDER` 优先、`ACCOUNT_ID` 回退作为 Polymarket 入账钱包，通过 Polymarket Bridge 生成 EVM 入金地址后广播交易；前端不输入充值地址、不接触私钥。Funding 状态会查询并展示后端资金钱包的 USDC/USDT Polygon 链上余额，余额查询失败只显示提示不阻断页面；当前仍不查询 allowance、POL gas 余额、确认数或 Polymarket pUSD 入账状态。
- 前端已移除 SSE 实时流机制，页面数据通过 REST API 加载；Rewards 工作台会额外每 10 秒静默刷新当前 snapshot，以反映 worker 写入的 AI advisory、信息风险、订单和账户状态；静默自动刷新遇到短暂网络失败时保留现有页面状态且不弹出“操作失败”，用户主动操作/筛选触发的失败仍会反馈。
- 新闻源可以抓取、去重、提升为 events/evidences，但尚未自动生成 signals。
- Rewards live maker 已接入真实 post-only 买单提交、撤单、本系统托管订单成交与计分同步、CLOB open-order 反查、可映射 active rewards BUY 收养/重开、成交后现金/库存/PnL 更新、sibling leg 撤单和 exit/flatten sell 下单；worker 在 managed order 同步后刷新账户开放买单总 notional 观测，并在新增买单准入时把未归属到本系统 managed order 的外部 BUY notional 从可用资金中保守扣除；confirmed fill 保护期外会刷新 CLOB 余额、资金钱包链上 pUSD 回退和 Data API 完整持仓快照，API 只从数据库读取且不再需要 Polymarket 凭证。仍未完成 SELL、非 rewards 市场、无法唯一映射 token 的账户范围外开放订单明细同步或奖励结算对账。实盘策略仍应沿用“本系统未成交 maker 买单不硬锁全局 pUSD、成交后才更新现金/库存并撤超额挂单；未知外部 BUY 保守占用可用资金”的资金模型。
- Rewards 低竞争市场 sleeve 已合并为统一机会评分：前端不再展示低竞争配置/报告/专用摘要，worker 不再构建独立候选 profile、probe source、shadow report、open-order 占比 cap、专用 last-look 或专用撤单 gate；`low_competition_*` 配置、legacy bucket、metrics 和 observation 表仅保留历史/API/DB 兼容。当前所有奖励市场统一通过 `opportunity_metrics` 综合竞争倍数、100U 日奖、资金占比、退出深度/滑点、坏成交恢复天数和盘口稳定性，并通过 `score_adjustment` 影响 quote plan score 与 `min_market_score` 资格判断；full tick 后置刷新机会指标时只允许降级或更新展示，不会把 provider/资金/盘口 gate 已阻塞的计划重新放行。
- Rewards 事件窗口已落地 `reward_market_event_windows`：多 source 候选按 active、confidence、source 优先级和更新时间选出 effective window，live cycle 写入 `RewardQuotePlan.event_window`。默认 `event_window_enabled=true`、`event_window_min_confidence=high`、stop-new/cancel/resume 为 10800/3600/3600 秒、未知事件时间 `observe`、Gamma 未审核日期 `ignore`；`StopNewQuotes` 和 unknown block 模式只阻断新增 BUY，不立即撤已有 BUY；`CancelOpenBuys`、`InEventWindow`、`PostEventCooldown` 会阻断新增 BUY 并触发已有 BUY 撤单；SELL exit 不因事件窗口阻断。provider prefilter 会跳过被事件窗口阻断新增且无开放订单/持仓的 condition，已有敞口 condition 仍保留 provider 风险覆盖。Gamma 日期候选默认只是低/中置信补充数据，除非配置提高信任或人工/官方 source 写入 high confidence，否则不会触发 hard gate。
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
cargo run -p polyedge-worker -- run-database-maintenance-once
cargo run -p polyedge-worker -- poll-news
cargo run -p polyedge-worker -- promote-news-events
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
cargo run -p polyedge-worker -- scan-smart-money-once
cargo run -p polyedge-worker -- poll-smart-money
cargo run -p polyedge-worker -- import-high-probability-outcomes outcomes.json
cargo run -p polyedge-worker -- build-high-probability-samples-once
cargo run -p polyedge-worker -- refresh-high-probability-buckets-once
cargo run -p polyedge-worker -- run-high-probability-backtest-once
```

## 配置要点

- 后端默认监听 `0.0.0.0:38001`。
- 默认 runtime mode 是 `live_auto`。
- Polymarket connector 没有 mock mode；未配置真实账户/私钥时，不要开启 Polymarket 私有订单、成交或用户 websocket worker 任务。
- `POLYEDGE_POLYMARKET__SIGNATURE_TYPE` 可选 `eoa`、`proxy`、`gnosis_safe`、`poly_1271`；新 Deposit Wallet 使用 `poly_1271`，并将 `POLYEDGE_POLYMARKET__FUNDER` 设置为 deposit wallet 地址。
- `POLYEDGE_POLYMARKET__POLYGON_RPC_URL` 默认 `https://polygon-bor-rpc.publicnode.com`；Rewards worker 用它读取资金钱包链上 pUSD 余额，Funding API 用它广播后端资金钱包 USDC/USDT 入金转账，生产环境可替换为自有或有 SLA 的 Polygon RPC。
- `POLYEDGE_WORKER__DATABASE_MAINTENANCE` 控制 API 内嵌 worker 的数据库维护循环；部署模板默认 `true` 且 `POLYEDGE_WORKER__DATABASE_MAINTENANCE_INTERVAL_SECS=3600`，本地 `packages/backend/.env.example` 默认关闭。
- 部署模板默认开启 news ingestion 的子系统/worker 开关，默认关闭新闻提升为 events/evidences、rewards、copytrade 和私有对账类 worker；旧 arbitrage radar 开关已移除。
- `POLYEDGE_NEWS__SOURCES_JSON` 未配置时使用代码默认 RSS/Atom 源列表；`deploy/.env.api.example` 已显式写入当前默认源列表，设置该变量会覆盖整个列表。新闻采集在部署模板中默认启用（`POLYEDGE_NEWS__ENABLED=true`、`POLYEDGE_WORKER__POLL_NEWS=true`），新闻提升为 events 仍需 `POLYEDGE_WORKER__PROMOTE_NEWS_EVENTS=true`。
- 默认 rewards bot worker 是 disabled；前端 `/rewards` 的 Run / Cancel / Reset 只会入队命令，且同账户同动作已有 pending/running 命令时会合并重复请求；worker 需要同时设置 `POLYEDGE_REWARDS__ENABLED=true` 和 `POLYEDGE_WORKER__POLL_REWARD_BOT=true` 才会领取并执行。`ai_advisory_enabled=true` 时，信息风险 provider 刷新由 rewards full tick 的专用 info-risk provider refresh 推进；未启用 AI advisory 但要独立异步扫描信息风险时，才需要额外设置 `POLYEDGE_WORKER__POLL_REWARD_INFO_RISKS=true` 并在 `/rewards` 配置中启用 `info_risk_enabled`。要产生新挂单和 live post-only 下单，还需要配置真实 Polymarket 凭证并确保 `polyedge-orderbook` 服务正在运行并同步了 reward 市场数据。
- 部署侧环境变量已精简为三个服务级模板：`deploy/.env.api.example`、`deploy/.env.orderbook.example`、`deploy/.env.front.example`。`deploy/.env.api.example` 同时包含 API、内嵌 worker runtime、Polymarket live/Deposit Wallet（`poly_1271`）、rewards AI/信息风险和 Smart Money signal advisory 可选凭证示例；新闻采集默认开启，其他后台 worker 循环默认关闭。高级轮询/阈值调参优先使用 Settings/runtime_config 或代码默认值。私钥和 AI provider key 只放 `deploy/.env.api`；front/orderbook 不持有 Polymarket 凭证，余额和持仓由 worker 同步到数据库后 API 从数据库读取。
- Rewards bot 的 `max_markets=0` 或 `max_open_orders=0` 表示不再新挂单；`quote_size_usd=0` 不再禁用报价，报价腿按 rewards 最小份额、Polymarket 1 美元最小名义金额和实际钱包余额准入处理。前端 `/rewards` 不再展示或提交 `per_market_usd`、`quote_size_usd`、`low_competition_per_market_usd` 额度字段，它们已从前端 `RewardBotConfigDto` 移除，仅后端配置兼容读取/序列化；漂移换价 guard 的 `requote_drift_confirm_sec`、`requote_drift_cooldown_sec`、`requote_drift_max_cancels_per_cycle` 已在报价构造配置区与 `requote_drift_cents` 一起暴露为可编辑输入。
- Rewards bot 的 `quote_bid_rank` 仅允许 `1`、`2`、`3`，默认 `1`；粗 tick 盘口按不同买价挂在买一、买二、买三，细 tick 盘口会从买一回退 `rank-1` 个 0.01 价格步长后选择不高于目标价的当前买盘档位，避免 0.001 tick 下买三只退两个细档。该检查只在 live placement 准备挂单时基于当前 orderbook 执行，不在 quote plan 构建阶段提前过滤候选；缺少、过期或接近 stale 边界的盘口会保持等待订阅数据返回，auto/enforce/dominant 下双边缺档可回退到目标档位存在且通过校验的单腿，否则非 transient 验证失败才写入 12 小时 `live_skip_until`/`live_skip_reason`。
- Rewards bot 的 `max_spread_cents` 限制为 `0.1..=99`；超过概率价格有效范围的输入会归一化为 99。
- Rewards bot 市场质量硬门槛默认是：`min_market_liquidity_usd=1000`、`min_market_volume_24h_usd=1000`、`min_hours_to_end=48`、`max_market_spread_cents=10`、`max_market_data_age_minutes=15`；通过门槛后再按奖励、流动性、成交量、剩余时长和奖励 spread 综合排序。`max_market_data_age_minutes` 同时驱动 orderbook Gamma priority sync 间隔，窗口越小，已注册/活跃/rewards 候选市场刷新越频繁，避免仅因全量 Gamma 目录慢而触发新鲜度撤单。
- Rewards bot 盘口选择默认 `quote_mode=double`、`selection_mode=observe`、`dominant_single_side_enabled=false`，保持 YES/NO 双边计划。启用 auto/enforce 后，planner 阶段只用 `dominant_min_probability` / `dominant_max_probability` 生成初步单边/双边/跳过模式；需要当前盘口的 `dominant_min_exit_depth_usd`、`max_top1_depth_share`、`max_top3_depth_share` 和 `max_book_hhi` 在 live placement materialize 阶段验证。双边目标档位、rewards spread、touch ask 或安全边际不满足时，会优先回退到仍满足这些 live 校验的单腿；两腿都不可行才跳过。`preferred_categories` 默认偏好 `politics,elections,geopolitics`，只作为排序加分。统一机会评分配置包含 `opportunity_metrics_enabled`、probe notional、最低 100U 日奖、最大竞争倍数、账户/单市场资金占比、退出深度/滑点、坏成交恢复天数、盘口稳定性窗口/样本/波动/跳变阈值和 reward/competition/exit/stability 权重；它适用于所有奖励市场，不再区分普通/低竞争 sleeve。AI advisory 配置包含 `ai_advisory_enabled`、`ai_provider=openai|anthropic`、`ai_request_format=openai_responses|openai_chat_completions|anthropic_messages`、TTL 和 `ai_advisory_batch_size`；信息风险配置包含 `info_risk_enabled`、`info_risk_mode=observe|enforce`、`info_risk_avoid_level=low|medium|high|critical|unknown`、TTL、`info_risk_batch_size`、`require_info_risk_before_first_quote` 和 `first_quote_quarantine_sec`（默认 600）。API key/base URL/model/timeout 和原有 advisory allow 最低置信度来自 worker 环境变量（如 `POLYEDGE_REWARDS__AI_OPENAI_API_KEY`、`POLYEDGE_REWARDS__AI_ANTHROPIC_API_KEY`、`POLYEDGE_REWARDS__AI_MODEL`、`POLYEDGE_REWARDS__AI_MIN_CONFIDENCE_BPS=5500`、`POLYEDGE_REWARDS__INFO_RISK_MIN_CONFIDENCE_BPS=7000`、`POLYEDGE_REWARDS__INFO_RISK_WEB_SEARCH_ENABLED=false`、`POLYEDGE_REWARDS__INFO_RISK_MAX_MARKETS_PER_CYCLE=50`），不会进入前端或 API snapshot；GLM/DeepSeek 使用 OpenAI-compatible base URL 和模型名配置，模型名包含 `glm` 或 `deepseek` 时后端强制 Chat Completions + `json_object` + `max_tokens`；strategy hint 应用阈值来自 rewards 配置 `ai_strategy_hint_min_confidence`，可在前端和 API snapshot 调整；AI advisory 每轮最大市场数环境变量已移除，信息风险每轮最大市场数环境变量现在作为 AI advisory 与 info-risk 各自 provider refresh 的上限，AI 侧按实际 provider request 消耗、batch fallback 也受同一上限、缓存命中不占名额，info-risk 侧按 selected condition 截断，默认 50，0 表示本轮不发 provider 请求；后台 provider refresh 整体 timeout 按 rewards poll interval 派生，默认 60 秒 poll 时最多 120 秒。可选第二个完全独立的 LLM 备用接口：同时配置 `POLYEDGE_REWARDS__AI_FALLBACK_PROVIDER`、`POLYEDGE_REWARDS__AI_FALLBACK_REQUEST_FORMAT`、`POLYEDGE_REWARDS__AI_FALLBACK_API_KEY`、`POLYEDGE_REWARDS__AI_FALLBACK_BASE_URL`、`POLYEDGE_REWARDS__AI_FALLBACK_MODEL` 五项后启用（同样不会进入前端或 API snapshot）；主接口（`ai_provider` 选定）调用因任意原因失败（网络/超时、HTTP 4xx/5xx、或返回无法解析的空响应）时，会用同一请求重试备用接口（可不同 provider/模型），两次尝试都写入 `llm_calls`（`fallback_used` 区分主备），advisory/info-risk 缓存按 `(provider,request_format,model,input_hash)` 各自独立存储，live tick gate 与缓存读取同时识别 primary 和 fallback 来源。备用 provider 仍配置为 `openai` 或 `anthropic`；fallback 模型名包含 `glm`/`deepseek` 时同样强制归一为 `openai_chat_completions`。
- Rewards bot 本系统未成交 post-only maker 买单不在本地按全局 notional 硬锁资金；不同 condition 可复用同一资金池，但同一 condition 的已有 managed BUY 剩余 notional 与待补 YES/NO 腿必须合计不超过最近同步的 `available_usd` 扣除未归属外部 BUY notional 后的余额，否则整组不挂。full tick 会在 AI/info-risk provider refresh 前对无开放订单/持仓的新 condition 先执行这项最低 rewards size 资金预检查，明显资金不足的计划不再消耗 provider token；已有订单/持仓 condition 跳过前置资金拦截，仍继续进入 provider 风险覆盖。CLOB open-order snapshot 会把可映射 active rewards BUY 收养/重开为 managed order，并且账户级 `external_buy_notional` 只累计 snapshot 中剩余量为正且状态非终态的 BUY；每次 CLOB open-order snapshot 还会用同一次 snapshot 的两侧相减冻结 `unmanaged_external_buy_notional = external_buy_notional - managed_external_buy_notional`（真正的非本系统外部占用），funding precheck 与统一机会评分直接读这个冻结值，而不是用旧的 `external_buy_notional(15s 慢字段) - managed(实时)` 重新相减——避免本系统在两次 snapshot 之间撤掉自己的 managed BUY 时把已撤单 notional 当外部占用、周期性把 `eligible_markets` 压到 0；SELL、非 rewards 市场和无法唯一映射 token 的外部开放订单明细仍缺失；`stale_book_ms` 默认 45000，配置归一化下限为 5000ms，不再允许生产配置把盘口年龄检查降到 0；worker 会对本地缺失、过期或超过新挂单 freshness headroom 的盘口用 orderbook 服务 HTTP batch 兜底刷新，新挂单要求盘口距离 stale 边界仍有余量；full tick 在 AI/info-risk gate、订单同步和账户同步之后，进入撤单/待提交 intent/新挂单前会对当前 open-like 订单与 eligible quote plan token 再做一次本地 cache / HTTP batch 刷新，避免 tick 前半段 I/O 耗时让初读盘口在 placement 阶段变旧；近期已提交 BUY 的单纯 stale 撤单会短暂 grace，缺盘口/空盘口和其他硬风险不延迟。
- Rewards live placement freshness headroom 默认保留 10 秒 stale 余量（短 stale 窗口保留半窗）；默认 `stale_book_ms=45000` 时，新挂单接受 `confirmed_at` age 约 35 秒以内的盘口，worker 本地 HTTP 预刷新阈值约 25 秒；worker 远程 batch 读取会把该预刷新阈值传给 orderbook 服务，服务端缓存也超过该年龄时会同步 CLOB `/books` 刷新，避免 orderbook 10 秒 poll 与 full tick I/O 把可挂市场长期卡在等待盘口。
- Rewards quote plan 的 midpoint/materialize 与 live 风控统一使用 `RewardOrderBook.confirmed_at` 判断盘口新鲜度，`observed_at` 只表示盘口内容版本和历史样本时间；安静市场只要 orderbook poll/WS 最近确认过，就不会因内容时间戳不变被误判为缺 fresh midpoint。
- Rewards bot 对外部订单 404 会先保持对账锁；若超过 5 分钟仍无 CLOB/Data API 成交证据，则将本地订单标记为 `cancelled`，使其不再计入开放挂单。普通已提交 open-like BUY 若在 CLOB open orders snapshot 中缺失或 snapshot 行已非 active 且无活跃对账锁，也会本地标记为 `cancelled`；其中撤单已 accepted 并处于 `cancel accepted; awaiting final reconciliation` 的 BUY，只要后续 open-order snapshot 确认该 external order 已不存在，也会自动关闭本地剩余量并释放新增买单锁，避免单笔已不存在订单长期死锁。提交结果未知订单现在会在恢复查询确认 CLOB 无对应 open 订单后经过 `LIVE_SUBMISSION_UNKNOWN_CLOSE_AFTER_SECS`（默认 600 秒）宽限自动本地关闭（与 404 锁一致），不再永久卡死；取消结果未知订单仍不会仅因本地等待超时 force-cancel。旧 `auto_cancel_stale_minutes` 配置键读取时忽略。
- 活跃 token 盘口事件会先进入独立 `RewardEventCancelGuard` hard-risk cancel worker：按更新 token 检查开放订单，事件路径会立即处理缺盘口/空盘口、SELL stale、计划失效、kill switch、post-only violation retry 等硬风险，BUY 还会检查深度/rank/盘口历史窗口；不做成交同步、账户同步、退出提交、重挂、报价漂移换价或定期 requote。
- Rewards fast reconcile 仍可被活跃 token 盘口事件直接唤醒，不再做固定 1 秒合并，并作为完整撤单检查、成交/退出处理和周期兜底；重型外部同步独立节流：托管订单状态最小 5 秒间隔，CLOB open-order snapshot 最小 15 秒间隔，managed scoring 按 `min_scoring_check_sec` 且归一化下限 15 秒，账户级 rewards earnings 与 balance/positions snapshot 最小 60 秒间隔；full tick 或 `run_once` 完整同步后会刷新这些节流时间戳。post-only violation 的 cancel rejected/unknown 会按最小 15 秒间隔重试，cancel accepted 但超过 30 秒仍未完成最终对账时会再次尝试撤单。
- Rewards poll loop 持有 live advisory lease 后会先尝试一次历史清理，之后每 5 天清理一次 5 天前的终态 managed orders（`cancelled`/`filled`/`error`）、`reward_risk_events` 和 `reward_low_competition_observations`，避免订单和事件表无限膨胀；清理不会删除 `planned`/`open`/`exit_pending`、fills、positions 或 account state，因此不影响当前挂单、持仓和成交账本。
- Rewards bot 不再用 `per_market_usd`、`quote_size_usd` 或 `low_competition_per_market_usd` 限制报价腿构造；live materializer 只满足报价腿按 CLOB 成本精度向上对齐后的 `rewards_min_size` 和 Polymarket 1 美元最小名义金额。新增报价是否可挂由实际钱包余额决定：同一 condition 已有 managed BUY 剩余 notional 与待补 YES/NO 腿总 notional 必须不超过最近同步的 `available_usd` 扣除未归属外部 BUY notional 后的余额；若待补最低 rewards size 腿已经放不下，provider 前 funding precheck 会先把无 active exposure 的新 condition 标记为不可挂并写入 funding reason，live placement 下单前仍会复核同一约束，等待后续余额或开放订单同步后重新评估。
- `POLYEDGE_ORDERBOOK_STREAM__MAX_TOKENS` 默认 3000；调高会增加 orderbook WS/poll 内存占用，调低会减少 rewards 盘口覆盖。每个 source（活跃 rewards、execution、最终 eligible rewards、AI provider 临时批次、其余候选 token）独立注册全量 token，由聚合层按固定优先级 `rewards_active > exec_orders > rewards_eligible > rewards_ai_provider > rewards_candidates > copytrade` 跨 source 去重并 take 上限截断；`rewards_eligible` 由周期任务注册全部最终 eligible quote plan token，pre-AI provider 候选通过 `rewards_ai_provider` 每批最多 10 个市场临时订阅，批次切换会取消上一批，且 AI refresh 会先保留开放订单/持仓，其余按统一 standard 候选顺序处理；rewards 新买单 intent 持久化后会即时刷新 `rewards_active` source。`POLYEDGE_ORDERBOOK_STREAM__REWARD_CANDIDATE_TOKEN_CAP` 默认 50，只限制 rewards 候选预热 source，不影响活跃订单、持仓、execution、最终 eligible 或 AI provider 临时批次；设为 0 可关闭候选预热以快速降带宽。`POLYEDGE_ORDERBOOK_STREAM__WS_CHUNK_SIZE` 默认 100，用于控制每条 Polymarket WS 连接承载的 token 数；调低会减少单连接消息压力、增加连接数，调高则相反。`POLYEDGE_ORDERBOOK_STREAM__POLL_RECONCILE_INTERVAL_SECS` 默认 10；调低会更快修复 WS 缺口但增加 CLOB `/books` 压力，调高可能让 rewards live placement 因盘口超过默认约 35 秒 placement 窗口而等待。`POLYEDGE_ORDERBOOK_STREAM__MAX_LEVELS_PER_SIDE` 默认 100，用于限制进程内缓存和 HTTP ingest 每个 token 的 bids/asks 保留深度；写入时先排序再裁剪，保留最优档位。poll 每周期会刷新全部注册 token；`POLYEDGE_ORDERBOOK_STREAM__STALE_THRESHOLD_MS=0` 只关闭年龄 stale 优先级。`POLYEDGE_ORDERBOOK_STREAM__WS_INCREMENTAL_RECONCILE`（默认 true）控制 token 集合成员变化时是保持 WS 连接做增量 subscribe/unsubscribe（true）还是整体重建连接（false，回滚用）；`POLYEDGE_ORDERBOOK_STREAM__FULL_RESYNC_INTERVAL_SECS`（默认 0=关）大于 0 时按周期强制 teardown+rebuild 全部 WS 连接，作为增量状态漂移的应急兜底（默认依赖 poll reconciler 与 SDK reconnect 全量重订阅兜底）。
- 默认跟单 worker 是 disabled；前端 `/copy-trading` 只提供启停跟踪配置、钱包管理和 Analyze 命令入队，不再暴露 Run / Cancel / Reset。worker 需要设置 `POLYEDGE_COPYTRADE__ENABLED=true` + `POLYEDGE_WORKER__POLL_COPYTRADE=true` 才会持续扫描源成交；`POLYEDGE_WORKER__ANALYZE_WALLETS=true` 仍用于独立钱包分析循环，前端 Analyze 命令也需要 worker 领取后才会更新分析统计。
- 默认 Smart Money 定时扫描 worker 是 disabled；需要设置 `POLYEDGE_WORKER__POLL_SMART_MONEY=true`，并通过 `/api/v1/smart-money/config` 或数据库配置把 Smart Money `enabled=true` 后，才会按 `POLYEDGE_WORKER__SMART_MONEY_INTERVAL_SECS`（默认 900 秒，runtime 下限 60 秒）持续 seed Data API leaderboard/copytrade 候选并扫描候选钱包，单轮扫描上限为 `task_limit` 且硬上限 50。只开启 worker 开关但 Smart Money config disabled 时不会抓 Polymarket Data API。
- 默认 High Probability observe worker 是 disabled；需要设置 `POLYEDGE_WORKER__POLL_HIGH_PROBABILITY_OBSERVE=true` 后，才会按 `POLYEDGE_WORKER__HIGH_PROBABILITY_OBSERVE_INTERVAL_SECS`（默认 300 秒，runtime 下限 60 秒）持续读取本地候选和 orderbook 服务缓存并写入只读 observations，单轮候选上限复用 `POLYEDGE_WORKER__TASK_LIMIT`。该循环不抓外部 Polymarket API、不下单。
- `POLYEDGE_POSTGRES__URL` / `POLYEDGE_REDIS__URL` 为空时，本地可能走内存路径，无法验证多进程共享状态和持久化 outbox。
- Postgres rewards live worker 在整个 poll loop 生命周期持有 advisory lease，因此 `POLYEDGE_POSTGRES__MAX_CONNECTIONS` 必须至少为 2（默认 20）。生产环境必须运行持续 poll loop 维持 CLOB heartbeat；`scan-rewards-once` 或有限 `max_cycles` 只适合诊断，进程结束后不能继续守护已提交订单。
- `POLYEDGE_ORDERBOOK__SERVICE_URL` 的代码默认值是 `http://localhost:38002`，只适用于宿主机直接运行；Docker Compose 同项目部署必须在 `deploy/.env.api` 使用 `http://polyedge-orderbook:38002`，跨服务器部署时使用 orderbook 服务器的实际地址（默认生产排查地址为 `http://100.87.45.72:38002`）。worker 会用同一地址转换为 `ws(s)://.../orderbook/stream` 连接内部盘口推送。Compose 不会再覆盖 `.env.api` 中的值。`POLYEDGE_ORDERBOOK__WRITE_TOKEN` 是 orderbook/API 内嵌 worker 部署必填共享密钥，分别放在 `deploy/.env.orderbook` 与 `deploy/.env.api` 且值必须一致，不放入 front 环境；`OrderbookHttpClient` 使用 5 秒连接超时和 30 秒请求超时，`OrderbookStreamClient` 建立内部 WS 连接最多等待 5 秒。
- Docker 部署中没有单独的 `polyedge-worker` service；`polyedge-api` 只加载 `.env.api` 并在同一进程内启动 worker runtime。部署模板默认启用新闻采集，其他后台循环仍显式设为 `false`；需要运行 rewards、copytrade、新闻提升或私有对账任务时必须在 `deploy/.env.api` 显式设为 `true`。市场同步和 orderbook 订阅由独立 `polyedge-orderbook` 服务管理，不需要在 worker 中启用。

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

部署脚本默认使用 `/tmp/polyedge-deploy.lock` 防止 cron/CI 重叠执行，默认 `COMPOSE_PARALLEL_LIMIT=1` 串行构建镜像。Auto 模式 per-service 独立检测：api、orderbook、front 各自独立镜像；`worker` 只是 api 目标兼容别名，因为 worker runtime 内嵌在 `polyedge-api` 中。容器未运行但 hash 未变时直接启动已有镜像。前端 `yarn build` 前会读取 `deploy/.env.front` 并把 `NEXT_PUBLIC_*` 写入静态 bundle，build 前会清理旧 `.next/` 和 `out/`，build 后会给 HTML 中的 `/_next/static/*.js/css` 引用追加 front hash query；前端 Nginx 对 HTML 与 `/_next/static/` 使用 `Cache-Control: no-cache, must-revalidate`，避免静态导出 chunk 文件名复用导致浏览器长期运行旧工作台代码。Compose 构建上下文已收窄：后端只上传 `bin/`，前端只上传 `packages/front/`，避免扫描本地 `packages/backend/target`、`node_modules`、`.next` 等大目录。跨服务器部署时每台服务器只需本地存在的二进制，脚本只检查目标服务所需的文件。
旧的 `packages/backend/Dockerfile` 仅作为仓库根 context 兼容模板保留，当前只复制默认构建脚本产出的 `bin/polyedge-api` 与 `bin/polyedge-orderbook`；Compose 部署不使用它，仍使用 `deploy/api.Dockerfile` 和 `deploy/orderbook.Dockerfile`。

## 关键入口

前端：

- `packages/front/src/lib/api/base.ts`
- `packages/front/src/lib/api/actions.ts` + `actions/`
- `packages/front/src/lib/api/copytrade.ts`
- `packages/front/src/proxy.ts`
- `packages/front/src/lib/i18n/*`
- `packages/front/src/features/funding/*`
- `packages/front/src/features/high-probability/*`
- `packages/front/src/features/copytrade/*`

后端：

- `packages/backend/api/src/lib.rs`
- `packages/backend/api/src/handlers/funding.rs`
- `packages/backend/api/src/handlers/rewards.rs`
- `packages/backend/api/src/handlers/copytrade.rs`
- `packages/backend/order/src/main.rs`
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
