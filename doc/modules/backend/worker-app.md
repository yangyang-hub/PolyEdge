# Worker App（后台任务服务）

最后更新：2026-07-11

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

生产模板默认开启，本地模板默认关闭。当前清理 raw events、过期 AI/info-risk cache、rewards price-history candles、fair-value history、strategy run ledger、order transitions、完成/失败控制命令、outbox/external dedup、LLM calls、audit logs 和 mode transitions。每个表每轮最多 20 批、每批 10,000 行；单表失败只记录 warn。

### register-orderbook-tokens

```text
执行订单 active token
    + rewards 活跃订单/持仓 token
    + 最终 eligible quote plan token
    + rewards 候选预热 token
    -> POST /orderbook/register
    -> OrderbookSubscriptionRegistry 聚合订阅集合
```

worker 不再直接维护常驻盘口流。它通过 `OrderbookHttpClient` 注册 token，并通过 `OrderbookStreamClient` 接收内部 WS 更新。注册 source 使用原子替换语义；空集合会做防抖，避免短暂查询失败清掉远端订阅。聚合优先级为 `rewards_active`、`exec_orders`、`rewards_eligible`、`rewards_candidates`。Provider refresh 不依赖 live 盘口，因此不再注册临时 provider source。

### rewards live loop

```text
poll_reward_bot_until_shutdown()
    -> 读取 RewardBotConfig
    -> 同步托管订单/成交/账户状态
    -> 读取 reward_markets + markets + orderbook 服务盘口
    -> RewardBotService.build_strategy_input 装配可序列化 RewardStrategyInput 快照（单一读路径 + 单一注入 now），from_strategy_input 派生 cycle
    -> RewardDecisionEngine 执行 pre-provider opportunity/fair-value/funding/selection gates
    -> 应用 AI advisory / info-risk cache
    -> RewardDecisionEngine 执行 first-quote gate 和最终 readiness/opportunity/fair-value/selection refresh
    -> 创建 strategy run ledger，保存 quote plans + decisions
    -> RewardActionPlanner 写入执行前 planned actions
    -> 提交、撤单、重挂、退出 SELL、merge intent
    -> 写入 heartbeat / fills / positions / events / llm_calls / actions / order transitions
```

Rewards market maker 只做 live 路径，LP 奖励是次级收益信号。新增 BUY 必须经过最终 quote plan、maker selection priority、fair-value raw/effective trading edge、当前盘口、`maker_market_budget_usd`、钱包余额、provider modifier、单侧库存、全局潜在成交暴露、事件窗口、盘口新鲜度、深度/rank/history/requote 和 kill switch 检查。全局库存上限同时预留托管及账户快照中的非托管 resting BUY，防止多个订单在下次 reconcile 前并发成交。Rewards minimum 不能覆盖这些风险预算。

Full tick 会通过 application 层 `RewardDecisionEngine` 在 pre-provider、post-provider 和最终 snapshot 三个阶段重算 opportunity/fair-value/readiness/selection 并重排 plans。Worker 仍负责读取 provider cache、触发 provider refresh、外部订单/账户同步和 live 下单/撤单。live placement 依顺序扫描 plans，因此 `max_markets` 和资金占用优先给 effective trading edge、退出能力和稳定性更好的市场；reward density 只是独立 10% 次级信号。

Live orderbook validation 失败只在当前计划上记录 60 秒 skip，下一次 full tick 会用最新盘口重新验证，不继承旧计划的安全边际、HHI、深度集中度或 spread 失败。worker 读取大量候选盘口时先保留 active order/position token、再补候选并受 `MAX_TOKENS` 总量限制；单个 orderbook HTTP batch 最多 500 token，避免一个 2,000-3,000 token 请求占满 30 秒 client timeout。

Full tick 现在会创建 shadow strategy run ledger：run 保存 account、trace、trigger、配置 hash、配置 JSON 和输入摘要；每次保存 quote plans 会写入同一 run 的 decision 快照，并把 `RewardQuotePlan.latest_run_id` 指向该 run。执行 merge create/execute、cancel/cancel-replace、pending submit 和 placement submit 前，worker 通过 application `RewardActionPlanner` 写入 `status=planned` 的 strategy actions；live tick outcome 持久化时会基于同一 trace 和 idempotency key 更新对应 actions 并追加 order transitions，不改变既有下单、撤单、成交、退出或 BalancedMerge 逻辑。fast reconcile 没有 strategy run 上下文，仍只走原有执行/持久化路径。

Standard quote materializer 默认从买一开始，最多搜索到 `quote_max_bid_rank`，选择首个同时满足 post-only、reward spread 和 `raw - uncertainty - provider edge buffer` 的价格；不会为了 LP reward 接受负交易 edge。Fair-value latest/history 继续用于审计，maker selection 的 edge 分只使用 `effective_edge_cents`，`reward_adjusted_edge_cents` 只展示。BUY submission last-look 使用相同 edge、单市场预算和全局潜在暴露口径。

provider refresh 是后台补缓存任务：同一 condition 的 AI advisory 与信息风险可由一次 combined provider 请求返回；实际外部请求写入 `llm_calls(task_type=reward_provider)`。AI payload 只含结构市场事实和完成的粗粒度 candles，输出 `allow/reduce/stop_new`、size multiplier 与 edge buffer；info-risk payload 只含稳定市场身份和证据搜索边界，可输出定向 cancel。Provider 不读盘口/账户/库存、不选择价格/方向/rank，且只写缓存。Info cancel 必须由代码验证置信度、24 小时内来源和可归因事件；breaking news 需要两个独立来源。

成交后不再执行 sibling blanket cancel；互补 BUY 继续由自身 edge、库存和显式风险动作管理。正常 `ExitAtMarkup` / `HoldAndRequote` maker SELL 以库存成本或加价为目标，`maker_max_exit_loss_cents` 只定义紧急 flatten 的独立风险 floor。`Adaptive` 根据 quote plan、event/provider/fair-value/live 硬风险和 floor 上方深度选择 maker hold/markup 或受控损失 flatten；定向 info cancel 只把命中的 outcome 视为 hard risk，互补库存保持 maker exit。原有 pending 重评、cancel-replace 冷却/次数/对账确认保障继续保留。BalancedMerge 继续发现配对库存并创建 merge intent。

同一 condition 的 standard/BalancedMerge 计划在订单撤单、event fast path 和 BUY last-look 中按 `(condition_id, strategy_profile)` 精确匹配；condition-scoped provider refresh 会聚合同市场全部 profile，只要任一 profile 通过 pre-provider gate 就不会被另一个 blocked profile 覆盖。

撤单按语义分层：缺/旧盘口、穿价、fair-value edge 失效、事件 cancel 与有证据 info cancel 属于 hard/adverse，立即或短确认撤销；安全目标价下调使用 `adverse_requote_*` 且不受竞争性限速；目标价上调才使用 `requote_drift_*` 的确认、冷却和每轮上限。计划因低分、等待 provider、预算不足或 stop-new 变为不可挂时，不会仅凭 `eligible=false` 撤销安全 resting BUY。

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
- `RewardBotConfig` 存在 `reward_bot_config` 表，控制 `maker_market_budget_usd`、动态 rank、交易 edge、机会评分、adaptive 退出、provider 动作阈值、事件窗口、库存偏斜、非对称 requote 和 BalancedMerge。旧 `per_market_usd` / `quote_size_usd` / `cancel_on_fill` 已删除。
- `POLYEDGE_ORDERBOOK__SERVICE_URL` 指向 orderbook 服务；`POLYEDGE_ORDERBOOK__WRITE_TOKEN` 用于 token 注册。

## 当前状态

- `polyedge-worker` 仍可单独执行 CLI，但常规部署由 API 内嵌 runtime 启动任务。
- 常驻 runtime 可启动 database maintenance、news ingest/promotion、orderbook token registration、rewards live loop、rewards info-risk scan、execution drain、paper/live 订单状态与成交对账、Polymarket 用户事件 WS。
- 市场目录同步、rewards catalog、price-history candles 和常驻 orderbook WS/poll cache 已迁移到 `polyedge-orderbook`。
- Rewards market maker 策略不依赖已删除的研究表；quote planning 使用市场质量、机会评分、maker selection score、fair-value、AI/info-risk、事件窗口、资金和 live 盘口风控。Full tick 的确定性计划变换已集中到 application `RewardDecisionEngine`，执行前 action proposal 已由 application `RewardActionPlanner` 生成，live 副作用仍由 worker 执行。
- Rewards full tick 的盘口验证失败不再跨 tick 锁定；worker 盘口读取总量受 `MAX_TOKENS` 限制，且到 orderbook 的按需刷新以最多 500 token/HTTP 请求分批，依赖 orderbook 服务统一调度 CLOB REST 请求。
- Rewards full tick 已写 strategy run/decision/action/order transition ledger，用于生产前演练审计；该 ledger 现在包含执行前 planned actions 和 outcome 更新后的动作结果，但尚未作为 live 交易决策输入。
- 数据库维护不再清理旧钱包类表；这些 schema 已从迁移和 `init.sql` 中移除。Strategy run ledger 的 completed/failed/cancelled runs 默认保留 90 天，order transitions 默认保留 180 天。

## 修改检查清单

- [ ] 新增 worker 任务时同步更新 CLI、`WorkerRuntime` 接线、配置模板和本文档。
- [ ] 修改 rewards 行为时同步检查 application service、store、frontend DTO 和 rewards 页面文档。
- [ ] 修改 orderbook 注册 source/优先级时同步更新 orderbook 文档和根 `AGENTS.md`。
- [ ] 运行 `cargo check --workspace --tests`。
