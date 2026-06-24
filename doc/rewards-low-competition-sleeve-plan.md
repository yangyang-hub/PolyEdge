# Rewards 低竞争市场 Sleeve 实现方案

最后更新：2026-06-24

状态：v2 已实现并默认关闭。当前已支持独立 `off/observe/enforce` profile、低竞争竞争份额与资金占比指标、前端配置/展示、独立市场数/开放订单/库存 gate、managed order bucket 持久化、跨周期 observation 表、snapshot shadow report、provider 前硬过滤和挂单后低竞争专用撤单 gate；自动切换/自动启用 `enforce` 仍未实现。

## 目标

在不放宽现有主 rewards 策略安全边界的前提下，增加一个可选的“小资金、低竞争、强风控”策略分层，用于观察或小额利用奖励竞争较小的盘口。

核心目标不是寻找“低流动性市场”，而是寻找：

1. 奖励竞争较小，单位资金预估 reward share 更高。
2. 买入后仍有可验证的退出深度。
3. 盘口短期稳定，且信息风险、结算风险和歧义风险低。
4. 单市场和全局资金暴露很小，失败时不会影响主策略。

现有主策略的默认硬过滤应保持不变：open/tradable、非高歧义、唯一 YES/NO token、最低流动性、最低 24h 成交量、最短剩余时间、Gamma spread、同步新鲜度、FDV/launch/token/official-result 事件风险等仍是默认主路径。

## 结论

落地状态按三阶段推进：

1. **observe**：已实现。只计算低竞争指标并写入 quote plan，不改变挂单行为。
2. **shadow report**：已实现。full tick 会保存低竞争 observation，snapshot 汇总最近 24 小时 competition share、挂单资金占比、预估奖励、可退出性、盘口稳定性、provider 拦截和保守退出滑点，并只给出“可考虑小额 enforce”的建议，不自动改配置。
3. **enforce**：已实现。只有 competition share / competition multiple、账户与单 condition 挂单资金占比、退出/稳定性指标达标，且 AI advisory 开启、info-risk enforce 开启时，小资金 sleeve 才能进入后续 AI/info-risk cache gate；同一 condition 的低竞争候选会替换 standard 候选，pre-LLM gate 会把低竞争候选和 standard 候选分桶，低竞争未通过 gate 不会降级成普通市场继续请求 provider；缺缓存或 provider 拒绝仍 fail closed。低竞争订单挂出后，常规撤单、待提交 intent、last-look 和事件驱动 hard-risk 撤单路径会先跑通用硬风控，再进入低竞争专用撤单 gate，不再执行普通市场的 requote drift、min depth、bid rank、depth drop、fill velocity、mass cancel 或 requote age；竞争恶化需要历史确认，资金占比超限立即撤，单纯 fresh book/history 数据不足不会只因该复核误撤。

不建议直接把 `min_market_liquidity_usd` / `min_market_volume_24h_usd` 全局调低，因为这会让主策略也暴露在低退出深度、逆向选择和可操纵盘口里。

## 指标定义

### 低竞争指标

`qualified_competition_usd`：奖励有效报价区间内的外部 bid notional。计算时应尽量排除本系统托管订单的剩余 notional，避免把自己的挂单误当成竞争。

`competition_probe_notional_usd`：用于判断“小资金能占多大份额”的探测金额，默认 10U。它与实际计划 notional 分开记录，避免 rewards 最小份额把真实挂单规模隐藏掉。

`competition_share_bps`：

```text
competition_probe_notional_usd * 10000
/ max(qualified_competition_usd + competition_probe_notional_usd, competition_probe_notional_usd)
```

默认 `low_competition_min_competition_share_bps=5000`，即 10U probe 需要占合格竞争池至少 50%。

`competition_multiple`：

```text
qualified_competition_usd / max(competition_probe_notional_usd, epsilon)
```

默认 `low_competition_max_competition_multiple=1`，即 10U probe 最多接受 10U 外部竞争资金。

`estimated_reward_per_100_usd_day`：

```text
total_daily_rate * 100
/ max(qualified_competition_usd + competition_probe_notional_usd, competition_probe_notional_usd)
```

该值只是上限估计。真实奖励还受 scoring 状态、在线时间、同价队列、市场规则和 Polymarket 结算口径影响。

`competition_density`：

```text
qualified_competition_usd / max(total_daily_rate, 1)
```

值越低说明每美元日奖励对应的竞争资金越少。

### 资金占比指标

账户有效可用资金：

```text
account_effective_available_usd =
  max(account.available_usd - max(account.external_buy_notional - managed_open_buy_notional, 0), 0)
```

`low_competition_open_buy_notional_usd_after_plan`：当前低竞争开放 BUY 挂单 notional 加上本计划仍缺的 BUY 腿 notional。

`condition_buy_notional_usd_after_plan`：同一 condition 全部开放 BUY 挂单 notional 加上本计划仍缺的 BUY 腿 notional。

`account_allocation_bps` 和 `market_allocation_bps` 分别用上述 after-plan notional 除以 `account_effective_available_usd`。默认上限为账户 1500bps、单 condition 500bps；0 表示关闭对应上限。

### 退出指标

`exit_depth_usd`：如果当前报价腿成交，能够用保守卖出路径退出的 bid depth。v1 可复用现有 `RewardBookSideMetrics.exit_depth_usd`，但 enforce 前应至少要求：

```text
exit_depth_usd >= max(
  low_competition_min_exit_depth_usd,
  planned_quote_notional_usd * low_competition_min_exit_depth_multiple
)
```

`exit_slippage_cents`：按计划 size 吃掉当前 bid 深度时，相对买入价或目标退出价的最坏滑点。observe 阶段可以先计算但不 gate；enforce 阶段应作为硬过滤。

### 稳定性指标

`midpoint_range_cents`：worker 本地盘口历史窗口内 YES/NO midpoint 的最大波动范围。初始窗口建议 10-30 分钟，样本不足时 fail closed。

`top_of_book_flip_count`：窗口内买一/卖一或目标 bid rank 的价格跳变次数。低成交市场不应只看成交量，必须看盘口是否被小资金频繁重排。

### 风险指标

低竞争 sleeve enforce 必须继续使用现有 AI advisory / info-risk 缓存 gate。建议 enforce 模式要求：

1. `info_risk_enabled=true` 且 `info_risk_mode=enforce`。
2. 信息风险缓存未过期，且风险等级低于配置的 avoid level。
3. AI advisory 如已启用，必须是高置信度 `allow`。
4. 临近结算、官方结果、文本歧义和高事件跳变风险一律不可绕过。

## 配置方案

新增配置已放在现有 `RewardBotConfig` 中，继续通过 `reward_bot_config` key-value 表持久化；配置本身不需要新增数据库迁移。

建议字段：

| 字段 | 默认值 | 说明 |
|---|---:|---|
| `low_competition_mode` | `off` | `off` / `observe` / `enforce` |
| `low_competition_max_markets` | `0` | sleeve 可同时参与的新市场数量；0 表示不实盘启用 |
| `low_competition_max_open_orders` | `0` | sleeve 开放买单上限 |
| `low_competition_per_market_usd` | `5` | 历史兼容配置字段；当前不再限制报价腿构造 |
| `low_competition_max_position_usd` | `10` | 单 token/市场库存上限 |
| `low_competition_probe_notional_usd` | `10` | 竞争探测金额；用于判断 10U 这类小资金占比 |
| `low_competition_min_competition_share_bps` | `5000` | probe 资金最低竞争份额；5000 表示 50% |
| `low_competition_max_competition_multiple` | `1` | 合格外部竞争资金相对 probe 的最大倍数 |
| `low_competition_max_account_allocation_bps` | `1500` | 低竞争开放 BUY 加计划后占账户有效可用资金上限 |
| `low_competition_max_market_allocation_bps` | `500` | 同 condition BUY 加计划后占账户有效可用资金上限 |
| `low_competition_candidate_liquidity_filter_enabled` | `false` | 历史兼容字段；后端归一化强制为 false |
| `low_competition_candidate_volume_filter_enabled` | `false` | 历史兼容字段；后端归一化强制为 false |
| `low_competition_min_market_liquidity_usd` | `0` | 历史兼容字段；后端归一化强制为 0 |
| `low_competition_min_market_volume_24h_usd` | `0` | 历史兼容字段；后端归一化强制为 0 |
| `low_competition_max_competition_usd` | `0` | 可选旧式绝对竞争资金上限；0 表示关闭 |
| `low_competition_min_reward_per_100_usd_day` | `0.25` | 每 100 美元 probe 资金的最低预估日奖励 |
| `low_competition_min_exit_depth_usd` | `50` | 最低退出深度 |
| `low_competition_min_exit_depth_multiple` | `3` | 退出深度至少覆盖计划 notional 的倍数 |
| `low_competition_max_midpoint_range_cents` | `2` | 历史窗口最大 midpoint 波动 |
| `low_competition_observation_window_sec` | `1800` | 盘口稳定性观察窗口 |
| `low_competition_min_book_samples` | `20` | 样本不足时不可 enforce |

前端可以先只暴露 `mode`、资金上限和核心阈值，其余参数后续放入高级配置。不要在 UI 文案中暗示该策略比主策略更安全；它只是更小额度、更严格 gate 的实验分层。

## 后端实现状态

### 1. 类型与配置

已落地：

- `crates/application/src/rewards/models.rs`
- `crates/application/src/rewards/config_impl.rs`
- `crates/application/src/rewards/low_competition.rs`
- `packages/front/src/lib/contracts/dto/rewards.ts`
- `packages/front/src/lib/api/actions.ts`
- `packages/front/src/features/rewards/types.ts`

新增枚举已实现：

```text
RewardLowCompetitionMode = off | observe | enforce
```

新增模型已实现：

```text
RewardLowCompetitionMetrics {
  planned_notional_usd,
  competition_probe_notional_usd,
  qualified_competition_usd,
  competition_share_bps,
  competition_multiple,
  estimated_reward_per_100_usd_day,
  competition_density,
  account_effective_available_usd,
  low_competition_open_buy_notional_usd,
  low_competition_open_buy_notional_usd_after_plan,
  condition_buy_notional_usd_after_plan,
  account_allocation_bps,
  market_allocation_bps,
  exit_depth_usd,
  exit_slippage_cents,
  midpoint_range_cents,
  top_of_book_flip_count,
  sample_count,
  eligible_for_low_competition,
  rejection_reasons
}
```

`RewardQuotePlan` 增加：

```text
strategy_bucket: standard | low_competition | none
low_competition_metrics: Option<RewardLowCompetitionMetrics>
```

该数据写入现有 `reward_quote_plans` JSON。`reward_managed_orders.strategy_bucket` 已通过 `0042_reward_order_strategy_bucket.sql` 持久化，用于区分标准订单和低竞争 sleeve 订单。低竞争跨周期 observation 通过 `0043_reward_low_competition_observations.sql` 创建，并由 `0046_reward_low_competition_competition_share.sql` 增加 competition share 与资金占比字段，供 snapshot shadow report 聚合。

### 2. 候选市场查询

已实现 profile 化过滤，不把主策略的 SQL 门槛全局放宽：

```text
RewardCandidateFilterProfile = standard | low_competition
```

`standard` 保持当前行为；`low_competition` 不使用 liquidity 或 24h volume 作为准入门槛，旧低竞争 liquidity/volume 字段只做兼容读取且归一化后强制关闭/置 0。低竞争 profile 继续共享以下硬过滤，并在候选查询中按奖励密度、较低 liquidity 和较低 24h volume 优先排序，避免被标准综合排序饿死：

1. active rewards catalog。
2. Gamma market open/tradable。
3. ambiguity_level 不是 high。
4. rewards spread 有效。
5. midpoint 在允许范围内。
6. end_at 非空且满足最短剩余时间。
7. Gamma spread 不超过上限。
8. market_synced_at 新鲜且不异常超前。
9. 唯一 YES/NO token。
10. FDV/launch/token/official-result 等高跳变关键词过滤。

当前实现仍复用候选查询的综合排序，并通过低竞争 gate 再过滤 competition share、资金占比、退出深度和稳定性；后续可再引入专门的低竞争排序：

```text
low_comp_score =
  reward_yield_score
  + exit_depth_score
  + stability_score
  - competition_score
  - illiquidity_penalty
  - stale_or_sparse_sample_penalty
```

### 3. 盘口和竞争计算

已落地：

- `crates/application/src/rewards/low_competition.rs`
- `crates/application/src/rewards/low_competition_cancel.rs`
- `apps/worker/src/worker/rewards/polling.rs`
- `apps/worker/src/worker/rewards/live_cancel.rs`
- `apps/worker/src/worker/rewards/live_risk.rs`

数据来源必须继续遵守仓库约束：worker 从 orderbook 服务 HTTP/内部 WS 本地缓存读取盘口，不能直接调用 CLOB/Gamma 外部 API。

v2 使用当前 orderbook top levels、账户状态、开放订单和 worker 本地盘口历史计算，并在 AI advisory / info-risk cache gate 完成后把低竞争 observation 写入 `reward_low_competition_observations`。该表保留每个账户、condition、观察时间、模式、计划 notional、probe notional、竞争资金、competition share、competition multiple、账户有效可用资金、低竞争与 condition BUY 挂单资金占比、预估 reward/100/day、退出深度/滑点、midpoint 波动、样本是否不足、低竞争 gate 结果、最终可挂结果、AI/信息风险拦截和主策略重叠标记。

### 4. Quote plan 与 placement gate

`low_competition_mode=observe`：

- 低竞争 bucket 计划固定不可挂，只记录指标；主策略 bucket 不受该模式影响。
- 只给 quote plan 附加 `low_competition_metrics` 和 `strategy_bucket` 候选标签。
- 前端 quote plan 表展示竞争资金、competition share、账户/单市场资金占比、预估 reward/100/day、退出深度、样本数和 midpoint 波动；顶部 shadow report 汇总最近 24 小时观察结果，供判断是否值得进入小额 enforce。

`low_competition_mode=enforce`：

- 只有通过 low competition gate 的计划才允许进入 sleeve。
- sleeve 资金、开放订单和库存上限独立于主策略，并且必须小于或等于全局上限。
- 新单仍要通过现有 live materializer、post-only、scoring、撤单风控、kill switch、AI advisory、info-risk 和账户外部 BUY notional 扣减。
- 缺少盘口历史、样本不足、info-risk 缓存缺失、AI 缓存缺失或 provider 低置信度时 fail closed。
- 实际计划腿仍由 rewards 最小份额、Polymarket 1 美元最小名义金额和 live placement 资金准入决定；低竞争 gate 用 probe notional 评估竞争程度，并用 after-plan 挂单占比限制资金暴露。
- 挂单后撤单使用低竞争专用策略：通用硬风控后不再跑普通市场的 depth/rank/history/requote 规则；competition share 低于入场阈值 80% 或 competition multiple 高于入场阈值 1.5 倍时，需要 30 秒历史盘口同向确认才撤；账户/单 condition 挂单资金占比超限立即撤；退出深度阈值为 `max(20U, 当前 condition BUY 剩余 notional * 1.5)`，退出滑点超过 3c 才撤，无法估算滑点或缺少历史样本不会单独撤单。

## 前端实现状态

落点：

- `src/lib/contracts/dto/rewards.ts`
- `src/lib/api/actions.ts`
- `src/features/rewards/components/rewards-config-panel.tsx`
- `src/features/rewards/components/rewards-advanced-config.tsx`
- `src/features/rewards/components/rewards-low-competition-report.tsx`
- `src/features/rewards/components/rewards-tables.tsx`
- `src/lib/i18n/dictionaries/rewards.ts`

已实现 UI：

1. 在策略配置中新增“低竞争观察”分组，默认关闭。
2. `observe` 模式只展示指标和标签，不改变执行按钮语义。
3. `enforce` 模式显示独立资金上限、开放订单上限、competition share/competition multiple、账户/单市场挂单占比、退出深度和最小预估奖励阈值。
4. Quote plans 表增加低竞争 badge 和摘要：competition share、账户/单市场挂单占比、预估 reward/100/day、竞争 notional、退出深度、样本数和 midpoint range。
5. 活动页增加低竞争观察报告，展示最近 24 小时 observation 数、通过率、competition share 中位数、账户/单市场占比 P90、reward 中位/P90、退出深度倍数、中点波动 P95、退出滑点 P95、样本不足、AI/信息风险拦截和主策略重叠。
6. 不新增前端外部 API 调用；所有数据来自 `/api/v1/rewards-bot` snapshot。

## Shadow Report

当前 snapshot 已内置最近 24 小时 shadow report。报告不只看候选数量，包括：

| 指标 | 判断 |
|---|---|
| 通过 low competition gate 的市场数 | 太少说明策略容量不足 |
| `competition_share_bps` 中位数 | 低于阈值说明 10U 不能占主要份额 |
| `account_allocation_bps` / `market_allocation_bps` P90 | 高于阈值说明挂单资金占比过大 |
| `estimated_reward_per_100_usd_day` 中位数和 P90 | 必须明显高于主策略资金机会成本 |
| `exit_depth_usd / planned_notional` | 低于 3 倍不建议 enforce |
| `midpoint_range_cents` P95 | 高于阈值说明“低波动”假设不成立 |
| 样本不足比例 | 高说明低成交导致指标不可置信 |
| 信息风险拒绝比例 | 高说明低竞争可能来自事件风险 |
| shadow fill 后保守退出损耗 | 一次坏成交不能吃掉多日奖励 |
| 与主策略候选重叠率 | 高则无需单独 sleeve；低则才有新增价值 |

当前自动建议只返回 `should_consider_enforce` 和原因文本，不会自动修改 `low_competition_mode`。建议启用 enforce 的最低条件：

1. 连续多天有稳定候选。
2. 保守估算净 reward 为正。
3. 绝大多数候选有足够退出深度和有效盘口样本。
4. 信息风险拒绝率可解释，且不是主要候选来源。
5. 小额资金上限下，单次极端损失不会超过多日预期奖励。

## 测试计划

后端：

1. `RewardBotConfig` 默认值、归一化和 patch 测试已覆盖低竞争配置序列化。
2. 低竞争 observe/enforce gate 已有应用层单测覆盖。
3. Shadow report 的样本量、分位数和推荐条件已有应用层单测覆盖。
4. `qualified_competition_usd` 排除本系统订单、10U probe competition share、账户/单市场 allocation cap 已有应用层单测覆盖。
5. `estimated_reward_per_100_usd_day`、退出深度、滑点和样本不足 fail closed 测试。
6. `observe` 不改变现有 eligible/placement 的回归测试。
7. `enforce` 下资金、库存、AI/info-risk、盘口历史缺失的阻断测试。

前端：

1. DTO 和 Server Action Zod 校验。
2. 配置面板字段编辑和保存。
3. Quote plan 指标展示、长拒绝原因换行和窄屏横向滚动。

运行验证：

```bash
cd packages
cargo check --workspace --tests
cargo test --workspace rewards
```

```bash
cd packages/front
pnpm lint
pnpm build
```

## 实施顺序

1. 增加配置、DTO 和 quote plan metrics 类型，默认 `off`。
2. 增加 observe 指标计算和日志，不改变实际挂单。
3. 前端展示 observe 指标。
4. 跑生产只读观察并生成汇报。
5. 根据汇报调阈值，再增加 enforce gate 和 sleeve 额度。
6. 小额实盘开启，逐日复盘成交、退出损耗和奖励。
