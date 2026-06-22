# 数据层（API Client + Actions + Contracts）

最后更新：2026-06-21

## 概述

前端数据层是前端与 Rust 后端通信的唯一通道。包含三个子层：`contracts/dto`（类型镜像）、`api/*.ts`（HTTP 客户端）和 `api/actions.ts` + `api/actions/`（Server Actions 写操作）。

## 设计目标

- **单一数据源**：所有 API 调用通过 `src/lib/api/*`，页面不在组件中直接 fetch
- **读写分离**：读操作按领域拆文件（`markets.ts`/`signals.ts` 等），写操作通过 `actions.ts` barrel 暴露并按领域拆到 `actions/`
- **类型安全**：DTO 类型从 `@/lib/contracts/dto` 引用，不在组件内重新定义

## 架构与关键文件

### 类型层 — `src/lib/contracts/`

| 文件 | 职责 |
|---|---|
| `dto.ts` | Barrel re-export，聚合 11 个 DTO 文件 |
| `dto/primitives.ts` | 基础类型（ApiMeta、ApiResponse 等） |
| `dto/market.ts` | MarketDto 及相关类型 |
| `dto/signal.ts` | SignalDto、SignalLifecycleState 等 |
| `dto/risk.ts` | RiskStateDto、RiskAlertDto 等 |
| `dto/arbitrage.ts` | ArbitrageScanDto、ArbitrageOpportunityDto 等 |
| `dto/rewards.ts` | RewardBotSnapshotDto、RewardBotConfigDto、RewardListPageDto 等 |
| `dto/copytrade.ts` | CopyTradeSnapshotDto、CopyTradeConfigDto 等 |
| `dto/position.ts` | PositionDto |
| `dto/news.ts` | 新闻相关 DTO |
| `dto/probability.ts` | 概率相关 DTO |
| `dto/wallet-analysis.ts` | WalletAnalysisReportDto |
| `api.ts` | API 响应信封类型 |

### HTTP 客户端层 — `src/lib/api/base.ts`（~263 行）

**核心导出：**
- `PolyEdgeApiError`：自定义错误类（code、requestId、traceId、retryable）
- `fetchContract<T>(path)` — GET 请求，返回单个类型化负载
- `fetchListContract<TLive, TFront>(path, options?)` — GET 列表请求，支持 `mapItem` 转换
- `fetchWriteContract<TLive, TFront>(path, init, options?)` — POST/PATCH 写操作，含 Idempotency-Key 和 step-up auth
- `buildQueryString(query)` — 构建 URL 查询参数
- `InternalApiStepUpScope` — 8 个提权操作范围

**连接机制：**
- 基础 URL 来自 `NEXT_PUBLIC_POLYEDGE_API_BASE_URL` 环境变量；静态部署且 front/API 分离时必须指向 Rust API（默认生产排查环境为 `http://100.87.45.72:38001`）
- 所有请求使用 `cache: "no-store"`；配置了 API base URL 时 `credentials: "omit"`，未配置时才使用同源
- 当前内网部署由 API 侧 `POLYEDGE_AUTH__DISABLED=true` 关闭权限校验，前端不需要发送 dev-auth header
- 旧 local dev-auth 模式仍可手动设置 `NEXT_PUBLIC_POLYEDGE_INTERNAL_AUTH_DEV_BYPASS=1` 发送 `X-PolyEdge-Dev-Auth` 头；默认前端 env 示例不再包含该遗留变量
- Step-up header 代码路径保留；API 免鉴权模式下不会校验 step-up code

### 领域 API 模块 — `src/lib/api/*.ts`

| 文件 | 行数 | 主要函数 | 方法 |
|---|---|---|---|
| `markets.ts` | 54 | `listMarkets`、`listMarketCategories` | GET |
| `signals.ts` | 66 | `listSignals`、`submitSignalExecutionRequest` | GET + POST |
| `risk.ts` | 77 | `readRiskState`、`listRiskAlerts`、`listRiskBuckets`、`requestModeSwitch`、`releaseRiskControls`、`setKillSwitchState` | GET + POST |
| `arbitrage.ts` | 48 | `listArbitrageScans`、`listArbitrageOpportunities`、`listArbitrageAnalysisRuns` | GET |
| `rewards.ts` | 63 | `readRewardBotSnapshot`、`updateRewardBotConfig`、`runRewardBotOnce`、`cancelRewardBotOrders`、`resetRewardBot` | GET + POST |
| `copytrade.ts` | 61 | `readCopyTradeSnapshot`、`updateCopyTradeConfig`、`addTrackedWallet`、`removeTrackedWallet`、`setWalletStatus`、`analyzeWallets` | GET + POST |
| `positions.ts` | 44 | `listPositions` | GET（含字段映射） |
| `events.ts` | 24 | `listEvents`、`listEvidences` | GET |
| `wallet-analysis.ts` | 13 | `analyzeWallet` | POST |
| `settings.ts` | 20 | `readRuntimeConfig`、`updateRuntimeConfig` | GET + POST |
| `news.ts` | 26 | 新闻数据 | GET |

### Server Actions — `src/lib/api/actions.ts` + `src/lib/api/actions/`

| 文件 | 职责 |
|---|---|
| `actions.ts` | 兼容 barrel，保持外部 `@/lib/api/actions` import 路径不变 |
| `actions/shared.ts` | `OperationActionResult`、成功/失败结果构造、API 错误标准化、operation id 和 decimal 校验 helper |
| `actions/risk.ts` | 风控 release / kill-switch actions |
| `actions/signals.ts` | 信号执行提交 action |
| `actions/rewards.ts` | Rewards bot 配置、run/cancel/reset actions |
| `actions/copytrade.ts` | 跟单配置、钱包管理和分析 actions |
| `actions/settings.ts` | Runtime config 更新 action |

**核心类型：**
- `OperationActionResult`：通用结果类型（ok、message、requestId、status、fieldErrors）
- `RewardBotActionResult`：扩展 OperationActionResult + snapshot
- `RuntimeConfigActionResult`：扩展 OperationActionResult + entries

**辅助函数：**
- `createActionSuccessResult` / `createActionFailureResult` — 构建标准化结果
- `apiActionFailure` — 错误标准化（PolyEdgeApiError → OperationActionResult）
- `actionOperationId` — 统一生成本地 operation id

**动作函数：** 模式切换、风控控制、kill switch、信号执行、奖励机器人 CRUD、跟单钱包管理/分析、运行时配置更新等

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

- 11 个领域 API 模块覆盖当前前端页面使用的后端端点，`base.ts` 和 `actions.ts`/`actions/` 提供共享请求与写操作封装
- `actions.ts` 只做兼容 re-export；具体 Server Actions 已按 risk、signals、rewards、copytrade、settings 拆到 `actions/`，共享结果构造和数字校验在 `actions/shared.ts`
- DTO 类型镜像当前前端消费的后端响应；`CopyTradeSnapshotDto` 已与只读跟踪后端对齐，只包含 config、status、wallets、source_trades、events，不再声明模拟账户、订单或持仓字段
- Rewards snapshot DTO 包含 `orders_page` 和 `low_competition_report`，但 `RewardBotConfigDto.execution_mode`、旧 `quote_edge_cents`、模拟填单参数和 stale force-cancel 参数已移除；报价配置改为 `quote_bid_rank: 1|2|3`（TypeScript 以 number 表达，Server Action 用 Zod 限制范围），并新增 liquidity/volume/end-time/spread/data-age 市场质量门槛。DTO 已镜像 rewards quote/selection mode、dominant 单边阈值、盘口集中度阈值、偏好分类、低竞争 sleeve 配置字段、AI advisory 配置字段（含 `ai_advisory_batch_size`）、信息风险配置字段（含 `info_risk_batch_size`）、drift 换价 guard（`requote_drift_confirm_sec` / `requote_drift_cooldown_sec` / `requote_drift_max_cancels_per_cycle`），以及 quote plan 的 `pre_ai_eligible` / `orderbook_token_ids` / `strategy_bucket` / `quote_mode` / `recommended_quote_mode` / `book_metrics` / `low_competition_metrics` / `ai_advisory` / `info_risk` / `live_skip_until` / `live_skip_reason`；低竞争 report DTO 镜像最近窗口 observation 数、通过/拦截比例、reward 分位数、退出深度倍数、midpoint P95、退出滑点 P95 和小额 enforce 建议；managed order DTO 也携带 `strategy_bucket`。`actions/rewards.ts` 校验低竞争 mode/额度/竞争/退出/稳定性阈值、OpenAI 与 Anthropic 请求格式匹配、AI/info-risk batch size 范围、drift 换价 guard 范围，并校验信息风险 observe/enforce、过滤等级和 TTL。AI API key、base URL 和模型名不进入 DTO，只从 worker 环境读取。`readRewardBotSnapshot()` 支持计划/订单分页、搜索、状态和排序 query；首屏 loader 显式请求 `plans_eligible=true`，与默认可挂页签一致。当前后端 handler 和 `orders_page` 都描述本地 managed-order 查询；账户余额和 positions 由 worker 同步到数据库后返回
- `/replay` 前端派生数据层已移除；当前没有面向控制台的 replay API 页面
- `MarketDto` 新增 `liquidity_usd` 与 `end_at`，镜像后端 `MarketData`
- positions.ts 是唯一使用 `mapItem` 做字段重命名的模块
- 当前静态部署使用 `NEXT_PUBLIC_POLYEDGE_API_BASE_URL` 浏览器直连 Rust API，不再通过前端 Nginx 反代 `/api/v1`
- 默认生产排查入口为 `http://192.168.31.5:33002/rewards`，该静态前端应通过 `NEXT_PUBLIC_POLYEDGE_API_BASE_URL=http://100.87.45.72:38001` 访问 API

## 修改检查清单

- [ ] 新增后端端点后，在对应的 `api/*.ts` 中添加调用函数
- [ ] 新增写操作后，在 `actions/` 对应领域文件中添加 Server Action，并从 `actions.ts` 暴露
- [ ] 新增 DTO 后，在 `contracts/dto/` 中添加 TypeScript 类型并在 `dto.ts` 中 re-export
- [ ] 所有 API 调用使用 `base.ts` 的原语，不直接使用 `fetch`
- [ ] 写操作必须使用 `fetchWriteContract`（自带 Idempotency-Key）
- [ ] 修改后运行 `npx tsc --noEmit` 类型检查
