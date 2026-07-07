# Rewards 做市商策略重构设计

最后更新：2026-07-07

状态：部分落地。本文描述把当前 rewards bot 从“奖励资格优先的安全挂单策略”重构为“利润优先、奖励增强的事件市场做市商策略”的目标架构、数据模型、决策公式和落地路线。当前已落地 Phase 0-2 的初版安全路径：fair value 快照读取、做市商 EV/shadow/guarded 决策、quote plan 诊断和决策审计表；默认仍是 `rewards_only` 且 `market_maker_enabled=false`，不会改变 live 下单。当前仓库事实仍以 `AGENTS.md`、`doc/modules/*` 和代码为准。

## 目标

核心目标是做市盈利，做市奖励只是正向 carry 和候选排序增强项：

1. 只在保守公平概率区间显示正期望时挂单。
2. 优先选择低竞争、高奖励、低信息跳变风险、可退出的市场。
3. 对没有明确外部数据源的事件市场，用历史 base rate、盘口微结构、事件风险和模型置信度给出概率区间，而不是伪造单点真值。
4. 所有外部数据仍由指定 producer 写入数据库或 orderbook 服务缓存；策略、API handler 和前端不得直接请求 Polymarket、新闻、LLM 或其他外部 API。
5. 先 shadow，再小额 guarded live；任何阶段都不能因为奖励高而绕过定价、退出和库存风控。

## 非目标

- 不把 LLM 当作最终 fair value oracle。
- 不做 taker 套利主策略；除受控平仓外，新增流动性以 post-only maker 为主。
- 不用“低竞争”替代“正 EV”。低竞争只说明奖励分母小，不代表市场价格合理。
- 不追求覆盖所有市场；高歧义、官方突发、内幕信息强、事件时间未知且不可控的市场应主动放弃。
- 默认不改动当前 live 下单路径；只有显式启用 `market_maker_guarded` 才允许后端按 EV 重定价或过滤 BUY quote leg。

## 当前状态判断

现有 rewards 路径已经具备可复用基础：

- `polyedge-orderbook` 负责市场目录、reward catalog、orderbook stream/cache、price-history candles。
- rewards worker 已有 post-only BUY、SELL exit intent、confirmed fill 对账、事件窗口 gate、AI/info-risk gate、机会评分、低竞争指标、BalancedMerge 和 hard-risk cancel。
- `high_probability` 模块已能从本地 outcome 标签和 rewards candles 构建历史样本、bucket stats 和只读 observations；新设计中它应作为 fair value / base-rate pricing provider 服务 Rewards 做市商，不再作为独立策略、paper 或 live 执行路线推进。
- Rewards 已新增 `strategy_mode=rewards_only|market_maker_shadow|market_maker_guarded`、`market_maker_*` 配置、初版做市商 EV 决策引擎、`reward_market_maker_decisions` 审计表和前端配置/诊断；shadow 只审计，guarded 默认关闭。

关键缺口：

- 做市商 EV 决策层已初版接入，但还没有充分 shadow 校准、按 decision id 的 PnL attribution 和类别级上线门槛。
- 默认 BUY 报价仍来自现有盘口档位 `quote_bid_rank`；只有显式 `market_maker_guarded` 才会按可盈利价格上限重定价或过滤 quote leg。
- SELL 退出仍以非亏损 floor 为核心，没有完整的 fair-value-aware inventory manager。
- rewards 预估已在初版引擎中复用 `opportunity_metrics.estimated_reward_per_100_usd_day` 进入统一目标函数，但仍缺独立 reward EV 校准表、实际 earning 误差反馈和 EV 变负撤单确认。

## 总体原则

### 利润优先

做市商策略的准入条件应是：

```text
total_ev_cents > min_total_ev_cents
```

其中：

```text
total_ev_cents =
  pricing_edge_cents
  + expected_reward_ev_cents
  - expected_exit_cost_cents
  - adverse_selection_cost_cents
  - inventory_penalty_cents
  - uncertainty_buffer_cents
```

奖励只能补充正 EV，不能让明显负 edge 的报价通过。

### 区间定价

没有明确数据源的市场不输出一个自信单点，而输出：

```text
fair_yes_probability_mid
fair_yes_probability_low
fair_yes_probability_high
confidence
model_version
reason_codes
```

BUY YES 的保守 edge 使用低分位：

```text
buy_yes_edge = fair_yes_probability_low - bid_price
```

BUY NO 的保守 edge 使用：

```text
buy_no_edge = (1 - fair_yes_probability_high) - no_bid_price
```

### 数据分层

策略层只能读取：

- Postgres 中的 `markets`、`reward_markets`、candles、outcomes、fair values、risk/advisory caches。
- orderbook 服务缓存。
- worker 已同步的账户、订单、成交、持仓、rewards earning 和 scoring 状态。

任何新外部数据源都必须通过独立 producer 写入数据库或缓存后再被策略消费。

## 目标架构

```text
External producers
  orderbook service market/reward/orderbook/candle sync
  outcome/settlement label producer
  category-specific data producers
  optional news/info-risk/advisory producers

Postgres + orderbook cache
  markets / reward_markets / reward_market_candles
  outcomes / historical samples / model stats
  fair value snapshots
  reward EV estimates
  market maker decisions
  account/orders/fills/positions

Application layer
  MarketMakerPricingService
  RewardEvEstimator
  MarketMicrostructureService
  InventoryRiskService
  MarketMakerDecisionEngine
  RewardBotService compatibility facade

Worker
  fair value refresh
  reward EV refresh
  shadow decision generation
  guarded quote materialization
  live submission/reconcile/cancel

API + Frontend
  strategy mode/config
  fair value and EV diagnostics
  quote decisions
  inventory and PnL attribution
  reward earning attribution
```

建议保留现有 `rewards` 产品入口，但在后端拆出更清晰的应用服务：

| 服务 | 职责 |
|---|---|
| `MarketMakerPricingService` | 生成 fair probability 区间和置信度 |
| `RewardEvEstimator` | 估算某个报价在 scoring 条件下的奖励 EV |
| `MarketMicrostructureService` | 计算盘口质量、深度、跳变、集中度、退出成本 |
| `InventoryRiskService` | 管理 token/condition/category/global 库存与 skew |
| `MarketMakerDecisionEngine` | 合并价格 edge、奖励、成本和风险，产出 allow/quote/cancel/hold |
| `RewardBotService` | 兼容现有 config、snapshot、orders 和 control command |

### High Probability 边界

`high_probability` 应保留研究、样本构建、bucket stats、walk-forward backtest 和模型校准职责，但输出目标要从“独立入场建议”改为“可审计 fair value snapshot”：

```text
condition_id
fair_yes_low / fair_yes_mid / fair_yes_high
confidence
uncertainty_cents
sample_count
bucket_key
model_version
input_hash
reason_codes
expires_at
```

Rewards 做市商只把这些字段当作 pricing component。最终 quote/skip/cancel/size/exit 由 `MarketMakerDecisionEngine` 合并 rewards EV、微结构、库存和 hard risk 后决定。`high_probability` 不创建订单、不维护 paper fills、不接 live connector、不拥有 kill switch。

## 定价体系

### Fair value 输入

每个 condition 至少组合四类信号：

1. **市场隐含先验**：当前 midpoint、recent trades、价格路径和流动性。
2. **历史 base rate**：按市场类型、价格 bucket、剩余时间、流动性、spread、规则风险聚合的历史胜率。
3. **类别模型**：有明确数据源的类别使用专用 producer，例如 crypto、sports、macro、election polling；没有明确数据源时只用 base rate + 盘口 + 风险折扣。
4. **风险调整**：事件窗口、info-risk、规则歧义、官方突发、新闻跳变、盘口稳定性和 LLM advisory 只做折扣或 gate，不直接提高 fair value。

### 市场分类策略

| 类别 | 定价方式 | 默认 live 策略 |
|---|---|---|
| Crypto price/range | 外部价格 producer + orderbook basis | 可 live，但要求高新鲜度 |
| Sports pre-game | 专用 odds/历史 producer，可后续补 | shadow 优先 |
| Macro/economic release | 时间窗口明确，事件前停止新增 | 仅远离 release 时小额 |
| Elections/politics | base rate + polling producer + risk tags | 小额，强不确定性 buffer |
| Official announcement/listing/court | 信息跳变强 | 默认拒绝新增 |
| Geopolitical/military/news-driven | 逆向选择强 | 默认拒绝新增 |
| Entertainment/personality | 数据弱、噪声高 | shadow 或拒绝 |

### Fair value 输出结构

已新增 `reward_market_fair_values`（0058），作为 High Probability fair value provider 输出：

```text
id
condition_id
model_version
source_set_hash
fair_yes_mid
fair_yes_low
fair_yes_high
confidence
uncertainty_cents
category
risk_tags jsonb
reason_codes jsonb
input_hash
computed_at
expires_at
```

同一 condition 可保留最新多版本记录，但 live 决策只读取当前启用模型版本且未过期的数据。

### 模型融合

初版使用保守加权集成：

```text
fair_mid =
  w_market * market_implied_probability
  + w_base_rate * historical_base_rate
  + w_category * category_model_probability
  + w_advisory * advisory_probability_adjustment
```

然后按风险和样本量给区间：

```text
uncertainty =
  model_error
  + category_risk_buffer
  + liquidity_buffer
  + event_jump_buffer
  + data_staleness_buffer

fair_low = clamp(fair_mid - uncertainty, 0, 1)
fair_high = clamp(fair_mid + uncertainty, 0, 1)
```

LLM advisory 只能扩大不确定性、降低 confidence、添加 risk tags 或建议更保守方向；除非经过回测校准，不直接把 LLM 输出当概率。

## 奖励 EV 估算

奖励 EV 的目的不是精确复刻 Polymarket 内部公式，而是得到足够保守的排序和准入估计。

对每个候选报价计算：

```text
qualified_competition_usd
own_qualified_notional_usd
estimated_scoring_share
expected_online_ratio
expected_reward_usd_per_day
expected_holding_days
expected_reward_ev_cents
```

核心估算：

```text
expected_reward_usd_per_day =
  total_daily_rate
  * estimated_scoring_share
  * expected_online_ratio
  * scoring_confidence

expected_reward_ev_cents =
  expected_reward_usd_per_day
  * expected_holding_days
  / max(position_size, epsilon)
  * 100
```

`estimated_scoring_share` 必须扣除本系统现有订单，避免把自己的挂单当成外部竞争。`scoring_confidence` 由 order scoring 查询、报价是否满足 min size、spread、存活时间、盘口新鲜度和历史 scoring 成功率决定。

建议新增 `reward_market_reward_ev_estimates`：

```text
condition_id
token_id
side
candidate_price
candidate_size
qualified_competition_usd
estimated_scoring_share
expected_reward_usd_per_day
expected_reward_ev_cents
scoring_confidence
computed_at
expires_at
```

## 微结构与退出成本

MarketMicrostructureService 需要为每个 token/condition 产出：

```text
best_bid / best_ask
target_bid_rank_price
spread_cents
bid_depth_usd
ask_depth_usd
exit_depth_usd
exit_slippage_cents
top1_depth_share
top3_depth_share
book_hhi
midpoint_range_cents
top_of_book_flip_count
cancel_rate_proxy
book_sample_count
confirmed_at
```

准入原则：

- 样本不足时 fail closed。
- 盘口过期、空盘口、best ask touch、spread 过宽、top depth 过度集中时不可新增。
- 退出深度必须覆盖计划 notional 的倍数要求。
- 低竞争市场必须更严格，因为它们更容易被小额订单操纵。

## 决策引擎

### 候选排序

候选 universe 仍从 `reward_markets` 开始，过滤 open/tradable、唯一 YES/NO、市场同步新鲜度、剩余时间、歧义和高跳变风险。

排序不再是单纯奖励或低竞争，而是：

```text
maker_priority_score =
  total_ev_cents
  * confidence
  * liquidity_quality_score
  * reward_capacity_score
  * inventory_capacity_score
```

候选偏好：

1. 正 pricing edge。
2. 低竞争、高 reward EV。
3. 高退出深度、低滑点。
4. 稳定盘口。
5. 明确事件时间且远离高跳变窗口。

### BUY 报价

对 YES：

```text
max_profitable_yes_bid =
  fair_yes_low
  + expected_reward_ev
  - expected_exit_cost
  - adverse_selection_cost
  - inventory_penalty
  - min_profit_margin
```

对 NO：

```text
max_profitable_no_bid =
  (1 - fair_yes_high)
  + expected_reward_ev
  - expected_exit_cost
  - adverse_selection_cost
  - inventory_penalty
  - min_profit_margin
```

目标报价从盘口中选择最高且仍不超过 `max_profitable_*_bid` 的 post-only 档位：

```text
target_bid = min(best_non_crossing_bid_candidate, max_profitable_bid)
```

如果为了 scoring 必须提高到某个价格，但该价格超过 `max_profitable_bid`，则不挂。

### 双边与单边

默认不是为了凑双边奖励而强行双边：

- YES 与 NO 都有正 total EV 时，允许双边。
- 只有单侧正 EV 时，只挂单侧。
- 双边合计成本必须满足安全边际：

```text
yes_bid + no_bid <= 1 - pair_safety_margin
```

- 如果双边中一侧负 EV，但奖励因双边 boost 让总 EV 转正，仍要求该侧 `pricing_edge >= -max_subsidized_negative_edge_cents`，防止用奖励长期补坏价。

### SELL 与库存

持仓后的 SELL 不应只看原始成本，还要看 fair value 和库存压力。

建议 SELL floor：

```text
min_sell_price =
  max(
    avg_cost + realized_profit_floor,
    fair_mid + maker_sell_markup - inventory_skew_discount
  )
```

行为：

- 正常库存：post-only SELL，价格不低于 `min_sell_price`，不主动亏损退出。
- 库存超限或 fair value 下修：降低 maker sell markup，但仍受 hard stop 策略控制。
- 事件窗口、信息风险升级、盘口质量恶化：停止新增 BUY，优先撤开放 BUY；SELL 是否降价需要独立风控配置，不能隐式亏损。
- YES/NO 均有库存且可合并时，优先评估 BalancedMerge。若总成本加链上/操作成本低于 1 且收益好于二级市场退出，则创建 merge intent。

### 撤单

开放 BUY 撤单原因分层：

1. Hard risk：kill switch、事件窗口、盘口缺失/过期、best ask touch、spread 超限、info-risk 升级。
2. EV 变负：fair value 更新、reward EV 降低、竞争恶化、退出成本升高。
3. Inventory：同 token/condition/category/global cap 超限。
4. Requote：目标价变化超过阈值且经过确认窗口。

EV 变负撤单建议使用确认窗口，避免模型和盘口微小抖动导致订单风暴；hard risk 不等待。

## 配置设计

保留旧配置兼容，新增策略模式：

```text
strategy_mode = rewards_only | market_maker_shadow | market_maker_guarded
```

建议新增核心配置：

| 字段 | 默认 | 说明 |
|---|---:|---|
| `market_maker_enabled` | false | 总开关 |
| `market_maker_mode` | `shadow` | `shadow` / `guarded` |
| `min_total_ev_cents` | 1.0 | 最低总 EV |
| `min_pricing_edge_cents` | 0.5 | 不含奖励的最低价格 edge |
| `max_reward_subsidized_negative_edge_cents` | 0.5 | 双边奖励允许补贴的最大负价格 edge |
| `min_fair_value_confidence` | 0.60 | fair value 最低置信度 |
| `max_uncertainty_cents` | 8.0 | 不确定性上限 |
| `low_competition_priority_enabled` | true | 低竞争高奖励排序增强 |
| `min_reward_ev_cents` | 0.0 | 最低奖励 EV；0 表示只排序不硬 gate |
| `max_condition_inventory_usd` | 20 | 单 condition 库存 |
| `max_category_inventory_usd` | 50 | 单类别库存 |
| `max_global_inventory_usd` | 100 | 全局库存 |
| `inventory_skew_cents_per_10_usd` | 0.5 | 库存越多，新增同向 BUY 越保守 |
| `fair_value_ttl_sec` | 300 | fair value 缓存 TTL |
| `reward_ev_ttl_sec` | 60 | reward EV 缓存 TTL |
| `ev_cancel_confirm_sec` | 30 | EV 变负撤单确认 |
| `shadow_min_observation_days` | 7 | shadow 最短观察期 |

旧 `opportunity_*` 配置可以逐步迁移为 `market_maker_*`，但不应一次删除，以免破坏前端和历史 config。

## 数据模型

### Fair value

`reward_market_fair_values` 如上。用于定价快照、缓存命中和审计；Rewards 做市商启用 shadow/guarded 时读取当前模型版本、未过期且未超过 Rewards 配置 TTL 的快照。

### Decision audit

已新增 `reward_market_maker_decisions`（0059）：

```text
id
run_id
account_id
condition_id
token_id
side
decision_type        -- quote / skip / cancel / hold / exit / merge
decision_status      -- allowed / blocked / shadow_allowed / shadow_blocked
target_price
target_size
fair_value_id
reward_ev_id
pricing_edge_cents
reward_ev_cents
exit_cost_cents
adverse_selection_cost_cents
inventory_penalty_cents
uncertainty_buffer_cents
total_ev_cents
reason_codes jsonb
inputs_hash
created_at
```

后续 live order 应引用最近一次 decision id，便于 PnL 和策略归因；当前已先保存独立决策审计。

### Quote plan 扩展

在 `RewardQuotePlan` 展示层增加：

```text
fair_value
edge_metrics
reward_ev_metrics
microstructure_metrics
inventory_metrics
market_maker_decision
```

历史兼容上可先放入 JSON metrics，待稳定后再拆 DTO 字段。

## Worker 流程

### Full tick

```text
load config/account/orders/positions
load reward markets
prefilter market quality and event hard risk
fetch/refresh local orderbook cache through orderbook service
compute microstructure metrics
load latest fair values
load/compute reward EV estimates
build market maker decisions
save shadow decisions and quote plans
apply AI/info-risk/event gates
sync managed orders/fills/scoring/open-order snapshot
refresh account/positions if safe
materialize guarded live quote intents when mode allows
submit post-only orders after 1s max-age last-look
save decision/order linkage
```

### Fast reconcile

```text
consume active orderbook updates
refresh active fair/reward EV if stale enough
evaluate hard-risk cancel immediately
evaluate EV/inventory cancel with confirmation window
submit/adjust SELL exits
discover BalancedMerge intents
sync fills/statuses on throttle
```

### Shadow mode

Shadow mode 仍生成：

- would_quote / would_skip
- target price/size
- expected EV
- actual current rewards-only action
- 之后的成交/盘口变化/PnL attribution

但不改变 live orders。进入 guarded live 前必须有足够 shadow 样本证明：

- 正 EV 决策组的后验表现优于被拒绝组。
- EV 变负撤单没有造成订单风暴。
- 低竞争高奖励市场没有显著更高的坏成交率。
- scoring 成功率和实际 reward earning 与估算误差在可接受范围内。

## 前端设计

`/rewards` 保留，但策略语言从“奖励机器人”逐步过渡到“Rewards 做市”。

新增视图：

1. **做市商概览**
   - 总 EV、pricing edge、reward EV、uncertainty、inventory、实际 PnL、rewards PnL 分解。
2. **Fair Value 表**
   - fair mid/low/high、confidence、model version、reason codes、TTL。
3. **Decision Audit**
   - 每个 condition 的 quote/skip/cancel 原因，显示公式分项。
4. **低竞争高奖励队列**
   - reward per 100U day、competition multiple、own share、scoring confidence。
5. **库存面板**
   - token/condition/category/global inventory、skew、退出计划、merge intents。

UI 必须明确区分：

- 当前 rewards-only 实际动作。
- market-maker shadow 建议。
- guarded live 已启用动作。

## 实施路线

### Phase 0：Schema 与只读审计

- 新增 fair value、reward EV、decision audit 表。
- Quote plan DTO 增加 JSON metrics。
- 不改变任何下单逻辑。

当前状态：部分完成。已落地 fair value 表、decision audit 表、quote plan `market_maker` JSON metrics 和前端 DTO；尚未新增独立 reward EV 表。

### Phase 1：Fair value shadow

- 复用 `high_probability` bucket stats 生成 rewards 子集 fair value snapshots。
- 只覆盖有足够历史样本和 candles 的市场。
- 所有其他市场标记 `fair_value_unavailable`，不能 live。

当前状态：完成初版。High Probability 已能刷新 `reward_market_fair_values`；Rewards 做市商 shadow 会读取 fair value 并对缺失/过期/低置信度/高不确定性快照 fail closed。

### Phase 2：Reward EV 与微结构 shadow

- 基于 orderbook top levels 和 active scoring 状态估算 reward EV。
- 记录 scoring success 和实际 reward earning，用于校准。
- 输出 `would_quote`，但仍不下单。

当前状态：部分完成。初版引擎复用统一 `opportunity_metrics` 的奖励密度、退出滑点、中点波动、top-of-book 跳变和库存状态，输出 shadow allowed/blocked 与 EV 分解并记录审计；尚未记录 scoring success/实际 reward earning 的校准反馈。

### Phase 3：Guarded live，小额单侧

- `market_maker_guarded` 只允许：
  - fair confidence 达标。
  - pricing edge 本身非负。
  - total EV 超阈值。
  - 单侧 BUY。
  - 单 condition/全局小额上限。
  - 低竞争高奖励优先。
- 禁止因为双边奖励补贴明显负 edge 的腿。

当前状态：代码路径已存在但默认关闭。`market_maker_guarded` 会按 fair value、总 EV、pricing edge floor、库存 cap 和最低 rewards size 重定价或过滤 BUY quote leg；shadow 中允许奖励补贴不超过配置上限的小额负 edge 用于观察，guarded 会重定价到非负/达标 edge。尚未建立 7-14 天 shadow 上线门槛、EV 变负撤单确认和完整 PnL attribution。

### Phase 4：双边与库存优化

- 允许双边，但两腿分别通过 edge gate。
- 接入 inventory skew。
- SELL exit 改为 fair-value-aware。
- BalancedMerge 接入 EV 比较。

### Phase 5：替代 rewards-only

- 默认策略切换为 market maker guarded。
- rewards-only 仅作为 fallback/diagnostic 模式保留。
- 前端文案和配置按新策略收敛。

## 回测与验证

必须建立三类验证：

1. **离线回测**
   - 使用历史 candles/outcomes。
   - 评估 fair value calibration、edge bucket PnL、最大回撤。
2. **Shadow live**
   - 比较 would_quote 与真实后续盘口/成交/结算。
   - 评估 reward EV 估算误差。
3. **Guarded live**
   - 小额真实账户。
   - 按 decision id 做 PnL attribution：

```text
realized_pnl
unrealized_pnl
reward_earned
exit_cost
bad_fill_loss
model_error
```

上线门槛：

- 至少 7-14 天 shadow。
- 至少 100 个有效 shadow decisions，或按类别达到最低样本数。
- Guarded live 日亏损、库存、订单风暴、scoring failure 都有硬上限。
- 能手动 kill switch 并确认撤单/退出路径正常。

## 风险与处理

| 风险 | 处理 |
|---|---|
| 无数据源市场误判 | 输出宽 fair interval，confidence 不达标则 shadow/拒绝 |
| 奖励高但价格差 | `min_pricing_edge` 和 `max_reward_subsidized_negative_edge` 硬 gate |
| 低竞争盘口被操纵 | 更高样本数、稳定性、深度和 HHI gate |
| 事件突发逆向选择 | 类别黑名单、事件窗口、info-risk、hard cancel |
| 库存积累 | inventory skew、condition/category/global cap、SELL/merge manager |
| 订单风暴 | EV cancel confirmation、requote cooldown、max cancels per cycle |
| 估算 reward EV 失真 | scoring 查询、实际 earning 校准、保守折扣 |
| 外部数据污染策略层 | producer-only 数据入口，策略只读 DB/cache |

## 关键验收标准

设计落地后，任一 live BUY 必须能回答：

1. 为什么这个市场可以做？
2. fair value 区间是什么，来自哪个模型版本？
3. 当前报价相对 fair value 的保守 edge 是多少？
4. 预期奖励 EV 是多少，竞争资金是多少？
5. 退出成本和库存惩罚是多少？
6. 为什么这个 size 合理？
7. 如果 fair value、盘口或事件风险变化，何时撤单？
8. 成交后如何退出或合并？

如果回答不了，就不应该 live 下单。

## 参考

- Polymarket CLOB / Rewards / Orders 文档：`https://docs.polymarket.com/`
- 预测市场价格可作为概率信号但会受流动性、风险偏好和市场结构影响：Wolfers & Zitzewitz, Prediction Markets, NBER Working Paper 10504 / Journal of Economic Perspectives 2004。
- 当前仓库事实来源：`AGENTS.md`、`doc/modules/backend/application.md`、`doc/modules/backend/worker-app.md`、`doc/modules/frontend/rewards.md`。
