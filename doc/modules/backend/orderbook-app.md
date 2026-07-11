# Orderbook App（市场同步与盘口服务）

最后更新：2026-07-11

## 概述

`polyedge-orderbook` 是独立的 Axum/Tokio 服务，crate 位于 `packages/backend/order`。它负责从 Polymarket 同步通用市场和奖励市场、维护 CLOB WS + poll 盘口流、保存进程内盘口缓存，并用低频限速的 CLOB `/prices-history` 同步 rewards token 5 分钟 price-history source K 线，通过 HTTP/内部 WS 向 API/worker 提供盘口读取、实时推送和订阅注册能力。

## 架构与关键文件

| 文件 | 职责 |
|---|---|
| `packages/backend/order/src/main.rs` | 服务入口：先 bind HTTP，再启动独立 Gamma full/priority sync、rewards catalog sync 和可重启盘口流后台任务；监听地址和 signal shutdown 复用 `polyedge-common` |
| `packages/backend/order/src/market_sync.rs` | Gamma full sync、priority condition sync（已注册 token/rewards 重点市场）、Gamma 日期候选写入 reward event windows 与 CLOB reward markets 单次同步实现 → Postgres |
| `packages/backend/order/src/candle_history.rs` | Rewards candle history sync：按奖励优先级选择 active reward token，限速调用 CLOB `/prices-history`，写入 `reward_market_candles` |
| `packages/backend/order/src/stream.rs` | 聚合 registry token，按 token 分片消费 CLOB `book` + `price_change` WS，并周期性全量 poll 注册 token 做 reconcile |
| `packages/backend/order/src/http_api.rs` | 盘口读取、批量读取、stats、内部 WS stream、token 注册/注销和内部 ingest HTTP API |
| `packages/backend/order/src/http_api/helpers.rs` | HTTP API 私有 helper：写认证、source/token/level 校验、错误/消息响应构造和 DTO 映射 |
| `packages/backend/order/src/http_api/tests.rs` | HTTP API helper 单元测试 |
| `packages/backend/order/src/updates.rs` | Orderbook cache 更新广播器：为 WS/poll/ingest 写入分配 sequence、fan-out 给内部 WS 客户端 |
| `packages/backend/crates/connectors/src/orderbook.rs` | Orderbook service client：HTTP 读写/注册和内部 WS stream 连接 |
| `packages/backend/crates/infrastructure/src/stores/rewards/postgres_candles.rs` | Rewards candle persistence：按 token/5m bucket upsert price-history source OHLC，并读取 AI advisory 所需源 K 线 |
| `packages/backend/crates/infrastructure/src/stores/orderbook_cache.rs` | `InMemoryOrderbookCache`：TTL、最优档排序、深度裁剪 |
| `packages/backend/crates/infrastructure/src/stores/orderbook_registry.rs` | 多来源 token 原子替换、优先级聚合和来源上限 |

## 启动流程

1. 加载 `Runtime` 并绑定 `0.0.0.0:${POLYEDGE_ORDERBOOK__PORT}`。
2. 立即暴露 `/healthz` 和 orderbook HTTP API，避免 Polymarket 延迟阻塞容器健康检查。
3. 后台启动 Gamma full sync 独立循环：initial 立即执行，之后按 `market_sync_interval_secs` 固定节拍同步；单次超时为 interval 的 80%，并限制在 60-240 秒。Gamma `/markets` offset 分页使用固定 page size 100，不再依赖旧 arbitrage scan_limit 配置。full sync upsert 会跳过同版本同内容的行，并只在 `synced_at` 超过 rewards 新鲜度窗口约三分之二时刷新安静市场。
4. 后台启动 Gamma priority sync 独立循环：优先刷新已注册 token 映射的 condition、活跃 rewards 订单/持仓、最终 eligible 或 pre-AI deterministic eligible quote plans 和放宽新鲜度后的 rewards 候选 condition；还有剩余额度时，用 active rewards catalog 中高奖励市场补足作为恢复种子；最多 500 个 condition，单次超时最多 120 秒。priority sync 继续强制刷新 `synced_at`，确保重点市场满足 rewards 新鲜度过滤。
5. priority sync 间隔由 rewards `max_market_data_age_minutes` 动态推导，约为新鲜度窗口三分之一，并限制在 30-300 秒；配置窗口越小，重点市场刷新越频繁。
6. 后台启动 rewards catalog 独立循环：initial 立即执行，之后每次完成后等待 `market_sync_interval_secs`；单次超时 45 分钟，超时或失败时保留上一版 rewards catalog。
7. 后台启动 rewards candle history sync：默认每 300 秒选择最多 600 个 active reward token，按 token 间至少 500ms 的请求间隔调用 CLOB `/prices-history`，首次保留 2 小时 backfill，后续抓取最近 15 分钟增量；遇到 429、认证错误、超时、常见 5xx 或解码失败会停止本轮，避免继续压外部 API。
8. 始终运行盘口流；没有注册 token 时每 10 秒等待一次。
9. registry token 成员集合变化会先等待短暂 debounce 并再次确认，仍变化时默认（`POLYEDGE_ORDERBOOK_STREAM__WS_INCREMENTAL_RECONCILE=true`）保持 WS 连接存活、只对 diff 做增量 subscribe/unsubscribe（SDK 按资产 refcount 只发新增/退订帧），不再整体重建；只有某个 chunk 的 reader 结束（连接死亡）才重建那一个 chunk，其余连接不受影响。`WS_INCREMENTAL_RECONCILE=false` 回退到成员变化即整体重建的旧行为。仅 token 顺序变化不触发任何重订/重连，poll reconciler 仍会使用最新聚合顺序。WS chunk 默认目标大小为 500，并受默认 8 连接硬预算保护；chunk 启动按 500ms 错峰，SDK 重连退避为 30-120 秒。

## HTTP API

| 方法与路径 | 行为 | 写认证 |
|---|---|---|
| `GET /healthz` | 进程健康检查 | 无 |
| `GET /orderbook/{token_id}` | 读取单 token 缓存盘口；不存在返回 404 | 无 |
| `POST /orderbook/batch` | 批量读取存在的缓存盘口；请求体可选 `refresh_if_stale_ms`，仅当目标 token 缺失或 `confirmed_at` 超过该年龄时由 orderbook 服务同步 CLOB `/books` 刷新后再返回 | 无 |
| `GET /orderbook/stats` | 返回 cache/registry 计数，以及 configured/effective WS chunk、最大连接预算和按当前 token 估算的连接数 | 无 |
| `GET /orderbook/stream` | 内部 WebSocket；推送规范化 `OrderbookStreamEvent`（sequence、reason、book）；可选 `?source=...` 只接收该 registry source 当前 token 的更新 | 无 |
| `POST /orderbook/register` | 原子替换一个 source 的有序 token 集合 | `x-polyedge-orderbook-token` |
| `DELETE /orderbook/register/{source}` | 删除一个 source | `x-polyedge-orderbook-token` |
| `POST /orderbook/ingest` | 校验整批盘口后批量写入缓存 | `x-polyedge-orderbook-token` |

写接口要求 `POLYEDGE_ORDERBOOK__WRITE_TOKEN` 已配置且请求头匹配；未配置时写接口返回 503。source 最长 64 字节，只允许 ASCII 字母数字、下划线、连字符、点和冒号；registry 最多保留 32 个非空 source，并在 registry 写锁内再次原子校验上限。

## 缓存与订阅约束

- 每个 source 的 `register_tokens()` 是原子全量替换，不是增量追加；orderbook HTTP 层收到空集合时等同删除 source。worker 周期注册任务会对成功空集合做防抖，`rewards_active`/`exec_orders` 连续 2 轮为空、`rewards_eligible`/`rewards_candidates` 连续 3 轮为空才真正发送空集合清源，查询失败或即时 active 刷新读到空集合会保留上一版 source。
- registry 聚合顺序由 infrastructure 固定，优先级为 `rewards_active`、`exec_orders`、`rewards_eligible`、`rewards_candidates`，最终受 `POLYEDGE_ORDERBOOK_STREAM__MAX_TOKENS` 限制；worker 周期注册任务为每个 source 独立注册全量 token，跨 source 去重和总量截断由聚合层完成。`rewards_eligible` 只注册最终保存后的 eligible quote plan token，`rewards_candidates` 预热 token 另受 `POLYEDGE_ORDERBOOK_STREAM__REWARD_CANDIDATE_TOKEN_CAP` 限制。AI/info-risk provider payload 已与 live orderbook 解耦，不再临时注册或等待盘口。
- Polymarket WS 订阅按 `POLYEDGE_ORDERBOOK_STREAM__WS_CHUNK_SIZE` 分片（默认目标 500 token/连接），并受 `POLYEDGE_ORDERBOOK_STREAM__WS_MAX_CONNECTIONS` 硬预算约束（默认 8）；若配置 chunk 过小，服务会按 `ceil(MAX_TOKENS / WS_MAX_CONNECTIONS)` 自动增大有效 chunk，保留 token 覆盖但限制独立 `ClobWsClient` 数量。所有初始、新增、reader-rebuild 和 full-resync chunk 按每连接 500ms（最多 5s）错峰启动；SDK 懒连接自己的 reconnect 退避配置为 30s 起步、最高 120s，避免多个后台 client 在 Cloudflare 429/1015 后同步快速重试。每个分片仍是长驻 session，快速 drain `book` / `price_change` 后交给缓存写入；外层 client/subscribe 初始化失败保留 2-60s per-chunk backoff。`WS_INCREMENTAL_RECONCILE=false` 时任意分片结束或失败仍会整体重建当前 lifecycle。
- stream refresh 由 registry 变更通知实时唤醒，并保留 `token_refresh_interval_secs` 定时兜底；成员集合变化后做短暂 debounce 再确认，默认增量模式（`WS_INCREMENTAL_RECONCILE=true`）下只对 diff 做增量 subscribe/unsubscribe、保持连接存活（先 subscribe 新集合再 unsubscribe 旧集合，确保共享 Market 通道始终非空），`false` 时回退到整体重建；仅 token 顺序变化不触发重连，poll reconciler 的共享 token 列表仍每次刷新为最新顺序。增量状态若与 CLOB 漂移，由 10s poll reconciler（数据新鲜度）和 SDK 自带的 reconnect 全量重订阅兜底；`POLYEDGE_ORDERBOOK_STREAM__FULL_RESYNC_INTERVAL_SECS>0` 时还会按周期强制 teardown+rebuild 全部连接作为应急逃生口（默认 0=关）。
- 所有缓存写入都会先把 bids 按价格降序、asks 按价格升序排序，再裁剪到 `POLYEDGE_ORDERBOOK_STREAM__MAX_LEVELS_PER_SIDE`，避免上游无序数据丢失 top-of-book。
- WS 同时消费完整 `book` 快照和挂单/撤单触发的 `price_change` 增量；无 size 的增量不修改深度，等待后续快照/poll 对账。
- 缓存拒绝 `observed_at` 早于当前条目的快照或增量；时间戳相同时 WS 条目优先于 poll，避免延迟 poll 覆盖更新盘口；若被拒绝写入携带更新的 `confirmed_at`，缓存会只合并最近确认时间并刷新 TTL，不替换盘口档位。
- 缓存 TTL 按本地写入时间计算；旧 `observed_at` 拒绝规则只适用于未过期条目，已过期条目可被后续 poll/ingest 覆盖恢复；年龄 stale threshold 使用 `confirmed_at`。
- HTTP ingest 会在写入前完成整批 token/price/size 校验，并同样按最优价格排序后裁剪。
- 每次 WS snapshot、WS price_change、poll reconcile 或 HTTP ingest 成功写入缓存后，都会向 `/orderbook/stream` 广播 `OrderbookStreamEvent`；广播消息携带单调 `sequence`、`reason` 和裁剪后的 `CachedOrderBook`。带 `?source=...` 的内部 WS 连接会按该 source 当前注册 token 过滤返回；底层 Polymarket WS 仍按聚合 token 统一订阅。慢消费者会在服务端日志记录 lag，客户端需断线后重新 HTTP bootstrap；`OrderbookStreamClient` 建立内部 WS 连接最多等待 5 秒，避免 orderbook 地址不可达时阻塞 worker 事件循环。
- Rewards candles 不再由每条 orderbook cache 更新派生，避免高频 WS `price_change` 把本地 candle 队列打满。`candle_history.rs` 独立按低频节拍读取 active reward markets，按 `total_daily_rate` 排序后去重 token 并受 `POLYEDGE_ORDERBOOK_STREAM__REWARD_CANDLE_HISTORY_MAX_TOKENS_PER_CYCLE` 限制；每个 token 调用 CLOB `/prices-history` 获取 5 分钟 fidelity 数据并写入 `reward_market_candles`。该数据源不是 bid/ask 盘口，持久化时 `best_bid_close` / `best_ask_close` 等于 provider price、`spread_cents_close=0`，`sample_count` 表示同 bucket 内持久化的 provider history 点数量，不表示成交量。
- Candle history sync 默认启用；关键限流配置为 `POLYEDGE_ORDERBOOK_STREAM__REWARD_CANDLE_HISTORY_SYNC_INTERVAL_SECS=300`、`POLYEDGE_ORDERBOOK_STREAM__REWARD_CANDLE_HISTORY_REQUEST_DELAY_MS=500`、`POLYEDGE_ORDERBOOK_STREAM__REWARD_CANDLE_HISTORY_MAX_TOKENS_PER_CYCLE=600`、`POLYEDGE_ORDERBOOK_STREAM__REWARD_CANDLE_HISTORY_BACKFILL_SECS=7200`、`POLYEDGE_ORDERBOOK_STREAM__REWARD_CANDLE_HISTORY_INCREMENTAL_SECS=900`。interval 会 clamp 到 60-3600 秒，请求间隔 clamp 到 250-10000ms，lookback clamp 到 5 分钟-24 小时；max tokens 设为 0 可跳过本轮 token 请求。
- poll reconciler 默认每 10 秒刷新当前注册 token，优先处理缺失、TTL 过期或超过 stale threshold 的 token，再覆盖其余 token，以修复未被检测到的 WS 增量丢失，并满足 rewards live placement 默认约 35 秒盘口新鲜度窗口；100-token `/books` 批次之间固定间隔 100ms，避免大量注册 token 在同一瞬间形成 REST 请求尖峰。poll 写入保留 CLOB 盘口 timestamp 作为 `observed_at`，用本地 poll 成功时间写 `confirmed_at`；`stale_threshold_ms <= 0` 只关闭年龄 stale 优先级。
- 后台 poll reconciler 与 HTTP `refresh_if_stale_ms` 按需刷新共享同一个公平串行闸门；按需请求在取得闸门后重新检查该 100-token chunk 是否仍 stale，避免 rewards full tick、cache prewarm、provider refresh 和后台 poll 对同一批 token 重复请求 CLOB。两条路径都在上游批次之间等待 100ms。
- `OrderbookHttpClient` 把单盘口 404 映射为 `None`，其他非成功 HTTP 状态映射为 dependency error。普通 `get_books()` 只读 orderbook 服务缓存；`get_books_with_max_age()` 会在 batch 请求中传入 `refresh_if_stale_ms`，orderbook 服务只对缺失或超过该确认年龄的 token 做同步 `/books` 刷新，刷新失败会记录 warn 并返回现有缓存，调用方仍按 `confirmed_at` fail closed。

## 数据流

```text
Gamma API / CLOB rewards API
    → market_sync.rs
    → markets / reward_markets / reward_market_event_windows (Postgres)

Orderbook registry + rewards active/planned/candidate/fallback condition ids
    → market_sync.rs priority sync
    → Gamma `/markets?condition_ids=...`
    → markets.synced_at refresh for priority markets
    → reward_market_event_windows Gamma candidate refresh for rewards markets

Worker register sources
    → POST /orderbook/register
    → OrderbookSubscriptionRegistry
    → CLOB WS + /books batch poll（poll 用 CLOB timestamp 写 `observed_at`、本地接收时间写 `confirmed_at`；遗漏/失败时回退 /book）
    → InMemoryOrderbookCache
    → API / Worker 通过 OrderbookHttpClient 读取
    → Worker 通过 GET /orderbook/stream 接收内部实时推送并维护本地 cache

Active reward tokens
    → candle_history.rs
    → CLOB `/prices-history?market=...&fidelity=5`
    → reward_market_candles (Postgres, 5m price-history source candles; rewards AI 在 application 层聚合为 1h)
```

## 当前状态与缺口

- 市场同步、registry、分片 WS `book` + `price_change`、全注册 token 周期 poll reconcile、HTTP 读取、内部 WS 推送和内部写认证已实现；poll 可修复 fresh cache 中未被察觉的 WS 增量丢失，并通过内部 WS 广播给 worker 本地 cache；内部 WS client 建连最多等待 5 秒，避免不可达地址阻塞调用方。
- HTTP API 主文件保留 handler 和 streaming 主流程；私有校验、响应构造、DTO 映射和写认证测试已按 `include!` 模式拆到 `http_api/` 子文件，路由和响应语义不变。
- Orderbook crate 已收敛到 `packages/backend/order`，仍作为 `packages/backend/Cargo.toml` Rust workspace member 构建。
- orderbook stream 的 token refresh 已接入 registry 变更通知，首次注册和后续成员变化可立即触发检查；仍避免仅因 registry 聚合顺序变化触发 WS 重订/重连，只有订阅 token 成员真实增删并经过短暂 debounce 后仍变化时，默认增量模式才对 diff 做 subscribe/unsubscribe（保持连接存活），`WS_INCREMENTAL_RECONCILE=false` 时才整体重建 Polymarket WS 订阅。
- orderbook stream 已加入 Cloudflare 429/1015 防护：默认 500-token chunk、8 连接预算、500ms chunk 启动错峰、SDK 30-120s reconnect backoff，以及 poll batch 100ms 间隔。启动日志同时输出 configured/effective chunk、连接预算和 SDK 退避参数，便于确认旧 runtime config 是否被自动收敛。
- CLOB REST orderbook 刷新已在进程内统一串行化；HTTP on-demand refresh 会在闸门内二次检查 stale，降低 rewards 批量拉取与 10 秒后台 reconcile 重叠时的重复流量。
- orderbook 缓存把盘口内容版本时间和最近确认时间拆开：`observed_at` 保留 WS/CLOB 响应 timestamp，`confirmed_at` 使用服务本地接收/写入时间表示刚确认过完整盘口；安静市场可能长期没有内容版本变化，但只要 poll 或按需 batch refresh 成功推进 `confirmed_at`，rewards live placement 就不会因内容版本不变被误判 stale。batch HTTP 普通读取通过一次 cache 批量读锁返回；带 `refresh_if_stale_ms` 的读取会先刷新缺失/超龄 token，再读缓存返回。
- Gamma full sync、Gamma priority sync 与 rewards 目录同步在 orderbook 服务中使用三个独立后台循环；rewards 分页和详情补全可能持续很多分钟，但不会阻塞 Gamma `markets.synced_at` 刷新。Gamma full/priority 写入 `markets` 时在 orderbook 进程内串行化，并由 Postgres `lock_timeout` / `statement_timeout` 快速失败，避免一次慢锁等待拖垮后续周期。priority sync 会在全量目录之间强制刷新重点 condition，避免已挂单/已订阅/rewards 筛选市场仅因目录新鲜度过低被策略撤单。rewards 详情补全后仍缺 token 或空目录异常时保留上一版 rewards catalog，不执行破坏性全量替换。
- Gamma market upsert 保存 `liquidity_usd`、`end_at` 和本地 `synced_at`；full sync 跳过同版本同内容行，并按 rewards 新鲜度窗口对安静市场做限频 `synced_at` refresh，priority sync 对重点市场强制 refresh。Postgres upsert 使用单条 `INSERT .. ON CONFLICT DO UPDATE WHERE` 表达新增、内容变化和 freshness-only 刷新。rewards 候选使用该本地同步时间判断目录新鲜度，不依赖市场是否刚好发生上游业务更新。
- Gamma full/priority sync 会从 `startDateIso`/`startDate`、`events[].startDate/endDate` 和 `hasReviewedDates` 派生 rewards 事件窗口候选，通过 `RewardBotService.upsert_market_event_windows()` 写入 `reward_market_event_windows`。默认 `event_window_gamma_unreviewed_dates_mode=ignore` 不保存未审核 Gamma 日期；`observe` 保存 low confidence，`medium_confidence` 保存 medium confidence；`hasReviewedDates=true` 保存为 `gamma_reviewed` medium confidence。默认 hard gate 最低置信度为 high，因此 Gamma 候选不会直接触发事件窗口硬拦截。
- Priority sync 使用本地 `markets` 表把 registry token 映射到 Gamma condition id；无 Postgres 时跳过该映射，仍可使用 rewards service 提供的重点 condition。active rewards catalog fallback 会在 priority 集合未满时按奖励排序继续补充 condition id，避免候选 freshness 全部过期后只能等待 full sync 恢复。
- 盘口只保存在单个 orderbook 进程内；服务重启会丢失缓存，横向多实例之间也不会共享缓存或 registry。
- Rewards token 的 5 分钟 source K 线已持久化到 Postgres，作为 AI advisory 小时级聚合输入；该 K 线由 orderbook 服务低频限速调用 CLOB `/prices-history` 写入，不再消费本地 WS/poll/ingest 高频更新，也不包含真实成交量。
- 读接口和 `/healthz` 当前不鉴权，应依赖内网边界限制访问。
- 市场同步失败或超时不会使 HTTP 健康检查失败；需要通过日志和数据新鲜度单独监控外部依赖。Gamma、CLOB rewards、order book 和 price-history 解码失败会在错误中携带最多 300 字节转义响应体 preview，便于区分 HTML、截断响应或上游结构漂移。

## 修改检查清单

- [ ] 修改 HTTP 路由或写认证时同步更新根 `AGENTS.md`、部署文档和 `OrderbookHttpClient`
- [ ] 修改缓存排序/裁剪/TTL 语义时同步更新 infrastructure 文档和回归测试
- [ ] 修改 registry source 或优先级时同步更新 worker 注册逻辑
- [ ] 运行 `cargo check --workspace --tests`
