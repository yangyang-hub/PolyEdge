# contracts（V4 HTTP DTO）

最后更新：2026-07-16

## 概述

`polyedge_contracts` 定义 `polyedge-server` 的公开 HTTP 请求/响应。Handler 与前端 TypeScript DTO 必须以这里为唯一字段契约，不在路由文件中内联公开 payload。

V4 契约覆盖身份、管理员、钱包加密导入、人工策略、跨用户跟随、执行批次、managed orders/positions、管理员外部 cash-flow 录入和 system runtime state；不公开 secret、venue fill/equity 写入或旧 markets/events/news/fair-value/funding/provider/orderbook-service DTO。

## 关键文件

| 文件 | 职责 |
|---|---|
| `src/lib.rs` | 当前 DTO re-export |
| `src/dto/common.rs` | `ApiMeta`、`ApiResponse<T>`、结构化错误、health/readiness |
| `src/identity.rs` | login/activate/reauth、用户管理和管理员资金汇总 DTO |
| `src/manual_trading.rs` | 钱包加密、策略、跟随、执行和账本 DTO |

## 核心请求

- `LoginRequest` / `ActivateUserRequest` / `ReauthenticateRequest`：session 身份流程。
- `CreateUserRequest` / `UpdateUserRequest` / `ReissueActivationTokenRequest`：管理员创建、改角色/状态及为 pending local 用户重签令牌；激活 token 只在成功响应返回一次。
- `CreateWalletAccountRequest` / `UpdateWalletAccountRequest`：钱包身份、`EncryptedWalletSecretInput`、交易开关与风险 policy；不接受明文私钥。
- `CreateMarketStrategyRequest`：一次提交人工 market、visibility、有效期、带 reward snapshot 的 version、quote slots 和 owner subscription 钱包。
- `UpdateMarketStrategyRequest`：更新自有策略的 market/status/visibility/有效期、owner 钱包或创建后续不可变版本。
- `CreateStrategySubscriptionRequest` / `UpdateStrategySubscriptionRequest`：跟随他人的 followable 策略，设置 follower 自有钱包、可选截止时间和 active/paused/stopped 状态。
- `CreateExecutionBatchRequest`：一个 `strategy_id` + 多个 `wallet_ids` + 可选 operator note。
- `CancelExecutionBatchRequest` / `CreateCancellationBatchRequest`：批次级或钱包/策略范围的保护性撤单。
- `RecordCashFlowRequest`：管理员为指定钱包录入 deposit/withdrawal/reward/fee/adjustment；时间不得早于钱包创建或超过服务端当前时间五分钟。
- `UpdateSystemRuntimeStateRequest`：全局 trading/kill-switch 状态与 operator note。

## 核心响应

- `AuthSessionData` / `CurrentUserData`：当前用户、session 到期与 recent-auth 时间。
- `WalletImportContextData`：一次性 context、公钥 JWK、算法和到期时间。
- `WalletAccountData`：account + secret metadata + risk + state，不含 ciphertext 或明文 secret。
- `MarketStrategyData`：strategy owner/visibility/有效期 + managed market/outcomes + published version/reward snapshot/slots，并附当前用户 subscription（若存在）。
- `StrategySubscriptionData`：源策略/源用户、owner|follower kind、有效停止时间和 follower 自有钱包绑定。
- `ExecutionBatchData` / `WalletExecutionJobData`：批次和逐钱包结果。
- `WriteOperationData`：accepted、operation id、resource id、completed/queued 状态。
- `ManagedOrderData`、`ManagedPositionData`：复用 domain 账本类型。
- `CashFlowData`：外部资金流及实际钱包 owner、录入管理员和发生时间，不包含 secret。
- `SystemRuntimeStateData`：全局交易与 kill switch 状态。
- `AdminFinanceSummaryData`：按用户聚合最新 equity snapshot；没有 snapshot 时可能为零且 `valuation_complete=false`。

## 契约规则

- 所有响应使用 `ApiResponse<T>`，携带 request/trace meta。
- Decimal 字段按 Rust serde 契约传输；前端不得擅自改成不兼容数字结构。
- operator note 单行且最多 500 字符。
- 常规业务写请求由 server 强制 `Idempotency-Key`、CSRF 和 exact Origin；危险操作要求 recent-auth session。创建用户与重签令牌使用不可泄露 token 的非重放幂等语义。
- 前端 execution batch 必须发送单一 `strategy_id`，不能发送旧 `strategy_version_ids[]`。

## 当前状态

身份、钱包 envelope、strategy/subscription、execution 与 cash-flow DTO 已被 server 使用。前端 `src/lib/contracts/dto/` 已按 auth/wallets/strategies/subscriptions/executions/trading 拆分并 re-export；当前没有 venue fills/equity 写入 DTO。

## 修改检查清单

- [ ] 先改 contract，再改 handler/store 和前端镜像。
- [ ] secret 永不进入 request echo/response/debug DTO。
- [ ] 删除字段时不保留旧数据兼容分支。
- [ ] 运行 backend check/tests 与 frontend typecheck/build。
