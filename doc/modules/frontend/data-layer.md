# 数据层（API Client + Actions + Contracts）

最后更新：2026-07-16

前端通信统一使用 cookie session、`credentials: include` 和写请求 `X-PolyEdge-CSRF-Token`；后端不再接受旧 CSRF header 别名或 Authorization/Bearer 兼容。`auth.ts`、`admin.ts`、`subscriptions.ts` 与 `wallet-security.ts` 覆盖认证、管理员视图、策略跟随和 WebCrypto hybrid envelope。静态导出不使用 Next Server Actions。

DTO 直接镜像 `packages/backend/crates/contracts/src/identity.rs`、`manual_trading.rs` 与对应 domain 类型：

- `dto/auth.ts`：session user、角色/状态、管理员用户和资金汇总。
- `dto/wallets.ts`：`WalletAccountData` 的 account、secret metadata、risk_policy、state 嵌套结构。
- `dto/strategies.ts`：market、outcomes、reward_terms、owner/visibility/有效期 strategy、published version、quote_slots 和当前用户 subscription。
- `dto/subscriptions.ts`：owner/follower subscription、有效截止时间和自有钱包绑定。
- `dto/executions.ts`：execution batch 与 wallet jobs；创建请求为一个 `strategy_id` 加多个 `wallet_ids`。
- `dto/trading.ts`：受管订单和持仓。
- `dto/settings.ts`：全局 kill-switch / trading runtime state。

当前 API 还包括 `/auth/*`、`/admin/users`、`/admin/finance`、`/security/wallet-import-contexts`、`/market-strategies/discover` 和 `/strategy-subscriptions`。`wallet-security.ts` 使用 WebCrypto 生成 AES-GCM key、加密 JSON payload，并用后端 RSA JWK 包裹 key；明文不会写入 local/session storage，临时 plaintext 与导出的 raw AES key 字节在使用后立即清零。

`base.ts` 不再接受或发送历史 `stepUpCode/stepUpScopes` 参数；危险操作统一依赖后端 recent-auth session。Fills、cash-flow/equity 明细、Funding、全市场、事件、新闻、AI/info-risk 和 fair-value API/DTO 均不存在。

前端运行时依赖保持最小化：未使用的 toast runtime、decimal 校验 helper 及其直接依赖已移除。
