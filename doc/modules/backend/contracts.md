# contracts（HTTP API DTO 层）

最后更新：2026-07-12

## 概述

`polyedge_contracts` crate 定义后端 HTTP API 的请求/响应 DTO。API handler 和前端 TypeScript DTO 镜像都以这里的结构为契约来源，避免路由响应和客户端类型漂移。

## 设计目标

- API handler 不内联定义公开请求/响应结构。
- DTO 按领域拆分到 `dto/` 子文件，并通过 `include!()` 暴露在 crate 根命名空间。
- 复用 `domain` crate 的核心枚举和数值类型，保持跨层一致。

## 架构与关键文件

| 文件 | 职责 |
|---|---|
| `lib.rs` | DTO crate 入口，内联当前 12 个 DTO 文件 |
| `dto/common.rs` | `ApiMeta`、`ApiResponse<T>`、`ApiListResponse<T>` 等共享信封 |
| `dto/system.rs` | 健康、就绪、系统模式和 runtime config |
| `dto/market.rs` | 市场列表、详情、分类和 bucket 状态 |
| `dto/news.rs` | 新闻源健康和 raw news event 查询 |
| `dto/risk.rs` | connector callback 兼容的 `RiskStateData` |
| `dto/execution.rs` | 执行请求、订单、草稿、持仓、成交和回调相关 DTO |
| `dto/pricing.rs` | 概率估计查询和响应 |
| `dto/callback.rs` | Connector callback 请求/响应 |
| `dto/query.rs` | 分页、排序等共享查询参数 |
| `dto/orderbook.rs` | Orderbook HTTP 代理 DTO |
| `dto/funding.rs` | Funding 状态、资产余额、转账请求和回执 |
| `dto/rewards.rs` | Rewards config/control 写请求，包含平铺 patch 和可选 operator note |

## 核心设计模式

- DTO 默认 derive `Debug, Clone, Serialize, Deserialize`。
- 查询类型使用 `#[serde(skip_serializing_if = "Option::is_none")]`。
- 列表查询支持分页参数。
- 响应使用 `ApiResponse<T>` 或 `ApiListResponse<T>` 信封。
- `RewardBotSnapshotQuery` 支持计划/订单分页、搜索、状态过滤和排序参数。
- `UpdateRewardBotConfigRequest` 保持现有 config patch 的平铺 JSON 兼容性，并把 `operator_note` 从策略配置字段中隔离；`RewardBotControlRequest` 是 run/cancel/reset 的严格请求体。
- `RewardStrategyRunsQuery`、`RewardStrategyDecisionsQuery`、`RewardStrategyActionsQuery`、`RewardOrderTransitionsQuery` 支持 rewards strategy ledger 查询分页和过滤。
- `MarketData` 包含 Gamma 同步的 `liquidity_usd` 与 `end_at`。

## 依赖关系

- 上游：`domain`（枚举和值对象）。
- 下游：`packages/backend/api` handler、前端 `src/lib/contracts/dto/` TypeScript 镜像。

## 当前状态

- 当前 DTO 覆盖 markets、events/evidences、news、orders、trades、pricing、rewards snapshot/control 查询参数与写请求、rewards strategy ledger 查询参数、runtime config、system mode、connector callback、orderbook 和 funding。
- 已删除旧钱包类与独立研究 DTO；`lib.rs` 不再 include 对应文件。
- `RiskStateData` 和 execution `PositionData` 仅保留给 connector callback 与内部执行链路兼容，不代表旧控制台风控页面仍存在。
- Funding DTO 覆盖后端资金钱包状态、USDC/USDT 链上余额和转账回执；请求只包含 `token_id`、`amount`、`confirmed`，不包含充值地址或私钥字段。
- Rewards snapshot/config/order/plan 响应体主要直接使用 application 层模型序列化；当前公开配置使用单一 `maker_market_budget_usd`，advisory 使用 V2 action/size/edge 字段，blocker counts 区分 provider、maker budget 与 inventory headroom。前端 DTO 必须同步镜像。

## 修改检查清单

- [ ] 新增/修改 API 端点时，先在此 crate 定义或更新 DTO。
- [ ] 新增 DTO 文件后在 `lib.rs` 中添加 `include!()`。
- [ ] 修改 DTO 后同步更新前端 TypeScript DTO 镜像。
- [ ] 运行 `cargo check --workspace --tests`。
