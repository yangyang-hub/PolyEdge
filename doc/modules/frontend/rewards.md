# Rewards（奖励机器人）

最后更新：2026-05-31

## 概述

`/rewards` 页面管理做市奖励机器人的生命周期：配置、运行模拟、查看订单/持仓/事件、取消和重置。

## 架构与关键文件

| 文件 | 职责 |
|---|---|
| `src/app/(console)/rewards/page.tsx` | 路由页面 |
| `src/features/rewards/components/rewards-workbench.tsx` | 主工作台组件 |
| `src/features/rewards/components/rewards-tables.tsx` | 表格组件（订单/持仓等） |
| `src/features/rewards/components/rewards-events-panel.tsx` | 事件面板 |
| `src/features/rewards/components/number-input.tsx` | 数值输入组件 |
| `src/features/rewards/loaders/rewards-page-data.ts` | 服务端数据装配 |
| `src/features/rewards/lib/rewards-helpers.ts` | 辅助函数 |
| `src/features/rewards/types.ts` | 类型定义（~26 行） |

## 核心类型（types.ts）

- **`NumberConfigKey`**：22 个数值配置参数的字符串联合类型 — `max_markets`、`max_open_orders`、`per_market_usd`、`quote_size_usd`、`min_daily_reward`、`max_spread_cents`、`quote_edge_cents`、`account_capital_usd` 等
- **`EventCategory`**：`"all" | "placements" | "cancels" | "fills" | "rewards"`

## API 依赖

- `src/lib/api/rewards.ts` — `readRewardBotSnapshot`、`updateRewardBotConfig`、`runRewardBotOnce`、`cancelRewardBotOrders`、`resetRewardBot`

## 关键交互

- **Run** → `runRewardBotOnce()` → 展示新的 snapshot
- **Cancel** → `cancelRewardBotOrders()` → 清空未成交订单
- **Reset** → `resetRewardBot()` → 重置资金池到初始资本
- **Config 编辑** → `updateRewardBotConfig(patch)` → 即时更新配置
- 事件面板支持按 `EventCategory` 过滤

## 数据流

所有 mutation 通过 Server Actions，每次返回完整的 `RewardBotSnapshotDto`，前端直接替换 snapshot 状态。

## i18n

使用 `rewards` 命名空间字典。

## 当前状态

- 完整的 Run / Cancel / Reset 交互
- 配置编辑（22 个数值参数）
- 配置提示说明 `max_markets=0`、`max_open_orders=0`、`quote_size_usd=0` 都会停止新挂单；已赚奖励只会在后端检测到新鲜缓存盘口后计提。
- 事件分类视图（挂单/撤单/吃单/奖励）
- 当前只做模拟，不会实盘下单

## 修改检查清单

- [ ] 新增配置参数时同步更新 `NumberConfigKey` 类型
- [ ] 新增事件类别时同步更新 `EventCategory`
- [ ] 修改后人工 smoke `/rewards` 页面（Run/Cancel/Reset、配置编辑、事件过滤）
