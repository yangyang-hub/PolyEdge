# 数据层（API Client + Actions + Contracts）

最后更新：2026-07-07

## 概述

前端数据层是前端与 Rust 后端通信的唯一通道。包含三个子层：`contracts/dto`（类型镜像）、`api/*.ts`（HTTP 客户端）和 `api/actions.ts` + `api/actions/`（Server Actions 写操作）。

## 设计目标

- **单一数据源**：所有 API 调用通过 `src/lib/api/*`，页面不在组件中直接 fetch
- **读写分离**：读操作按领域拆文件（`markets.ts`/`rewards.ts`/`funding.ts` 等），写操作通过 `actions.ts` barrel 暴露并按领域拆到 `actions/`
- **类型安全**：DTO 类型从 `@/lib/contracts/dto` 引用，不在组件内重新定义

## 架构与关键文件

### 类型层 — `src/lib/contracts/`

| 文件 | 职责 |
|---|---|
| `dto.ts` | Barrel re-export，聚合 11 个 DTO 文件 |
| `dto/primitives.ts` | 基础类型（ApiMeta、ApiResponse 等） |
| `dto/market.ts` | MarketDto 及相关类型 |
| `dto/rewards.ts` | RewardBotSnapshotDto、RewardBotConfigDto、RewardListPageDto 等 |
| `dto/copytrade.ts` | CopyTradeSnapshotDto、CopyTradeConfigDto 等 |
| `dto/smart-money.ts` | SmartMoneySnapshotDto、SmartMoneyConfigDto、候选钱包/画像/评分/源交易/信号/decision/advisory 类型 |
| `dto/high-probability.ts` | HighProbabilitySnapshotDto、HighProbabilityConfigDto、HighProbabilityResearchReportDto、HighProbabilityBacktestReportDto、HighProbabilityBacktestExitRuleReportDto、HighProbabilityBacktestRunDto、HighProbabilityBacktestTradeDto、HighProbabilityFairValueDto、bucket stats 和 observation 类型 |
| `dto/news.ts` | 新闻相关 DTO |
| `dto/probability.ts` | 概率相关 DTO |
| `dto/wallet-analysis.ts` | WalletAnalysisReportDto |
| `dto/funding.ts` | FundingStatusDto、带余额的 FundingTokenDto、FundingTransferDto |
| `dto/settings.ts` | RuntimeConfigEntryDto、RuntimeConfigUpdateDto |
| `api.ts` | API 响应信封类型 |

### HTTP 客户端层 — `src/lib/api/base.ts`（~323 行）

**核心导出：**
- `PolyEdgeApiError`：自定义错误类（code、requestId、traceId、retryable）
- `fetchContract<T>(path)` — GET 请求，返回单个类型化负载
- `fetchListContract<TLive, TFront>(path, options?)` — GET 列表请求，支持旧版直接数组响应和后端分页信封 `{ data: { data, page }, meta }`，并支持 `mapItem` 转换
- `fetchWriteContract<TLive, TFront>(path, init, options?)` — POST/PATCH 写操作，含 Idempotency-Key 和 step-up auth
- `buildQueryString(query)` — 构建 URL 查询参数
- `InternalApiStepUpScope` — 9 个提权操作范围（含 `funding_transfer`）

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
| `rewards.ts` | 63 | `readRewardBotSnapshot`、`updateRewardBotConfig`、`runRewardBotOnce`、`cancelRewardBotOrders`、`resetRewardBot` | GET + POST |
| `copytrade.ts` | 61 | `readCopyTradeSnapshot`、`updateCopyTradeConfig`、`addTrackedWallet`、`removeTrackedWallet`、`setWalletStatus`、`analyzeWallets` | GET + POST |
| `smart-money.ts` | 31 | `readSmartMoneySnapshot`、`updateSmartMoneyConfig`、`updateSmartMoneyCandidateStatus` | GET + POST |
| `high-probability.ts` | 60 | `readHighProbabilitySnapshot`、`readHighProbabilityConfig`、`readHighProbabilityBuckets`、`readHighProbabilityReport`、`readHighProbabilityBacktests`、`readHighProbabilityBacktestRuns`、`readHighProbabilityBacktestTrades`、`readHighProbabilityFairValues` | GET |
| `events.ts` | 24 | `listEvents`、`listEvidences` | GET |
| `wallet-analysis.ts` | 13 | `analyzeWallet` | POST |
| `settings.ts` | 20 | `readRuntimeConfig`、`updateRuntimeConfig` | GET + POST |
| `news.ts` | 26 | 新闻数据 | GET |
| `funding.ts` | 21 | `readFundingStatus`、`submitFundingTransfer` | GET + POST |

### Server Actions — `src/lib/api/actions.ts` + `src/lib/api/actions/`

| 文件 | 职责 |
|---|---|
| `actions.ts` | 兼容 barrel，保持外部 `@/lib/api/actions` import 路径不变 |
| `actions/shared.ts` | `OperationActionResult`、成功/失败结果构造、API 错误标准化、operation id 和 decimal 校验 helper |
| `actions/rewards.ts` | Rewards bot 配置、run/cancel/reset actions |
| `actions/copytrade.ts` | 跟单配置、钱包管理和分析 actions |
| `actions/smart-money.ts` | Smart Money 配置保存和候选钱包状态更新 actions |
| `actions/settings.ts` | Runtime config 更新 action |
| `actions/funding.ts` | 后端资金钱包 Polymarket 入金 action |

**核心类型：**
- `OperationActionResult`：通用结果类型（ok、message、requestId、status、fieldErrors）
- `RewardBotActionResult`：扩展 OperationActionResult + snapshot
- `RuntimeConfigActionResult`：扩展 OperationActionResult + entries

**辅助函数：**
- `createActionSuccessResult` / `createActionFailureResult` — 构建标准化结果
- `apiActionFailure` — 错误标准化（PolyEdgeApiError → OperationActionResult）
- `actionOperationId` — 统一生成本地 operation id

**动作函数：** 奖励机器人 CRUD、跟单钱包管理/分析、运行时配置更新、Funding 入金等

## 数据流

```
Page / ClientDataBoundary（静态导出下浏览器执行首屏 loader）
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

- 10 个领域 API 模块覆盖当前前端页面使用的后端端点、Smart Money foundation 端点和 High Probability Pricing 只读研究端点，`base.ts` 和 `actions.ts`/`actions/` 提供共享请求与写操作封装
- `fetchListContract()` 已兼容 events/evidences/news 等后端分页信封，同时保留旧版直接数组列表响应兼容；调用方继续读取统一的 `ApiListResponse<T>`
- `actions.ts` 只做兼容 re-export；具体 Server Actions 已按 rewards、copytrade、settings、funding 拆到 `actions/`，共享结果构造和数字校验在 `actions/shared.ts`
- 旧 `/radar`、`/signals`、`/positions`、`/risk` 页面对应的 API 模块、Server Actions 和 DTO 类型镜像已移除；Rewards 和 wallet-analysis 内部仍可在自身 DTO 中表达持仓/风险字段
- DTO 类型镜像当前前端消费的后端响应；`CopyTradeSnapshotDto` 已与只读跟踪后端对齐，只包含 config、status、wallets、source_trades、events，不再声明模拟账户、订单或持仓字段
- Smart Money DTO/API/action 已镜像 `/api/v1/smart-money` foundation snapshot、配置保存和候选钱包状态更新；`SmartMoneyConfigDto` 包含 signal advisory provider/request format/model 以及 signal advisory 并发开关/最大并发，Server Action 会校验 Anthropic 只能使用 Messages、OpenAI-compatible 不能使用 Anthropic Messages，并把并发数限制在 1–10；snapshot DTO 已包含 recent_trades、recent_signals、recent_decisions 和 recent_signal_advisories，`/copy-trading` 页面已消费 snapshot，并提供 Smart Money 配置保存、候选池查看、候选状态更新、recent_signals 信号流和最近 signal advisory 展示入口，但 deterministic decision 详情 UI、钱包详情、纸面表现和完整 Smart Money 工作台仍未实现。
- High Probability DTO/API 已镜像 `/api/v1/high-probability`、`/config`、`/buckets`、`/report`、`/backtests`、`/backtest-runs`、`/backtest-runs/{run_id}/trades` 和 `/fair-values` 只读研究端点，供 `/high-probability` 页面展示配置、bucket stats、observations、样本覆盖、加权基础研究指标、即时 walk-forward baseline 回测指标、退出规则对比、持久化历史回测 run、最新 run 交易明细和 `reward_market_fair_values` fair value 诊断表；当前没有写操作或交易动作。
- Funding DTO/API/action 已接入 `/api/v1/funding` 与 `/api/v1/funding/transfer`；状态响应会携带后端资金钱包 USDC/USDT Polygon 链上余额和可选余额查询错误。当前内网免鉴权部署下，前端只提交 token、amount 和 confirmed，不提交二次确认码、Polymarket 充值地址或任何私钥材料。
- Rewards snapshot DTO 包含 `orders_page`、`llm_usage`、quote plan 的 `opportunity_metrics` 和历史兼容的 `low_competition_report`；当前后端返回的 `low_competition_report` 为 `null`。`RewardBotConfigDto.execution_mode`、旧 `quote_edge_cents`、模拟填单参数和 stale force-cancel 参数已移除；报价配置改为 `quote_bid_rank: 1|2|3`（TypeScript 以 number 表达，Server Action 用 Zod 限制范围），并新增 liquidity/volume/end-time/spread/data-age 市场质量门槛。DTO 仍保留后端兼容字段 `per_market_usd`、`quote_size_usd`、`low_competition_per_market_usd`，但前端 Server Action 校验会剥离这些字段，不再把它们作为可编辑配置提交。DTO 已镜像 rewards quote/selection mode、dominant 单边阈值、盘口集中度阈值、偏好分类、统一机会评分 `opportunity_*` 配置字段（竞争倍数、100U 日奖、资金占比、退出深度/滑点、坏成交恢复天数、盘口样本/波动/跳变和权重）、AI advisory 配置字段（`ai_provider_concurrency_enabled`、`ai_provider_primary_max_concurrency`、`ai_provider_fallback_max_concurrency`、`ai_strategy_hint_enabled`、`ai_strategy_hint_min_confidence`、TTL 等）、信息风险配置字段（`require_info_risk_before_first_quote`、`first_quote_quarantine_sec`、TTL 等）、drift 换价 guard（`requote_drift_confirm_sec` / `requote_drift_cooldown_sec` / `requote_drift_max_cancels_per_cycle`），以及 quote plan 的 `pre_ai_eligible` / `quote_readiness` / `orderbook_token_ids` / `strategy_bucket` / `quote_mode` / `recommended_quote_mode` / `book_metrics` / `opportunity_metrics` / `ai_advisory` / `info_risk` / `live_skip_until` / `live_skip_reason` / `first_quote_observed_at`。AI strategy hint 不新增 DTO 字段，而是从 `ai_advisory.metrics.strategy_hint` 解析。`low_competition_*` 配置字段、`strategy_bucket=low_competition`、`low_competition_metrics` 和低竞争 report DTO 仅用于历史响应/旧 API payload 兼容；`actions/rewards.ts` 保存配置时强制 `low_competition_mode=off`、独立低竞争市场/订单/全局占比为 0，并关闭/清零旧低竞争 liquidity/volume 过滤字段，前端不再展示或提交独立低竞争配置。status DTO 镜像 `ready_quote_markets`、`waiting_orderbook_markets` 和 `provider_pending_markets`，用于区分真实可立即报价、等待盘口和等待 provider 风控的计划数量；`RewardLlmCallDailyStatsDto` 镜像 UTC 日期、AI advisory 调用数、info-risk 调用数、总调用数和失败数。`RewardAiProvider` 前端 union 只包含 `openai | anthropic`；`actions/rewards.ts` 校验 OpenAI-compatible/Anthropic 请求格式匹配（Anthropic 只允许 Messages，OpenAI-compatible 不允许 Anthropic Messages）、AI provider 主/备并发数 1–10、AI strategy hint 开关/置信度范围、opportunity 数值范围、drift 换价 guard 范围，并校验信息风险 observe/enforce、过滤等级、TTL 和首单观察窗口。GLM/DeepSeek/Agnes 不作为前端 provider，运行时通过 worker 环境中的 OpenAI-compatible base URL 和模型名识别。AI API key、base URL 和模型名不进入 DTO，只从 worker 环境读取。`readRewardBotSnapshot()` 支持计划/订单分页、搜索、状态和排序 query；首屏 loader 显式请求 `plans_eligible=true`，与默认可挂页签一致。当前后端 handler 和 `orders_page` 都描述本地 managed-order 查询，`orders_status=filled` 会包含部分成交订单；账户余额和 positions 由 worker 同步到数据库后返回
- Rewards DTO 新增 `RewardStrategyProfile = "standard" | "balanced_merge"`；`RewardBotConfigDto` 镜像 `balanced_merge_*` 配置字段，`RewardQuotePlanDto` 和 `ManagedRewardOrderDto` 镜像可选 `strategy_profile`。Server Action 会校验合并策略的独立市场/订单上限、edge、市场评分、低成交量/流动性阈值、最大价差、挂单档位和未配对库存上限；前端表格用该字段区分标准 profile 与成交后合并 profile。
- `/replay` 前端派生数据层已移除；当前没有面向控制台的 replay API 页面
- `MarketDto` 新增 `liquidity_usd` 与 `end_at`，镜像后端 `MarketData`
- 当前静态部署使用 `NEXT_PUBLIC_POLYEDGE_API_BASE_URL` 浏览器直连 Rust API，不再通过前端 Nginx 反代 `/api/v1`
- 默认生产排查入口为 `http://192.168.31.5:33002/rewards`，该静态前端应通过 `NEXT_PUBLIC_POLYEDGE_API_BASE_URL=http://100.87.45.72:38001` 访问 API

## 修改检查清单

- [ ] 新增后端端点后，在对应的 `api/*.ts` 中添加调用函数
- [ ] 新增写操作后，在 `actions/` 对应领域文件中添加 Server Action，并从 `actions.ts` 暴露
- [ ] 新增 DTO 后，在 `contracts/dto/` 中添加 TypeScript 类型并在 `dto.ts` 中 re-export
- [ ] 所有 API 调用使用 `base.ts` 的原语，不直接使用 `fetch`
- [ ] 写操作必须使用 `fetchWriteContract`（自带 Idempotency-Key）
- [ ] 修改后运行 `npx tsc --noEmit` 类型检查
