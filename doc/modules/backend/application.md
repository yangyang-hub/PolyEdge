# Backend Application Crate

最后更新：2026-07-08

## 模块边界

`packages/backend/crates/application` 是后端用例层。它定义业务服务、Store trait、运行时模型、命令模型和纯计算 helper，不直接访问 SQL、HTTP 或外部 API。具体持久化在 `infrastructure`，外部系统访问在 `connectors`。

## 关键文件

| 文件 | 作用 |
|---|---|
| `src/lib.rs` | application crate 对外 re-export |
| `src/rewards.rs` + `src/rewards/*` | Rewards market maker：配置、规划、机会评分、fair-value、事件窗口、AI advisory、info-risk、live 订单模型和 snapshot |
| `src/rewards/service.rs` + `src/rewards/service/*` | `RewardBotService` 与 `RewardBotStore` trait，控制命令、snapshot、分页、缓存读取 |
| `src/rewards/config_impl.rs` | `RewardBotConfig` 默认值、归一化和 patch 应用 |
| `src/rewards/engine.rs` | `RewardDecisionEngine`：pre-provider、post-provider 和最终 snapshot 的纯决策变换入口 |
| `src/rewards/strategy_input.rs` | `RewardStrategyInput` 可序列化 tick 输入快照与 `RewardLiveCycle::from_strategy_input` 桥接（engine 借用入参为 `RewardLiveEngineInput`） |
| `src/rewards/planner.rs` | deterministic quote plan 构建 |
| `src/rewards/planner_selection.rs` | auto/dominant 单边选择和盘口集中度指标 |
| `src/rewards/planner_live.rs` | live orderbook materialization 与下单前盘口校验 |
| `src/rewards/opportunity_metrics.rs` | 竞争度、奖励密度、资金占用、退出能力和盘口稳定性评分 |
| `src/rewards/fair_value.rs` | 基于 YES/NO 当前盘口和短窗口历史中点的 fair-value 估计、edge 计算和 quote gate |
| `src/rewards/market_selection.rs` | 做市市场选择优先级：把基础质量、opportunity、fair-value edge、退出能力、稳定性、竞争和风险合成为 `selection_score` |
| `src/rewards/event_window.rs` | 事件窗口 hard gate |
| `src/rewards/ai_advisory_models.rs` | AI advisory request/decision/cache 模型 |
| `src/rewards/ai_advisory_payload.rs` | advisory payload、当前盘口定价上下文和 1h candle 聚合 |
| `src/rewards/info_risk_models.rs` | 信息风险 request/decision/cache 模型 |
| `src/rewards/provider_models.rs` | combined provider request/decision 模型 |
| `src/rewards/provider_prefilter.rs` | provider 调用前 hard filter |
| `src/rewards/run_ledger_models.rs` | Rewards strategy run、decision、action 和 order transition 审计模型 |
| `src/rewards/runtime_models.rs` | rewards account/order/position/fill/merge/event/report/snapshot 运行时模型 |
| `src/maintenance.rs` | 数据库 retention cutoffs、report 和 store port |
| `src/orderbook_cache.rs` | cached orderbook 与内部 stream event 模型 |
| `src/orderbook_registry.rs` | 多来源 orderbook token registry trait |
| `src/funding.rs` | Funding service models/ports |
| `src/auth.rs` / `src/mode_state.rs` / `src/risk.rs` | 鉴权、模式状态、风险状态应用模型 |

## 核心数据结构

- `RewardBotConfig`：做市策略配置。当前保留 execution、market filter、opportunity metrics、fair-value、quote construction、adaptive post-fill exit、holding-period adaptive exit reselection、BalancedMerge、AI advisory、info-risk、event-window、inventory 和 live risk 参数。
- `RewardFairValueEstimate` / `RewardFairValueDecision` / `RewardQuoteEdge`：fair-value 估计、每条 leg 的 raw/effective edge、rewards rebate 折扣、不确定性和最终 gate 结果。
- `RewardQuotePlan`：quote plan snapshot。包含 strategy profile、quote mode、book metrics、opportunity metrics、market selection metrics、fair-value decision、AI advisory、info-risk、event-window、legs、readiness 和 live skip 状态。`score` 保留基础市场质量分，`selection_score` 是做市资金优先级分。
- `RewardStrategyInput` / `RewardLiveCycle::from_strategy_input` / `RewardDecisionEngine` / `RewardDecisionSet`：`RewardStrategyInput` 是一次 tick 的 owned、可序列化只读输入快照（config、candidate markets、pre-application plans、books、book history、account、open orders、positions、event windows、now、force_orders），由 `RewardBotService::build_strategy_input` 作为单一读路径装配，供回放与审计；`RewardLiveCycle::from_strategy_input` 把快照桥接成 engine 可变 working cycle（markets 从 candidates 投影、should_execute 从 `config.enabled || force_orders` 派生，其余字段拷贝）。engine 借用入参为 `RewardLiveEngineInput<'a>`（cycle + books + book history + now），返回更新后的 cycle、fair-value estimates、资金预检/first-quote/readiness 变更统计，不访问 DB、HTTP 或 connector。Provider cache 在 engine 阶段之间由 worker 应用，未纳入快照（Phase 4 v2）。
- `RewardStrategyRun` / `RewardStrategyDecision` / `RewardStrategyAction` / `RewardOrderTransition`：做市策略运行审计 ledger。Full tick 会记录 run 配置 hash、输入摘要、计划决策快照、从 tick outcome 派生的动作和托管订单状态变迁；`RewardQuotePlan.latest_run_id` 指向生成当前计划快照的最新 run。
- `RewardBotStore`：application 层持久化 port。覆盖 config、markets、quote plans、orders、fills、positions、events、account state、merge intents、fair-value estimates、candles、AI/info-risk cache、LLM calls、heartbeat、control commands 和历史清理。
- `RewardMarketCandle`：orderbook 服务写入的 5m price-history source candle；AI payload 在 application 层聚合成最多 24 根 1h candle。
- `DatabaseMaintenanceCutoffs` / `DatabaseMaintenanceReport`：统一 retention 配置和清理统计，覆盖 strategy run ledger、order transitions、fair-value history、candles、缓存和审计/幂等表。

## 当前状态

- Rewards market maker 是当前核心策略模块，运行路径为 live-only；成交后退出支持固定策略和 `adaptive` 策略选择配置，adaptive 退出会在本地 `ExitPending` SELL 提交前按重查周期、冷却和单单重选上限持续重评并持久化当前具体策略；`adaptive_exit_cancel_replace_enabled` 默认关闭，开启后已提交的 adaptive 退出 SELL 在策略切换或价格漂移超阈值时会先撤单，替换退出单必须等待对账确认剩余持仓后恢复，撤单结果未知时绝不补单，与本地重选共享同一套节流预算（`exit_reselect_count` / 冷却 / 单单上限）和每 tick 撤换上限。
- BalancedMerge candidate profile 与 standard profile 可在同一 condition 下并存，quote plan 按 `(condition_id, strategy_profile)` 持久化，避免低成交量配对合并计划被 standard 计划覆盖。
- Quote planning 只依赖数据库中的 reward markets、Gamma markets、orderbook 服务缓存、price-history candles、AI/info-risk cache 和本地配置。Full tick 的 pre-provider gates、post-provider first-quote gate 和最终 snapshot refresh 已通过 `RewardDecisionEngine` 集中为纯决策变换；provider cache 读取、外部账户同步和 live 下单/撤单仍留在 worker。Full tick 输入由 `RewardBotService::build_strategy_input` 作为单一读路径装配成可序列化 `RewardStrategyInput` 快照（注入单一 `now`），再经 `RewardLiveCycle::from_strategy_input` 派生 engine 可变 cycle，engine 行为不变；`prepare_live_cycle` 退化为该路径的薄委托。
- Unified opportunity metrics 是 LP rewards 的统一评分层；竞争度、奖励密度、退出能力和盘口稳定性均作为做市策略内部指标处理，不再拆出独立观察模块。
- Market selection 以 `selection_score` 作为最终排序和资金优先级。该分数在 opportunity metrics 与 fair-value 之后计算，综合基础市场质量、奖励密度、fair-value edge、退出能力、盘口稳定性，并惩罚拥挤、资金占用和事件/AI/info-risk/fair-value/readiness 风险；`score` 不再作为 live 市场选择的主排序。
- Strategy run ledger 已落地为 shadow 记录层：它不改变 live 下单/撤单决策，但让每轮 full tick 的配置、输入、决策、动作和订单状态变迁可通过 store/API 查询，用于生产前演练审计和后续回放基础。
- Database maintenance cutoffs 已覆盖 strategy run ledger：completed/failed/cancelled runs 默认保留 90 天并级联 decisions/actions，order transitions 默认保留 180 天。
- Fair-value gate 默认启用：worker 用当前 YES 中点、反向 NO 中点和短窗口历史 median 估计 fair value，要求 BUY 报价保留 raw edge 和扣除不确定性后的 effective edge；历史估计写入 latest/history 表用于审计和回测。
- AI advisory 和 info-risk 只通过 provider cache 影响 live tick；外部 provider refresh 由 worker 后台任务写缓存，不阻塞 API handler。
- Funding、orderbook cache/registry、maintenance、auth/mode/risk 仍作为 application-level ports/models 保留。

## 已移除

- 历史跟踪/分析/独立研究模块的 service、store trait、运行时模型和 re-export 已移除。
- 历史 fair-value EV strategy mode 已移除；当前 fair-value 是做市报价硬 gate，不再作为独立策略模式存在。

## 已知缺口

- 生产级会话/权限 UX 仍不完整。
- Rewards live 私有任务需要真实凭证、小额演练和运维 runbook。
