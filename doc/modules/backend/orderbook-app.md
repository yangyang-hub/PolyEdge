# Orderbook App（市场同步与盘口服务）

最后更新：2026-06-19

## 概述

`polyedge-orderbook` 是独立的 Axum/Tokio 服务，crate 位于 `packages/orderbook`。它负责从 Polymarket 同步通用市场和奖励市场、维护 CLOB WS + poll 盘口流、保存进程内盘口缓存，从内部盘口更新派生 rewards token midpoint K 线，并通过 HTTP/内部 WS 向 API/worker 提供盘口读取、实时推送和订阅注册能力。

## 架构与关键文件

| 文件 | 职责 |
|---|---|
| `packages/orderbook/src/main.rs` | 服务入口：先 bind HTTP，再启动独立 Gamma full/priority sync、rewards catalog sync 和可重启盘口流后台任务；监听地址和 signal shutdown 复用 `polyedge-common` |
| `packages/orderbook/src/market_sync.rs` | Gamma full sync、priority condition sync（已注册 token/rewards 重点市场）与 CLOB reward markets 单次同步实现 → Postgres |
| `packages/orderbook/src/stream.rs` | 聚合 registry token，按 token 分片消费 CLOB `book` + `price_change` WS，并周期性全量 poll 注册 token 做 reconcile |
| `packages/orderbook/src/http_api.rs` | 盘口读取、批量读取、stats、内部 WS stream、token 注册/注销和内部 ingest HTTP API |
| `packages/orderbook/src/updates.rs` | Orderbook cache 更新广播器：为 WS/poll/ingest 写入分配 sequence、fan-out 给内部 WS 客户端，并异步记录 rewards midpoint candles |
| `packages/backend/crates/connectors/src/orderbook.rs` | Orderbook service client：HTTP 读写/注册和内部 WS stream 连接 |
| `packages/backend/crates/infrastructure/src/stores/rewards/postgres_candles.rs` | Rewards candle persistence：按 token/5m bucket upsert orderbook midpoint OHLC，并读取 AI advisory 最近 K 线 |
| `packages/backend/crates/infrastructure/src/stores/orderbook_cache.rs` | `InMemoryOrderbookCache`：TTL、最优档排序、深度裁剪 |
| `packages/backend/crates/infrastructure/src/stores/orderbook_registry.rs` | 多来源 token 原子替换、优先级聚合和来源上限 |

## 启动流程

1. 加载 `Runtime` 并绑定 `0.0.0.0:${POLYEDGE_ORDERBOOK__PORT}`。
2. 立即暴露 `/healthz` 和 orderbook HTTP API，避免 Polymarket 延迟阻塞容器健康检查。
3. 后台启动 Gamma full sync 独立循环：initial 立即执行，之后按 `market_sync_interval_secs` 固定节拍同步；单次超时为 interval 的 80%，并限制在 60-240 秒。full sync upsert 会跳过同版本同内容的行，并只在 `synced_at` 超过 rewards 新鲜度窗口约三分之二时刷新安静市场。
4. 后台启动 Gamma priority sync 独立循环：优先刷新已注册 token 映射的 condition、活跃 rewards 订单/持仓、eligible quote plans 和放宽新鲜度后的 rewards 候选 condition；还有剩余额度时，用 active rewards catalog 中高奖励市场补足作为恢复种子；最多 500 个 condition，单次超时最多 120 秒。priority sync 继续强制刷新 `synced_at`，确保重点市场满足 rewards 新鲜度过滤。
5. priority sync 间隔由 rewards `max_market_data_age_minutes` 动态推导，约为新鲜度窗口三分之一，并限制在 30-300 秒；配置窗口越小，重点市场刷新越频繁。
6. 后台启动 rewards catalog 独立循环：initial 立即执行，之后每次完成后等待 `market_sync_interval_secs`；单次超时 45 分钟，超时或失败时保留上一版 rewards catalog。
7. 始终运行盘口流；没有注册 token 时每 10 秒等待一次。
8. registry token 成员集合变化、WS 结束或 stream 报错后，按 `restart_interval_secs` 重建订阅；仅 token 顺序变化不会重连，poll reconciler 仍会使用最新聚合顺序。

## HTTP API

| 方法与路径 | 行为 | 写认证 |
|---|---|---|
| `GET /healthz` | 进程健康检查 | 无 |
| `GET /orderbook/{token_id}` | 读取单 token 缓存盘口；不存在返回 404 | 无 |
| `POST /orderbook/batch` | 批量读取存在的缓存盘口 | 无 |
| `GET /orderbook/stats` | 返回 cache 条目数、registry 来源数、registry 去重 token 数 | 无 |
| `GET /orderbook/stream` | 内部 WebSocket；推送规范化 `OrderbookStreamEvent`（sequence、reason、book） | 无 |
| `POST /orderbook/register` | 原子替换一个 source 的有序 token 集合 | `x-polyedge-orderbook-token` |
| `DELETE /orderbook/register/{source}` | 删除一个 source | `x-polyedge-orderbook-token` |
| `POST /orderbook/ingest` | 校验整批盘口后批量写入缓存 | `x-polyedge-orderbook-token` |

写接口要求 `POLYEDGE_ORDERBOOK__WRITE_TOKEN` 已配置且请求头匹配；未配置时写接口返回 503。source 最长 64 字节，只允许 ASCII 字母数字、下划线、连字符、点和冒号；registry 最多保留 32 个非空 source，并在 registry 写锁内再次原子校验上限。

## 缓存与订阅约束

- 每个 source 的 `register_tokens()` 是原子全量替换，不是增量追加；空集合等同删除 source。
- registry 聚合顺序由 infrastructure 固定，优先级为 `rewards_active`、`exec_orders`、`rewards_eligible`、`rewards_candidates`、`copytrade`，最终受 `POLYEDGE_ORDERBOOK_STREAM__MAX_TOKENS` 限制；worker 周期注册任务为每个 source 独立注册全量 token，跨 source 去重和总量截断由聚合层完成，`rewards_eligible` 注册全部 eligible quote plan token（不再因 active 持仓覆盖而被清空，避免两个注册者交替写入触发 WS 重建振荡）；`rewards_candidates` 预热 token 还受 `POLYEDGE_ORDERBOOK_STREAM__REWARD_CANDIDATE_TOKEN_CAP` 限制，避免候选池填满全局订阅预算。
- Polymarket WS 订阅按 `POLYEDGE_ORDERBOOK_STREAM__WS_CHUNK_SIZE` 分片成多条连接，降低单连接高消息量导致 SDK broadcast lag 的风险；任意分片结束或失败时，当前 stream lifecycle 会整体重建。
- stream refresh 只用 token 成员集合判断是否需要重建 WS 订阅，避免候选/eligible 查询排序抖动导致同一批 token 反复断线重连；poll reconciler 的共享 token 列表仍每次刷新为最新顺序。
- 所有缓存写入都会先把 bids 按价格降序、asks 按价格升序排序，再裁剪到 `POLYEDGE_ORDERBOOK_STREAM__MAX_LEVELS_PER_SIDE`，避免上游无序数据丢失 top-of-book。
- WS 同时消费完整 `book` 快照和挂单/撤单触发的 `price_change` 增量；无 size 的增量不修改深度，等待后续快照/poll 对账。
- 缓存拒绝 `observed_at` 早于当前条目的快照或增量；时间戳相同时 WS 条目优先于 poll，避免延迟 poll 覆盖更新盘口。
- 缓存 TTL 按本地写入时间计算；旧 `observed_at` 拒绝规则只适用于未过期条目，已过期条目可被后续 poll/ingest 覆盖恢复。
- HTTP ingest 会在写入前完成整批 token/price/size 校验，并同样按最优价格排序后裁剪。
- 每次 WS snapshot、WS price_change、poll reconcile 或 HTTP ingest 成功写入缓存后，都会向 `/orderbook/stream` 广播 `OrderbookStreamEvent`；广播消息携带单调 `sequence`、`reason` 和裁剪后的 `CachedOrderBook`。慢消费者会在服务端日志记录 lag，客户端需断线后重新 HTTP bootstrap；`OrderbookStreamClient` 建立内部 WS 连接最多等待 5 秒，避免 orderbook 地址不可达时阻塞 worker 事件循环。
- 每次广播缓存更新时，orderbook 服务会从当前 best bid/ask 派生 5 分钟 midpoint candle，并写入 `reward_market_candles`；只有 bid/ask 都有效且对应 token 属于 active reward market 时才记录，字段是盘口派生 OHLC、收盘 bid/ask、spread 和 sample_count，不表示真实成交 K 线或成交量。
- poll reconciler 每个周期都会刷新当前注册 token，优先处理缺失、TTL 过期或超过 stale threshold 的 token，再覆盖其余 token，以修复未被检测到的 WS 增量丢失；`stale_threshold_ms <= 0` 只关闭年龄 stale 优先级。
- `OrderbookHttpClient` 把单盘口 404 映射为 `None`，其他非成功 HTTP 状态映射为 dependency error。

## 数据流

```text
Gamma API / CLOB rewards API
    → market_sync.rs
    → markets / reward_markets (Postgres)

Orderbook registry + rewards active/planned/candidate/fallback condition ids
    → market_sync.rs priority sync
    → Gamma `/markets?condition_ids=...`
    → markets.synced_at refresh for priority markets

Worker register sources
    → POST /orderbook/register
    → OrderbookSubscriptionRegistry
    → CLOB WS + /books batch poll（使用 CLOB 响应 timestamp；遗漏/失败时回退 /book）
    → InMemoryOrderbookCache
    → reward_market_candles (Postgres, 5m midpoint candles for rewards AI)
    → API / Worker 通过 OrderbookHttpClient 读取
    → Worker 通过 GET /orderbook/stream 接收内部实时推送并维护本地 cache
```

## 当前状态与缺口

- 市场同步、registry、分片 WS `book` + `price_change`、全注册 token 周期 poll reconcile、HTTP 读取、内部 WS 推送和内部写认证已实现；poll 可修复 fresh cache 中未被察觉的 WS 增量丢失，并通过内部 WS 广播给 worker 本地 cache；内部 WS client 建连最多等待 5 秒，避免不可达地址阻塞调用方。
- Orderbook crate 已从 `packages/backend/apps/orderbook` 拆到顶层 `packages/orderbook`，仍作为 `packages/Cargo.toml` Rust workspace member 构建。
- orderbook stream 的 token refresh 已避免仅因 registry 聚合顺序变化触发 WS 重连；只有订阅 token 成员真实增删时才重建 Polymarket WS 订阅。
- poll 盘口保留 CLOB 返回的服务端毫秒时间戳，不再用 HTTP 响应完成时间伪造新鲜度；batch HTTP 读取通过一次 cache 批量读锁返回。
- Gamma full sync、Gamma priority sync 与 rewards 目录同步在 orderbook 服务中使用三个独立后台循环；rewards 分页和详情补全可能持续很多分钟，但不会阻塞 Gamma `markets.synced_at` 刷新。Gamma full/priority 写入 `markets` 时在 orderbook 进程内串行化，并由 Postgres `lock_timeout` / `statement_timeout` 快速失败，避免一次慢锁等待拖垮后续周期。priority sync 会在全量目录之间强制刷新重点 condition，避免已挂单/已订阅/rewards 筛选市场仅因目录新鲜度过低被策略撤单。rewards 详情补全后仍缺 token 或空目录异常时保留上一版 rewards catalog，不执行破坏性全量替换。
- Gamma market upsert 保存 `liquidity_usd`、`end_at` 和本地 `synced_at`；full sync 跳过同版本同内容行，并按 rewards 新鲜度窗口对安静市场做限频 `synced_at` refresh，priority sync 对重点市场强制 refresh。Postgres upsert 使用单条 `INSERT .. ON CONFLICT DO UPDATE WHERE` 表达新增、内容变化和 freshness-only 刷新。rewards 候选使用该本地同步时间判断目录新鲜度，不依赖市场是否刚好发生上游业务更新。
- Priority sync 使用本地 `markets` 表把 registry token 映射到 Gamma condition id；无 Postgres 时跳过该映射，仍可使用 rewards service 提供的重点 condition。active rewards catalog fallback 会在 priority 集合未满时按奖励排序继续补充 condition id，避免候选 freshness 全部过期后只能等待 full sync 恢复。
- 盘口只保存在单个 orderbook 进程内；服务重启会丢失缓存，横向多实例之间也不会共享缓存或 registry。
- Rewards token 的 5 分钟 midpoint K 线已持久化到 Postgres，作为 AI advisory 的短期价格结构输入；该 K 线由 orderbook 内部 WS/poll/ingest 更新派生，不包含真实成交量。
- 读接口和 `/healthz` 当前不鉴权，应依赖内网边界限制访问。
- 市场同步失败或超时不会使 HTTP 健康检查失败；需要通过日志和数据新鲜度单独监控外部依赖。

## 修改检查清单

- [ ] 修改 HTTP 路由或写认证时同步更新根 `AGENTS.md`、部署文档和 `OrderbookHttpClient`
- [ ] 修改缓存排序/裁剪/TTL 语义时同步更新 infrastructure 文档和回归测试
- [ ] 修改 registry source 或优先级时同步更新 worker 注册逻辑
- [ ] 运行 `cargo check --workspace --tests`
