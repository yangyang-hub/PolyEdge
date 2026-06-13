# Dashboard（仪表盘）

最后更新：2026-06-13

## 概述

`/dashboard` 页面展示系统整体概览：关键指标、市场摘要、信号状态、风控状态等。页面通过 REST API 初始加载，不再描述前端实时流。

## 架构与关键文件

| 文件 | 职责 |
|---|---|
| `src/app/(console)/dashboard/page.tsx` | 路由页面 |
| `src/features/dashboard/components/dashboard-overview.tsx` | 主组件 |
| `src/features/dashboard/loaders/dashboard-page-data.ts` | 服务端数据装配 |

## 数据流

Loader 调用多个 API 模块（markets、signals、risk 等）聚合仪表盘数据，通过 props 传递给组件。

## API 依赖

- `src/lib/api/markets.ts` — 市场摘要
- `src/lib/api/signals.ts` — 信号状态
- `src/lib/api/risk.ts` — 风控状态

## i18n

使用 `dashboard` 命名空间字典。

## 当前状态

已实现，展示系统概览视图；顶部状态文案描述后端 REST/API 数据同步，不再使用“市场数据流已同步”或“实时信号”表述。

## 修改检查清单

- [ ] 新增仪表盘指标时更新 loader 和组件
- [ ] 修改后人工 smoke `/dashboard` 页面
- [ ] 同步更新 i18n 字典
