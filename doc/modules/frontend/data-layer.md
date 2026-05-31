# 数据层（API Client + Actions + Contracts）

最后更新：2026-05-31

## 概述

前端数据层是前端与 Rust 后端通信的唯一通道。包含三个子层：`contracts/dto`（类型镜像）、`api/*.ts`（HTTP 客户端）和 `api/actions.ts`（Server Actions 写操作）。

## 设计目标

- **单一数据源**：所有 API 调用通过 `src/lib/api/*`，页面不在组件中直接 fetch
- **读写分离**：读操作按领域拆文件（`markets.ts`/`signals.ts` 等），写操作集中在 `actions.ts`
- **类型安全**：DTO 类型从 `@/lib/contracts/dto` 引用，不在组件内重新定义

## 架构与关键文件

### 类型层 — `src/lib/contracts/`

| 文件 | 职责 |
|---|---|
| `dto.ts` | Barrel re-export，聚合 12 个领域 DTO 文件 |
| `dto/primitives.ts` | 基础类型（ApiMeta、ApiResponse 等） |
| `dto/market.ts` | MarketDto 及相关类型 |
| `dto/signal.ts` | SignalDto、SignalLifecycleState 等 |
| `dto/risk.ts` | RiskStateDto、RiskAlertDto 等 |
| `dto/arbitrage.ts` | ArbitrageScanDto、ArbitrageOpportunityDto 等 |
| `dto/rewards.ts` | RewardBotSnapshotDto、RewardBotConfigDto 等 |
| `dto/copytrade.ts` | CopyTradeSnapshotDto、CopyTradeConfigDto 等 |
| `dto/position.ts` | PositionDto |
| `dto/news.ts` | 新闻相关 DTO |
| `dto/probability.ts` | 概率相关 DTO |
| `dto/replay.ts` | 回放相关 DTO |
| `dto/wallet-analysis.ts` | WalletAnalysisReportDto |
| `api.ts` | API 响应信封类型 |
| `realtime.ts` | 实时数据类型 |

### HTTP 客户端层 — `src/lib/api/base.ts`（~263 行）

**核心导出：**
- `PolyEdgeApiError`：自定义错误类（code、requestId、traceId、retryable）
- `fetchContract<T>(path)` — GET 请求，返回单个类型化负载
- `fetchListContract<TLive, TFront>(path, options?)` — GET 列表请求，支持 `mapItem` 转换
- `fetchWriteContract<TLive, TFront>(path, init, options?)` — POST/PATCH 写操作，含 Idempotency-Key 和 step-up auth
- `buildQueryString(query)` — 构建 URL 查询参数
- `InternalApiStepUpScope` — 8 个提权操作范围

**连接机制：**
- 基础 URL 来自 `NEXT_PUBLIC_POLYEDGE_API_BASE_URL` 环境变量
- 所有请求使用 `cache: "no-store"` 和 `credentials: "same-origin"`
- Dev 模式下通过 `X-PolyEdge-Dev-Auth` 头绕过认证
- Step-up 认证通过 `X-PolyEdge-Step-Up-Code` + `X-PolyEdge-Step-Up-Scopes` 头实现

### 领域 API 模块 — `src/lib/api/*.ts`

| 文件 | 行数 | 主要函数 | 方法 |
|---|---|---|---|
| `markets.ts` | 55 | `listMarkets`、`listMarketCategories` | GET |
| `signals.ts` | 67 | `listSignals`、`submitSignalExecutionRequest` | GET + POST |
| `risk.ts` | 110 | `readRiskState`、`listRiskAlerts`、`listRiskBuckets`、`requestModeSwitch`、`releaseRiskControls`、`setKillSwitchState` | GET + POST |
| `arbitrage.ts` | 49 | `listArbitrageScans`、`listArbitrageOpportunities`、`listArbitrageAnalysisRuns` | GET |
| `rewards.ts` | 45 | `readRewardBotSnapshot`、`updateRewardBotConfig`、`runRewardBotOnce`、`cancelRewardBotOrders`、`resetRewardBot` | GET + POST |
| `copytrade.ts` | 86 | `readCopyTradeSnapshot`、`updateCopyTradeConfig`、`addTrackedWallet`、`removeTrackedWallet`、`setWalletStatus`、`runCopyTradeOnce`、`analyzeWallets`、`cancelCopyTradeOrders`、`resetCopyTrade` | GET + POST |
| `positions.ts` | 45 | `listPositions` | GET（含字段映射） |
| `events.ts` | 25 | `listEvents`、`listEvidences` | GET |
| `wallet-analysis.ts` | 13 | `analyzeWallet` | POST |
| `settings.ts` | 21 | `readRuntimeConfig`、`updateRuntimeConfig` | GET + POST |
| `news.ts` | — | 新闻数据 | GET |
| `research.ts` | — | 研究数据 | GET |

### Server Actions — `src/lib/api/actions.ts`

**核心类型：**
- `OperationActionResult`：通用结果类型（ok、message、requestId、status、fieldErrors）
- `RewardBotActionResult`：扩展 OperationActionResult + snapshot
- `RuntimeConfigActionResult`：扩展 OperationActionResult + entries

**辅助函数：**
- `createActionSuccessResult` / `createActionFailureResult` — 构建标准化结果
- `apiActionFailure` — 错误标准化（PolyEdgeApiError → OperationActionResult）

**动作函数（15+）：** 模式切换、风控控制、kill switch、信号执行、奖励机器人 CRUD、跟单 CRUD、运行时配置更新等

## 数据流

```
Server Component（页面）
    ↓ props
Loader（features/*/loaders/*-page-data.ts）
    ↓ 调用
API Module（src/lib/api/*.ts）
    ↓ 使用
base.ts（fetchContract / fetchListContract / fetchWriteContract）
    ↓
Rust Backend（/api/v1/...）


Client Component（交互）
    ↓ 调用
Server Action（src/lib/api/actions.ts）
    ↓ 调用
API Module（src/lib/api/*.ts）
    ↓
Rust Backend
    ↓ 返回
OperationActionResult → 更新 UI 状态
```

## 当前状态

- 14 个 API 模块文件覆盖所有后端端点
- `actions.ts` 集中管理所有写操作的 Server Actions
- DTO 类型镜像完整覆盖后端 contracts crate
- positions.ts 是唯一使用 `mapItem` 做字段重命名的模块

## 修改检查清单

- [ ] 新增后端端点后，在对应的 `api/*.ts` 中添加调用函数
- [ ] 新增写操作后，在 `actions.ts` 中添加 Server Action
- [ ] 新增 DTO 后，在 `contracts/dto/` 中添加 TypeScript 类型并在 `dto.ts` 中 re-export
- [ ] 所有 API 调用使用 `base.ts` 的原语，不直接使用 `fetch`
- [ ] 写操作必须使用 `fetchWriteContract`（自带 Idempotency-Key）
- [ ] 修改后运行 `npx tsc --noEmit` 类型检查
