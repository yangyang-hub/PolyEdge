# Worker App（后台任务服务）

最后更新：2026-07-07

## 概述

`polyedge-worker` 位于 `packages/backend/apps/worker`。当前它同时提供运维 CLI 和可嵌入 API 进程的 `WorkerRuntime`。Docker 部署中不再常驻单独 worker 容器，`polyedge-api` 会按 `WorkerSettings` 在同进程启动后台任务。

当前 worker 聚焦市场数据配套任务、新闻采集、执行/对账和 LP rewards 自动化。旧钱包类模块和独立研究 worker 已删除；对应 CLI、轮询开关、handler、service、store 与数据库表不再存在。

## 设计目标

- 每个后台任务都可通过 CLI 单独运行，便于运维诊断。
- 常驻任务通过 `WorkerRuntime` 统一启动、重启、关闭和记录日志。
- Handler 只入队命令或读 snapshot，策略执行和外部 API 调用由 worker/orderbook 服务负责。
- Rewards 策略只从 Postgres、orderbook 服务缓存和本地 worker cache 读取市场数据，不在策略路径直接抓外部市场 API。

## 架构与关键文件

| 文件 | 职责 |
|---|---|
| `lib.rs` | CLI 参数解析、任务聚合和 `run_cli()` 入口 |
| `main.rs` | 薄 CLI 入口 |
| `worker/service.rs` | `WorkerRuntime` 生命周期与任务编排；启动 live 任务前检查 Polymarket 凭证完整性 |
| `worker/service_info_risk.rs` | Rewards 异步信息风险扫描 runtime 接线 |
| `worker/database_maintenance.rs` | 数据库 retention 清理任务 |
| `worker/orderbook_registration.rs` | 周期性向 orderbook 服务注册 rewards/执行订单 token |
| `worker/market_sync.rs` | 市场同步 CLI 兼容入口；常驻同步已迁移到 `polyedge-orderbook` |
| `worker/news.rs`、`news_helpers.rs`、`news_promotion.rs` | RSS/Atom 新闻采集和事件/证据提升 |
| `worker/execution_dispatch.rs`、`execution_queue.rs`、`execution_reconcile.rs` | 执行请求分发、队列管理和成交/状态对账 |
| `worker/orderbook_stream.rs` | 旧盘口流 CLI 兼容入口；常驻盘口流由 `polyedge-orderbook` 提供 |
| `worker/polymarket_config.rs`、`polymarket_events.rs` | Live Polymarket connector 配置和用户事件 WS |
| `worker/rewards.rs` | Rewards live tick、控制命令、计划保存和订单生命周期入口 |
| `worker/rewards/account_sync.rs` | 外部余额、开放订单、持仓和当日 rewards earnings 同步 |
| `worker/rewards/live_sync.rs` | 托管订单成交/状态同步、reset/cancel-all 语义 |
| `worker/rewards/live_orders.rs` | 成交入账、撤单、退出/flatten intent 和 merge intent 发现 |
| `worker/rewards/live_submission.rs` | live 单笔订单提交和 submission marker |
| `worker/rewards/live_pending.rs` | durable intent 提交/恢复、BUY last-look 和 SELL 持仓裁剪 |
| `worker/rewards/live_orderbook_risk.rs` | 新挂单新鲜度、stale grace 和等待原因 |
| `worker/rewards/live_requote.rs` | 报价漂移确认、冷却和单轮撤单限速 |
| `worker/rewards/live_placement_limits.rs` | 资金预算、同 condition BUY cap 和最低 rewards size 缺口 |
| `worker/rewards/live_cancel.rs` | 撤单候选和撤单原因分流 |
| `worker/rewards/live_risk.rs` | 下单前/撤单风控 helper |
| `worker/rewards/orderbook_events.rs` | 内部 orderbook WS 消费、本地盘口 cache 和活跃 token wake |
| `worker/rewards/event_cancel.rs` | 盘口事件驱动的 hard-risk cancel-only 快路径 |
| `worker/rewards/polling.rs` | rewards 常驻 poll loop、fast reconcile、外部同步节流和本地盘口预热 |
| `worker/rewards/provider_advisory.rs` | AI advisory 缓存 gate、pre-provider 硬过滤和 LLM 调用记录 |
| `worker/rewards/provider_refresh_orderbook.rs` | provider refresh 前的盘口准备和临时 token 注册 |
| `worker/rewards/provider_refresh.rs` | 后台 combined provider refresh，仅写 advisory/info-risk 缓存 |
| `worker/rewards/provider_refresh_candidates.rs` | provider refresh 候选排序 |
| `worker/rewards/provider_fallback.rs` | 可选备用 LLM provider 重试与缓存读取 |
| `worker/rewards/provider_content_filter.rs` | provider 内容过滤失败处理和 fail-closed 缓存 |
| `worker/rewards/info_risk.rs` | 独立信息风险扫描、缓存写入和 quote plan 风险应用 |
| `worker/shared.rs` | 共享辅助函数 |

## CLI 子命令

| 命令 | 描述 |
|---|---|
| `run`（默认） | 启动兼容常驻 worker runtime |
| `drain-execution-queue` | 处理排队的执行请求 |
| `ingest-news-once` | 一次性采集新闻源 |
| `poll-news` | 持续新闻采集 |
| `promote-news-events` | 将 raw news 提升为 events/evidences |
| `run-database-maintenance-once` | 一次性执行 retention 清理 |
| `scan-rewards-once` | 一次性执行 rewards live tick 或控制命令 |
| `poll-reward-bot` | 持续执行 rewards live loop |
| `scan-reward-info-risks-once` | 一次性扫描 rewards 信息风险 |
| `poll-reward-info-risks` | 持续扫描 rewards 信息风险 |
| `sync-markets-once` | 兼容的一次性市场同步入口 |
| `reconcile-paper-fills` | Paper 成交对账 |
| `poll-paper-order-statuses` | Paper 订单状态轮询 |
| `poll-polymarket-order-statuses` | Live Polymarket 订单状态轮询 |
| `reconcile-polymarket-fills` | Live Polymarket 成交对账 |
| `consume-polymarket-user-events` | 消费 Polymarket 用户事件 WS |

## 核心数据流

### database-maintenance

```text
run_database_maintenance_once()
    -> DatabaseMaintenanceService.prune_history(now)
    -> PostgresDatabaseMaintenanceStore 分批 DELETE
    -> 输出逐表 deleted 计数和 total_deleted
```

生产模板默认开启，本地模板默认关闭。当前清理 raw events、过期 AI/info-risk cache、rewards price-history candles、完成/失败控制命令、outbox/external dedup、LLM calls、audit logs 和 mode transitions。每个表每轮最多 20 批、每批 10,000 行；单表失败只记录 warn。

### register-orderbook-tokens

```text
执行订单 active token
    + rewards 活跃订单/持仓 token
    + 最终 eligible quote plan token
    + provider refresh 临时缺口 token
    + rewards 候选预热 token
    -> POST /orderbook/register
    -> OrderbookSubscriptionRegistry 聚合订阅集合
```

worker 不再直接维护常驻盘口流。它通过 `OrderbookHttpClient` 注册 token，并通过 `OrderbookStreamClient` 接收内部 WS 更新。注册 source 使用原子替换语义；空集合会做防抖，避免短暂查询失败清掉远端订阅。聚合优先级为 `rewards_active`、`exec_orders`、`rewards_eligible`、`rewards_ai_provider`、`rewards_candidates`。

### rewards live loop

```text
poll_reward_bot_until_shutdown()
    -> 读取 RewardBotConfig
    -> 同步托管订单/成交/账户状态
    -> 读取 reward_markets + markets + orderbook 服务盘口
    -> 构建 quote plans 和 opportunity metrics
    -> 应用 AI advisory / info-risk / event window / funding / live orderbook gates
    -> 保存 quote plans
    -> 提交、撤单、重挂、退出 SELL、merge intent
    -> 写入 heartbeat / fills / positions / events / llm_calls
```

LP rewards 策略只做 live 路径。新增 BUY 必须经过最终 quote plan、当前盘口、资金、事件窗口、AI/info-risk、盘口新鲜度、深度/rank/history/requote 和 kill switch 检查。已有订单由 fast reconcile 和独立事件撤单 worker 兜底；活跃 token 的 orderbook 更新会立即触发 cancel-only 风控，不等待完整 full tick。

provider refresh 是后台补缓存任务：同一 condition 的 AI advisory 与信息风险可由一次 combined provider 请求返回；实际外部请求写入 `llm_calls(task_type=reward_provider)`。缺 provider、缓存缺失或 enforce 模式风险拦截时 fail closed。provider 只写缓存，不直接下单或修改最终可挂集合。

SELL 退出 intent 使用非亏损 floor。`ExitAtMarkup`、`HoldAndRequote` 和外部库存补退出走 post-only maker SELL；`FlattenImmediately` 只有在 best bid 不低于 floor 时才用非 post-only FAK/taker SELL。BalancedMerge 会发现可配对库存并创建 `reward_merge_intents`，只有显式开启自动执行后才通过 Safe proxy wallet 广播 CTF merge。

### news

```text
settings.news.sources
    -> RssNewsConnector.fetch()
    -> NewsIngestionService.ingest_source_items()
    -> raw_news_events 去重写入
```

未配置 `POLYEDGE_NEWS__SOURCES_JSON` 时使用 `settings/defaults.rs` 中的默认 RSS/Atom 源。新闻提升任务会把 raw news 转为 events/evidences 候选。

### execution / reconciliation

执行任务保留 paper 与 Polymarket live 两类 connector。生产中 live 任务只有在 `POLYEDGE_POLYMARKET__ACCOUNT_ID`、`POLYEDGE_POLYMARKET__PRIVATE_KEY` 以及三项 API credential 配置完整时才会启动；否则只记录一次 warn 并跳过对应常驻 job。

## 配置关系

- `WorkerSettings` 控制常驻任务开关与轮询间隔。
- `RewardsSettings` 控制 rewards live worker 的进程级启用、poll 间隔和 provider 环境密钥。
- `RewardBotConfig` 存在 `reward_bot_config` 表，控制市场筛选、机会评分、quote/selection、AI/info-risk、事件窗口、库存、requote 和 BalancedMerge 等业务参数。
- `POLYEDGE_ORDERBOOK__SERVICE_URL` 指向 orderbook 服务；`POLYEDGE_ORDERBOOK__WRITE_TOKEN` 用于 token 注册。

## 当前状态

- `polyedge-worker` 仍可单独执行 CLI，但常规部署由 API 内嵌 runtime 启动任务。
- 常驻 runtime 可启动 database maintenance、news ingest/promotion、orderbook token registration、rewards live loop、rewards info-risk scan、execution drain、paper/live 订单状态与成交对账、Polymarket 用户事件 WS。
- 市场目录同步、rewards catalog、price-history candles 和常驻 orderbook WS/poll cache 已迁移到 `polyedge-orderbook`。
- Rewards LP 策略不依赖已删除的研究表或 EV 审计表；quote planning 使用市场质量、机会评分、AI/info-risk、事件窗口、资金和 live 盘口风控。
- 数据库维护不再清理旧钱包类表；这些 schema 已从迁移和 `init.sql` 中移除。

## 修改检查清单

- [ ] 新增 worker 任务时同步更新 CLI、`WorkerRuntime` 接线、配置模板和本文档。
- [ ] 修改 rewards 行为时同步检查 application service、store、frontend DTO 和 rewards 页面文档。
- [ ] 修改 orderbook 注册 source/优先级时同步更新 orderbook 文档和根 `AGENTS.md`。
- [ ] 运行 `cargo check --workspace --tests`。
