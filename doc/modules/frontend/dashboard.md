# Dashboard（仪表盘）

最后更新：2026-06-27

## 概述

`/dashboard` 页面展示系统整体概览：市场摘要、事件/证据统计和新闻源健康状态。页面通过 REST API 初始加载，不再描述前端实时流。

## 架构与关键文件

| 文件 | 职责 |
|---|---|
| `src/app/(console)/dashboard/page.tsx` | 路由页面 |
| `src/features/dashboard/components/dashboard-overview.tsx` | 主组件 |
| `src/features/dashboard/loaders/dashboard-page-data.ts` | 服务端数据装配 |

## 数据流

Loader 调用 markets、events 和 news API 聚合仪表盘数据，通过 props 传递给组件。

## API 依赖

- `src/lib/api/markets.ts` — 市场摘要
- `src/lib/api/events.ts` — 事件和证据统计
- `src/lib/api/news.ts` — 新闻源健康状态

## i18n

使用 `dashboard` 命名空间字典。

## 当前状态

已实现，展示系统概览视图；旧信号和风控卡片已移除，当前顶部指标聚焦市场、事件、证据和新闻源同步状态。

- 2026-06-27：移除 signals/risk API 依赖，仪表盘改为市场/事件/新闻源健康概览。

## 修改检查清单

- [ ] 新增仪表盘指标时更新 loader 和组件
- [ ] 修改后人工 smoke `/dashboard` 页面
- [ ] 同步更新 i18n 字典
