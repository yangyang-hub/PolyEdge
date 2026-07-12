# Dashboard（仪表盘）

最后更新：2026-07-12

## 概述

`/dashboard` 页面展示系统整体概览：市场覆盖与可交易数、活跃事件、新闻源健康、热门市场和最新事件。页面通过 `ClientDataBoundary` 在客户端调用 loader 加载 Rust API 数据，不直接访问外部数据源。

## 架构与关键文件

| 文件 | 职责 |
|---|---|
| `src/app/(console)/dashboard/page.tsx` | 路由页面 |
| `src/features/dashboard/components/dashboard-overview.tsx` | 主组件 |
| `src/features/dashboard/loaders/dashboard-page-data.ts` | Feature 数据装配；当前由 client boundary 调用 |
| `src/components/shared/client-data-boundary.tsx` | 统一处理加载、错误和重试 |
| `src/hooks/use-pagination.ts` | 热门市场本地分页 |

## 数据流

Loader 并行调用 markets、events 和 news source-health API，在前端派生可交易市场、活跃事件和降级新闻源数，并把市场、事件、源健康视图模型传给组件。

## 核心数据结构

- Loader 返回 `metrics`、`markets`、`events`、`sourceHealth` 四组纯展示数据。
- 市场行包含问题、分类、中点价和可交易状态；新闻源行包含类型、最后更新、健康分和 tone。

## API 依赖

- `src/lib/api/markets.ts` — 市场摘要
- `src/lib/api/events.ts` — 事件和证据统计
- `src/lib/api/news.ts` — 新闻源健康状态

## i18n

使用 `dashboard` 命名空间字典。

## 当前状态

已实现当前系统概览。顶部指标聚焦市场覆盖、可交易市场、活跃事件和新闻源健康；下方显示热门市场、最新事件与最多 10 个新闻源。市场列表使用前端本地分页。旧 signals/risk 卡片和实时流已移除。

## 修改检查清单

- [ ] 新增仪表盘指标时更新 loader 和组件
- [ ] 修改后人工 smoke `/dashboard` 页面
- [ ] 同步更新 i18n 字典
