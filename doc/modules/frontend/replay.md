# Replay（回放）

最后更新：2026-05-31

## 概述

`/replay` 页面用于历史数据回放，验证策略行为或调试数据管道。

## 架构与关键文件

| 文件 | 职责 |
|---|---|
| `src/app/(console)/replay/page.tsx` | 路由页面 |
| `src/features/replay/components/replay-workbench.tsx` | 主组件 |
| `src/features/replay/loaders/replay-page-data.ts` | 服务端数据装配 |

## 当前状态

基础框架已具备。

## 修改检查清单

- [ ] 修改后人工 smoke `/replay` 页面
