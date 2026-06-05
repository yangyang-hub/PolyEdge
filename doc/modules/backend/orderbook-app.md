# Orderbook App（市场同步与盘口服务）

最后更新：2026-06-04

## 概述

`polyedge-orderbook` 是独立的 Axum/Tokio 服务，负责从 Polymarket 同步通用市场和奖励市场、维护 CLOB WS + poll 盘口流、保存进程内盘口缓存，并通过 HTTP 向 API/worker 提供盘口读取和订阅注册能力。

## 架构与关键文件

| 文件 | 职责 |
|---|---|
| `apps/orderbook/src/main.rs` | 服务入口：先 bind HTTP，再启动市场同步和可重启盘口流后台任务 |
| `apps/orderbook/src/market_sync.rs` | Gamma markets + CLOB reward markets → Postgres |
| `apps/orderbook/src/stream.rs` | 聚合 registry token，消费 CLOB `book` + `price_change` WS，并周期性全量 poll 注册 token 做 reconcile |
| `apps/orderbook/src/http_api.rs` | 盘口读取、批量读取、stats、token 注册/注销和内部 ingest HTTP API |
| `crates/infrastructure/src/stores/orderbook_cache.rs` | `InMemoryOrderbookCache`：TTL、最优档排序、深度裁剪 |
| `crates/infrastructure/src/stores/orderbook_registry.rs` | 多来源 token 原子替换、优先级聚合和来源上限 |

## 启动流程

1. 加载 `Runtime` 并绑定 `0.0.0.0:${POLYEDGE_ORDERBOOK__PORT}`。
2. 立即暴露 `/healthz` 和 orderbook HTTP API，避免 Polymarket 延迟阻塞容器健康检查。
3. 后台执行 initial market sync，之后按 `market_sync_interval_secs` 周期同步。
4. 始终运行盘口流；没有注册 token 时每 10 秒等待一次。
5. registry token 集合变化、WS 结束或 stream 报错后，按 `restart_interval_secs` 重建订阅。

## HTTP API

| 方法与路径 | 行为 | 写认证 |
|---|---|---|
| `GET /healthz` | 进程健康检查 | 无 |
| `GET /orderbook/{token_id}` | 读取单 token 缓存盘口；不存在返回 404 | 无 |
| `POST /orderbook/batch` | 批量读取存在的缓存盘口 | 无 |
| `GET /orderbook/stats` | 返回 cache 条目数、registry 来源数、registry 去重 token 数 | 无 |
| `POST /orderbook/register` | 原子替换一个 source 的有序 token 集合 | `x-polyedge-orderbook-token` |
| `DELETE /orderbook/register/{source}` | 删除一个 source | `x-polyedge-orderbook-token` |
| `POST /orderbook/ingest` | 校验整批盘口后批量写入缓存 | `x-polyedge-orderbook-token` |

写接口要求 `POLYEDGE_ORDERBOOK__WRITE_TOKEN` 已配置且请求头匹配；未配置时写接口返回 503。source 最长 64 字节，只允许 ASCII 字母数字、下划线、连字符、点和冒号；registry 最多保留 32 个非空 source，并在 registry 写锁内再次原子校验上限。

## 缓存与订阅约束

- 每个 source 的 `register_tokens()` 是原子全量替换，不是增量追加；空集合等同删除 source。
- registry 聚合顺序由 infrastructure 固定，优先级为 `rewards_active`、`exec_orders`、`rewards_candidates`、`copytrade`，最终受 `POLYEDGE_ORDERBOOK_STREAM__MAX_TOKENS` 限制。
- 所有缓存写入都会先把 bids 按价格降序、asks 按价格升序排序，再裁剪到 `POLYEDGE_ORDERBOOK_STREAM__MAX_LEVELS_PER_SIDE`，避免上游无序数据丢失 top-of-book。
- WS 同时消费完整 `book` 快照和挂单/撤单触发的 `price_change` 增量；无 size 的增量不修改深度，等待后续快照/poll 对账。
- 缓存拒绝 `observed_at` 早于当前条目的快照或增量，避免延迟 poll/WS 覆盖更新盘口。
- HTTP ingest 会在写入前完成整批 token/price/size 校验，并同样按最优价格排序后裁剪。
- poll reconciler 每个周期都会刷新当前注册 token，优先处理缺失、TTL 过期或超过 stale threshold 的 token，再覆盖其余 token，以修复未被检测到的 WS 增量丢失；`stale_threshold_ms <= 0` 只关闭年龄 stale 优先级。
- `OrderbookHttpClient` 把单盘口 404 映射为 `None`，其他非成功 HTTP 状态映射为 dependency error。

## 数据流

```text
Gamma API / CLOB rewards API
    → market_sync.rs
    → markets / reward_markets (Postgres)

Worker register sources
    → POST /orderbook/register
    → OrderbookSubscriptionRegistry
    → CLOB WS + /books batch poll（遗漏/失败时回退 /book）
    → InMemoryOrderbookCache
    → API / Worker 通过 OrderbookHttpClient 读取
```

## 当前状态与缺口

- 市场同步、registry、WS `book` + `price_change`、全注册 token 周期 poll reconcile、HTTP 读取和内部写认证已实现；poll 可修复 fresh cache 中未被察觉的 WS 增量丢失。
- Gamma 与 rewards 目录同步相互独立；rewards 分页、详情补全或空目录异常时保留上一版 rewards catalog，不执行破坏性全量替换。
- 盘口只保存在单个 orderbook 进程内；服务重启会丢失缓存，横向多实例之间也不会共享缓存或 registry。
- 读接口和 `/healthz` 当前不鉴权，应依赖内网边界限制访问。
- 市场同步失败不会使 HTTP 健康检查失败；需要通过日志和数据新鲜度单独监控外部依赖。

## 修改检查清单

- [ ] 修改 HTTP 路由或写认证时同步更新根 `AGENTS.md`、部署文档和 `OrderbookHttpClient`
- [ ] 修改缓存排序/裁剪/TTL 语义时同步更新 infrastructure 文档和回归测试
- [ ] 修改 registry source 或优先级时同步更新 worker 注册逻辑
- [ ] 运行 `cargo check --workspace --tests`
