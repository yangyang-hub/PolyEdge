# 数据层（API Client + Actions + Contracts）

最后更新：2026-07-15

前端所有后端通信通过 `src/lib/api/base.ts`。读取按 V3 领域拆为 `wallets.ts`、`strategies.ts`、`operations.ts`、`settings.ts`；写操作由 `src/lib/api/actions.ts` 暴露，并统一携带 `Idempotency-Key`、request id、错误元数据和受保护操作的 step-up scope/code。静态导出不使用 Next Server Actions，mutation 是浏览器调用的 API client。

DTO 直接镜像 `packages/backend/crates/contracts/src/manual_trading.rs` 与对应 domain 类型：

- `dto/wallets.ts`：`WalletAccountData` 的 account、credential、risk_policy、state 嵌套结构。
- `dto/strategies.ts`：market、outcomes、reward_terms、strategy、published version、quote_slots、wallet_targets。
- `dto/executions.ts`：execution batch 与 wallet jobs；创建请求为一个 `strategy_id` 加多个 `wallet_ids`。
- `dto/trading.ts`：受管订单和持仓。
- `dto/settings.ts`：全局 kill-switch / trading runtime state。

当前 API：`/api/v1/wallets`、`/market-strategies`、`/execution-batches`、`/cancellation-batches`、`/orders`、`/positions`、`/system/runtime-state`。Fills、Funding、runtime-config、全市场、事件、新闻、Rewards、AI/info-risk 和 fair-value API/DTO 均不在前端数据层中。
