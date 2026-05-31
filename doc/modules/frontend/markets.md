# Markets（市场）

最后更新：2026-05-31

## 概述

`/markets` 页面展示 Polymarket 预测市场列表，支持按状态、类别、可交易性筛选和排序。

## 架构与关键文件

| 文件 | 职责 |
|---|---|
| `src/app/(console)/markets/page.tsx` | 路由页面 |
| `src/features/markets/components/markets-workbench.tsx` | 主组件 |
| `src/features/markets/loaders/markets-page-data.ts` | 服务端数据装配 |

## API 依赖

- `src/lib/api/markets.ts` — `listMarkets(params)`、`listMarketCategories()`

## 数据流

Loader 从 URL 查询参数构建 `MarketListParams`，调用 `listMarkets()` 和 `listMarketCategories()`，组装 `MarketsPageData` 传给组件。

## i18n

使用 `markets` 命名空间字典。

## 当前状态

已实现，支持分页、筛选、排序。

## 修改检查清单

- [ ] 新增筛选条件时同步更新 `MarketListParams` 类型和 API 调用
- [ ] 修改后人工 smoke `/markets` 页面
