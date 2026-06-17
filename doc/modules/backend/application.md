# application（应用/服务层）

最后更新：2026-06-17

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
| `rewards/` | 做市奖励：`RewardBotService`、`RewardBotStore`、质量过滤/排序、盘口指标/单边报价推荐、AI advisory 输入/决策/执行约束、异步信息风险缓存、priority condition 列表、live-only 状态与订单分页查询、events/fills/open_order_count 内存缓存、in-process command wake channel；配置默认值/归一化/patch 逻辑拆在 `config_impl.rs`，运行时模型拆在 `runtime_models.rs`，quote/selection/AI 枚举拆在 `quote_selection_models.rs`，AI 模型拆在 `ai_advisory_models.rs`，信息风险模型拆在 `info_risk_models.rs`，deterministic 盘口选择 helper 拆在 `planner_selection.rs` |
| `copytrade/` | 钱包跟踪与分析：`CopyTradeService`、`CopyTradeStore`、tracked wallets、source trades、钱包分析和控制命令队列；旧模拟引擎已移除 |
| `arbitrage/` | 套利：`ArbitrageService`、`ArbitrageStore`、机会检测/验证 |
| `news_ingestion.rs` | 新闻采集：`NewsIngestionService`、`NewsIngestionStore` |
| `orderbook_cache.rs` | 盘口缓存：`OrderbookCache` trait、`CachedOrderBook` 和内部推送事件 `OrderbookStreamEvent` |
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
- AI Advisory / Info Risk：`latest_market_advisory`、`save_market_advisory`、`latest_market_info_risk(s)`、`save_market_info_risk`，按 condition/provider/request_format/model/input_hash 缓存未过期模型判断；信息风险结果还保存 query hash、风险等级、风险类型、方向性、来源和有效期
- Orders/Positions/Events：完整 CRUD；订单支持 `RewardOrderListQuery` 后端分页并在 snapshot 中返回 `orders_page`；live worker 可按 external Polymarket order id 查 managed order、用 fill id 做成交幂等，并通过 `latest_fill_at(account_id)` 查询账户最近 confirmed fill 时间
- State Tick：`apply_tick_outcome`（原子持久化 orders/fills/positions/ledger/events，不修改奖励市场目录或 quote plan 快照）、`apply_account_sync`（更新账户；`Some(positions)` 原子替换该账户全部持仓，`None` 保留持仓）、`reset_state`（重置账户状态、清空 orders/fills/positions）
- Control Commands：`enqueue_control_command`、`claim_next_control_command`、`complete_control_command`、`fail_control_command`

**服务：** `RewardBotService` — 读写配置、市场管理、快照聚合、订单分页快照、live tick 计划准备、轻量 live state 读取、priority rewards condition 列表、rewards 控制命令入队/领取/完成状态管理。服务内部缓存 config、account、positions、events（最新 200 条）、fills（最新 200 条）、external_open_order_count 和 worker heartbeat，API 与内嵌后台 runtime 共享实例时直接读写这些热状态；缓存为空时回退到数据库。`status.open_orders` 只统计已有 `external_order_id` 的 open-like managed orders，本地尚未提交的 planned/exit intent 仍留在 worker 队列但不作为 Polymarket 开放挂单展示；`status.error` 只从当前 open-like 订单的活跃对账锁推导，不会被历史 critical event 永久污染。配置保存和控制命令入队会推进 runtime revision 并通过 command_wake channel 立即唤醒 worker poll loop。奖励市场目录替换拒绝空 snapshot，修改 `account_id` 前会检查旧账户状态。面向控制台的 snapshot 不携带全量 active reward markets。缓存辅助方法拆分在 `service_cache.rs`，账户/订单/成交/事件/report/snapshot 运行时类型拆分在 `runtime_models.rs`。

**执行模式：**
- `RewardExecutionMode` 枚举仅保留 `Live` 变体，`FromStr` 仍把旧字符串（`validation`、`dry_run`、`paper`、`simulation`）归一为 live；`execution_mode` 字段已从 `RewardBotConfig` / patch 中移除，Store 读取旧 `execution_mode` 配置键时直接忽略。
- `RewardBotConfig.quote_bid_rank` 仅允许 1–3，分别选择 YES/NO 盘口中第 1/2/3 个不同买价，默认 1（买一）；旧的中间价偏移字段 `quote_edge_cents` 已移除。
- `RewardBotConfig.quote_mode=double|auto` 与 `selection_mode=observe|enforce` 控制确定性盘口选择。默认 `double + observe` 保持既有双边报价；`auto + enforce + dominant_single_side_enabled=true` 时，planner 可在 YES/NO 概率达到 `dominant_min_probability..dominant_max_probability` 且退出深度、top1/top3 深度占比和 HHI 通过阈值后生成 `single_yes` / `single_no` 单腿计划。`observe` 只在 quote plan 记录推荐模式和 `book_metrics`，不改变实际挂单。
- AI advisory 定义低频模型判断的输入、输出和执行约束：`build_reward_ai_advisory_request()` 从开放订单、持仓和已通过初步 planner 筛选的 eligible 奖励市场、确定性计划、账户/仓位/开放订单和盘口 top levels 构建结构化 payload，并用 SHA-256 生成 input hash；`apply_reward_ai_advisories()` 会把 advisory 挂到 quote plan，并在 `ai_advisory_enabled=true` 时把缺少 advisory、低置信度、`watch/avoid` 或 `quote_mode=none` 的原 eligible 计划改为不可挂。默认 `ai_advisory_enabled=false`；配置 wire value 使用 `ai_provider=openai|anthropic` 和 `ai_request_format=openai_responses|openai_chat_completions|anthropic_messages`，后端兼容读取旧 `open_ai*` 拼写但序列化始终输出无下划线 `openai*`。provider confidence 在 connector 解析时钳制到 `0..=1`；AI 开启后只有置信度达到 worker 设置阈值的 `allow` 决策才放行新增挂单。`single_yes/single_no` 只能在 `selection_mode=enforce` 且 `quote_mode=auto` 下把已经 eligible 的双边计划收窄为单腿，不能绕过市场质量、盘口和风控硬过滤。
- Info risk 定义异步信息流风险判断的输入、输出和执行约束：`build_reward_info_risk_assessment_request()` 从奖励市场、当前 quote plan、账户/仓位/开放订单和策略配置构建结构化 payload 与搜索 query，并用 SHA-256 生成 query/input hash；`apply_reward_info_risks()` 会把最新未过期风险挂到 quote plan。默认 `info_risk_enabled=false`；provider confidence 在 connector 解析时钳制到 `0..=1`。当 `info_risk_mode=enforce` 时，缺少未过期风险缓存会 fail closed，eligible 计划会被置为不可挂；已有风险达到置信度阈值且命中 `info_risk_avoid_level`、临近结算或官方结果风险也会把计划置为不可挂。该结果不能绕过市场质量、盘口和风控硬过滤。
- 市场质量默认门槛为 `min_market_liquidity_usd=1000`、`min_market_volume_24h_usd=1000`、`min_hours_to_end=48`、`max_market_spread_cents=10`、`max_market_data_age_minutes=15`。候选 prefilter 还会拒绝 FDV/launch-day、token launch/airdrop、official-result/listing 这类高跳变事件风险市场。旧 `reward_competition_factor`、`single_sided_divisor_c`、`fill_rate_per_tick`、`max_fill_ratio` 和 `auto_cancel_stale_minutes` 配置键读取时忽略。
- Worker 使用当前 quote plan 通过 `LivePolymarketConnector` 提交 post-only token 买单，并对本系统托管的 live 订单执行撤单；该模式由 rewards 配置控制，与全局 system mode 解耦，但遵守 `RiskService` 全局 kill switch。Polymarket 返回 `matched` / `delayed` 等非 live 接受状态时，worker 会把它视为 post-only 安全违规并立即尝试撤单，并保留为待最终成交/取消对账状态。

**控制命令类型：**
- `RewardControlAction`：`run_once`、`cancel_all`、`reset`
- `RewardControlCommandStatus`：`pending`、`running`、`completed`、`failed`
- `RewardControlCommand`：API 与 worker 之间的数据库命令消息

**订单分页类型：**
- `RewardOrderListQuery`：orders search/status/sort/page/page_size 查询
- `RewardListPage`：`page`、`page_size`、`total_items`、`total_pages`
- `RewardBotSnapshot.orders_page`：service 层当前本地 managed `orders` 数组对应的分页元数据

**Tick 结果类型：** `RewardTickOutcome`（在 `rewards/engine.rs` 中定义），包含 account、markets、plans、orders、positions、fills、events 和 report。模拟引擎已移除；生产 live 路径通过 worker 的 `LivePolymarketConnector` 直接执行。

**资金与盘口约束：**
- 未成交 post-only maker 买单不在本地按全局 notional 硬锁同一笔 USDC，同一资金池可同时在多个不同市场报价。
- 买单计划把 `per_market_usd` 作为 YES + NO 两腿总预算：先把 `rewards_min_size` 向上对齐到 CLOB 两位小数成本精度要求，再保障两腿的有效最小份额，最后按各腿距离 `min(quote_size_usd, account_capital_usd)` 目标 notional 的缺口分配剩余额度；实际计划数量也预先按同一精度向下对齐，避免 connector 提交时缩量后跌破奖励最小份额。
- 报价价格直接取 `quote_bid_rank` 指定的盘口档位；双边计划要求 YES/NO 两腿都存在目标档位，单边计划只要求目标侧存在。目标档位价格距离各自中间价超过 `min(market rewards_max_spread, config.max_spread_cents)` 时，计划标记为不可挂且不回退其他档位。开放订单与最新目标档位价格相差超过 `requote_drift_cents` 时撤单重挂。
- `max_spread_cents` 归一化范围是 `0.1..=99`，与前端校验及二元概率价格有效范围一致；市场 `rewards_max_spread` 按 CLOB 原始 cents 直接使用，不做百分比换算。
- `max_markets=0`、`max_open_orders=0` 或 `quote_size_usd=0` 表示不再新挂单。
- 缺少新鲜缓存盘口时不会提交新 post-only 订单。placement 必须看到 YES/NO 两腿的新鲜盘口。
- 全局敞口门槛使用「已有库存 notional + 当前候选单腿 notional」做准入。
- 单次 rewards tick 使用 `list_reward_run_candidate_markets()` 从 `reward_markets` 读取候选池；Postgres 路径关联 Gamma `markets`，硬过滤非 open/tradable、高歧义、低流动性、低 24h 成交量、临近结算、Gamma spread 过宽、同步过期或异常超前、奖励不足、奖励 spread 无效、最小份额预算不可行、不具备唯一 YES/NO token 以及 FDV/launch/token/official-result 等高事件跳变风险市场。默认 midpoint 仍受 `min_midpoint..max_midpoint` 限制；auto 单边开启后 SQL 会额外允许 `dominant_min_probability..dominant_max_probability` 及其反向区间进入候选。SQL 按 CLOB 原始 cents 使用 rewards spread，再按奖励、流动性、成交量、剩余时长和有效奖励 spread 的综合质量分排序；planner 可对 `preferred_categories` 命中的 Gamma 分类追加评分。
- `list_priority_reward_condition_ids()` 为 orderbook priority sync 返回重点 condition：当前开放/持仓市场优先，其次 eligible quote plans，最后使用放宽新鲜度窗口的 rewards 候选，以便 stale catalog 仍能恢复重点市场 `markets.synced_at`。

**live 资金模型：**
- Rewards live maker 下单沿用跨市场软资金复用语义：不同 condition 的本系统未成交 post-only/GTC 买单可复用同一资金池；但 Polymarket 会对同一 condition 的全部开放 BUY 订单累计做余额有效性检查，因此 placement 会先计算该 condition 已有 managed BUY 剩余 notional 与待补 YES/NO 腿总 notional。账户开放 BUY 总额会同步到 `external_buy_notional`；其中无法归属到本系统 managed order 的外部 BUY notional 会先从 `available_usd` 中保守扣除，再做同 condition 准入，避免人工/其它机器人挂单与本系统新单叠加。账户范围外开放订单明细仍未按 condition 映射。
- Live 新挂单仍要求目标 YES/NO 两腿都有非空盘口；`stale_book_ms` 默认 45000，高于 orderbook 默认 30 秒 poll 周期，`stale_book_ms=0` 只关闭盘口年龄检查，不允许在盘口缺失或空盘口时下单。新建 quote intent 与已落库待提交 BUY 在提交前都会复用 live 撤单风控（计划仍 eligible、报价漂移、min depth、bid rank、depth drop、fill velocity、mass cancel、kill switch 等），风险不通过的本地 intent 会在提交前取消。live reconcile 会对本系统托管的开放订单读取活跃 token 盘口；盘口缺失、空盘口或超过 `stale_book_ms` 会触发立即撤单，即使 `enabled=false` 已停止新增报价。
- Live `reset` 不清空本地账本或删除托管订单；worker 会先按 cancel-all 语义撤销本系统托管 live 订单，若任一 Polymarket 撤单被拒绝，则命令失败并保留本地状态以避免孤儿实盘订单。
- 风险控制重点放在成交后：trade 达到 `CONFIRMED` 后，worker 对本系统托管 rewards 订单按 external trade id 幂等更新现金、库存、fills 和 PnL，并撤掉 sibling legs；新挂单的 per-token 和全局库存门槛都使用「已有库存 notional + 当前候选订单 notional」准入。
- 本地仍需保留 `max_open_orders`、`max_markets`、单市场预算、per-token 库存和 kill-switch；这些限制控制操作风险和订单风暴，而不是把所有开放买单当作已消耗资金。
- 当前 live 已具备 post-only token 买单提交、订单撤单、本系统托管订单 confirmed 成交同步、同轮多笔 trade 累计入账、成交后对侧 buy sibling 撤单以及 `ExitAtMarkup` / `FlattenImmediately` sell 下单；既有 sell exit 不属于 sibling cancel 目标。报价与退出订单先持久化 intent，买入 fill 与退出 intent 同事务写入，提交结果未知时保持 open-like 锁定状态等待开放订单匹配恢复或人工对账，不自动重复提交；外部订单 404 会先保持 open-like 对账锁，若超过 5 分钟仍无 CLOB/Data API 成交证据则自动本地标记为 `cancelled`，提交结果未知和取消结果未知仍不会因本地超时而自动 force-cancel。worker 还会用 CLOB open orders snapshot 反查普通 managed BUY：已提交、open-like 且无提交未知、404、pending cancel、post-only violation 等对账锁的 BUY 若已不在外部开放订单列表中，会本地关闭为 `cancelled`，释放开放挂单计数；sell exit 不走该快速关闭路径。`ExitAtMarkup` 价格向上取整到 0.01 tick；明确退出拒单使用有界退避并在达到最大拒绝次数后停止自动重试，提交前低于 1 美元最小名义金额的退出单会进入短 reason 的 dust deferred 状态，每 300 秒重新评估但不重复拼接历史原因；FAK flatten 重试刷新当前 bid 价格时保留既有退避计数，并在同 token 退出未完成时暂停新增买单。`FlattenImmediately` 无 bid 或 FAK 终态部分成交后仍有持仓时会保留本地 deferred exit 并重试。worker 会把外部 balance、账户开放买单总 notional 观测和完整 positions 快照写入 store，资金钱包地址优先使用 `FUNDER`，且 CLOB balance 为 0/失败时会用链上 pUSD 余额回填账户 snapshot；账户开放买单总 notional 与 managed BUY open-list 反查不受 confirmed fill 保护期影响，并用于估算未归属到本系统的外部 BUY notional 以保守限制新增买单；balance/positions 替换仍会根据 `latest_fill_at` 在 confirmed fill 后保护本地账户状态 120 秒。保护期结束后，成功 positions 快照原子替换该账户全部持仓，失败时保留上一版。账户范围外开放订单明细和奖励结算对账仍是缺口。
- 未决提交、待最终对账或外部订单 404 会暂停新增 live 买单但继续卖出退出；外部订单 404 锁超过 5 分钟且仍无成交证据时会自动本地关闭；post-only exit 取消后的 replacement 保留 post-only 策略。
- 新建或恢复的 buy order 初始 `scoring=false`，只有 CLOB `orders_scoring` 权威查询可以置为 true；`min_depth_usd` 检查会从聚合盘口中扣除本系统订单自身的剩余 notional，只把外部支撑深度计入阈值。
- 仍需要用真实小额账户验证跨市场资金复用和账户范围外开放订单的组合影响，不能依赖 venue 替系统做组合风险管理。
- 参考官方文档：Order Lifecycle / Requirements 和 Orders Overview / Validity Checks，后续实现时需要复核最新文档。

### copytrade — 钱包跟踪与分析

**Store Trait：** `CopyTradeStore`
- Config/Wallets/SourceTrades/Events/AccountState 读写，以及旧 Orders/Positions 表的兼容读写路径
- Source trades：记录 Data API 检测到的钱包成交，按 deterministic id 去重
- Wallet analysis：保存钱包滚动统计（trades、volume、PnL、win_rate、ROI 等）
- Control Commands：`enqueue_control_command`、`claim_next_control_command`、`complete_control_command`、`fail_control_command`

**服务：** `CopyTradeService` — 配置管理、钱包管理、source trade 检测与记录、钱包分析统计、控制命令入队/领取/完成状态管理。

**控制命令类型：** `CopyControlAction`（run_once/analyze_wallets/cancel_all/reset）、`CopyControlCommandStatus`、`CopyControlCommand`

**当前语义：**
- Copytrade 不下单、不撤单，也不维护模拟资金账本、模拟订单、模拟持仓或 PnL 面板。
- Worker 从 Polymarket Data API 读取 active tracked wallets 的 activity/positions，用 `detect_and_record_source_trades()` 写入 source trades。
- `AnalyzeWallets` 控制命令和 `analyze-wallets-once` 会读取同一批钱包输入并更新钱包分析统计。
- `RunOnce`、`CancelAll`、`Reset` 仍作为数据库控制命令兼容值存在；当前 worker 中这些动作是 no-op，不应在产品文案里描述成真实跟单或模拟交易。
- 旧 `copytrade_copy_orders`、`copytrade_positions`、`copytrade_account_state` 表仍存在用于迁移兼容和历史数据，但当前前端/API snapshot 不再展示模拟账户、订单或持仓。

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

**类型：**
- `CachedOrderBook`：token_id、bids、asks、observed_at、source
- `OrderbookStreamReason`：`book`、`price_change`、`poll_reconcile`、`ingest`
- `OrderbookStreamEvent`：orderbook 服务内部 WS 推送事件，包含单调 sequence、reason 和规范化 `CachedOrderBook`

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
- Rewards 已移除旧 validation/simulation tick 引擎，仅保留 live-only 配置、quote planner、确定性盘口指标/单边 quote mode、AI advisory 输入/决策/缓存端口、信息风险输入/决策/缓存端口、状态类型和增量持久化端口。
- Rewards live 模式已接入质量硬过滤与综合排序、post-only token 买单、撤单、本系统托管订单成交同步、成交后现金/库存/PnL 更新、可持续重试的 exit/flatten sell、CLOB open-order 反查、外部余额/完整持仓快照、managed order scoring 和 UTC 当日账户级 maker rewards 同步（聚合端点优先、明细端点 fallback）；新增买单会把未归属到本系统 managed order 的外部 BUY notional 从可用资金中保守扣除。账户范围外开放订单明细与奖励结算对账仍待完成。
- Rewards 保留数据库控制命令队列用于持久恢复，API 入队后通过共享 runtime revision 立即唤醒后台执行。
- Copytrade 已精简为只读钱包跟踪和分析：API 负责钱包配置和控制命令入队，worker 负责检测 source trades 与执行 Analyze；Run/Cancel/Reset 兼容命令当前不执行交易逻辑。
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
