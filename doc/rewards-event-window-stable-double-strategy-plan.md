# Rewards 事件窗口与稳定双边策略设计方案

最后更新：2026-06-27

状态：核心事件窗口已实现。当前仓库已具备 `reward_market_event_windows` 数据层、Gamma 日期候选同步、事件窗口配置、planner/live placement/live cancel/provider prefilter 硬 gate、前端配置字段和单元测试。外部官方赛程/日历 producer、manual override 管理 UI、稳定双边独立展示、互补 YES/NO 持仓合并或 redeem 执行链路仍未实现，不能描述为当前可用能力。

## 背景

Rewards 做市策略在安静市场上可以依靠买一/买二挂单获取 maker rewards，但部分市场在真实事件临近时会从低波动状态切换到高波动状态。例如体育比赛开赛、财报发布、经济数据公布、投票截止、官方结果发布或 token unlock。开赛前几天盘口可能稳定，开赛前数小时信息和交易强度会快速变化，此时继续被动挂买单容易遭遇逆向选择。

另一个相关机会是低波动二元市场中同时在 YES 和 NO 买一挂单。如果两边成交价之和小于 1 且留有安全边际，理论上可以通过持有互补 outcome 或合并/redeem 降低方向风险。但当前系统成交后主要走 sibling cancel 和 SELL 退出，不支持自动合并互补持仓。

## 目标

- 建立结构化事件时间数据层，支持按市场关联的真实事件时间做硬风控。
- 在 planner、live placement 和 live cancel 三个阶段执行事件窗口 gate，避免只依赖 AI 判断。
- 将稳定双边策略建模为独立 quote mode / strategy profile，只在盘口稳定、可退出和事件窗口安全时启用。
- 把 AI advisory 和 info-risk 用作辅助解释、候选时间提取和不确定性提示，而不是最终下单权限来源。
- 为后续互补 YES/NO 持仓合并或 redeem 设计清晰执行边界。

## 非目标

- 不让 API handler、前端或策略代码直接抓取外部赛程、Gamma、CLOB 或新闻数据。
- 不把 Polymarket Gamma `startDate` 直接当作比赛开始时间硬执行。
- 不把 LLM 输出作为唯一事件时间来源。
- 第一阶段不实现自动 merge/redeem，不改变当前 BUY fill 后 sibling cancel + SELL 退出语义。
- 不放宽现有 rewards 市场质量、盘口、资金、库存、kill switch 和 provider fail-closed 风控。

## 当前基础

已可复用能力：

- `polyedge-orderbook` 统一同步 Gamma 市场、reward markets、orderbook cache 和 reward price-history candles。
- Rewards planner 已支持 `quote_mode=double|auto`，默认双边报价；live materializer 会验证双边目标档位、spread、安全边际和 fallback 单腿。
- `quote_bid_rank=1|2|3` 支持买一/买二/买三，默认买一。
- `opportunity_metrics` 已计算竞争倍数、奖励密度、退出深度、入场/退出滑点、坏成交恢复天数、盘口样本数、midpoint range 和 top-of-book flip count。
- AI advisory payload 已包含当前盘口、pricing context、最近 1h candles 和 cache TTL horizon，可输出 `allow_quote` 与 `strategy_hint`。
- info-risk 已支持 `allow_quote`、`resolution_imminent`、官方结果和近期信息风险 fail-closed。
- BUY 成交后当前会按配置撤 sibling BUY，并生成 SELL 退出 intent。

主要缺口：

- `RewardMarket` 仅持有 `end_at`，没有 `event_start_at` / `event_end_at` / `event_time_source` / `event_time_confidence`。
- Gamma connector 当前只解析 `endDate`，未解析 `startDate`、`startDateIso`、`events[].startDate/endDate` 或 event metadata。
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
| Polymarket Gamma reviewed dates | 候选或硬 gate | medium/high | 仅在 `hasReviewedDates=true` 且字段语义匹配时提升 |
| Polymarket Gamma raw dates | 候选 | low/medium | `startDate` 可能是市场开放时间，需要校验 |
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

已实现表：

```sql
CREATE TABLE reward_market_event_windows (
    condition_id TEXT NOT NULL REFERENCES reward_markets(condition_id) ON DELETE CASCADE,
    source TEXT NOT NULL,
    event_type TEXT NOT NULL,
    event_start_at TIMESTAMPTZ,
    event_end_at TIMESTAMPTZ,
    confidence TEXT NOT NULL,
    source_url TEXT,
    source_payload JSONB NOT NULL DEFAULT '{}'::JSONB,
    notes TEXT NOT NULL DEFAULT '',
    active BOOLEAN NOT NULL DEFAULT TRUE,
    reviewed_by TEXT,
    reviewed_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (condition_id, source)
);
```

可选派生快照（当前未建 DB view，effective selection 由 application/store 查询实现）：

```sql
CREATE VIEW reward_market_effective_event_windows AS
SELECT DISTINCT ON (condition_id)
  condition_id,
  event_type,
  event_start_at,
  event_end_at,
  confidence,
  source,
  source_url,
  updated_at
FROM reward_market_event_windows
WHERE active
ORDER BY
  condition_id,
  CASE confidence
    WHEN 'high' THEN 3
    WHEN 'medium' THEN 2
    ELSE 1
  END DESC,
  CASE source
    WHEN 'manual' THEN 5
    WHEN 'official' THEN 4
    WHEN 'sports_api' THEN 4
    WHEN 'economic_calendar' THEN 4
    WHEN 'gamma_reviewed' THEN 3
    WHEN 'gamma' THEN 2
    ELSE 1
  END DESC,
  updated_at DESC;
```

Application 层模型建议：

```text
RewardMarketEventWindow {
  condition_id,
  event_type,
  event_start_at,
  event_end_at,
  confidence,
  source,
  source_url,
  notes,
  updated_at
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
if no trusted event_start_at:
  unknown_event_time_mode 决定 allow/observe/block

if now >= event_start_at - cancel_open_buy_before_start_sec:
  禁止新 BUY，并撤已有 BUY

else if now >= event_start_at - stop_new_quote_before_start_sec:
  禁止新 BUY，不强制撤已有 BUY

if event_start_at <= now <= event_end_at + resume_after_event_end_sec:
  禁止新 BUY；已有 BUY 按配置撤或保持已撤状态
```

默认建议：

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

## 稳定双边策略

### 策略模式

建议新增：

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

`stable_double` 不是替代现有 `double`，而是加严后的专用模式。也可以先作为 `opportunity_metrics` 下的 enforce gate 落地，暂不新增 quote mode。

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

默认建议：

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

Quote plan 建议新增展示字段：

```text
event_window_status
event_window_reason
event_start_at
event_end_at
event_time_source
event_time_confidence
stable_double_metrics
```

控制台展示：

- 状态列显示“事件窗口停挂 / 即将撤单 / 稳定双边通过 / 稳定双边样本不足”。
- 配置面板展示事件窗口阈值、未知时间处理、稳定双边 observe/enforce。
- 风险摘要显示事件时间来源和置信度，避免误把 Gamma 市场开放时间当开赛时间。

## 实施计划

### Phase 0：调研与样本验证

- 抽样 Gamma 原始字段：`startDate`、`startDateIso`、`endDate`、`events[].startDate/endDate`、`hasReviewedDates`、event metadata。
- 按 sports / macro / earnings / crypto / politics 分类验证字段语义。
- 形成 source confidence 规则。
- 输出一组人工标注样本用于回归测试。

验收：

- 至少 50 个市场样本，标注 Gamma 日期是否等于真实事件时间。
- 明确哪些类别允许 Gamma reviewed date 升级为 hard gate。

### Phase 1：事件窗口数据层

- 已完成：新增 `0054_reward_market_event_windows.sql`。
- 已完成：Application 增加模型、Store trait 方法和 effective window 查询。
- 已完成：Infrastructure 增加 Postgres/in-memory 实现。
- 已完成：Orderbook/Gamma sync 解析 Gamma 日期作为低/中置信候选；默认 `gamma_unreviewed_dates_mode=ignore`，不直接硬 gate。
- 增加 worker CLI 或 admin path 支持 manual override 导入。

验收：

- 可按 condition 查询 effective event window。
- 无 event window 时保持当前行为。
- Gamma 候选不会默认触发硬拦截。

### Phase 2：事件窗口硬 Gate

- 已完成：`RewardBotConfig` 增加事件窗口配置，并接入 Postgres key-value、API patch、前端 DTO/schema/config panel。
- 已完成：live cycle 写入事件窗口状态；`CancelOpenBuys` / `InEventWindow` / `PostEventCooldown` 会使计划 hard-block，`StopNewQuotes` 只阻断新增 BUY。
- 已完成：Live placement 和 BUY 提交前 last-look 会阻断进入事件窗口的新 BUY intent；已有 live BUY 只在撤单窗口撤。
- 已完成：Fast reconcile / event cancel 共用 `live_cancel_reason`，事件窗口撤 BUY 会产生专用 reason；SELL exit 不因事件窗口阻断。
- 已完成：Provider refresh prefilter 跳过被事件窗口阻断新增的无敞口计划；已有订单/持仓仍优先保留用于风险解释。

验收：

- 单元测试覆盖 stop-new、cancel-open、in-event、cooldown、unknown policy。
- 已有 SELL exit 不被事件窗口阻断。
- 事件窗口跨越时已有 BUY 会被撤，新 BUY 不会提交。

### Phase 3：结构化外部日历 Producer

- 先选一个高价值类别接入，例如体育赛程或经济数据日历。
- Producer 写入 `reward_market_event_windows`，只由 worker/orderbook 服务运行。
- 增加 source payload、source URL 和 stale 处理。
- 对延期/取消事件更新 event window。

验收：

- 对目标类别能稳定生成 high confidence event_start_at。
- 外部源失败时保留上一版并降低新写入风险，不在策略路径直接回源。

### Phase 4：稳定双边 Observe

- 已部分完成：当前统一 `opportunity_metrics` 已复用 book history 计算 midpoint range、top-of-book flip、退出深度、坏成交恢复天数、竞争倍数和 100U 日奖，并在 quote plan/front 表格展示。
- 未完成：独立 `stable_double_metrics` 命名结构和专用“稳定双边通过/样本不足”状态列。
- 增加配置和前端展示。

验收：

- 表格能解释每个市场为何通过/不通过稳定双边。
- observe 不改变当前订单提交数量和方向。

### Phase 5：稳定双边 Enforce

- 已部分完成：统一 `opportunity_metrics` 已参与 score/eligibility，事件窗口已作为新增 BUY/撤 BUY 前置硬 gate，默认买一和安全边际沿用现有 live materializer。
- 未完成：独立 stable-double enforce mode；当前仍通过 `quote_mode=double|auto`、`selection_mode`、机会评分和 live materializer 组合实现，不提供单独稳定双边策略 profile。
- 当前 BUY 成交后仍沿用 sibling cancel + SELL exit。

验收：

- 只在全部稳定条件满足时双边挂单。
- 任一稳定性、事件窗口、退出深度或 provider gate 失败都会 fail closed。
- 小额实盘 smoke 后再提高额度。

### Phase 6：互补持仓合并 / Redeem

- 设计并实现 CTF merge/redeem connector。
- 增加 merge intent、状态机、幂等、对账和 UI。
- 先 observe 识别可合并持仓，再 paper，再 guarded live。

验收：

- 能在测试账户小额完成合并并对账。
- 失败不会重复提交或破坏 positions/account state。
- merge/redeem 关闭前不得在 UI 中描述为已实现。

## 测试计划

- 已完成：Application unit tests 覆盖事件窗口 stop-new、cancel-open、in-event、cooldown、unknown policy 和 confidence gate。
- 已完成：Backend compile check 覆盖 Postgres/in-memory store wiring、Gamma parser/orderbook producer、worker live cancel/placement wiring。
- 待补：Store integration tests 覆盖 Postgres upsert/effective query/manual override 优先级。
- 待补：Worker integration tests 覆盖 live placement 事件窗口 last-look、event cancel BUY 撤单、SELL exit 不阻断。
- 待补：Connector fixture tests 覆盖 Gamma 日期解析不误升 high confidence。
- 待补：独立 stable-double observe/enforce 测试；当前稳定性逻辑由统一 `opportunity_metrics` 覆盖，没有独立 stable-double profile。
- Integration/smoke：本地 Postgres + orderbook cache + worker run-once，验证 quote plan reason 和撤单事件。

## 风险与缓解

| 风险 | 缓解 |
|---|---|
| Gamma `startDate` 语义不稳定 | 默认只作为候选；需要 reviewed/official/manual 才 hard gate |
| 外部赛程源延迟或错误 | 保留 source/confidence、人工 override、stale 检查 |
| 事件延期导致过早停挂或误恢复 | Producer 支持更新；post-event cooldown；info-risk 辅助提示 |
| 稳定双边被低样本盘口误判 | 样本不足 fail closed；要求 top-of-book flip 和 midpoint range 同时达标 |
| 双边成交后无合并能力 | Phase 1-5 沿用 sibling cancel + SELL exit，不承诺合并 |
| 合并链路链上失败 | Phase 6 增加 intent 状态机、幂等和对账后再 live |

## 推荐优先级

优先做 Phase 1-2。它们能直接降低“事件临近还在挂买单”的主要实盘风险，且不依赖复杂 merge/redeem。

第二优先级是 Phase 4-5，将现有机会评分和盘口历史收敛成可解释的稳定双边 gate。

Phase 6 应最后做，并且必须通过小额真实账户验证后再开启。
