# Backend Application Crate

最后更新：2026-07-11

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
| `src/rewards/action_planner.rs` | `RewardActionPlanner`：把 worker 已确定的订单/merge intent 副作用候选转换为执行前 planned action ledger row |
| `src/rewards/planner.rs` | deterministic quote plan 构建 |
| `src/rewards/planner_selection.rs` | auto/dominant 单边选择和盘口集中度指标 |
| `src/rewards/planner_live.rs` | live orderbook materialization 与下单前盘口校验 |
| `src/rewards/opportunity_metrics.rs` | 竞争度、奖励密度、资金占用、退出能力和盘口稳定性评分 |
| `src/rewards/fair_value.rs` | 基于 YES/NO 当前盘口和短窗口历史中点的 fair-value 估计、edge 计算和 quote gate |
| `src/rewards/market_selection.rs` | 做市市场选择优先级：把基础质量、opportunity、fair-value edge、退出能力、稳定性、竞争和风险合成为 `selection_score` |
| `src/rewards/event_window.rs` | 事件窗口 hard gate |
| `src/rewards/ai_advisory_models.rs` | AI advisory request/decision/cache 模型 |
| `src/rewards/ai_advisory_payload.rs` | AI advisory 使用的 1h candle 聚合、payload 与稳定 cache summary；不包含 live 盘口/账户上下文 |
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

- `RewardBotConfig`：做市策略配置。V2 关键字段包括首选/最深报价档位、`maker_market_budget_usd`、库存偏斜、AI/info-risk 动作阈值、非对称 requote 和独立的最大退出损失 floor；旧 `per_market_usd`、`quote_size_usd`、`cancel_on_fill` 与环境变量置信度阈值已移除。
- `RewardFairValueEstimate` / `RewardFairValueDecision` / `RewardQuoteEdge`：fair-value 估计与每条 leg 的 raw/effective trading edge。`effective_edge_cents` 只扣不确定性并用于 gate/edge priority，`reward_adjusted_edge_cents` 再加入预期 LP rebate 但只供展示/审计；LP rebate 永远不能补贴失败的交易 edge。
- `RewardProviderAction` / `RewardMarketAdvisory` / `RewardMarketInfoRisk`：统一 provider 动作。AI 仅使用 `allow/reduce/stop_new` 和有界 size/edge modifier；info-risk 还可产生 `cancel_yes/cancel_no/cancel_all`。旧 suitability、AI quote mode 和 exit policy 不再属于核心模型。
- Info-risk 定向 cancel 会把计划切为互补单边并保留该侧完整预算；`stop_new` / `cancel_all` 才将新单 multiplier 归零。动作方向与 `directional_risk` 不一致时降级为 stop-new。
- `RewardQuotePlan`：quote plan snapshot。包含 strategy profile、quote mode、book metrics、opportunity metrics、market selection metrics、fair-value decision、AI advisory、info-risk、event-window、legs、readiness 和 live skip 状态。`score` 保留基础市场质量分，`selection_score` 是做市资金优先级分。
- `RewardStrategyInput` / `RewardLiveCycle::from_strategy_input` / `RewardDecisionEngine` / `RewardDecisionSet`：`RewardStrategyInput` 是一次 tick 的 owned、可序列化只读输入快照（config、candidate markets、pre-application plans、books、book history、account、open orders、positions、event windows、now、force_orders），由 `RewardBotService::build_strategy_input` 作为单一读路径装配，供回放与审计；`RewardLiveCycle::from_strategy_input` 把快照桥接成 engine 可变 working cycle（markets 从 candidates 投影、should_execute 从 `config.enabled || force_orders` 派生，其余字段拷贝）。engine 借用入参为 `RewardLiveEngineInput<'a>`（cycle + books + book history + now），返回更新后的 cycle、fair-value estimates、资金预检/first-quote/readiness 变更统计，不访问 DB、HTTP 或 connector。Provider cache 在 engine 阶段之间由 worker 应用，未纳入快照（Phase 4 v2）。
- `RewardStrategyRun` / `RewardStrategyDecision` / `RewardStrategyAction` / `RewardOrderTransition`：做市策略运行审计 ledger。Full tick 会记录 run 配置 hash、输入摘要、计划决策快照、从 tick outcome 派生的动作和托管订单状态变迁；`RewardQuotePlan.latest_run_id` 指向生成当前计划快照的最新 run。
- `RewardActionPlanner` / `RewardOrderActionProposal` / `RewardMergeActionProposal`：执行前 action proposal 转换层。它不访问 DB、HTTP 或 connector，只生成 `RewardStrategyAction(status=planned)`；order proposal 使用与 outcome 派生 action 相同的 `trace_id:order:{managed_order_id}` idempotency key，merge execute 使用独立 `:execute` 后缀。
- `RewardBotStore`：application 层持久化 port。覆盖 config、markets、quote plans、orders、fills、positions、events、account state、merge intents、fair-value estimates、candles、AI/info-risk cache、LLM calls、heartbeat、control commands 和历史清理。
- `RewardMarketCandle`：orderbook 服务写入的 5m price-history source candle；AI payload 在 application 层聚合成最多 24 根 1h candle。
- `DatabaseMaintenanceCutoffs` / `DatabaseMaintenanceReport`：统一 retention 配置和清理统计，覆盖 strategy run ledger、order transitions、fair-value history、candles、缓存和审计/幂等表。

## 当前状态

- Rewards market maker 是当前核心策略模块，运行路径为 live-only；成交后退出支持固定策略和 `adaptive` 策略选择配置，adaptive 退出会在本地 `ExitPending` SELL 提交前按重查周期、冷却和单单重选上限持续重评并持久化当前具体策略；`adaptive_exit_cancel_replace_enabled` 默认关闭，开启后已提交的 adaptive 退出 SELL 在策略切换或价格漂移超阈值时会先撤单，替换退出单必须等待对账确认剩余持仓后恢复，撤单结果未知时绝不补单，与本地重选共享同一套节流预算（`exit_reselect_count` / 冷却 / 单单上限）和每 tick 撤换上限。
- BalancedMerge candidate profile 与 standard profile 可在同一 condition 下并存，quote plan 按 `(condition_id, strategy_profile)` 持久化，避免低成交量配对合并计划被 standard 计划覆盖。
- Quote planning 只依赖数据库中的 reward markets、Gamma markets、orderbook 服务缓存、price-history candles、AI/info-risk cache 和本地配置。Full tick 的 pre-provider gates、post-provider first-quote gate 和最终 snapshot refresh 已通过 `RewardDecisionEngine` 集中为纯决策变换；provider cache 读取、外部账户同步和 live 下单/撤单仍留在 worker。Full tick 输入由 `RewardBotService::build_strategy_input` 作为单一读路径装配成可序列化 `RewardStrategyInput` 快照（注入单一 `now`），再经 `RewardLiveCycle::from_strategy_input` 派生 engine 可变 cycle，engine 行为不变；`prepare_live_cycle` 退化为该路径的薄委托。
- Unified opportunity metrics 是 LP rewards 的统一评分层；竞争度、奖励密度、退出能力和盘口稳定性均作为做市策略内部指标处理，不再拆出独立观察模块。
- Market selection 以 `selection_score` 作为最终排序和资金优先级。该分数在 opportunity metrics 与 fair-value 之后计算，以 effective fair-value edge、退出能力和盘口稳定性为主，奖励密度只占独立 10% 次级权重，并惩罚拥挤、资金占用和事件/AI/info-risk/fair-value/readiness 风险；`score` 不再作为 live 市场选择的主排序。基础 `score` 中 LP 奖励与 rewards spread 合计也封顶 10%。
- Strategy run ledger 已落地为 shadow 记录层：它不改变 live 下单/撤单决策，但让每轮 full tick 的配置、输入、决策、动作和订单状态变迁可通过 store/API 查询，用于生产前演练审计和后续回放基础。Phase 3 第一层已加入 `RewardActionPlanner`，worker 会在 merge create/execute、cancel/cancel-replace、pending submit 和 placement submit 前写入 planned actions；现有 outcome 持久化仍负责把同一 idempotency-keyed action 更新为实际结果。
- Database maintenance cutoffs 已覆盖 strategy run ledger：completed/failed/cancelled runs 默认保留 90 天并级联 decisions/actions，order transitions 默认保留 180 天。
- Standard live materializer 从 `quote_bid_rank` 到 `quote_max_bid_rank` 逐档搜索第一个满足 post-only、reward spread 和 trading-edge 约束的价格；rank 表示相对 best bid 的 1 cent 竞争带，即使稀疏盘口没有精确对应深度也可构造有效 maker 价。BalancedMerge 继续使用独立固定 rank。
- Fair-value gate 默认启用：worker 用当前 YES 中点、反向 NO 中点和短窗口历史 median 估计 fair value，要求 BUY 报价保留 raw edge 和扣除不确定性后的 effective trading edge；LP rebate 只进入 reward-adjusted edge。历史估计写入 latest/history 表用于审计和回测。
- Provider payload 与 cache key 排除 live orderbook、quote price/side/rank、账户余额和库存；AI 只接收稳定市场身份与完成的粗粒度 candle，info-risk 只接收市场身份、评估时间和搜索边界。低置信度 AI 非 allow 动作确定性降级为 0.5 倍 `reduce`；info-risk cancel 还必须满足新鲜、可归因和独立来源规则。
- `eligible` 现在只表示能否新增报价；已有订单的 cancel 动作独立评估。Stop-new 不会因为计划不可挂而自动撤销安全订单。
- Live orderbook validation skip 只保留 60 秒用于 fast-path 抑制和审计；每个 full tick 都基于最新 candidate/books/config 重新构建计划，不继承上一轮 skip。standard 与 BalancedMerge 即使共享 condition，也不会再因只按 condition 继承旧 skip 而互相污染。
- AI advisory 和 info-risk 只通过 provider cache 影响 live tick；外部 provider refresh 由 worker 后台任务写缓存，不阻塞 API handler。
- Funding、orderbook cache/registry、maintenance、auth/mode/risk 仍作为 application-level ports/models 保留。

## 已移除

- 历史跟踪/分析/独立研究模块的 service、store trait、运行时模型和 re-export 已移除。
- 历史 fair-value EV strategy mode 已移除；当前 fair-value 是做市报价硬 gate，不再作为独立策略模式存在。

## 已知缺口

- 生产级会话/权限 UX 仍不完整。
- Rewards live 私有任务需要真实凭证、小额演练和运维 runbook。
