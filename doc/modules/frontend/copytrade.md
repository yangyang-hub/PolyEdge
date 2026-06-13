# Copy Trading（钱包跟踪）

最后更新：2026-06-13

## 概述

`/copy-trading` 页面是只读钱包跟踪和分析工作台：管理 Polymarket 钱包地址、启停 worker 扫描、提交钱包分析命令，并展示源钱包成交和事件日志。当前跟单模拟引擎、订单、持仓和 PnL 面板已移除，页面不会下单或撤单。

## 架构与关键文件

| 文件 | 职责 |
|---|---|
| `src/app/(console)/copy-trading/page.tsx` | 路由页面 |
| `src/features/copytrade/components/copytrade-workbench.tsx` | 主工作台组件：状态指标、跟踪控制、钱包管理、源成交与事件列表 |
| `src/features/copytrade/loaders/copytrade-page-data.ts` | 服务端数据装配 |

## API 依赖

- `src/lib/api/copytrade.ts` — `readCopyTradeSnapshot`、`updateCopyTradeConfig`、`addTrackedWallet`、`removeTrackedWallet`、`setWalletStatus`、`analyzeWallets`

## 关键交互（copytrade-workbench.tsx）

**状态管理：** `useState` 管理 snapshot、draft（当前只暴露 `enabled`）、feedback、pending、walletAddress、walletLabel。

**分页：** 两个分页列表（source_trades、events），每页 20 条，使用 `usePagination` hook。

**操作函数：**
- `runAction(action)` — 设置 pending → 执行 async action → `applyResult(result)` 更新状态
- `applyResult(result)` — 将 `CopyTradeActionResult` 应用到 snapshot 和 draft

**Server Actions（从 actions.ts 导入）：**
- `addTrackedWalletAction` — 添加跟踪钱包
- `removeTrackedWalletAction` — 移除跟踪钱包
- `setCopytradeWalletStatusAction` — 设置钱包状态（active/paused）
- `updateCopyTradeConfigAction` — 保存跟踪配置
- `analyzeCopytradeWalletsAction` — 入队钱包分析命令

## 数据流

```
Loader（copytrade-page-data.ts）
    → readCopyTradeSnapshot()
    → 返回 CopyTradeSnapshotDto（config/status/wallets/source_trades/events）

Client Component（copytrade-workbench.tsx）
    → 接收 initialSnapshot prop
    → 用户操作 → Server Action → API 写配置/钱包或入队 analyze 命令
    → 返回当前 snapshot → worker 后续处理后由下一次页面加载体现变化
```

## i18n

使用 `copytrade` 命名空间字典，文案明确当前是“只读跟踪”，不会显示模拟资金、订单、持仓或撤单/重置操作。

## 当前状态

- 钱包管理（添加/移除/暂停/激活）
- 启停跟踪配置保存
- Analyze 钱包分析命令入队
- 源成交列表和事件列表分页展示
- `CopyTradeSnapshotDto` 与后端只读 snapshot 对齐，不再声明 `account`、`orders`、`positions` 或 `status.open_orders`
- 前端不再暴露 Run / Cancel / Reset，因为 worker 中对应 copytrade 控制命令当前是 no-op 或仅历史兼容
- 跟单不会下单；未处理 source trades 按时间排序并记录

## 修改检查清单

- [ ] 新增操作时在 `actions.ts` 中添加 Server Action，在 `copytrade.ts` 中添加 API 调用
- [ ] 新增 snapshot 字段时同步更新 `CopyTradeSnapshotDto` 和页面映射
- [ ] 修改后人工 smoke `/copy-trading` 页面（钱包管理、启停保存、分析命令、分页）
