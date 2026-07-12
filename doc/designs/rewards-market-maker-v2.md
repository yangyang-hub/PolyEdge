# Rewards Market Maker V2 设计

最后更新：2026-07-12

状态：已按本方案实现，最终验证命令见本文末尾；仓库实时状态仍以 `AGENTS.md` 和模块文档为准。

## 目标

把 Rewards 策略从“满足奖励条件的被动买单 + 成交后退出”调整为持续做市策略：

1. 交易收益和库存安全是报价准入条件。
2. LP rewards / maker rebate 只通过受限的 reward-density 次级权重参与市场排序，并用于收益展示；不能补贴或覆盖交易 edge、退出和库存风险。
3. 实时价格、档位、数量、撤单和库存偏斜全部由确定性代码决定。
4. AI advisory 只评估慢速结构风险；info-risk 只评估有来源的事件/新闻风险。
5. `eligible` 只表达是否允许新增；“停止新增”和“撤销已有订单”使用独立动作语义。

## 决策层次

```text
market/catalog hard filters
    -> live book + market-implied fair value
    -> trading edge gate (不含 LP 收益)
    -> inventory-aware price/size construction
    -> provider slow-risk modifiers
    -> event/info-risk open-order action
    -> live last-look
    -> place / keep / cancel / replace
```

## 报价规则

- `quote_bid_rank` 是首选档位，默认买一；`quote_max_bid_rank` 是最深候选档位。
- 标准策略按买一到最大档位依次检查，选择第一个满足以下条件的价格：
  - post-only，不触碰卖一；
  - 位于 rewards 最大有效 spread 内；
  - `raw_edge - uncertainty - provider_edge_buffer >= min_effective_edge`；
  - 双边 BUY 仍满足 condition safety margin；
  - 当前方向没有超过库存上限。
- BalancedMerge 保留独立固定档位和 paired-cost edge 约束。
- 双边实际下单资金不再固定 50/50。已有某 outcome 库存时，减少该 outcome 的新增 BUY，增加互补 outcome 的预算；每条腿仍不得低于 rewards minimum size。
- `maker_market_budget_usd` 是同一 condition 所有托管 BUY 的硬上限。钱包、provider multiplier、单侧库存 headroom 和包含 resting BUY 的全局潜在成交暴露进一步收紧预算；reward minimum 超限时停止新增。

## Fair-value 与奖励

- `effective_edge_cents` 只表示交易 edge：`raw_edge - uncertainty`。
- `expected_reward_rebate_cents` 继续估计 LP 次级收益。
- `reward_adjusted_edge_cents = effective_edge + expected_reward_rebate` 只用于展示与审计，不进入交易 edge gate 或 edge 排序分。
- 基础市场质量中 LP 相关分数合计最多 10%；最终 `selection_score` 的独立 reward-density 权重为 10%，其余主要权重给 effective edge、退出能力、稳定性和风险。
- Market-implied fair value 使用 midpoint parity、top-of-book microprice imbalance 与短窗历史；最终 edge 扣除动态 uncertainty 和 provider buffer，并在失败时继续搜索更深 rank。Selection 的正向权重固定为 base 15% / reward 10% / fair-value edge 20% / exit 30% / stability 25%，edge 4c 才满分。
- 准入只检查 raw/effective trading edge，奖励不能把失败的交易 edge 变成通过。

## Provider 合约

### AI advisory

AI 不输出价格、bid rank、单双边方向或绝对资金上限。输出：

- `action`: `allow | reduce | stop_new`
- `size_multiplier`: `0..1`
- `edge_buffer_cents`: 非负、受代码上限约束
- `confidence`、`reasons`、`metrics`

低置信度非 allow 结果确定性降级为 `reduce`（0.5 倍新单预算），不能直接停止或撤单。

### Info-risk

输出：

- `action`: `allow | reduce | stop_new | cancel_yes | cancel_no | cancel_all`
- 既有 risk level/type/directional risk、event time、confidence、summary 和 sources

只有置信度达标且具备 24 小时内可归因来源的 cancel 动作才能撤已有订单；breaking-news cancel 需要两个独立来源。普通预测不确定性、无来源猜测和 provider 故障只允许 `reduce` / `stop_new`。
`cancel_yes` / `cancel_no` 只删除并撤销命中的 outcome，互补侧保留完整做市预算；只有 `stop_new` / `cancel_all` 把 condition 新单预算归零。
`directional_risk` 定义为“哪个 outcome 的 resting BUY 会遭受逆向选择”，必须与定向 cancel 一致，不表示预测赢家：证据提高 YES 概率时通常是 NO BUY 不安全，应为 `cancel_no`；反之为 `cancel_yes`。

## 撤单分类

| 类别 | 例子 | 行为 |
|---|---|---|
| Emergency | kill switch、盘口缺失/过期、post-only 穿价、官方结果 | 立即撤，不受普通换价限速 |
| Adverse | 新安全目标价低于当前 BUY、当前订单 trading edge 失效 | 短确认后优先撤 |
| Inventory | 单边库存或全局风险超限 | 撤增加风险的一侧，保留/增强互补侧 |
| Competitive | 新目标价高于当前订单、队列或 LP 效率变差 | 慢确认、冷却和每轮上限 |
| Stop-new | 低机会分、provider pending、低置信度事件 | 不新增，不因该原因撤已有安全订单 |

漂移必须非对称：过于激进的旧 BUY 使用 `adverse_requote_*`；落后于盘口的旧 BUY 使用现有 competitive requote 节流。

## 成交后库存

- 成交后不再提供整组撤单开关；互补 BUY 由 edge、库存上限和显式风险动作独立管理。
- 保留互补 BUY 可以减少方向库存，paired cost 满足条件时进入 BalancedMerge。
- 标准策略仍创建退出 intent，但退出 floor 改为受配置控制的最大损失预算，而不是永久“不亏”硬约束。
- 退出和互补 BUY 同时存在时，禁止继续增加同 outcome 风险。

## 持久化与审计

- advisory 表只持久化 `action`、`size_multiplier`、`edge_buffer_cents`、confidence 与审计 JSON；旧 suitability/quote mode/exit policy 不再进入核心模型或 schema。
- info-risk 表持久化 `action`。
- quote plan / strategy decision 完整 JSON 保留 provider 动作和 reward-adjusted edge；摘要列使用 `ai_action` / `info_risk_action`，blocker 独立标记 provider size、maker budget 与 inventory headroom。
- 配置仍使用 `reward_bot_config` key-value；新增 V2 配置不增加配置表列。
- Durable BUY 在真实提交前重新读取当前 kill switch，读取失败也禁止下单。控制命令、外部事件和幂等请求均使用短租约与 owner/version fencing；过期执行者不能提交 terminal 状态。
- 任一 outcome 的 orderbook 更新会以 condition 为单位刷新 YES/NO 双边盘口、重算 fair value，并检查该 condition 全部 resting BUY。
- BalancedMerge 在广播前原子进入 `broadcasting` 状态；缺少已持久化 tx hash 的 broadcasting intent 只能人工/链上对账，禁止自动重播。`completed` intent 不再永久抵扣未来配对库存。

## 交付验收

- application/connectors/infrastructure/worker 测试覆盖动态档位、奖励不参与 gate、provider 动作、非对称撤单和库存预算。
- 前端能够配置和解释 V2 参数，并展示 provider action 与 trading/reward-adjusted edge。
- `init.sql` 与 baseline migration 一致。
- `cargo fmt --all`、`cargo check --workspace --tests`、`cargo test --workspace`、前端 typecheck/lint/build 全部通过。
