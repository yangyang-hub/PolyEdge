# contracts（HTTP API DTO 层）

最后更新：2026-06-29

## 概述

`polyedge_contracts` crate 定义所有 HTTP API 的请求/响应 DTO（数据传输对象）。API handler 和前端客户端共享同一套类型定义，消除契约漂移。

## 设计目标

- 单一契约源：API handler 不内联定义请求/响应结构
- DTO 通过 `include!()` 按领域拆分到子文件，统一在 crate 根命名空间
- 依赖 `domain` crate 的核心类型和 `rust_decimal`，保持类型一致性

## 架构与关键文件

| 文件 | 职责 |
|---|---|
| `lib.rs` | 模块入口（~30 行），通过 `include!()` 内联 12 个 DTO 文件 |
| `dto/common.rs` | `ApiMeta`、`ApiResponse<T>`、共享信封类型 |
| `dto/system.rs` | 系统级：`HealthData`、`ReadinessData`、`SystemModeData`、`RuntimeConfigEntryData` 等 |
| `dto/market.rs` | 市场：`MarketData`、`MarketListQuery`、`MarketListResponse`、`MarketCategoryData`、`BucketStatus` |
| `dto/news.rs` | 新闻：`NewsRawEventData`、`NewsSourceHealthData` 及查询类型 |
| `dto/pricing.rs` | Pricing：`ProbabilityEstimateData` |
| `dto/risk.rs` | Connector callback 兼容：`RiskStateData` |
| `dto/execution.rs` | 执行：`SubmitExecutionRequest`、`ExecutionRequestData`、`OrderData`、`OrderDraftData`、`PositionData`、`TradeData`、回调类型 |
| `dto/callback.rs` | 连接器回调请求/响应类型 |
| `dto/query.rs` | 共享查询参数类型 |
| `dto/orderbook.rs` | Orderbook HTTP 代理 DTO |
| `dto/wallet_analysis.rs` | 钱包分析：`WalletAnalysisData`、`WalletProfileData`、`WalletPnlData`、`WalletStyleData`、`WalletRiskData` 等 |
| `dto/funding.rs` | Polymarket 入金：`FundingStatusData`、带余额的 `FundingTokenData`、`FundingTransferRequest`、`FundingTransferData` |

## 核心设计模式

- 所有 DTO derive `Debug, Clone, Serialize, Deserialize`
- 查询类型使用 `#[serde(skip_serializing_if = "Option::is_none")]`
- 列表查询支持分页（limit/offset/cursor）
- `RewardBotSnapshotQuery` 支持订单分页参数：`orders_page`、`orders_page_size`，并保留订单搜索/状态/排序参数
- `MarketData` 包含 `liquidity_usd` 和 `end_at`，与 application `MarketView` 及前端 `MarketDto` 保持一致
- 响应使用 `ApiResponse<T>` 信封：`{ data: T, meta: ApiMeta }`
- 列表响应使用 `ApiListResponse<T>`：`{ data: Vec<T>, meta: ApiMeta, total_count: i64 }`

## 依赖关系

- **上游**：`domain`（核心枚举和数值类型）
- **下游**：`packages/backend/api`（handler 中使用 DTO 作为请求/响应类型）、前端 `src/lib/contracts/dto/`（TypeScript 类型镜像）

## 当前状态

- 12 个 DTO 文件全部实现
- 覆盖 markets、events/evidences、news、orders、trades、pricing、rewards 查询参数、runtime config、system mode、connector callback、orderbook、wallet-analysis 和 funding。旧 signals/risk/arbitrage 控制台 DTO 已移除；`RiskStateData` 和 execution `PositionData` 仅保留给 connector callback 与内部执行链路响应兼容
- Funding DTO 覆盖后端资金钱包入金状态、支持资产、USDC/USDT 链上余额和转账回执；请求只包含 `token_id`、`amount`、`confirmed`，不包含充值地址或私钥字段。
- Rewards snapshot 查询契约包含订单后端分页字段，响应分页元数据由 application 的 `RewardBotSnapshot.orders_page` 序列化输出
- Rewards snapshot 的 `orders` 与 `orders_page` 都描述本地 managed-order 查询；外部账户全量开放订单不属于当前响应契约
- contracts crate 目前只定义 Rewards snapshot 查询参数（计划/订单分页、搜索、状态和排序）；Rewards snapshot/config/order/plan 响应体直接使用 application 层模型序列化。前端 `src/lib/contracts/dto/rewards.ts` 需要跟随 application 模型镜像，如 `strategy_profile` 和 `balanced_merge_*` 配置字段；本 crate 未新增对应 DTO 文件

## 修改检查清单

- [ ] 新增/修改 API 端点时，必须先在此 crate 中定义 DTO
- [ ] 新增 DTO 文件后在 `lib.rs` 中添加 `include!()`
- [ ] 新增枚举 DTO 时确保与 `domain` crate 的对应枚举一致
- [ ] 修改 DTO 后同步更新前端 `src/lib/contracts/dto/` 中的 TypeScript 类型
- [ ] 运行 `cargo check --workspace --tests`
