# Markets（市场）

最后更新：2026-07-12

## 概述

`/markets` 页面展示 Polymarket 预测市场列表和结算详情，支持可交易性快捷筛选、类别筛选、24h 成交量排序和服务端分页。

## 架构与关键文件

| 文件 | 职责 |
|---|---|
| `src/app/(console)/markets/page.tsx` | 路由页面 |
| `src/features/markets/components/markets-workbench.tsx` | 主组件 |
| `src/features/markets/loaders/markets-page-data.ts` | Feature 数据装配；当前由 client boundary 调用 |
| `src/features/markets/lib/use-markets-query.ts` | 客户端筛选/排序/分页请求、取消过期请求和选中状态 |
| `src/features/markets/lib/markets-query.ts` | 查询参数与 20 条页大小 |
| `src/features/markets/lib/markets-mappers.ts` | Market/Event DTO 到表格和详情视图模型的映射 |
| `src/features/markets/types.ts` | 筛选、排序和视图模型类型 |

## API 依赖

- `src/lib/api/markets.ts` — `listMarkets(params)`、`listMarketCategories()`

## 数据流

Loader 并行调用 `listMarkets()`、`listEvents()` 和 `listMarketCategories()` 装配首屏，默认选中 blocked/观察市场或首条市场。交互后由 `useMarketsQuery()` 根据本地 filter/category/sort/page 状态重新请求 markets；首屏 events 缓存在 hook 中，筛选和翻页不重复请求 events。新请求会 abort 上一个请求，防止过期响应覆盖。

## 核心类型

- `MarketFilter = "all" | "review_queue" | "watch_only"`，分别映射全部、`blocked`、`observe_only`。
- `SortDir = "desc" | "asc" | "none"`，只对 `volume_24h` 排序。
- `MarketViewModel` 用于表格；`MarketDetailViewModel` 包含 condition id、slug、结算来源、edge-case notes 和关联事件。

## i18n

使用 `markets` 命名空间字典。

## 当前状态

已实现每页 20 条的服务端分页、全部/审查队列/仅观察筛选、类别筛选和 24h 成交量升降序。右侧结算视图展示可交易性、歧义等级、结算来源、edge-case notes、关联事件和 Polymarket 链接。当前没有独立的 market status 筛选 UI，也不在 URL 中持久化筛选状态。

## 修改检查清单

- [ ] 新增筛选条件时同步更新 `MarketListParams` 类型和 API 调用
- [ ] 修改后人工 smoke `/markets` 页面
