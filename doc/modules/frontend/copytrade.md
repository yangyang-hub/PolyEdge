# Copy Trading（跟单）

最后更新：2026-06-04

## 概述

`/copy-trading` 页面管理跟单机器人：跟踪钱包、配置策略参数、向 worker 提交运行/分析/取消/重置命令、查看成交/订单/事件。

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
- `runCopyTradeOnceAction` — 入队一次跟单循环命令
- `analyzeCopytradeWalletsAction` — 入队钱包分析命令
- `cancelCopyTradeOrdersAction` — 入队取消所有订单命令
- `resetCopyTradeAction` — 入队重置模拟命令

## 数据流

```
Loader（copytrade-page-data.ts）
    → readCopyTradeSnapshot()
    → 返回 CopyTradeSnapshotDto

Client Component（copytrade-workbench.tsx）
    → 接收 initialSnapshot prop
    → 用户操作 → Server Action → API 入队控制命令 → 返回当前 snapshot → worker 处理后后续 snapshot 体现变化
```

## i18n

使用 `copytrade` 命名空间字典。

## 当前状态

- 完整的 Run / Analyze / Cancel / Reset 入队交互
- 钱包管理（添加/移除/暂停/激活）
- 配置编辑
- 三个分页列表（成交/订单/事件）
- `mode=live` 已结构化支持但未接入真实下单
- 跟单循环、钱包分析、撤单和重置由 worker 执行，API 不直接执行任务
- 模拟跟单按 source trade 时间顺序处理，暂停钱包和 wallet+token cooldown 会跳过遗留交易；同一 tick 内 per-wallet/per-market/total exposure 会累计并硬裁剪新买单
- `MirrorPortfolioWeight` 使用源钱包完整持仓组合权重；无本地持仓的 sell 不会生成模拟收益，crossed order 会完整成交并释放 reserve

## 修改检查清单

- [ ] 新增操作时在 `actions.ts` 中添加 Server Action，在 `copytrade.ts` 中添加 API 调用
- [ ] 新增配置参数时同步更新 DTO 和表单
- [ ] 修改后人工 smoke `/copy-trading` 页面（所有操作、分页、钱包管理）
