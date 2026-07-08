# Rewards（LP rewards）

最后更新：2026-07-08

## 概述

`/rewards` 页面管理 Polymarket 做市 live 策略：配置策略参数、向 worker 提交 run/cancel/reset 命令、查看报价计划、托管订单、持仓、成交和事件。`/rewards/fair-value` 独立展示 fair-value 估值、edge 和 gate 结果。页面只支持 live 路径，不提供模拟模式。

## 架构与关键文件

| 文件 | 职责 |
|---|---|
| `src/app/(console)/rewards/page.tsx` | 路由页面 |
| `src/app/(console)/rewards/fair-value/page.tsx` | fair-value 工作台路由 |
| `src/features/rewards/components/rewards-workbench.tsx` | 主工作台编排：概览、活动、配置、风险 tabs |
| `rewards-fair-value-workbench.tsx` | fair-value 估值、confidence、uncertainty、edge 和决策表格 |
| `rewards-overview-cards.tsx` | 顶部执行概览、每日 LLM 调用统计、操作中心和关键指标 |
| `rewards-config-panel.tsx` | 策略配置面板：执行、市场筛选、机会评分、报价构造、fair-value、成交后合并、盘口选择、AI/信息风险、库存与控制 |
| `rewards-opportunity-config.tsx` | 统一机会评分配置 |
| `rewards-opportunity-summary.tsx` | quote plan 行内机会评分摘要 |
| `rewards-advanced-config.tsx` | 盘口选择、AI advisory、信息风险和事件窗口配置 |
| `rewards-risk-config.tsx` | 库存、撤单、深度、velocity、requote 和 reconcile 配置 |
| `rewards-config-fields.tsx` | 配置面板共享字段组件 |
| `rewards-tables.tsx` | 报价计划和托管订单表格入口，重导出成交/持仓/事件表 |
| `rewards-fills-table.tsx` | 成交记录表格 |
| `rewards-positions-table.tsx` | 持仓表格和 PnL 展示 |
| `rewards-events-table.tsx` | 风险/操作事件表格 |
| `rewards-table-controls.tsx` | 搜索、筛选、分页和排序控件 |
| `rewards-events-panel.tsx` | 事件面板 |
| `number-input.tsx` | 数值输入组件 |
| `loaders/rewards-page-data.ts` | 首屏数据装配 |
| `loaders/rewards-fair-value-page-data.ts` | fair-value 页面数据装配 |
| `lib/rewards-helpers.ts` | readiness、事件分类和 AI strategy hint metrics helper |
| `types.ts` | `NumberConfigKey`、`EventCategory` |

## 核心类型

- `NumberConfigKey` 覆盖当前可编辑数值配置：市场上限、开放订单上限、最低日奖、市场质量门槛、机会评分 `opportunity_*`、fair-value `fair_value_*`、dominant 单边阈值、盘口集中度阈值、AI advisory TTL、provider 并发、信息风险 TTL、事件窗口秒数、首单观察窗口、报价构造、adaptive post-fill 与 pending-exit 重评、库存、requote、BalancedMerge、深度/velocity/reconcile 等字段。
- `RewardFairValueEstimateDto` / `RewardFairValueDecisionDto` / `RewardQuoteEdgeDto` 映射后端 fair-value 估计、组件、edge、rewards rebate 折扣和 gate 结果。
- `EventCategory = "all" | "placements" | "cancels" | "fills" | "rewards"`。

## API 依赖

- `src/lib/api/rewards.ts`：`readRewardBotSnapshot`、`updateRewardBotConfig`、`runRewardBotOnce`、`cancelRewardBotOrders`、`resetRewardBot`。
- `readRewardBotSnapshot()` 传递计划/订单分页、搜索、状态和排序 query；首屏请求 `plans_eligible=true`，与默认“可挂”页签一致。
- Snapshot 不返回全量 reward markets。页面使用 `status.markets_tracked`、`eligible_markets`、`ready_quote_markets`、`waiting_orderbook_markets`、`provider_pending_markets`、`blocker_counts`、`quote_plans[].opportunity_metrics`、AI/info-risk 字段和 `llm_usage` 展示市场覆盖、最终可挂、实时可报价、等待 provider、资金不足、live 盘口验证、风险拦截和每日 LLM 调用。
- `available_usd`、positions 和当日奖励来自 worker 写入数据库的账户快照；API handler 不持有 Polymarket 私钥，也不直接请求私有账户数据。

## 关键交互

- Run：`runRewardBotOnce()` 写入 `run_once` 控制命令，worker 领取后执行一轮 live 策略。
- Cancel open orders：`cancelRewardBotOrders()` 写入 `cancel_all` 控制命令，worker 撤销本系统托管 live 订单。
- Reset：`resetRewardBot()` 写入 `reset` 控制命令，worker 按 cancel-all 处理并重置本地策略状态。
- Config：`updateRewardBotConfig(patch)` 保存配置并返回最新 snapshot。
- 挂单档位：`quote_bid_rank=1|2|3` 对应买一/买二/买三；最终下单前仍用当前 orderbook 做 live 验证。
- 漂移换价：`requote_drift_cents` 配合确认窗口、订单冷却和单轮最大撤单数，避免盘口抖动导致大规模撤空。
- 盘口选择：`quote_mode=double|auto` 与 `selection_mode=observe|enforce` 控制双边/单边候选；live placement 阶段用当前 orderbook 验证退出深度、集中度、档位和安全边际。
- 成交后合并：`balanced_merge_enabled` 默认关闭；开启后后端追加 `balanced_merge` profile 候选，同一 condition 与 standard 冲突时 standard 优先。该 profile 固定 YES/NO 双边 BUY，一侧成交后不生成 SELL、不撤对侧 BUY，full tick/fast reconcile 会发现可配对库存并写入 merge intent。`balanced_merge_auto_execute_enabled` 默认关闭，开启后 worker 通过 Safe proxy wallet 提交 CTF merge。
- AI/信息风险：页面保存 provider 类型、request format、TTL、并发上限、AI strategy hint、信息风险模式/等级和首单 gate。API key、base URL、模型名、超时和 web search 开关只来自 worker 环境变量。worker 用 combined provider refresh 补齐缓存，同一 condition 的 advisory/info-risk 都到期时可合并为一次外部请求。
- 事件窗口：页面配置启用开关、最低置信度、赛前停止新增、赛前撤 BUY、赛后恢复冷却、未知事件时间处理和 Gamma 未审核日期处理。
- 成交后退出：`exit_at_markup` / `hold_and_requote` / `flatten_immediately` / `adaptive` 都基于非亏损 floor。post-only SELL 会在可能穿盘口时改挂当前卖一；固定 flatten 只有 best bid 不低于 floor 才使用 FAK/taker SELL；`adaptive` 会额外按深度/溢价参数、quote plan 和硬风险选择具体退出策略，并在本地未提交的 `ExitPending` SELL 持仓期间按重评周期、重选冷却、单单上限和最小改善门槛继续选择当前更合适的退出方式。
- 机会评分：统一 `opportunity_*` 配置把竞争倍数、100U 日奖、账户/单市场资金占比、退出深度、入场退出滑点、坏成交恢复天数、盘口样本、中点波动、top-of-book 跳变和权重转为综合分。
- Fair-value：`fair_value_*` 配置控制估值启用、历史记录、最低 confidence、raw/effective edge、不确定性缓冲、rewards rebate 折扣、YES/NO 中点偏离上限和历史样本窗口；`/rewards/fair-value` 页面展示最近 quote plans 的估值和拦截原因。
- 表格刷新：页面每 10 秒静默刷新当前 snapshot；手动搜索/分页/操作使用单调请求序号，只接收最新响应。

## 数据流

```text
Page loader
    -> readRewardBotSnapshot(query)
    -> Rewards workbench

User mutation
    -> Server Action
    -> Rewards API
    -> RewardBotService command/config write
    -> worker live loop processes command
    -> next snapshot reflects DB state
```

所有 mutation 通过 Server Actions。Run / Cancel / Reset 返回“命令已入队后的当前 snapshot”，不代表外部订单已经完成变化；实际状态由 worker 后续写库后反映。

## 当前状态

- 顶部概览展示 live 启停/运行状态、实时可报价、最终可挂、候选计划、已拦截、等待 provider、资金不足、live 盘口验证、AI/info-risk 拦截、钱包余额和每日 LLM 统计。
- 操作中心集中 Run / Save / Cancel / Reset，并明确这些命令可能提交或取消 Polymarket live 订单。
- 配置面板只暴露当前仍生效的参数；成交后策略包含固定模式和 adaptive 模式，选择 adaptive 时显示深度、溢价、风险触发、回退策略和 pending-exit 重评节流参数；旧模拟资金、固定单腿金额和已删除诊断配置不再出现。
- 主策略页 header 提供 Fair value 入口；fair-value 页单独显示 tracked/pass/blocked/avg confidence 指标和 quote plan 估值审计表。
- 市场筛选面板公开最低流动性、24h 成交量、剩余结算时间、Gamma spread 和目录同步年龄门槛。
- 竞争度只作为统一机会评分的一部分展示；fair-value 作为独立做市定价 gate 展示，不再作为旧 EV strategy mode。
- 报价计划默认展示通过非盘口依赖过滤且等待 live 盘口验证的候选；`quote_readiness` 区分可报价、等待盘口、等待 AI/信息风险和已拦截。
- Managed orders 表格发送后端分页/搜索/状态/排序 query；“已成交”筛选包含部分成交订单；订单行展示退出策略来源、当前具体退出策略和 adaptive 重选次数。
- 持仓和订单表格展示 API snapshot 注入的 `token_quotes`（best bid/ask/mark price）；缺盘口时显示 `—`，不阻断页面。
- 当日已赚奖励展示 worker 同步的 UTC 当日 maker rewards，worker 优先读取 CLOB 聚合端点，失败时回退明细端点。
- 页面不直接访问 Polymarket 私有账户；账户余额、positions 和本系统托管订单都从数据库读取。

## i18n

使用 `rewards` 命名空间字典。

## 修改检查清单

- [ ] 新增数值配置时同步更新 `NumberConfigKey`、DTO、Server Action 校验和配置面板。
- [ ] 新增事件类别时同步更新 `EventCategory` 和字典。
- [ ] 修改 snapshot query 时同步更新 API 模块、loader 和后端 handler。
- [ ] 修改后运行 `yarn build`，并人工 smoke `/rewards` 的配置保存、Run/Cancel/Reset、表格搜索/分页。
