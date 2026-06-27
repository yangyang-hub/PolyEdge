# Copy Trading（钱包跟踪）

最后更新：2026-06-27

## 概述

`/copy-trading` 页面是只读钱包跟踪和 Smart Money foundation 工作台：管理 Polymarket 钱包地址、启停 copytrade worker 扫描、提交钱包分析命令，展示源钱包成交和事件日志，并读取 Smart Money foundation snapshot 展示/保存 Smart Money 配置、自动发现候选、画像/评分摘要、候选状态操作和 deterministic 信号流。当前跟单模拟引擎、订单、持仓和 PnL 面板已移除，页面不会下单或撤单。

## 架构与关键文件

| 文件 | 职责 |
|---|---|
| `src/app/(console)/copy-trading/page.tsx` | 路由页面 |
| `src/features/copytrade/components/copytrade-workbench.tsx` | 主工作台组件：状态指标、Smart Money 配置/候选面板挂载、跟踪控制、钱包管理、源成交与事件列表 |
| `src/features/copytrade/components/smart-money-config-panel.tsx` | Smart Money 配置面板：启用/发现/advisory 开关、模式、signal advisory provider/request format/model 和基础阈值保存 |
| `src/features/copytrade/components/smart-money-candidates-panel.tsx` | Smart Money 候选池面板：候选、profile/score 摘要、watch/tracked/blocked/rejected 状态更新 |
| `src/features/copytrade/components/smart-money-signals-panel.tsx` | Smart Money 信号流面板：展示 recent_signals 的源价格、当前价格、滑点、状态、拒绝原因和最近 signal advisory 建议 |
| `src/features/copytrade/loaders/copytrade-page-data.ts` | 服务端数据装配：并行读取 copytrade snapshot 与 Smart Money snapshot |

## API 依赖

- `src/lib/api/copytrade.ts` — `readCopyTradeSnapshot`、`updateCopyTradeConfig`、`addTrackedWallet`、`removeTrackedWallet`、`setWalletStatus`、`analyzeWallets`
- `src/lib/api/smart-money.ts` — `readSmartMoneySnapshot`、`updateSmartMoneyConfig`、`updateSmartMoneyCandidateStatus`

## 关键交互（copytrade-workbench.tsx）

**状态管理：** `useState` 管理 snapshot、draft（当前只暴露 `enabled`）、feedback、pending、walletAddress、walletLabel。

**分页：** source_trades/events 每页 20 条；Smart Money 候选池每页 12 条；Smart Money 信号流每页 12 条；均使用 `usePagination` hook。

**操作函数：**
- `runAction(action)` — 设置 pending → 执行 async action → `applyResult(result)` 更新状态
- `applyResult(result)` — 将 `CopyTradeActionResult` 应用到 snapshot 和 draft

**Server Actions（从 actions.ts 导入）：**
- `addTrackedWalletAction` — 添加跟踪钱包
- `removeTrackedWalletAction` — 移除跟踪钱包
- `setCopytradeWalletStatusAction` — 设置钱包状态（active/paused）
- `updateCopyTradeConfigAction` — 保存跟踪配置
- `analyzeCopytradeWalletsAction` — 入队钱包分析命令
- `updateSmartMoneyConfigAction` — 保存 Smart Money 配置（enabled、mode、discovery/advisory 开关、signal advisory provider/request format/model 和基础阈值）
- `updateSmartWalletCandidateStatusAction` — 设置 Smart Money 候选状态（candidate/watch/tracked/blocked/rejected）

## 数据流

```
Loader（copytrade-page-data.ts）
    → readCopyTradeSnapshot()
    → readSmartMoneySnapshot()
    → 返回 CopyTradeSnapshotDto（config/status/wallets/source_trades/events）和 SmartMoneySnapshotDto（status/config/candidates/profiles/scores/recent_trades/recent_signals）

Client Component（copytrade-workbench.tsx）
    → 接收 initialSnapshot 与 initialSmartMoneySnapshot prop
    → 用户操作 → Server Action → API 写配置/钱包或入队 analyze 命令
    → 返回当前 snapshot → worker 后续处理后由下一次页面加载体现变化
```

## i18n

使用 `copytrade` 命名空间字典，文案明确当前是“只读跟踪”，不会显示模拟资金、订单、持仓或撤单/重置操作。

## 当前状态

- 钱包管理（添加/移除/暂停/激活）
- 启停跟踪配置保存
- Analyze 钱包分析命令入队
- Smart Money 配置保存：enabled、mode、候选发现/advisory 开关、signal advisory provider/request format/model、样本/成交量/可跟分/滑点/深度/敞口阈值；provider key/base URL/timeout 只在后端 `.env.api` 配置，不进入前端
- Smart Money 候选池展示：候选来源、候选状态、profile/score 摘要、发现/分析时间
- Smart Money 候选状态操作：设置 watch/tracked/blocked/rejected；该前端操作只更新候选池状态，不直接触发信号生成或订单执行
- Smart Money 信号流展示：展示 recent_signals 的钱包、condition/token、方向、源成交价、当前价格、滑点、observe/rejected 状态、拒绝原因、最近 signal advisory 的 allow/observe/reject 建议、置信度、provider/model、摘要和生成时间；当前信号只用于观察和诊断，不代表可执行跟随订单
- 源成交列表和事件列表分页展示
- `CopyTradeSnapshotDto` 与后端只读 snapshot 对齐，不再声明 `account`、`orders`、`positions` 或 `status.open_orders`
- 前端不再暴露 Run / Cancel / Reset，因为 worker 中对应 copytrade 控制命令当前是 no-op 或仅历史兼容
- 跟单不会下单；Smart Money foundation 配置只影响后续 worker 发现、评分和 deterministic signal gate，当前只展示 observe/rejected 信号，不生成可执行跟随订单、纸面订单或实盘订单；未处理 source trades 按时间排序并记录

## 修改检查清单

- [ ] 新增操作时在 `actions.ts` 中添加 Server Action，在 `copytrade.ts` 中添加 API 调用
- [ ] 新增 snapshot 字段时同步更新 `CopyTradeSnapshotDto` 和页面映射
- [ ] 修改 Smart Money 候选操作时同步更新 `SmartMoneySnapshotDto` / `smart-money.ts` / `actions/smart-money.ts`
- [ ] 修改后人工 smoke `/copy-trading` 页面（钱包管理、启停保存、分析命令、Smart Money 候选状态、候选分页、信号表和信号分页）
