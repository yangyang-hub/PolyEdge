# Signals（信号）

最后更新：2026-06-13

## 概述

`/signals` 页面展示交易信号，支持按置信度筛选、查看详情和提交执行请求。这是系统的信号检查界面；当前不提供前端审批/拒绝操作。

## 架构与关键文件

| 文件 | 职责 |
|---|---|
| `src/app/(console)/signals/page.tsx` | 路由页面 |
| `src/features/signals/components/signals-workbench.tsx` | 主交互组件 |
| `src/features/signals/components/signals-table.tsx` | 信号列表表格 |
| `src/features/signals/components/signals-detail-panel.tsx` | 信号详情面板 |
| `src/features/signals/loaders/signals-page-data.ts` | 服务端数据装配 |
| `src/features/signals/lib/signals-helpers.ts` | 信号格式化/推导辅助函数 |
| `src/features/signals/types.ts` | 视图模型类型（~62 行） |

## 核心类型（types.ts）

- **`SignalItem`**：信号视图模型（35+ 字段）— id、lifecycleState、marketQuestion、confidenceValue、side、fairPrice、marketPrice、edge、stateLabel、stateTone、evidenceLines 等
- **`SelectedSignal`**：详情面板用的精简视图
- **`RuntimeControls`**：`{ mode, modeLabel, killSwitch }`
- **`SignalsWorkbenchProps`**：页面 props（activeCount、runtimeControls、signals[]、selectedSignal）
- **`SignalFilter`**：`"all" | "high_confidence"`
- **`SignalActionDialog`**：`"execution" | null`

## API 依赖

- `src/lib/api/signals.ts` — `listSignals(query)`
- `src/lib/api/risk.ts` — `readRiskState()`（获取 runtime controls）
- `src/lib/api/actions.ts` — 信号执行提交 Server Action

## 关键交互

- 选择信号 → 展示详情面板
- 提交执行请求 → 调用 `submitSignalExecutionRequest` Server Action → 展示结果反馈
- 按 `SignalFilter` 过滤信号列表

## i18n

使用 `signals` 和 `enums` 命名空间字典。

## 当前状态

- 已实现信号生命周期展示和执行提交交互
- 页面通过 REST API 初始加载，不再使用“实时信号/实时流”文案
- 前端不暴露审批/拒绝按钮；相关流程如需恢复，应先确认后端权限、状态机和 API 契约
- 2026-06-13：清理组件未使用代码并收窄表格分页重置 effect 依赖；当前执行提交按钮仍保持禁用

## 修改检查清单

- [ ] 新增信号字段时同步更新 `SignalItem` 类型和 loader 映射
- [ ] 新增交互时在 `signals-workbench.tsx` 中实现
- [ ] 修改后人工 smoke `/signals` 页面（信号选择、执行对话框、反馈）
