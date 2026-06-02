# Rewards（奖励机器人）

最后更新：2026-06-02

## 概述

`/rewards` 页面管理做市奖励机器人的生命周期：配置策略参数、向 worker 提交运行/取消/重置命令、查看订单/持仓/事件。仅支持实盘（live）模式。

## 架构与关键文件

| 文件 | 职责 |
|---|---|
| `src/app/(console)/rewards/page.tsx` | 路由页面 |
| `src/features/rewards/components/rewards-workbench.tsx` | 主工作台编排：状态/操作区、指标条、活动/配置/风控 tabs |
| `src/features/rewards/components/rewards-overview-cards.tsx` | 顶部执行概览、操作中心和关键指标条 |
| `src/features/rewards/components/rewards-config-panel.tsx` | 分组策略配置面板（执行、市场筛选、报价构造、库存与控制） |
| `src/features/rewards/components/rewards-tables.tsx` | 表格组件（订单/持仓等） |
| `src/features/rewards/components/rewards-events-panel.tsx` | 事件面板 |
| `src/features/rewards/components/number-input.tsx` | 数值输入组件 |
| `src/features/rewards/loaders/rewards-page-data.ts` | 服务端数据装配 |
| `src/features/rewards/lib/rewards-helpers.ts` | 辅助函数 |
| `src/features/rewards/types.ts` | 类型定义（~26 行） |

## 核心类型（types.ts）

- **`NumberConfigKey`**：数值配置参数的字符串联合类型 — `max_markets`、`max_open_orders`、`per_market_usd`、`quote_size_usd`、`min_daily_reward`、`max_spread_cents`、`quote_edge_cents`、`account_capital_usd` 等
- **`EventCategory`**：`"all" | "placements" | "cancels" | "fills" | "rewards"`

## API 依赖

- `src/lib/api/rewards.ts` — `readRewardBotSnapshot`、`updateRewardBotConfig`、`runRewardBotOnce`、`cancelRewardBotOrders`、`resetRewardBot`
- `readRewardBotSnapshot()` 会传递订单分页/搜索/状态/排序 query；后端返回 `orders` 当前页和 `orders_page` 总数元数据

## 关键交互

- **Run** → `runRewardBotOnce()` → API 写入 `run_once` 控制命令，worker 领取后执行一轮 live 策略
- **Cancel open orders** → `cancelRewardBotOrders()` → API 写入 `cancel_all` 控制命令，worker 领取后撤销 Polymarket live 托管订单
- **Reset** → `resetRewardBot()` → API 写入 `reset` 控制命令，worker 领取后按 cancel-all 撤销 live 订单
- **Config 编辑** → `updateRewardBotConfig(patch)` → 即时更新配置
- 事件面板支持按 `EventCategory` 过滤
- 页面默认展示活动视图：左侧候选报价计划，右侧托管订单与本地库存，下方事件/成交流；策略配置和风控配置通过 tabs 切换，减少实盘盯盘时的配置噪音。

## 数据流

所有 mutation 通过 Server Actions。配置保存会立即返回更新后的 `RewardBotSnapshotDto`；Run / Cancel / Reset 只表示命令已入队，返回的是入队后的当前 snapshot，实际订单/资金池变化会在 worker 处理命令后出现在后续 snapshot 中。

## i18n

使用 `rewards` 命名空间字典。

## 当前状态

- 完整的 Run / Cancel / Reset 入队交互
- 顶部执行概览展示实盘模式、启停/运行状态、市场就绪度、可用资金比例、最近扫描/运行时间和事件触发计数。
- 操作中心集中 Run / Save / Cancel / Reset，文案提醒当前命令可能提交或取消 Polymarket 实盘订单。
- 配置编辑按执行、市场筛选、报价构造、库存与控制分组，包含数值参数、布尔开关和成交后策略。
- 配置不包含 `execution_mode` 选择器（始终为 live）。提示说明 `max_markets=0`、`max_open_orders=0`、`quote_size_usd=0` 都会停止新挂单。
- 报价计划默认展示可挂市场，本地支持全部/可挂/不可挂切换，并用状态标记说明每个当前候选计划是否符合最终过滤要求。
- Managed orders 表格使用后端分页（默认每页 15 条），翻页、搜索、状态过滤和排序都会重新请求 `/api/v1/rewards-bot`。
- Positions 表格展示 rewards 库存账本；实盘库存仍以后端真实成交对账缺口为准。
- 事件分类视图（挂单/撤单/吃单/奖励）
- live 模式已接入 post-only 买单提交和撤单；真实成交对账、成交后卖出/平仓、真实库存/资金同步和奖励结算对账仍是后端缺口

## 修改检查清单

- [ ] 新增配置参数时同步更新 `NumberConfigKey` 类型
- [ ] 新增事件类别时同步更新 `EventCategory`
- [ ] 修改后人工 smoke `/rewards` 页面（Run/Cancel/Reset、配置编辑、事件过滤）
