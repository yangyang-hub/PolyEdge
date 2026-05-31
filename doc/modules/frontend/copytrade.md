# Copy Trading（跟单）

最后更新：2026-05-31

## 概述

`/copy-trading` 页面管理跟单机器人：跟踪钱包、配置策略参数、运行模拟、查看成交/订单/事件、取消和重置。

## 架构与关键文件

| 文件 | 职责 |
|---|---|
| `src/app/(console)/copy-trading/page.tsx` | 路由页面 |
| `src/features/copytrade/components/copytrade-workbench.tsx` | 主工作台组件 |
| `src/features/copytrade/loaders/copytrade-page-data.ts` | 服务端数据装配 |

## API 依赖

- `src/lib/api/copytrade.ts` — `readCopyTradeSnapshot`、`updateCopyTradeConfig`、`addTrackedWallet`、`removeTrackedWallet`、`setWalletStatus`、`runCopyTradeOnce`、`analyzeWallets`、`cancelCopyTradeOrders`、`resetCopyTrade`

## 关键交互（copytrade-workbench.tsx）

**状态管理：** `useState` 管理 snapshot、draft（配置）、feedback、pending、walletAddress、walletLabel

**分页：** 三个独立的分页列表（source_trades、orders、events），每页 20 条，使用 `usePagination` hook

**操作函数：**
- `runAction(action)` — 设置 pending → 执行 async action → `applyResult(result)` 更新状态
- `applyResult(result)` — 将 `CopyTradeActionResult` 应用到 snapshot 和 draft

**Server Actions（从 actions.ts 导入）：**
- `addTrackedWalletAction` — 添加跟踪钱包
- `removeTrackedWalletAction` — 移除跟踪钱包
- `setCopytradeWalletStatusAction` — 设置钱包状态（active/paused）
- `updateCopyTradeConfigAction` — 更新配置
- `runCopyTradeOnceAction` — 运行一次跟单
- `analyzeCopytradeWalletsAction` — 分析钱包
- `cancelCopyTradeOrdersAction` — 取消所有订单
- `resetCopyTradeAction` — 重置模拟

## 数据流

```
Loader（copytrade-page-data.ts）
    → readCopyTradeSnapshot()
    → 返回 CopyTradeSnapshotDto

Client Component（copytrade-workbench.tsx）
    → 接收 initialSnapshot prop
    → 用户操作 → Server Action → API → 返回新 snapshot → 更新状态
```

## i18n

使用 `copytrade` 命名空间字典。

## 当前状态

- 完整的 Run / Analyze / Cancel / Reset 交互
- 钱包管理（添加/移除/暂停/激活）
- 配置编辑
- 三个分页列表（成交/订单/事件）
- `mode=live` 已结构化支持但未接入真实下单

## 修改检查清单

- [ ] 新增操作时在 `actions.ts` 中添加 Server Action，在 `copytrade.ts` 中添加 API 调用
- [ ] 新增配置参数时同步更新 DTO 和表单
- [ ] 修改后人工 smoke `/copy-trading` 页面（所有操作、分页、钱包管理）
