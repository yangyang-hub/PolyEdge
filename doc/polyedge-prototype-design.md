# PolyEdge 前端原型设计文档

> **状态（2026-06-14）**：本文是前端原型设计背景，包含已移除或未落地的 approvals、research/replay 和早期页面规划。当前控制台页面以 [../AGENTS.md](../AGENTS.md)、[../README.md](../README.md) 和 [modules/frontend/](modules/frontend/) 为准。

## 1. 文档目标

本文档用于指导 PolyEdge 前端原型页面设计，面向低保真线框图、中保真交互原型和高保真视觉稿。

它不讨论前端实现细节，而是回答以下问题：

1. 需要画哪些页面。
2. 每个页面的核心目标是什么。
3. 每个页面应该放哪些模块。
4. 页面之间如何跳转。
5. 哪些状态和弹层必须进入原型。

本文档的主要输入来自：

1. [polyedge-frontend-design.md](./polyedge-frontend-design.md)
2. [polyedge-design.md](./polyedge-design.md)
3. [polyedge-api-contract.md](./polyedge-api-contract.md)
4. [polyedge-auth-design.md](./polyedge-auth-design.md)

如需基于这些原型进一步确定实现框架与依赖，请参考 [polyedge-frontend-ui-stack.md](./polyedge-frontend-ui-stack.md)。

---

## 2. 原型设计范围

当前原型以桌面端内部交易控制台为主，目标分辨率建议优先按 `1440px` 宽度设计。

本轮原型重点覆盖：

1. 全局控制台布局。
2. `dashboard`
3. `markets`
4. `events`
5. `signals`
6. `positions`
7. `risk`
8. `approvals`
9. `research/replay`
10. 通用弹层和状态页面

本轮原型不要求优先细化：

1. 复杂移动端交易交互。
2. 对外营销页面。
3. 全量设置页。
4. 精细动画和视觉动效。

---

## 3. 目标用户

### 3.1 `viewer`

目标：

1. 查看系统状态。
2. 观察事件、市场和信号。
3. 进入回放和研究页面。

### 3.2 `operator`

目标：

1. 审核待确认信号。
2. 查看订单、持仓和风险说明。
3. 在限定权限内执行撤单或手动确认。

### 3.3 `risk_admin`

目标：

1. 监控系统模式。
2. 处理风险告警。
3. 触发或解除 kill switch。
4. 处理高优先级异常。

原型中应默认以 `operator + risk_admin` 的复合视角设计，这样信息密度和操作链路才不会偏轻。

---

## 4. 原型设计原则

1. 解释链路优先。
   用户必须能从 signal 追到 market、event、evidence 和 risk decision。
2. 桌面优先。
   交易和审批页面默认采用高信息密度布局，不为移动端牺牲主工作流。
3. 列表加详情。
   大多数页面优先采用“主列表 + 右侧详情抽屉/详情面板”。
4. 关键操作显式确认。
   审批、撤单、切模式、kill switch 都必须进入原型。
5. 状态完整。
   不能只画正常态，还要画空态、错误态、断流态和权限受限态。
6. 实时区域边界清晰。
   只在必要区域强调实时更新，不把整页都做成跳动面板。

---

## 5. 原型交付优先级

| 优先级 | 页面/对象 | 说明 |
| --- | --- | --- |
| `P0` | 全局布局 | 决定所有页面的骨架 |
| `P0` | `dashboard` | 系统首页，承载最核心概览 |
| `P0` | `signals` | 交易与审批的主操作页面 |
| `P0` | `approvals` | 高风险动作和人工确认入口 |
| `P0` | `risk` | 风险处置页面 |
| `P1` | `markets` | 市场浏览与结算语义检查 |
| `P1` | `events` | 事件流和证据解释页面 |
| `P1` | `positions` | 持仓、PnL、主题敞口 |
| `P2` | `research/replay` | 研究型原型，可后补 |
| `P2` | `settings` | 首版只需低保真说明 |

---

## 6. 全局信息架构

### 6.1 顶层导航

建议左侧导航固定包含：

1. Dashboard
2. Markets
3. Events
4. Signals
5. Positions
6. Risk
7. Approvals
8. Replay
9. Settings

### 6.2 全局固定区域

原型中所有主页面都建议包含以下固定区域：

1. Top Bar
2. Side Nav
3. Main Content
4. Bottom Status Rail

### 6.3 全局布局草图

```text
+----------------------------------------------------------------------------------+
| Top Bar: logo | environment | mode | alert summary | search | user | kill switch |
+----------------------+-----------------------------------------------------------+
| Side Nav             | Main Content                                              |
| dashboard            | page title                                                |
| markets              | page subtitle / filters                                   |
| events               |                                                           |
| signals              | list / cards / charts / drawers                           |
| positions            |                                                           |
| risk                 |                                                           |
| approvals            |                                                           |
| replay               |                                                           |
+----------------------+---------------------------------------------+-------------+
| Bottom Status Rail: api health | market stream | event stream | risk engine      |
+----------------------------------------------------------------------------------+
```

---

## 7. 全局组件清单

原型中应先统一这些基础组件，再复用到各页面：

1. 页面标题区
2. 筛选栏
3. KPI 卡片
4. 数据表格
5. 时间线列表
6. 详情抽屉
7. 右侧解释面板
8. 状态标签
9. 风险告警条
10. 审批确认弹层
11. 二次确认弹层
12. 空态卡片
13. 错误态卡片
14. 断流提示条

### 7.1 状态标签建议

原型中至少要统一以下标签体系：

1. `mode`
   `research`、`paper_trade`、`manual_confirm`、`live_auto`
2. `signal_state`
   `new`、`active`、`weakened`、`invalidated`、`reversed`、`executed`、`expired`
3. `ambiguity_level`
   `low`、`medium`、`high`
4. `severity`
   `info`、`warning`、`critical`
5. `tradability_status`
   `auto`、`manual_review`、`blocked`

---

## 8. 页面原型说明

### 8.1 `dashboard`

#### 页面目标

让用户在 10 秒内回答：

1. 系统当前处于什么模式。
2. 是否有风险告警。
3. 是否有待审批信号。
4. 当前哪些市场或事件最值得关注。

#### 页面模块

1. 顶部系统状态条
2. 今日核心指标卡
3. 实时信号面板
4. 风险告警面板
5. 待审批队列
6. 热点市场列表
7. 最新事件流

#### 线框草图

```text
+----------------------------------------------------------------------------------+
| Dashboard                                                                        |
| system mode | kill switch | daily pnl | open alerts | pending approvals          |
+----------------------+----------------------+-------------------------------------+
| Real-time Signals    | Risk Alerts          | Pending Approvals                   |
| signal table/list    | critical alerts      | queue with actions                  |
+---------------------------------------------+-------------------------------------+
| Hot Markets                                  | Latest Events                       |
| ranked market list                           | time-ordered event feed             |
+----------------------------------------------------------------------------------+
```

#### 关键交互

1. 点击 signal 行，打开 signal 详情抽屉。
2. 点击风险告警，跳转 `risk` 页面并定位原因。
3. 点击审批项，进入 `approvals` 页面或直接打开审批弹层。

#### 必画状态

1. 正常态
2. 有 critical alert 的告警态
3. 无待审批项空态
4. 实时流断开态

---

### 8.2 `markets`

#### 页面目标

让用户快速浏览市场，并验证：

1. 市场价格与流动性如何。
2. 市场是否可交易。
3. 结算语义是否清晰。
4. 有哪些关联事件和证据。

#### 页面模块

1. 市场筛选栏
2. 市场列表表格
3. 市场详情抽屉
4. 结算语义面板
5. 关联事件列表
6. 证据摘要卡

#### 线框草图

```text
+----------------------------------------------------------------------------------+
| Markets                                                                          |
| filters: category | tradability | ambiguity | search                             |
+----------------------------------------------+-----------------------------------+
| market list/table                             | market detail drawer              |
| question                                      | question                          |
| best bid / ask                                | price + liquidity                 |
| ambiguity level                               | resolution source                 |
| tradability                                   | edge cases                        |
| updated at                                    | related events / evidences        |
+----------------------------------------------+-----------------------------------+
```

#### 关键交互

1. 选中某个 market，右侧打开详情。
2. 在详情中点击相关 event，跳转 `events` 页面。
3. 点击相关 signal，跳转 `signals` 页面并携带筛选。

#### 必画状态

1. 正常列表态
2. `manual_review` 市场详情态
3. `blocked` 市场态
4. 无搜索结果空态

---

### 8.3 `events`

#### 页面目标

让用户理解事件流是如何被系统转换成可交易认知的。

#### 页面模块

1. 时间范围筛选
2. 原始事件流
3. 标准化事件卡
4. market mapping 结果
5. evidence 列表
6. 解释与理由面板

#### 线框草图

```text
+----------------------------------------------------------------------------------+
| Events                                                                           |
| filters: source | status | market | time range                                   |
+-------------------------------+--------------------------------------------------+
| event timeline/list           | selected event detail                            |
| source / summary / time       | normalized summary                               |
| relevance badge               | related markets                                  |
| review flag                   | evidence list                                    |
|                               | llm/rule reason                                  |
+-------------------------------+--------------------------------------------------+
```

#### 关键交互

1. 点击事件卡，查看完整 evidence。
2. 点击 evidence 对应 market，跳转 `markets`。
3. 点击 signal 生成记录，跳转 `signals`。

#### 必画状态

1. 事件流正常态
2. 高歧义事件态
3. `observe_only` 态
4. 数据源异常提示态

---

### 8.4 `signals`

#### 页面目标

这是最关键的操作页面，用来判断：

1. 当前有哪些可行动 signal。
2. 这些 signal 的 edge、confidence 和 lifecycle state 是什么。
3. 是否应该审批、观察、撤销或忽略。

#### 页面模块

1. signal 筛选栏
2. signal 主列表
3. posterior / edge 摘要区
4. signal 详情抽屉
5. risk decision 面板
6. 操作区

#### 线框草图

```text
+----------------------------------------------------------------------------------+
| Signals                                                                          |
| filters: state | side | confidence | approval required                           |
+------------------------------------------+---------------------------------------+
| signal list                              | signal detail                          |
| market                                   | market summary                         |
| fair price / market price / edge         | posterior / edge                       |
| confidence                               | evidence stack                         |
| lifecycle state                          | risk decision                          |
| approval status                          | actions                                |
+------------------------------------------+---------------------------------------+
```

#### 详情区必须包含

1. 市场问题
2. market price
3. fair price / posterior
4. edge
5. confidence
6. evidence 列表
7. risk check 结果
8. signal state history
9. 操作按钮

#### 关键交互

1. 批准 signal
2. 拒绝 signal
3. 查看完整 risk reason
4. 跳转相关 market / event

#### 必画状态

1. `active`
2. `weakened`
3. `invalidated`
4. 需人工审批态
5. 审批后反馈态

---

### 8.5 `positions`

#### 页面目标

展示仓位、盈亏和风险暴露，帮助用户理解组合风险。

#### 页面模块

1. PnL 摘要卡
2. 持仓列表
3. 主题风险桶
4. 市场分布图
5. 减仓建议或风险提示

#### 线框草图

```text
+----------------------------------------------------------------------------------+
| Positions                                                                        |
| daily pnl | realized pnl | unrealized pnl | gross exposure | net exposure        |
+------------------------------------------+---------------------------------------+
| positions table                          | exposure breakdown                     |
| market / side / qty / avg cost / pnl     | theme buckets                          |
|                                          | event clusters                         |
+------------------------------------------+---------------------------------------+
```

#### 必画状态

1. 正常持仓态
2. 无持仓空态
3. 某主题敞口过高警示态

---

### 8.6 `risk`

#### 页面目标

集中展示风险状态、限制原因和应急操作入口。

#### 页面模块

1. 全局风险摘要
2. 告警列表
3. 风险桶面板
4. kill switch 状态区
5. 模式切换区
6. 审计摘要

#### 线框草图

```text
+----------------------------------------------------------------------------------+
| Risk                                                                             |
| current mode | kill switch | daily loss usage | alert count                       |
+----------------------------------------+-----------------------------------------+
| alert list                             | global controls                         |
| severity / reason / target             | switch mode                             |
| created at                             | trigger kill switch                     |
|                                         | release controls                        |
+----------------------------------------+-----------------------------------------+
| risk buckets                                                                     |
| market / theme / cluster exposure                                                |
+----------------------------------------------------------------------------------+
```

#### 关键交互

1. 查看某条风险告警详情。
2. 打开模式切换弹层。
3. 打开 kill switch 二次确认弹层。

#### 必画状态

1. 正常态
2. warning 态
3. critical + kill switch 激活态
4. 权限不足态

---

### 8.7 `approvals`

#### 页面目标

作为人工确认中心，集中处理高风险动作。

#### 页面模块

1. 待审批队列
2. 审批详情抽屉
3. 审批理由输入区
4. 风险和审计上下文
5. 批准 / 拒绝按钮

#### 线框草图

```text
+----------------------------------------------------------------------------------+
| Approvals                                                                        |
| filters: type | severity | status                                                |
+---------------------------------------+------------------------------------------+
| approval queue                        | selected approval detail                  |
| signal / mode switch / kill switch    | reason context                            |
| ambiguity / risk level                | evidence / risk / audit summary           |
| created by / created at               | approve / reject                          |
+---------------------------------------+------------------------------------------+
```

#### 关键交互

1. 输入审批理由。
2. 执行批准。
3. 执行拒绝。
4. 审批后返回列表并更新状态。

#### 必画弹层

1. 批准二次确认弹层
2. 拒绝确认弹层
3. 权限不足弹层

---

### 8.8 `research/replay`

#### 页面目标

帮助研究人员回放某次运行，查看事件、证据、posterior 和 signal 的时间演化。

#### 页面模块

1. run 选择器
2. 时间轴
3. 回放控制条
4. 事件面板
5. signal 演化面板
6. 指标摘要面板

#### 线框草图

```text
+----------------------------------------------------------------------------------+
| Replay                                                                           |
| run selector | play / pause | speed | time cursor                                |
+----------------------------------------+-----------------------------------------+
| timeline                               | detail panel                             |
| events / evidence / signals            | selected point detail                    |
|                                         | posterior and state transition           |
+----------------------------------------+-----------------------------------------+
```

#### 必画状态

1. 正常回放态
2. 无可用 run 空态
3. 回放加载态

---

## 9. 通用弹层与特殊状态

这些内容必须单独画，不应只在页面备注里描述：

1. 批准 signal 弹层
2. 拒绝 signal 弹层
3. 切换 mode 弹层
4. 触发 kill switch 弹层
5. 释放 kill switch 弹层
6. 权限不足弹层
7. 系统错误弹层
8. 实时流断开提示条
9. 空状态页面
10. 加载骨架态

---

## 10. 页面之间的核心流转

原型至少要覆盖以下五条主流程：

### 10.1 从 Dashboard 进入信号审批

```text
dashboard
-> pending approvals
-> approvals detail
-> approve / reject modal
-> success feedback
-> signal detail
```

### 10.2 从 Market 验证结算语义

```text
markets list
-> market detail
-> resolution rules
-> related events
-> related signal
```

### 10.3 从 Event 追踪到 Signal

```text
event timeline
-> event detail
-> evidence list
-> related market
-> signal detail
```

### 10.4 处理风险异常

```text
dashboard alert
-> risk page
-> alert detail
-> mode switch or kill switch modal
-> audit feedback
```

### 10.5 回放研究过程

```text
replay run select
-> timeline move
-> inspect event/evidence
-> inspect signal transition
-> read summary metrics
```

---

## 11. 页面字段清单建议

原型阶段不需要完整 API 字段，但以下字段建议在页面里有明确落点。

### 11.1 `Market`

1. `question`
2. `best_bid`
3. `best_ask`
4. `mid_price`
5. `volume_24h`
6. `ambiguity_level`
7. `tradability_status`
8. `updated_at`

### 11.2 `Event`

1. `source`
2. `summary`
3. `relevance_score`
4. `confidence`
5. `status`
6. `related_market_ids`

### 11.3 `Evidence`

1. `direction`
2. `strength`
3. `source_reliability`
4. `novelty`
5. `resolution_relevance`
6. `status`
7. `expires_at`

### 11.4 `Signal`

1. `market_price`
2. `fair_price`
3. `edge`
4. `confidence`
5. `lifecycle_state`
6. `reason`
7. `updated_at`

### 11.5 `RiskState`

1. `mode`
2. `kill_switch`
3. `daily_pnl`
4. `gross_exposure`
5. `net_exposure`
6. `open_alerts`

---

## 12. 原型稿交付建议

建议分三轮输出：

### 12.1 第一轮：低保真

目标：

1. 确认页面数量。
2. 确认布局骨架。
3. 确认页面间跳转。

至少输出：

1. 全局布局
2. `dashboard`
3. `signals`
4. `approvals`
5. `risk`

### 12.2 第二轮：中保真

目标：

1. 补齐主要页面。
2. 加入状态标签、抽屉、弹层。
3. 补齐异常态和权限态。

### 12.3 第三轮：高保真

目标：

1. 补齐视觉系统。
2. 加入图表和重点信息层级。
3. 强化风险和审批动作的视觉区分。

---

## 13. 原型评审清单

每次评审原型时，建议按下面的问题过一遍：

1. 首页是否能在 10 秒内传达系统状态。
2. 用户是否能从 signal 快速追到 evidence 和 risk reason。
3. 高风险操作是否足够显式且安全。
4. 页面是否区分了正常态、空态、错误态和断流态。
5. 信息是否过多但缺少层级。
6. 关键流程是否需要反复跳页。
7. 是否有页面只展示结果，没有展示原因。

---

## 14. 推荐原型输出清单

如果交给产品或设计师，建议最终至少产出这些 frame：

1. 全局控制台 Layout
2. Dashboard 默认态
3. Dashboard 告警态
4. Markets 列表态
5. Market 详情抽屉
6. Events 列表和详情
7. Signals 列表态
8. Signal 详情态
9. Approve Signal 弹层
10. Risk 页面正常态
11. Risk 页面 critical 态
12. Kill Switch 弹层
13. Approvals 队列态
14. Replay 页面
15. 权限不足态
16. 断流态
17. 空态
18. 错误态

这 18 个 frame 足以支持首轮原型评审和后续实现拆分。
