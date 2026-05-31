# Events（事件）

最后更新：2026-05-31

## 概述

`/events` 页面展示市场事件和证据，关联到具体市场，支持按状态筛选。

## 架构与关键文件

| 文件 | 职责 |
|---|---|
| `src/app/(console)/events/page.tsx` | 路由页面 |
| `src/features/events/components/events-workbench.tsx` | 主组件 |
| `src/features/events/loaders/events-page-data.ts` | 服务端数据装配 |

## API 依赖

- `src/lib/api/events.ts` — `listEvents(query)`、`listEvidences(query)`

## i18n

使用 `shared` 命名空间字典（events 无独立命名空间）。

## 当前状态

已实现，展示事件列表和关联证据。

## 修改检查清单

- [ ] 修改后人工 smoke `/events` 页面
- [ ] 新增事件字段时同步更新 DTO
