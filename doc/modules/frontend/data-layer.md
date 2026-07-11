# 数据层（API Client + Actions + Contracts）

最后更新：2026-07-11

## 概述

前端数据层是 Next.js 控制台与 Rust API 通信的唯一通道，包含三层：`contracts/dto` TypeScript DTO 镜像、`api/*.ts` 读写客户端、`api/actions.ts` + `api/actions/` Server Actions。页面和组件不直接 `fetch` 后端。

当前数据层只覆盖控制台仍存在的页面：Dashboard、Markets、Events、Rewards、Funding、Settings。

## 设计目标

- 所有后端请求通过 `src/lib/api/*` 和 `src/lib/api/base.ts`。
- 读操作按领域拆文件；写操作通过 Server Actions 统一处理错误、幂等键和表单校验。
- DTO 从 `@/lib/contracts/dto` 引用，不在组件中重复定义响应结构。
- 静态部署时浏览器通过 `NEXT_PUBLIC_POLYEDGE_API_BASE_URL` 直连 Rust API。

## 类型层 — `src/lib/contracts/`

| 文件 | 职责 |
|---|---|
| `dto.ts` | Barrel re-export，聚合当前 DTO 文件 |
| `dto/primitives.ts` | `ApiMeta`、`ApiResponse`、分页、Rewards strategy run/action 枚举和基础类型 |
| `dto/market.ts` | `MarketDto`、市场列表、分类和事件/证据 DTO |
| `dto/news.ts` | 新闻源健康、raw news event DTO |
| `dto/probability.ts` | 概率估计 DTO |
| `dto/rewards.ts` | Rewards snapshot/config/order/plan/position/fill/event/LLM usage/strategy ledger DTO |
| `dto/funding.ts` | Funding 状态、资产余额、转账请求和回执 DTO |
| `dto/settings.ts` | Runtime config DTO |
| `api.ts` | API 响应信封类型 |

旧前端路由对应的 DTO 文件已删除，`dto.ts` 不再 re-export。

## HTTP 客户端层

`src/lib/api/base.ts` 提供共享请求原语：

- `PolyEdgeApiError`：封装 code、requestId、traceId、retryable。
- `fetchContract<T>()`：GET 单个类型化 payload。
- `fetchListContract<TLive, TFront>()`：GET 列表，兼容后端分页信封并支持 item map。
- `fetchWriteContract<TLive, TFront>()`：POST/PATCH 写操作，自动带 Idempotency-Key 和 step-up header。
- `buildQueryString()`：构建 query string。
- `InternalApiStepUpScope`：包含 Funding 转账等敏感操作 scope。

基础 URL 来自 `NEXT_PUBLIC_POLYEDGE_API_BASE_URL`。设置该变量时请求使用 `credentials: "omit"`；未设置时才走同源。当前内网部署通常由 API 侧 `POLYEDGE_AUTH__DISABLED=true` 关闭权限校验，前端默认不发送 dev-auth header。

## 领域 API 模块

| 文件 | 主要函数 | 方法 |
|---|---|---|
| `markets.ts` | `listMarkets`、`listMarketCategories` | GET |
| `events.ts` | `listEvents`、`listEvidences` | GET |
| `news.ts` | 新闻源健康和 raw news 查询 | GET |
| `rewards.ts` | `readRewardBotSnapshot`、配置/控制 mutation、strategy run ledger 单页与全量分页查询函数 | GET + POST |
| `funding.ts` | `readFundingStatus`、`submitFundingTransfer` | GET + POST |
| `settings.ts` | `readRuntimeConfig`、`updateRuntimeConfig` | GET + POST |

## Server Actions

| 文件 | 职责 |
|---|---|
| `actions.ts` | 兼容 barrel，保持 `@/lib/api/actions` import 路径 |
| `actions/shared.ts` | `OperationActionResult`、成功/失败结果构造、API 错误标准化、operation id、decimal 校验 |
| `actions/rewards.ts` | Rewards 配置保存和 run/cancel/reset actions |
| `actions/funding.ts` | Funding 转账 action |
| `actions/settings.ts` | Runtime config 更新 action |

Server Actions 只用于写操作。读数据由 Server Component/loader 或 client boundary 调用领域 API 模块。

## 数据流

```text
Page / ClientDataBoundary
    -> feature loader
    -> src/lib/api/*.ts
    -> base.ts
    -> Rust API

Client interaction
    -> Server Action
    -> src/lib/api/*.ts
    -> Rust API
    -> OperationActionResult
```

## Rewards DTO 状态

- `readRewardBotSnapshot()` 支持计划/订单分页、搜索、状态和排序 query；首屏 loader 显式请求 `plans_eligible=true`。
- Snapshot DTO 包含 blocker 的 provider/market-budget/inventory-headroom 分类、`reward_adjusted_edge_cents`、V2 provider action、orders page、LLM usage、token quotes、run ledger 与 BalancedMerge 字段。
- `RewardBotConfigDto` 镜像 `maker_market_budget_usd`、首选/最深 rank、库存偏斜、AI/info-risk 动作阈值、非对称 requote、受控退出损失 floor 和 adaptive/BalancedMerge 字段；旧 `per_market_usd`、`quote_size_usd`、`cancel_on_fill` 与 strategy-hint 字段已删除。
- `ManagedRewardOrderDto` 暴露退出策略来源、当前具体退出策略、风险 floor、adaptive 重选次数和最近重选时间，用于订单表展示持仓期退出重评状态。
- `RewardStrategyRunDto`、`RewardStrategyDecisionDto`、`RewardStrategyActionDto` 和 `RewardOrderTransitionDto` 镜像后端 strategy ledger；`src/lib/api/rewards.ts` 提供 runs、decisions、actions 和 order transitions 的只读 GET，并为 Decision Analytics 以 500/page 并行补齐 decisions/actions 剩余分页。
- API key、provider base URL、模型名和请求超时不进入前端 DTO，只从 worker 环境变量读取。
- 竞争相关展示只来自 `opportunity_metrics`；前端 DTO 不再包含独立竞争配置、报告或指标对象。
- Rewards 已移除依赖已删除研究表的诊断字段；quote plan 不再携带对应 EV 诊断对象。

## 当前状态

- 6 个领域 API 模块覆盖当前前端页面和 Rust API 端点。
- `fetchListContract()` 兼容 events/evidences/news 等分页信封，调用方读取统一 `ApiListResponse<T>`。
- Funding DTO/API/action 已接入 `/api/v1/funding` 与 `/api/v1/funding/transfer`；前端只提交 token、amount、confirmed，不提交充值地址或私钥。
- Rewards strategy ledger DTO/API 已接入 `/api/v1/rewards-bot/runs*` 和 `/api/v1/rewards-bot/orders/{managed_order_id}/transitions`，仅用于只读审计视图。
- `MarketDto` 镜像后端 `MarketData` 中的 `liquidity_usd` 与 `end_at`。
- 静态部署通过 `NEXT_PUBLIC_POLYEDGE_API_BASE_URL` 直连 Rust API，不依赖前端 Nginx 反代 `/api/v1`。

## 修改检查清单

- [ ] 新增后端端点后，在对应 `api/*.ts` 添加调用函数。
- [ ] 新增写操作后，在 `actions/` 对应领域文件添加 Server Action，并从 `actions.ts` 暴露。
- [ ] 新增 DTO 后，在 `contracts/dto/` 添加 TypeScript 类型并在 `dto.ts` re-export。
- [ ] 所有 API 调用使用 `base.ts` 原语，不直接使用 `fetch`。
- [ ] 写操作必须使用 `fetchWriteContract`。
- [ ] 修改后运行 `yarn build` 或 TypeScript 检查。
