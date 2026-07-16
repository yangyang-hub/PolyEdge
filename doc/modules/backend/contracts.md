# contracts（V3 HTTP DTO）

最后更新：2026-07-15

## 概述

`polyedge_contracts` 定义 `polyedge-server` 的公开 HTTP 请求/响应。Handler 与前端 TypeScript DTO 必须以这里为唯一字段契约，不在路由文件中内联公开 payload。

V3 契约只覆盖健康/就绪、钱包、人工策略、执行批次、批量撤单、managed orders/positions 和 system runtime state；不再公开 fills、markets/events/news/rewards/fair-value/funding/provider/orderbook-service 旧 DTO。

## 关键文件

| 文件 | 职责 |
|---|---|
| `src/lib.rs` | 当前 DTO re-export |
| `src/dto/common.rs` | `ApiMeta`、`ApiResponse<T>`、结构化错误、health/readiness |
| `src/manual_trading.rs` | V3 request/data/query DTO |

## 核心请求

- `CreateWalletAccountRequest` / `UpdateWalletAccountRequest`：钱包身份、credential locator、交易开关与风险 policy；不接受私钥。
- `CreateMarketStrategyRequest`：一次提交人工 market、rewards terms、version、quote slots 和 `wallet_ids`。
- `UpdateMarketStrategyRequest`：更新 market/status 或创建后续不可变版本。
- `CreateExecutionBatchRequest`：一个 `strategy_id` + 多个 `wallet_ids` + 可选 operator note。
- `CancelExecutionBatchRequest` / `CreateCancellationBatchRequest`：批次级或钱包/策略范围的保护性撤单。
- `UpdateSystemRuntimeStateRequest`：全局 trading/kill-switch 状态与 operator note。

## 核心响应

- `WalletAccountData`：account + credential ref + risk + state，不含 secret。
- `MarketStrategyData`：strategy + managed market/outcomes/reward terms + published version/slots/targets。
- `ExecutionBatchData` / `WalletExecutionJobData`：批次和逐钱包结果。
- `WriteOperationData`：accepted、operation id、resource id、completed/queued 状态。
- `ManagedOrderData`、`ManagedPositionData`：复用 domain 账本类型。
- `SystemRuntimeStateData`：全局交易与 kill switch 状态。

## 契约规则

- 所有响应使用 `ApiResponse<T>`，携带 request/trace meta。
- Decimal 字段按 Rust serde 契约传输；前端不得擅自改成不兼容数字结构。
- operator note 单行且最多 500 字符。
- 业务写请求由 server 强制 `Idempotency-Key`；危险操作另要求 step-up scope。
- 前端 execution batch 必须发送单一 `strategy_id`，不能发送旧 `strategy_version_ids[]`。

## 当前状态

V3 manual-trading DTO 已被 server API 使用。前端 `src/lib/contracts/dto/` 与表单必须逐字段镜像；任何嵌套/字段名变化都要同时验证 Rust serde 与 TypeScript 类型检查。

## 修改检查清单

- [ ] 先改 contract，再改 handler/store 和前端镜像。
- [ ] secret 永不进入 request echo/response/debug DTO。
- [ ] 删除字段时不保留旧数据兼容分支。
- [ ] 运行 backend check/tests 与 frontend typecheck/build。
