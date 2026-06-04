# application（应用/服务层）

最后更新：2026-06-04

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
| `rewards/` | 做市奖励：`RewardBotService`、`RewardBotStore`、live-only 状态与订单分页查询 |
| `copytrade/` | 跟单：`CopyTradeService`、`CopyTradeStore`、确定性模拟引擎和风险准入 |
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
- Markets：`upsert_markets`、`list_markets`、`list_all_active_markets`、`active_market_summary`
- Quote Plans：`save_quote_plans`（替换当前计划快照）、`list_quote_plans`
- Orders/Positions/Events：完整 CRUD；订单支持 `RewardOrderListQuery` 后端分页并在 snapshot 中返回 `orders_page`；live worker 可按 external Polymarket order id 查 managed order，并用 fill id 做成交幂等
- State Tick：`apply_tick_outcome`（原子持久化 orders/fills/positions/ledger/events，不修改奖励市场目录或 quote plan 快照）、`reset_state`（重置账户状态、清空 orders/fills/positions）
- Control Commands：`enqueue_control_command`、`claim_next_control_command`、`complete_control_command`、`fail_control_command`

**服务：** `RewardBotService` — 读写配置、市场管理、快照聚合、订单分页快照、live tick 计划准备、轻量 live state 读取、rewards 控制命令入队/领取/完成状态管理；不再依赖全局 `ModeStateStore`。修改 `account_id` 前会检查旧账户是否仍有 open-like 订单或非零持仓，有任一存在则拒绝切换。面向控制台的 snapshot 只通过 `active_market_summary` 返回市场统计（`status.markets_tracked` / `last_scan_at`），不读取或携带全量 active reward markets，避免 `/rewards` 首屏响应随奖励市场数量膨胀；sync/cancel/reconcile 使用的 `current_live_cycle_state` 也不扫描全量 active reward markets，完整候选市场只在 full tick 的 `prepare_live_cycle` 中传入。

**执行模式：**
- `RewardExecutionMode` 枚举仅保留 `Live` 变体，`FromStr` 仍把旧字符串（`validation`、`dry_run`、`paper`、`simulation`）归一为 live；`execution_mode` 字段已从 `RewardBotConfig` / patch 中移除，Store 读取旧 `execution_mode` 配置键时直接忽略。
- Worker 使用当前 quote plan 通过 `LivePolymarketConnector` 提交 post-only token 买单，并对本系统托管的 live 订单执行撤单；该模式由 rewards 配置控制，与全局 system mode 解耦，但遵守 `RiskService` 全局 kill switch。Polymarket 返回 `matched` / `delayed` 等非 live 接受状态时，worker 会把它视为 post-only 安全违规并立即尝试撤单，并保留为待最终成交/取消对账状态。

**控制命令类型：**
- `RewardControlAction`：`run_once`、`cancel_all`、`reset`
- `RewardControlCommandStatus`：`pending`、`running`、`completed`、`failed`
- `RewardControlCommand`：API 与 worker 之间的数据库命令消息

**订单分页类型：**
- `RewardOrderListQuery`：orders search/status/sort/page/page_size 查询
- `RewardListPage`：`page`、`page_size`、`total_items`、`total_pages`
- `RewardBotSnapshot.orders_page`：service 层当前 `orders` 数组对应的分页元数据；API live overlay 覆盖 `orders` 后目前不会同步改写它

**Tick 结果类型：** `RewardTickOutcome`（在 `rewards/engine.rs` 中定义），包含 account、markets、plans、orders、positions、fills、events 和 report。模拟引擎已移除；生产 live 路径通过 worker 的 `LivePolymarketConnector` 直接执行。

**资金与盘口约束：**
- 未成交 post-only maker 买单不在本地按全局 notional 硬锁同一笔 USDC，同一资金池可同时在多个不同市场报价。
- 买单计划的单腿目标 notional 使用 `min(quote_size_usd, account_capital_usd)`，再受 `per_market_usd / 2` 和 Polymarket `rewards_min_size` 约束；如果该预算无法满足 `rewards_min_size`，计划会标记为不 eligible。
- `max_markets=0`、`max_open_orders=0` 或 `quote_size_usd=0` 表示不再新挂单。
- 缺少新鲜缓存盘口时不会提交新 post-only 订单。placement 必须看到 YES/NO 两腿的新鲜盘口。
- 全局敞口门槛使用「已有库存 notional + 当前候选单腿 notional」做准入。
- 单次 rewards tick 使用 `list_reward_run_candidate_markets()` 从 `reward_markets` 读取候选池；Postgres 路径会关联 Gamma `markets`，优先选择 open + tradable 且 `volume_24h` 高的市场，再按 active、token、最低日奖励、有效奖励 spread、下单开关做无需盘口的预过滤；只有通过预过滤的奖励市场会读取 orderbook 服务缓存并生成当前 quote plan 快照。

**live 资金模型：**
- Rewards live maker 下单沿用软资金复用语义：未成交的 post-only/GTC maker 买单是链下签名挂单，不在本地策略层按全局 notional 硬锁同一笔 USDC；同一资金池可同时在多个不同市场报价。
- Live 新挂单仍要求目标 YES/NO 两腿都有非空盘口；`stale_book_ms=0` 只关闭盘口年龄检查，不允许在盘口缺失或空盘口时下单。live reconcile 会对本系统托管的开放订单读取活跃 token 盘口；盘口缺失、空盘口或超过 `stale_book_ms` 会触发立即撤单，即使 `enabled=false` 已停止新增报价。
- Live `reset` 不清空本地账本或删除托管订单；worker 会先按 cancel-all 语义撤销本系统托管 live 订单，若任一 Polymarket 撤单被拒绝，则命令失败并保留本地状态以避免孤儿实盘订单。
- 风险控制重点放在成交后：trade 达到 `CONFIRMED` 后，worker 对本系统托管 rewards 订单按 external trade id 幂等更新现金、库存、fills 和 PnL，并撤掉 sibling legs；新挂单的 per-token 和全局库存门槛都使用「已有库存 notional + 当前候选订单 notional」准入。
- 本地仍需保留 `max_open_orders`、`max_markets`、单市场预算、per-token 库存和 kill-switch；这些限制控制操作风险和订单风暴，而不是把所有开放买单当作已消耗资金。
- 当前 live 已具备 post-only token 买单提交、订单撤单、本系统托管订单 confirmed 成交同步、同轮多笔 trade 累计入账、成交后 sibling leg 撤单以及 `ExitAtMarkup` / `FlattenImmediately` sell 下单；`FlattenImmediately` 无 bid 或 FAK 终态部分成交后仍有持仓时会保留本地 deferred exit 并重试。独立账户余额/库存全量对账、订单计分查询和奖励结算对账仍是缺口。
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

**跟单决策与风控语义：**
- 未处理 source trades 按 `source_timestamp + id` 排序后依次决策；暂停/删除钱包遗留的未处理交易会以 `wallet_paused` 跳过。
- cooldown 按 wallet + token 生效，并用已有 open orders / positions 的最近时间初始化；同一 tick 内已接受的交易会立即推进 cooldown。
- per-wallet、per-market、total exposure 使用运行中累加器，并把同一 tick 先前计划的买单计入；新买单 size 会硬裁剪到三个 cap 的剩余 headroom。
- UTC 日期切换时在 risk gate 前重置 `daily_realized_pnl`。
- `MirrorPortfolioWeight` 使用源钱包全部持仓的 mark value 计算真实市场权重；缺少组合总值时回退固定小额，而不是退化成 all-in。
- source trade ID 包含 wallet、tx、token、side、标准化 price/size 和 timestamp，既保持重扫幂等，也区分同交易哈希/同秒内的多笔 fill。
- 无本地持仓的 sell 会被拒绝，避免模拟账本产生 phantom proceeds；crossed/marketable order 会完整成交，resting 概率成交才应用 `max_fill_ratio`，小于 0.01 share 的尾差会被吸收以释放 reserve。

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
- `get_book(token_id)`、`set_book(book)`、`set_books(books)`、`get_stale_tokens(token_ids, max_age_ms)`、`entry_count()`
- `max_age_ms <= 0` 表示关闭年龄 stale 检查，但具体实现仍可按 TTL 判定过期。

**类型：** `CachedOrderBook`（token_id、bids、asks、observed_at、source）

**实现：**
- `InMemoryOrderbookCache`（infrastructure crate）— 进程内缓存，仅供 orderbook 服务使用
- `OrderbookHttpClient`（connectors crate）— HTTP 客户端，Worker 和 API 通过此实现调用 orderbook 服务

### orderbook_registry — 盘口订阅注册中心

**Trait：** `OrderbookSubscriptionRegistry`
- `register_tokens(source, token_ids) -> Result<()>` 原子替换来源有序 token 集合；另有 `unregister_source`、`unregister_tokens`、`list_all_tokens()`、`total_token_count()`、`source_count()`、`has_source()`、`changed_since()`

**实现：**
- `InMemoryOrderbookSubscriptionRegistry`（infrastructure crate）— 进程内注册中心，仅供 orderbook 服务使用，保留来源内顺序并按 live rewards / execution / candidates / copytrade 优先级聚合
- `OrderbookHttpClient`（connectors crate）— HTTP 客户端，Worker 通过此实现携带共享写 token 注册 token 到 orderbook 服务；注册失败返回错误

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
rewards ← (独立；仅支持 live 实盘模式)
copytrade ← (独立，集成 wallet_analysis)
arbitrage ← (可能使用 orderbook_cache)
news_ingestion ← (独立，输出供 signal pipeline 使用)
orderbook_cache ← (共享基础设施 trait)
```

## 当前状态

- 所有模块已实现完整的 Store trait 和 Service struct
- Rewards 已移除旧 validation/simulation tick 引擎，仅保留 live-only 配置、quote planner、状态类型和增量持久化端口。
- Rewards live 模式已接入 post-only token 买单、撤单、本系统托管订单成交同步、成交后现金/库存/PnL 更新以及 exit/flatten sell 下单；独立账户余额/库存全量对账、订单计分查询和奖励结算对账仍待完成。
- Rewards 已具备数据库控制命令队列，API 负责入队，worker 负责执行 run/cancel/reset。
- Copytrade 已具备数据库控制命令队列，API 负责入队，worker 负责执行 run/analyze/cancel/reset。
- Copytrade 模拟决策已按时间顺序处理并在同一 tick 内执行 cooldown、暂停钱包、日亏损和运行中 exposure cap；crossed fill、无仓卖出和组合权重计算已收敛到资金账本一致语义。
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
