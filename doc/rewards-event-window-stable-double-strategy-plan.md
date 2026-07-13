# Rewards 事件窗口与稳定双边策略设计方案

最后更新：2026-07-13

> 历史设计记录：事件窗口和 BalancedMerge 已继续演进；AI 盘口 payload、成交后 sibling cancel 等旧描述已被 [Rewards Market Maker V2](designs/rewards-market-maker-v2.md) 取代，不代表当前行为。

历史状态快照：本文撰写阶段已完成核心事件窗口，但当时尚未完成互补持仓合并执行链路。该描述不再代表仓库当前能力；BalancedMerge 与当前缺口请以 `AGENTS.md` 和 V2 设计为准。

## 2026-07-13 落地结果

- 事件窗口 gate 已进入 planner、live placement 和 condition-scoped orderbook event cancel。Gamma producer 仅从 `gameStartTime` / `events[].startTime` 构造显式 scheduled-event 候选，`startDate` / `endDate` 只保留为市场生命周期/解析截止元数据，不能进入 hard gate。
- 事件候选现在保留 `source`、`event_time_role`、`start_source_field`、`end_policy` 等 provenance；quote-plan 表格直接展示 source / role / start field / end policy，避免将市场开放时间误认为离散事件起点。
- 上游事件窗口已阻断且没有可评估 edge 时，fair-value 标记为 `not_evaluated`；它不计作 fair-value 失败，不进入 pass/blocked 统计，也不触发 fair-value selection risk penalty。
- Rewards Replay 当前 capture schema 已升为 V3，继续使用 V2 引入的紧凑 top-of-book history、final delta 和 normalized expected-plan hash；读取/回放仍兼容 V1 和 V2，缺失 `schema_version` 的 fixture 仍按 V1 解析。
- “stable double” 独立 mode 仍未按本文落地，已被 Rewards Market Maker V2 的 `quote_mode=double|auto`、统一机会/稳定性/退出评分、fair-value gate 与 live materializer 路径取代；`BalancedMerge` 是独立固定档位 profile，不等于本文原设计的 stable-double mode。
- 成交后 sibling blanket cancel 已移除；互补 BUY 按自身 edge、库存和显式风险动作独立管理。
- BalancedMerge merge intent、`broadcasting` fence、tx-hash fencing 和 receipt reconciliation 已落地；缺少已持久化 tx hash 的 broadcasting intent 禁止自动重播。

本文保留原始方案作为历史记录；实施阶段中的“已完成/未完成”状态已按 2026-07-13 实现更新，标记为“历史建议/原验收”的默认值和 mode 不能用作现行配置或能力清单。

## 原始背景（历史）

Rewards 做市策略在安静市场上可以依靠买一/买二挂单获取 maker rewards，但部分市场在真实事件临近时会从低波动状态切换到高波动状态。例如体育比赛开赛、财报发布、经济数据公布、投票截止、官方结果发布或 token unlock。开赛前几天盘口可能稳定，开赛前数小时信息和交易强度会快速变化，此时继续被动挂买单容易遭遇逆向选择。

另一个相关机会是低波动二元市场中同时在 YES 和 NO 买一挂单。如果两边成交价之和小于 1 且留有安全边际，理论上可以通过持有互补 outcome 或合并/redeem 降低方向风险。本文原始撰写时，系统成交后主要走 sibling cancel 和 SELL 退出，尚不支持自动合并互补持仓。

## 原始目标（历史）

- 建立结构化事件时间数据层，支持按市场关联的真实事件时间做硬风控。
- 在 planner、live placement 和 live cancel 三个阶段执行事件窗口 gate，避免只依赖 AI 判断。
- 将稳定双边策略建模为独立 quote mode / strategy profile，只在盘口稳定、可退出和事件窗口安全时启用。
- 把 AI advisory 和 info-risk 用作辅助解释、候选时间提取和不确定性提示，而不是最终下单权限来源。
- 为后续互补 YES/NO 持仓合并或 redeem 设计清晰执行边界。

## 原始非目标（历史）

- 不让 API handler、前端或策略代码直接抓取外部赛程、Gamma、CLOB 或新闻数据。
- 不把 Polymarket Gamma `startDate` 直接当作比赛开始时间硬执行。
- 不把 LLM 输出作为唯一事件时间来源。
- 第一阶段不实现自动 merge/redeem，不改变当前 BUY fill 后 sibling cancel + SELL 退出语义。
- 不放宽现有 rewards 市场质量、盘口、资金、库存、kill switch 和 provider fail-closed 风控。

## 历史撰写时基础

已可复用能力：

- `polyedge-orderbook` 统一同步 Gamma 市场、reward markets、orderbook cache 和 reward price-history candles。
- Rewards planner 已支持 `quote_mode=double|auto`，默认双边报价；live materializer 会验证双边目标档位、spread、安全边际和 fallback 单腿。
- `quote_bid_rank=1|2|3` 支持买一/买二/买三，默认买一。
- `opportunity_metrics` 已计算竞争倍数、奖励密度、退出深度、入场/退出滑点、坏成交恢复天数、盘口样本数、midpoint range 和 top-of-book flip count。
- AI advisory payload 已包含当前盘口、pricing context、最近 1h candles 和 cache TTL horizon，可输出 `allow_quote` 与 `strategy_hint`。
- info-risk 已支持 `allow_quote`、`resolution_imminent`、官方结果和近期信息风险 fail-closed。
- BUY 成交后当时会按配置撤 sibling BUY，并生成 SELL 退出 intent。

当时主要缺口（不代表 2026-07-13 仓库现状）：

- `RewardMarket` 仅持有 `end_at`，没有 `event_start_at` / `event_end_at` / `event_time_source` / `event_time_confidence`。
- Gamma connector 当时只解析 `endDate`，未区分 market lifecycle、resolution deadline 与显式 scheduled event metadata。
- Gamma `startDate` 往往表示市场开放或事件页开始，不一定是比赛开赛或数据发布时间。
- 没有外部结构化赛程/日历 producer。
- 没有互补 YES/NO 持仓合并、split、redeem 或链上 CTF 操作对账。

## 数据来源设计

### 来源优先级

事件时间应按可信度分层：

| 来源 | 用途 | 默认可信度 | 说明 |
|---|---|---|---|
| Manual override | 硬 gate | high | 人工确认，优先级最高 |
| 官方/结构化日历 | 硬 gate | high | 体育联盟赛程、交易所公告、经济日历、财报日历、官方投票截止 |
| Polymarket Gamma reviewed scheduled events | 候选或硬 gate | medium | 只接受显式 `gameStartTime` / `events[].startTime`，且必须满足 hard-gate shape |
| Polymarket Gamma raw scheduled events | 候选 | low | 未审核候选按配置 observe/ignore/medium-confidence；`startDate` 不是候选起点 |
| News/RSS/Data API | 候选/补充 | medium | 适合发现变更或延期 |
| AI extracted | 候选/解释 | low | 需要人工或结构化源确认后才能硬 gate |

### 推荐外部源

- 体育：SportRadar、The Odds API、ESPN schedule、官方 league API。
- 财经：NASDAQ/Polygon/FMP earnings calendar、公司 IR、SEC/EDGAR、交易所公告。
- 宏观：FRED/官方统计局/经济日历供应商、央行日程。
- 加密：token unlock calendar、项目治理日历、链上 proposal deadline。
- 政治/选举：官方选举委员会、政府公告、法院 docket。

### 数据获取约束

所有外部数据必须由后台 producer 同步到数据库。Rewards planner、worker live tick、API handler 和前端只能读 Postgres 或 orderbook 服务缓存，不能在决策路径直接访问外部 API。

## 数据模型

当前核心字段（完整约束见 `0003_reward_event_window_semantics.sql` 和 `init.sql`）：

```sql
CREATE TABLE reward_market_event_windows (
    condition_id TEXT NOT NULL REFERENCES reward_markets(condition_id) ON DELETE CASCADE,
    source TEXT NOT NULL,
    event_key TEXT NOT NULL,
    event_type TEXT NOT NULL,
    event_time_role TEXT NOT NULL,      -- event_occurrence | market_lifecycle | resolution_deadline | unknown
    schedule_status TEXT NOT NULL,      -- scheduled | conflicting | finished | withdrawn | unknown
    time_precision TEXT NOT NULL,       -- exact | date_only | inferred | unknown
    start_source_field TEXT,
    end_policy TEXT NOT NULL,           -- explicit | point | until_market_closed | unknown
    event_start_at TIMESTAMPTZ,
    event_end_at TIMESTAMPTZ,
    confidence TEXT NOT NULL,
    source_url TEXT,
    source_payload JSONB NOT NULL DEFAULT '{}'::JSONB,
    notes TEXT NOT NULL DEFAULT '',
    active BOOLEAN NOT NULL DEFAULT TRUE,
    hard_gate_eligible BOOLEAN NOT NULL DEFAULT FALSE,
    producer_version BIGINT NOT NULL DEFAULT 1,
    source_updated_at TIMESTAMPTZ,
    observed_at TIMESTAMPTZ,
    expires_at TIMESTAMPTZ,
    reviewed_by TEXT,
    reviewed_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (condition_id, source, event_key)
);

CREATE TABLE reward_event_window_source_versions (
    source TEXT NOT NULL,
    condition_id TEXT NOT NULL REFERENCES reward_markets(condition_id) ON DELETE CASCADE,
    producer_version BIGINT NOT NULL,
    source_updated_at TIMESTAMPTZ,
    observed_at TIMESTAMPTZ NOT NULL,
    snapshot_hash TEXT NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (source, condition_id)
);
```

当前不建只能返回单条 condition 记录的 DB view。Store 会返回 condition 下的全部有效候选；application 先按 `event_key` 在 source priority、hard-gate shape、schedule status、confidence 和新鲜度上选出该事件的最佳候选，再在多个独立事件中选择动作最严格的 assessment。这样 manual withdrawal 可覆盖同 `event_key` 的 Gamma 候选，但 condition 下不同事件不会相互覆盖。

Application 层已实现模型：

```text
RewardMarketEventWindow {
  condition_id,
  source,
  event_key,
  event_type,
  event_time_role,
  schedule_status,
  time_precision,
  start_source_field,
  end_policy,
  event_start_at,
  event_end_at,
  confidence,
  source_url,
  source_payload,
  notes,
  active,
  hard_gate_eligible,
  producer_version,
  source_updated_at,
  observed_at,
  expires_at,
  updated_at
}

RewardEventWindowAssessment {
  status,
  reason,
  event_key,
  event_time_role,
  schedule_status,
  time_precision,
  start_source_field,
  end_policy,
  event_start_at,
  event_end_at,
  source,
  confidence,
  producer/audit timestamps
}

RewardEventWindowSourceSnapshot {
  source,
  producer_version,
  observed_at,
  coverage[{ condition_id, source_updated_at }],
  windows
}

RewardEventWindowConfig {
  enabled,
  min_confidence_for_hard_gate,
  stop_new_quote_before_start_sec,
  cancel_open_buy_before_start_sec,
  resume_after_event_end_sec,
  unknown_event_time_mode: allow | observe | block,
  gamma_unreviewed_dates_mode: ignore | observe | medium_confidence
}
```

## 事件窗口硬 Gate

### Gate 状态

对每个 condition 计算：

```text
NoEventWindow
SafeBeforeWindow
StopNewQuotes
CancelOpenBuys
InEventWindow
PostEventCooldown
ExpiredOrResolved
UntrustedEventTime
```

基础规则：

```text
if condition 没有任何离散事件候选:
  NoEventWindow；即使 unknown_event_time_mode=block 也不阻断

if event_time_role != event_occurrence:
  只作生命周期/解析元数据审计，不进入 hard gate

if 已有预期事件候选，但不满足 active + scheduled + exact +
   hard_gate_eligible + start_source_field + event_start_at + valid end_policy:
  unknown_event_time_mode 决定 allow/observe/block

if now >= event_start_at - cancel_open_buy_before_start_sec:
  禁止新 BUY，并撤已有 BUY

else if now >= event_start_at - stop_new_quote_before_start_sec:
  禁止新 BUY，不强制撤已有 BUY

end_policy=explicit:
  以 event_end_at 为结束，再加 resume_after_event_end_sec

end_policy=point:
  以 event_start_at 作为点事件结束，再加 resume_after_event_end_sec

end_policy=until_market_closed:
  事件开始后持续阻断/撤 BUY，直到候选失效、撤回或市场关闭，不伪造 event_end_at
```

历史通用默认建议；当前空库 production live-drill profile 实际使用 Medium confidence、6 小时 stop-new、2 小时 cancel-open，其余以后端 config 为准：

| 配置 | 默认值 |
|---|---:|
| `event_window_enabled` | `true` |
| `min_confidence_for_hard_gate` | `high` |
| `stop_new_quote_before_start_sec` | `10800`（3 小时） |
| `cancel_open_buy_before_start_sec` | `3600`（1 小时） |
| `resume_after_event_end_sec` | `3600`（1 小时） |
| `unknown_event_time_mode` | `observe` |
| `gamma_unreviewed_dates_mode` | `ignore` |

### 插入位置

1. Candidate/list query：可选预过滤，降低后续计算量，但不作为唯一保护。
2. Planner/live cycle：给 quote plan 写入 `event_window` assessment；`StopNewQuotes` 和 unknown block 模式只阻断新增 BUY，`CancelOpenBuys`、`InEventWindow`、`PostEventCooldown` 才把计划 hard-block 并触发已有 BUY 撤单。
3. Provider prefilter：进入窗口的无敞口计划不请求 AI，避免浪费 provider 调用；已有订单/持仓仍可优先刷新风险缓存用于解释。
4. Live placement：提交前重新检查事件窗口，防止 plan 与实际 POST 之间跨入风险窗口。
5. Event-driven hard-risk cancel：活跃 token 收到 orderbook 更新时，对已进入 `CancelOpenBuys` / `InEventWindow` 的 BUY 立即撤单。
6. Fast reconcile：周期兜底撤单，避免错过事件驱动 wake。

### 撤单语义

- BUY：事件窗口硬 gate 命中后撤单。
- SELL exit：不因事件窗口阻断；退出 intent 应继续执行非亏损/配置允许的退出逻辑。
- Pending/unknown/cancel reconciliation：不重复提交，不破坏现有对账锁；只记录 gate reason。
- 已有持仓：不自动亏损平仓，除非后续新增独立 flatten policy。

### Fair-value 交互语义

- 事件窗口在 fair-value edge 生成前已阻断新 BUY，且没有 edge 可评估时，decision 写入 `assessment_status=not_evaluated`。
- `not_evaluated` 不是 `passed=false` 的 gate 失败；fair-value 页将其单独统计，Runs ledger 的 `fair_value_passed` 保持空值，blocker codes 也不添加 `fair_value`。
- 市场选择只对 `assessment_status=evaluated && passed=false` 加 fair-value selection risk penalty；`not_evaluated` 的阻断责任归于上游 event-window gate，不重复惩罚。

## 稳定双边策略

> 以下为历史独立 mode 提案，当前没有 `stable_double` / `RewardStableDoubleMode` 可配置实现。现行路径由 Rewards Market Maker V2 的 `quote_mode=double|auto`、统一 opportunity/stability/exit scoring、fair-value gate 和 live materializer 承担；`BalancedMerge` 不是该 mode 的更名。

### 策略模式

历史建议（未实现）：

```text
RewardQuoteMode:
  double
  auto
  stable_double

RewardStableDoubleMode:
  off
  observe
  enforce
```

`stable_double` 原定义是加严后的专用模式，但该 API/配置未落地；不应从当前已有的双边报价或 BalancedMerge profile 推断它已完成。

### 准入条件

所有条件必须满足：

```text
事件窗口状态为 SafeBeforeWindow 或 NoEventWindow 且 unknown policy 允许
YES/NO 两腿都有新鲜盘口
目标档位为买一或配置档位，且不触碰 best ask
YES bid + NO bid <= 1 - safety_margin
两腿 midpoint 到目标 bid 的距离在 rewards spread 内
两腿 exit depth >= max(min_exit_depth_usd, planned_notional * exit_depth_multiple)
按计划 size 估算的 entry/exit slippage <= 阈值
盘口历史样本数 >= min_book_samples
midpoint_range_cents <= max_midpoint_range_cents
top_of_book_flip_count <= max_top_of_book_flip_count
qualified competition / reward density 达标
无 active reconciliation lock、unknown submission、external 404 锁
AI/info-risk 若开启必须允许，缺缓存按现有配置 fail closed
```

历史默认建议（当前无对应配置键）：

| 配置 | 默认值 |
|---|---:|
| `stable_double_mode` | `observe` |
| `stable_double_quote_bid_rank` | `1` |
| `stable_double_safety_margin_cents` | `2` |
| `stable_double_observation_window_sec` | `1800` |
| `stable_double_min_book_samples` | `30` |
| `stable_double_max_midpoint_range_cents` | `2` |
| `stable_double_max_top_of_book_flip_count` | `6` |
| `stable_double_min_exit_depth_usd` | `50` |
| `stable_double_min_exit_depth_multiple` | `2` |
| `stable_double_max_entry_exit_slippage_cents` | `1` |
| `stable_double_max_condition_notional_usd` | `10` |

### 成交后处理

Phase 1 使用现有语义：

```text
任一 BUY 成交 -> 撤 sibling BUY -> 按 post_fill_strategy 创建 SELL 退出 intent
```

Phase 2 才引入互补持仓合并：

```text
YES position size = y
NO position size = n
mergeable_size = min(y, n)
if mergeable_size >= min_merge_size and total_cost < merge_value - fees - gas_buffer:
  撤剩余相关 BUY
  提交 merge/redeem
  对账持仓减少与资金/收益入账
```

合并链路必须具备：

- connector 支持对应 CTF/Polymarket merge 或 redeem 操作。
- 幂等 external action id。
- pending/unknown/retry 状态。
- gas、approval、链上失败和部分成功处理。
- 与 account positions snapshot 的一致性对账。
- UI 展示 merge pending / merged / failed。

在这些能力未实现前，文案和配置不得描述为“自动合并套利”。

## AI 与 Info-Risk 角色

AI advisory 适合做：

- 从 market question、description、event metadata 和新闻摘要中提取候选 `expected_event_at`。
- 判断事件时间是否不确定、是否存在延期/临时变更。
- 给出 `strategy_hint`：降低 notional、切单边、提高 bid rank、跳过市场。
- 给控制台提供人类可读原因。

AI 不适合做：

- 唯一事件时间来源。
- 绕过硬 gate。
- 在缺少结构化时间时批准高风险临近事件市场。

Info-risk 应增强：

```text
expected_event_at
do_not_quote_before
do_not_quote_until
event_time_confidence
event_time_sources
```

这些字段默认只写入缓存和观察报告；只有来源经 manual/official/sports/economic producer 确认后，才进入硬 gate。

## API 与前端展示

Quote plan 已有 `event_window` assessment，其中可审计字段包括：

```text
status
reason
event_key
event_time_role
schedule_status
time_precision
start_source_field
end_policy
event_start_at
event_end_at
source
confidence
hard_gate_eligible
producer_version
source_updated_at / observed_at / expires_at
```

控制台展示：

- Quote plans 的 readiness 单元格显示事件窗口状态，并显式列出 `source / role / start source field / end policy` provenance。
- Fair-value 工作台单独显示 `not_evaluated`，不将上游事件阻断误报为 fair-value 失败。
- 配置面板已展示事件窗口阈值和未知时间处理；未实现独立 stable-double observe/enforce 配置或 `stable_double_metrics`。

## 实施计划

### Phase 0：调研与样本验证

- 已完成代码语义分离：`startDate` / `startDateIso` 只是 market lifecycle，`endDate` / `endDateIso` 只是 resolution deadline；只有 `gameStartTime` / `events[].startTime` 能生成 scheduled-event 候选。
- 已完成 Gamma fixture 覆盖：字段一致、时间冲突、sports/非 sports、finished 事件和多事件分离。
- 未完成：按 sports / macro / earnings / crypto / politics 做足量真实市场字段语义验证。
- 部分完成：代码内已形成 Gamma source confidence/hard-gate shape 规则，外部类别规则仍待 producer 落地。
- 未完成：输出足量人工标注样本用于回归测试。

剩余验收：

- 至少 50 个市场样本，标注 Gamma 日期是否等于真实事件时间。
- 明确哪些类别允许 Gamma reviewed explicit schedule 升级为 hard gate。

### Phase 1：事件窗口数据层

- 已完成：冻结 baseline 上由 `0003_reward_event_window_semantics.sql` 增加 provenance、multi-event identity、hard-gate shape constraint 和 source-version fence，`init.sql` 同步最新结构。
- 已完成：Application 增加模型、source snapshot replace contract、Store trait 方法和多候选 assessment。
- 已完成：Infrastructure 的 Postgres/in-memory 实现按 `(condition_id, source, event_key)` 保存候选，支持 covered-condition tombstone、幂等重放、stale snapshot fence 和过期过滤。
- 已完成：Orderbook/Gamma sync 只解析显式 scheduled-event 字段；Gamma sports scheduled event 使用 `event_occurrence + exact + start_source_field + until_market_closed`，lifecycle/deadline 日期不能 hard gate。
- 未完成：增加 worker CLI 或 admin path 支持 manual override 导入。

验收：

- 可按 condition 查询全部有效 event candidates，同一 `event_key` 可由高优先级 source 覆盖，不同事件保持独立。
- 无 event window 时保持当前行为。
- Gamma lifecycle/deadline 不会触发硬拦截；只有满足 hard-gate shape 且达到配置置信度的 scheduled event 才能参与。

### Phase 2：事件窗口硬 Gate

- 已完成：`RewardBotConfig` 增加事件窗口配置，并接入 Postgres key-value、API patch、前端 DTO/schema/config panel。
- 已完成：live cycle 写入事件窗口状态；`CancelOpenBuys` / `InEventWindow` / `PostEventCooldown` 会使计划 hard-block，`StopNewQuotes` 只阻断新增 BUY。
- 已完成：Live placement 和 BUY 提交前 last-look 会阻断进入事件窗口的新 BUY intent；已有 live BUY 只在撤单窗口撤。
- 已完成：Fast reconcile / event cancel 共用 `live_cancel_reason`，事件窗口撤 BUY 会产生专用 reason；SELL exit 不因事件窗口阻断。
- 已完成：Provider refresh prefilter 跳过被事件窗口阻断新增的无敞口计划；已有订单/持仓仍优先保留用于风险解释。
- 已完成：Quote-plan API/DTO 保留 event provenance，前端展示 source / role / start field / end policy。
- 已完成：事件窗口先阻断且无 edge 时，fair-value 使用 `not_evaluated`，不计失败、blocker 或 selection risk penalty。

验收：

- 单元测试覆盖 stop-new、cancel-open、in-event、cooldown、unknown policy。
- 已有 SELL exit 不被事件窗口阻断。
- 事件窗口跨越时已有 BUY 会被撤，新 BUY 不会提交。

### Replay schema（跨阶段已完成）

- 当前新 capture 写入 schema V3，保留 V2 的 decision-window top-of-book 历史压缩、final delta 和 normalized expected-plan hash 比较。
- Replay validator/执行器同时接受 V1、V2、V3；V1 仍可使用完整 final state / expected plans，V2 仍可使用紧凑字段，缺少 `schema_version` 按 V1 兼容。
- V3 的目的是固化当前嵌套决策模型语义，不是删除 V1/V2 读取路径。

### Phase 3：结构化外部日历 Producer

- 未完成：先选一个高价值类别接入，例如体育赛程或经济数据日历。
- 未完成：Producer 通过 source snapshot contract 写入 `reward_market_event_windows`，只由 worker/orderbook 服务运行。
- 未完成：接入 source payload、source URL、stale/expiry 和延期/取消更新。

验收：

- 对目标类别能稳定生成 high confidence event_start_at。
- 外部源失败时保留上一版并降低新写入风险，不在策略路径直接回源。

### Phase 4：稳定双边 Observe

- V2 替代路径已完成相关共享能力：统一 `opportunity_metrics` 复用 book history 计算 midpoint range、top-of-book flip、退出深度、坏成交恢复天数、竞争倍数和 100U 日奖，并在 quote plan/front 表格展示。
- 未完成且已被 V2 路径取代：独立 `stable_double_metrics`、observe mode 配置和专用“稳定双边通过/样本不足”状态列。

原 observe 验收（历史记录，不宣称独立 mode 已通过）：

- 表格能解释每个市场为何通过/不通过稳定双边。
- observe 不改变当前订单提交数量和方向。

### Phase 5：稳定双边 Enforce（历史计划，已由 V2 路径取代）

- V2 替代路径已落地：统一 `opportunity_metrics` 参与 score/eligibility，事件窗口是新增 BUY/撤 BUY 前置硬 gate，档位与安全边际由现有 live materializer 校验。
- 独立 stable-double enforce mode 没有完成，也不是待补同名 profile；当前通过 `quote_mode=double|auto`、`selection_mode`、机会/稳定性/退出评分、fair-value gate 和 live materializer 组合实现 V2 做市路径。
- 当时 BUY 成交后仍沿用 sibling cancel + SELL exit；该语义现已移除。

原 enforce 验收（历史记录，不宣称独立 mode 已通过）：

- 只在全部稳定条件满足时双边挂单。
- 任一稳定性、事件窗口、退出深度或 provider gate 失败都会 fail closed。
- 小额实盘 smoke 后再提高额度。

### Phase 6：互补持仓合并 / Redeem（后续以 BalancedMerge 形态落地）

原计划：

- 设计并实现 CTF merge/redeem connector。
- 增加 merge intent、状态机、幂等、对账和 UI。
- 先 observe 识别可合并持仓，再 paper，再 guarded live。

原验收：

- 能在测试账户小额完成合并并对账。
- 失败不会重复提交或破坏 positions/account state。
- merge/redeem 关闭前不得在 UI 中描述为已实现。

后续结果：BalancedMerge 已实现 guarded merge intent、广播 fencing 和 receipt 对账，但仍需真实凭证、资金、gas/approval 准备和小额 live drill；这不表示任意市场或任意互补持仓都会自动合并。

## 测试计划

- 已完成：Application unit tests 覆盖 stop-new、cancel-open、in-event、cooldown、无事件不误 block、conflicting expected event、lifecycle 非 hard gate、`until_market_closed`、multi-event 最严格聚合和 manual withdrawal 覆盖 Gamma。
- 已完成：In-memory/Postgres store tests 覆盖 source snapshot replacement、missing-key tombstone、stale fence、过期过滤、非法 hard-gate shape 拒绝，以及前向迁移隔离 legacy Gamma rows。Postgres 用例需要 `POLYEDGE_TEST_DATABASE_URL` 才执行。
- 已完成：Gamma connector/orderbook producer fixtures 覆盖显式 start provenance、冲突时间、sports hard-gate shape、非 sports observe-only、finished 和 multi-event。
- 已完成：Replay tests 覆盖当前 V3 capture，以及 V1、缺失版本按 V1、V2 紧凑 fixture 的反序列化/回放兼容。
- 待补：Worker integration tests 覆盖 live placement 事件窗口 last-look、event cancel BUY 撤单、SELL exit 不阻断。
- 不适用：独立 stable-double observe/enforce 测试，因为该 mode 已被 V2 路径取代；现行稳定性逻辑应由统一 `opportunity_metrics`、fair-value、event-window 和 live-materializer 测试覆盖。
- Integration/smoke：本地 Postgres + orderbook cache + worker run-once，验证 quote plan reason 和撤单事件。

## 风险与缓解

| 风险 | 缓解 |
|---|---|
| Gamma lifecycle 日期被误当事件时间 | `startDate` / `endDate` 只写入 lifecycle/deadline 元数据；hard gate 要求 `event_occurrence + exact + start_source_field + valid end_policy` |
| 外部赛程源延迟或错误 | 保留 source/confidence、人工 override、stale 检查 |
| 事件延期导致过早停挂或误恢复 | Producer 支持更新；post-event cooldown；info-risk 辅助提示 |
| 稳定双边被低样本盘口误判 | 样本不足 fail closed；要求 top-of-book flip 和 midpoint range 同时达标 |
| 双边成交后无合并能力 | 历史风险；当前由独立 BalancedMerge profile 和 merge intent 处理，不恢复 blanket sibling cancel |
| 合并链路链上失败 | 已增加 broadcasting/tx-hash fence 和 receipt 对账；无 hash 的未知广播状态 fail closed |

## 当前后续优先级

- 事件窗口主路径已落地；后续优先补高价值结构化外部日历 producer、manual override 操作路径、真实市场人工标注样本和 worker live integration/smoke。
- 不再建议新建独立 stable-double mode；应继续在 Rewards Market Maker V2 的统一评分、fair-value、事件窗口和 live materializer 内做可解释性与校准。
- BalancedMerge 代码链路已落地，但小额真实账户验证、gas/approval 准备和 ops runbook 仍是上线前要求。
