# Positions（持仓）

最后更新：2026-06-26

## 概述

`/positions` 页面展示当前持仓列表，包含数量、成本、PnL 等信息。

## 架构与关键文件

| 文件 | 职责 |
|---|---|
| `src/app/(console)/positions/page.tsx` | 路由页面 |
| `src/features/positions/components/positions-workbench.tsx` | 主组件 |
| `src/features/positions/loaders/positions-page-data.ts` | 服务端数据装配 |

## API 依赖

- `src/lib/api/positions.ts` — `listPositions(query)`（含字段映射：`net_quantity` → `quantity`、`avg_cost` → `average_cost`、`connector_name` → `bucket_name`）

## 特殊注意

`positions.ts` 是唯一使用 `fetchListContract` 的 `mapItem` 做字段重命名的 API 模块，因为后端字段名与前端 DTO 不完全一致。

## i18n

使用 `positions` 命名空间字典。

## 当前状态

已实现，展示持仓列表和汇总。

- 详情面板展示买一（`market.best_bid`）、卖一（`market.best_ask`）、盈亏金额（realized+unrealized）、盈亏百分比（总盈亏 / 成本基准）；买一/卖一缺盘口时回退到 `mark_price`，成本基准为 0 时盈亏百分比显示 `—`。表格列与汇总不变。
- 2026-06-13：清理组件未使用代码并收窄分页重置 effect 依赖；交互与 API 依赖不变。

## 修改检查清单

- [ ] 修改后端持仓字段时同步更新 `positions.ts` 中的 `mapItem` 映射
- [ ] 修改后人工 smoke `/positions` 页面
