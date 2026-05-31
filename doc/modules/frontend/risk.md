# Risk（风控中心）

最后更新：2026-05-31

## 概述

`/risk` 页面展示风控状态、告警列表、风险桶，支持 kill switch 触发/释放和系统模式切换。

## 架构与关键文件

| 文件 | 职责 |
|---|---|
| `src/app/(console)/risk/page.tsx` | 路由页面 |
| `src/features/risk/components/risk-control-center.tsx` | 主组件 |
| `src/features/risk/components/risk-controls-sidebar.tsx` | 风控操作侧边栏 |
| `src/features/risk/components/risk-metrics-overview.tsx` | 风控指标概览 |
| `src/features/risk/components/risk-action-dialogs.tsx` | 操作对话框（kill switch / release） |
| `src/features/risk/components/risk-audit-log.tsx` | 审计日志 |
| `src/features/risk/loaders/risk-page-data.ts` | 服务端数据装配 |
| `src/features/risk/lib/risk-stream.ts` | SSE 流式更新 |
| `src/features/risk/types.ts` | 类型定义（~6 行） |

## 核心类型（types.ts）

- **`RiskPageData`**：从 loader 推导的类型 `Awaited<ReturnType<typeof getRiskPageData>>`
- **`RiskDialog`**：`"release" | "kill_switch" | null`
- **`RiskAlertFilter`**：`"all" | "unresolved" | "watching"`

## API 依赖

- `src/lib/api/risk.ts` — `readRiskState`、`listRiskAlerts`、`listRiskBuckets`、`requestModeSwitch`、`releaseRiskControls`、`setKillSwitchState`

## 关键交互

- **Kill Switch 触发** → `setKillSwitchState(true)` → 需要 step-up 认证（`system_kill_switch_trigger`）
- **Kill Switch 释放** → `releaseRiskControls()` → 需要 step-up 认证（`system_kill_switch_release`）
- **模式切换** → `requestModeSwitch()` → 需要 step-up 认证（`system_mode_switch`）
- **告警过滤** → 按 `RiskAlertFilter` 过滤列表

## 权限

`/risk` 路由要求 `risk_admin` 角色（前端当前 auth mode 为 `off`，无实际限制）。

## i18n

使用 `risk` 命名空间字典。

## 当前状态

- 完整的风控仪表盘
- Kill switch 触发/释放（需 step-up 认证）
- 模式切换
- SSE 流式更新
- 审计日志

## 修改检查清单

- [ ] 新增风控指标时更新 loader 和 `risk-metrics-overview.tsx`
- [ ] 修改 step-up scope 时同步更新 `base.ts` 中的 `InternalApiStepUpScope`
- [ ] 修改后人工 smoke `/risk` 页面（kill switch 对话框、模式切换、告警过滤）
