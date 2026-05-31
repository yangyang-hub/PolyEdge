# contracts（HTTP API DTO 层）

最后更新：2026-05-31

## 概述

`polyedge_contracts` crate 定义所有 HTTP API 的请求/响应 DTO（数据传输对象）。API handler 和前端客户端共享同一套类型定义，消除契约漂移。

## 设计目标

- 单一契约源：API handler 不内联定义请求/响应结构
- DTO 通过 `include!()` 按领域拆分到子文件，统一在 crate 根命名空间
- 依赖 `domain` crate 的核心类型，保持类型一致性

## 架构与关键文件

| 文件 | 职责 |
|---|---|
| `lib.rs` | 模块入口（~30 行），通过 `include!()` 内联 11 个 DTO 文件 |
| `dto/common.rs` | `ApiMeta`、`ApiResponse<T>`、共享信封类型 |
| `dto/system.rs` | 系统级：`HealthData`、`ReadinessData`、`SystemModeData`、`RuntimeConfigEntryData`、`KillSwitchData` 等 |
| `dto/market.rs` | 市场：`MarketData`、`MarketListQuery`、`MarketListResponse`、`MarketCategoryData`、`BucketStatus` |
| `dto/news.rs` | 新闻：`NewsRawEventData`、`NewsSourceHealthData` 及查询类型 |
| `dto/signal.rs` | 信号：`SignalData`、`SignalListQuery`、`RecomputeSignalData`、`ProbabilityEstimateData` 等 |
| `dto/risk.rs` | 风控：`RiskAlertData`、`RiskBucketData`、`RiskStateData`、`AlertSeverity`、`AlertStatus` |
| `dto/execution.rs` | 执行：`SubmitExecutionRequest`、`ExecutionRequestData`、`OrderData`、`OrderDraftData`、`PositionData`、`TradeData`、回调类型 |
| `dto/arbitrage.rs` | 套利：`ArbitrageScanData`、`ArbitrageOpportunityData`、`ArbitrageOpportunityValidationData`、`ArbitrageAnalysisRunData` |
| `dto/callback.rs` | 连接器回调请求/响应类型 |
| `dto/query.rs` | 共享查询参数类型 |
| `dto/wallet_analysis.rs` | 钱包分析：`WalletAnalysisData`、`WalletProfileData`、`WalletPnlData`、`WalletStyleData`、`WalletRiskData` 等 |

## 核心设计模式

- 所有 DTO derive `Debug, Clone, Serialize, Deserialize`
- 查询类型使用 `#[serde(skip_serializing_if = "Option::is_none")]`
- 列表查询支持分页（limit/offset/cursor）
- 响应使用 `ApiResponse<T>` 信封：`{ data: T, meta: ApiMeta }`
- 列表响应使用 `ApiListResponse<T>`：`{ data: Vec<T>, meta: ApiMeta, total_count: i64 }`

## 依赖关系

- **上游**：`domain`（核心枚举和数值类型）
- **下游**：`apps/api`（handler 中使用 DTO 作为请求/响应类型）、前端 `src/lib/contracts/dto/`（TypeScript 类型镜像）

## 当前状态

- 11 个领域 DTO 模块全部实现
- 覆盖 markets、events、signals、orders、trades、positions、risk、arbitrage、rewards、copytrade、wallet-analysis、news、system

## 修改检查清单

- [ ] 新增/修改 API 端点时，必须先在此 crate 中定义 DTO
- [ ] 新增 DTO 文件后在 `lib.rs` 中添加 `include!()`
- [ ] 新增枚举 DTO 时确保与 `domain` crate 的对应枚举一致
- [ ] 修改 DTO 后同步更新前端 `src/lib/contracts/dto/` 中的 TypeScript 类型
- [ ] 运行 `cargo check --workspace --tests`
