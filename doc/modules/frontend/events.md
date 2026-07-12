# Events（事件）

最后更新：2026-07-12

## 概述

`/events` 页面展示市场事件时间线、首条关联证据、reason trace 和关联市场。当前页面一次加载事件/证据/市场数据，在客户端选中事件和本地分页；旧关联信号展示已移除。

## 架构与关键文件

| 文件 | 职责 |
|---|---|
| `src/app/(console)/events/page.tsx` | 路由页面 |
| `src/features/events/components/events-workbench.tsx` | 主组件 |
| `src/features/events/loaders/events-page-data.ts` | Feature 数据装配；当前由 client boundary 调用 |
| `src/components/shared/truncate-text.tsx` | reason trace 截断/展开 |
| `src/hooks/use-pagination.ts` | 事件时间线本地分页 |

## 数据流与结构

Loader 并行读取 events、evidences 和 markets，选择首个 active 事件（无 active 时回退到首条），然后把每个事件映射为展示模型。展示模型包含状态/relevance/confidence、reason trace、最多 6 个关联市场，以及该事件找到的首条 evidence 及其 direction/strength/reliability/novelty/resolution relevance。

## API 依赖

- `src/lib/api/events.ts` — `listEvents(query)`、`listEvidences(query)`
- `src/lib/api/markets.ts` — `listMarkets()`，用于解析关联市场标题

## i18n

使用 `shared` 命名空间字典（events 无独立命名空间）。

## 当前状态

已实现事件时间线、选中详情、候选证据、reason trace 和关联市场。时间线每页 15 条，为前端本地分页；当前每个事件只展示找到的第一条 evidence，不是证据全量列表。页面没有独立的状态筛选控件。

## 修改检查清单

- [ ] 修改后人工 smoke `/events` 页面
- [ ] 新增事件字段时同步更新 DTO
