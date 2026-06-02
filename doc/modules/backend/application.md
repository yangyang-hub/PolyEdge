# application（应用/服务层）

最后更新：2026-06-02

## 概述

`polyedge_application` crate 是系统最大的 crate，包含所有业务逻辑服务、Store trait（端口定义）、命令/查询类型和领域视图模型。它定义了系统**做什么**，但不关心**怎么做**（由 infrastructure/connectors 实现）。

## 设计目标

- 遵循六边形架构：定义 Store trait 作为端口，不依赖具体实现
- 所有业务编排逻辑集中在本层
- 通过 `Arc<dyn XxxStore>` 和 `Arc<dyn XxxService>` 实现依赖注入

## 架构与关键文件

| 文件/目录 | 职责 |
|---|---|
| `lib.rs` | 模块声明 + `pub use` 收敛对外 API（~86 行） |
| `system_mode.rs` | 系统模式管理：`SystemModeService`、`ModeStateStore`、`IdempotencyStore`、`AuditLogSink`、`AuthenticatedActor` |
| `market_event/` | 核心市场/事件/信号：`MarketEventService`、`MarketEventStore`（最大 Store trait） |
| `execution/` | 执行管道：`ExecutionService`（组合 MarketEventService + RiskService） |
| `risk.rs` | 风控：`RiskService`、`RiskStateStore`、`RiskPolicy`、kill-switch 命令 |
| `rewards/` | 做市奖励：`RewardBotService`、`RewardBotStore`、live 执行模式、订单分页查询 |
| `copytrade/` | 跟单：`CopyTradeService`、`CopyTradeStore`、模拟引擎 |
| `arbitrage/` | 套利：`ArbitrageService`、`ArbitrageStore`、机会检测/验证 |
| `news_ingestion.rs` | 新闻采集：`NewsIngestionService`、`NewsIngestionStore` |
| `orderbook_cache.rs` | 盘口缓存：`OrderbookCache` trait |
| `orderbook_registry.rs` | 盘口订阅注册中心：`OrderbookSubscriptionRegistry` trait，多来源 token 聚合 |
| `wallet_analysis/` | 钱包分析：纯计算（无 I/O），`build_wallet_analysis_report` |
| `list_filters.rs` | 通用分页/过滤辅助 |

## 核心数据结构与服务

### system_mode — 系统治理骨架

**Store Traits（3 个）：**
- `ModeStateStore`：`current() -> ModeSnapshot`、`transition(command) -> ModeSnapshot`
- `IdempotencyStore`：`begin(request) -> IdempotencyBegin`、`complete(request, response_json)`、`fail(request, error_code)`
- `AuditLogSink`：`append(entry)`

**关键类型：**
- `AuthenticatedActor`：user_id、session_id、roles、request_id、ip、user_agent — 被几乎所有模块引用
- `ModeSnapshot`：当前 SystemMode、环境、版本、更新时间
- `ModeTransitionCommand`：目标模式、原因、幂等键、请求哈希、actor、required_scope
- `AuditLogEntry`：完整审计日志条目

**服务：** `SystemModeService` — 管理模式转换（含幂等性检查和审计日志）

### market_event — 核心数据管理

**Store Trait：** `MarketEventStore` — 系统最大的 Store trait
- Markets：`list_markets`、`count_markets`、`get_market`、`upsert_markets`、`list_market_categories`
- Signals：`get_signal`、`list_signals`、`recompute_signal`、`approve_signal`、`reject_signal`
- Events/Evidence：`list_events`、`list_evidences`、`list_probability_estimates`
- Execution：`list_order_drafts`、`list_execution_requests`、`submit_execution_request`
- Orders/Trades/Positions：`list_orders`、`list_trades`、`get_order_by_external_ref`、`list_positions`
- Dispatch/Reconciliation：`list_dispatch_candidates`、`list_reconciliation_candidates`、`mark_execution_submitted`、`reconcile_execution_fill` 等

**服务：** `MarketEventService` — 所有方法是对 Store 的薄代理

### execution — 执行管道

**服务：** `ExecutionService`
- 依赖：`Arc<MarketEventService>` + `Arc<RiskService>` + `Arc<dyn AuditLogSink>`
- 不定义自己的 Store trait，复用 `MarketEventStore`
- 关键方法：`submit_execution_request`（校验信号 ID 和原因，委托到 MarketEventStore）

### risk — 风控

**关键类型：**
- `RiskStateSnapshot`：kill_switch、daily_pnl、gross/net_exposure、open_alerts、version
- `RiskPolicy`：可配置阈值（exposure_reference_nav、min_signal_confidence、max_daily_loss 等）
- `ApproveSignalCommand`/`RejectSignalCommand`：乐观并发控制（expected_version）

**服务：** `RiskService` — 信号审批/拒绝、kill-switch 触发/释放

### rewards — 做市奖励

**Store Trait：** `RewardBotStore`
- Config：`load_config`、`save_config`（key-value 模式）
- Markets：`upsert_markets`、`list_markets`、`list_all_active_markets`
- Quote Plans：`save_quote_plans`（替换当前计划快照）、`list_quote_plans`
- Orders/Positions/Events：完整 CRUD；订单支持 `RewardOrderListQuery` 后端分页并在 snapshot 中返回 `orders_page`；live worker 可按 external Polymarket order id 查 managed order，并用 fill id 做成交幂等
- State Tick：`apply_simulation_tick`（历史命名；原子持久化 markets/plans/orders/fills/positions/ledger/events，供 live 本地状态更新复用）、`reset_simulation`（保留给本地/历史 validation 状态重置；live Reset 不调用它清账）
- Control Commands：`enqueue_control_command`、`claim_next_control_command`、`complete_control_command`、`fail_control_command`

**服务：** `RewardBotService` — 读写配置、市场管理、快照聚合、订单分页快照、live tick 计划准备、rewards 控制命令入队/领取/完成状态管理；不再依赖全局 `ModeStateStore`

**执行模式：**
- `RewardExecutionMode` 枚举保留用于向后兼容（serde 反序列化旧配置），但所有值均映射为 `Live`。`is_validation()` 始终返回 `false`，`is_live()` 始终返回 `true`。
- Worker 使用当前 quote plan 通过 `LivePolymarketConnector` 提交 post-only token 买单，并对本系统托管的 live 订单执行撤单；该模式由 rewards 配置控制，与全局 system mode 解耦。Polymarket 返回 `matched` / `delayed` 等非 live 接受状态时，worker 会把它视为 post-only 安全违规并立即尝试撤单，不把该订单标为正常 open。

**控制命令类型：**
- `RewardControlAction`：`run_once`、`cancel_all`、`reset`
- `RewardControlCommandStatus`：`pending`、`running`、`completed`、`failed`
- `RewardControlCommand`：API 与 worker 之间的数据库命令消息

**订单分页类型：**
- `RewardOrderListQuery`：orders search/status/sort/page/page_size 查询
- `RewardListPage`：`page`、`page_size`、`total_items`、`total_pages`
- `RewardBotSnapshot.orders_page`：当前 `orders` 数组对应的分页元数据

**Tick 引擎：** `run_reward_simulation_tick`（历史命名；在 `rewards/engine.rs` 中，通过 `include!` 拆分到 `engine/{reconcile,fills,quoting,rewards_calc,state}.rs`）。`is_validation()` 条件已移除，始终执行 `accrue_rewards`。

**资金与盘口约束：**
- 未成交 post-only maker 买单不在本地按全局 notional 硬锁同一笔 USDC，同一资金池可同时在多个不同市场报价。
- 买单计划的单腿目标 notional 使用 `min(quote_size_usd, account_capital_usd)`，再受 `per_market_usd / 2` 和 Polymarket `rewards_min_size` 约束；如果该预算无法满足 `rewards_min_size`，计划会标记为不 eligible。
- `max_markets=0`、`max_open_orders=0` 或 `quote_size_usd=0` 表示不再新挂单。
- 缺少新鲜缓存盘口时不会提交新 post-only 订单。placement 必须看到 YES/NO 两腿的新鲜盘口。
- 全局敞口门槛使用「已有库存 notional + 当前候选单腿 notional」做准入。
- 单次 rewards tick 使用 `list_reward_run_candidate_markets()` 只从 `reward_markets` 表读取候选池（默认至少 100；上限随 `max_markets` / `max_open_orders` 扩展到 `u16::MAX`），再按 active、token、最低日奖励、有效奖励 spread、下单开关做无需盘口的预过滤；只有通过预过滤的奖励市场会读取 worker 进程内 orderbook cache 并生成当前 quote plan 快照。

**live 资金模型：**
- Rewards live maker 下单沿用软资金复用语义：未成交的 post-only/GTC maker 买单是链下签名挂单，不在本地策略层按全局 notional 硬锁同一笔 USDC；同一资金池可同时在多个不同市场报价。
- Live 新挂单仍要求目标 YES/NO 两腿都有非空盘口；`stale_book_ms=0` 只关闭盘口年龄检查，不允许在盘口缺失或空盘口时下单。live reconcile 会对本系统托管的开放订单读取活跃 token 盘口；盘口缺失、空盘口或超过 `stale_book_ms` 会触发立即撤单，即使 `enabled=false` 已停止新增报价。
- Live `reset` 不清空本地账本或删除托管订单；worker 会先按 cancel-all 语义撤销本系统托管 live 订单，若任一 Polymarket 撤单被拒绝，则命令失败并保留本地状态以避免孤儿实盘订单。
- 风险控制重点放在成交后：真实成交或部分成交回报后，worker 对本系统托管 rewards 订单按 external trade id 幂等更新现金、库存、fills 和 PnL，并撤掉 sibling legs；新挂单的 per-token 和全局库存门槛都使用「已有库存 notional + 当前候选订单 notional」准入。
- 本地仍需保留 `max_open_orders`、`max_markets`、单市场预算、per-token 库存和 kill-switch；这些限制控制操作风险和订单风暴，而不是把所有开放买单当作已消耗资金。
- 当前 live 已具备 post-only token 买单提交、订单撤单、本系统托管订单成交同步、成交后 sibling leg 撤单以及 `ExitAtMarkup` / `FlattenImmediately` sell 下单；独立账户余额/库存全量对账、订单计分查询和奖励结算对账仍是缺口。
- 仍需要用真实小额账户验证 Polymarket CLOB 的 balance/allowance validity checks，尤其同市场内开放订单对可下单 size 的影响；跨市场资金复用可以作为 rewards maker 策略假设，但不能依赖 venue 替我们做组合风险管理。
- 参考官方文档：Order Lifecycle / Requirements 和 Orders Overview / Validity Checks，后续实现时需要复核最新文档。

### copytrade — 跟单

**Store Trait：** `CopyTradeStore`
- Config/Wallets/SourceTrades/Orders/Positions/Events/AccountState 完整 CRUD
- 原子 tick：`apply_copy_tick(outcome, trace_id)`
- Control Commands：`enqueue_control_command`、`claim_next_control_command`、`complete_control_command`、`fail_control_command`

**服务：** `CopyTradeService` — 配置管理、钱包管理、跟单模拟、控制命令入队/领取/完成状态管理

**控制命令类型：** `CopyControlAction`（run_once/analyze_wallets/cancel_all/reset）、`CopyControlCommandStatus`、`CopyControlCommand`

**模拟引擎：** `run_copy_simulation_tick`、`compute_copy_size`

### arbitrage — 套利

**Store Trait：** `ArbitrageStore`
- Scan lifecycle、market book snapshots、opportunities、validations、analysis runs、events

**核心函数：** `detect_arbitrage_opportunities`、`validate_arbitrage_opportunity`、`build_arbitrage_analysis`

### news_ingestion — 新闻采集

**Store Trait：** `NewsIngestionStore`
- `insert_raw_news_event`（SHA-256 去重）、`record_news_source_success/failure`、`list_news_source_health`

**服务：** `NewsIngestionService` — 批量采集、去重、健康追踪

### orderbook_cache — 盘口缓存

**Trait：** `OrderbookCache`
- `get_book(token_id)`、`set_book(book)`、`set_books(books)`、`get_stale_tokens(token_ids, max_age_ms)`

**类型：** `CachedOrderBook`（token_id、bids、asks、observed_at、source）

**实现：**
- `InMemoryOrderbookCache`（infrastructure crate）— 进程内缓存，仅供 orderbook 服务使用
- `OrderbookHttpClient`（connectors crate）— HTTP 客户端，Worker 和 API 通过此实现调用 orderbook 服务

### orderbook_registry — 盘口订阅注册中心

**Trait：** `OrderbookSubscriptionRegistry`
- `register_tokens(source, token_ids)`、`unregister_source(source)`、`list_all_tokens()`、`changed_since(instant)`

**实现：**
- `InMemoryOrderbookSubscriptionRegistry`（infrastructure crate）— 进程内注册中心，仅供 orderbook 服务使用
- `OrderbookHttpClient`（connectors crate）— HTTP 客户端，Worker 通过此实现注册 token 到 orderbook 服务

### wallet_analysis — 钱包分析

**纯计算模块**，无 Store trait、无 Service struct
- 输入：`ClosedPositionInput`、`TradeInput`、`OpenPositionInput`、`ActivityInput`
- 输出：`WalletAnalysisReport`（profile/pnl/activity/categories/style/risk/top_markets/recent_trades 等）
- 入口：`build_wallet_analysis_report()` — 同步纯函数

## 模块依赖关系

```
system_mode ← (基础，无依赖)
    ↑
market_event ← (核心数据)
    ↑
execution ← (组合 market_event + risk + audit)
    ↑
risk ← (依赖 market_event + system_mode)
    ↑
rewards ← (独立；执行模式由 RewardBotConfig.execution_mode 控制)
copytrade ← (独立，集成 wallet_analysis)
arbitrage ← (可能使用 orderbook_cache)
news_ingestion ← (独立，输出供 signal pipeline 使用)
orderbook_cache ← (共享基础设施 trait)
```

## 当前状态

- 所有模块已实现完整的 Store trait 和 Service struct
- Rewards 具备 validation 事件验证引擎和 live-first 执行模式；validation 资金池对开放买单使用软复用，成交时才消耗现金，但不再计提模拟奖励收益。
- Rewards live 模式已接入 post-only token 买单、撤单、本系统托管订单成交同步、成交后现金/库存/PnL 更新以及 exit/flatten sell 下单；独立账户余额/库存全量对账、订单计分查询和奖励结算对账仍待完成。
- Rewards 已具备数据库控制命令队列，API 负责入队，worker 负责执行 run/cancel/reset。
- Copytrade 已具备数据库控制命令队列，API 负责入队，worker 负责执行 run/analyze/cancel/reset。
- Wallet analysis 是纯计算，已完全实现
- Arbitrage 是只读链路（发现/记录/校验/分析/展示），不会创建执行请求

## 修改检查清单

- [ ] 新增 Store trait 方法后，同步更新 `infrastructure` 中的 Postgres 和 in-memory 实现
- [ ] 修改 Service 方法后，同步更新 `apps/api` 中的 handler 和 `apps/worker` 中的 worker
- [ ] 修改视图/命令类型后，同步更新 `contracts` crate 中的 DTO
- [ ] 新增模块后在 `lib.rs` 中添加 `mod` 声明和 `pub use` 导出
- [ ] 使用 `include!` 拆分时，被 include 文件不写自己的 `use`
- [ ] 文件行数不超过 500（软上限）/ 800（硬上限）
- [ ] 运行 `cargo check --workspace --tests`
