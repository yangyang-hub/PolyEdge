# 数据库（V4 Multi-user Clean Deploy Schema）

最后更新：2026-07-16

## 概述

V4 schema 服务于多用户人工市场策略、稳定 quote slots、多钱包批量执行和跨用户策略跟随。项目明确不兼容旧数据：不迁移旧 markets/events/news/evidences/rewards catalog/candles/fair-value/AI/provider/replay 表，也不兼容 V3 的共享 actor、credential locator 或 strategy wallet target，部署时必须创建空数据库。

核心原则：

- 主键使用 `BIGINT GENERATED ALWAYS AS IDENTITY`；session 和一次性 import context 使用 opaque UUID；
- 价格、数量、余额和名义金额使用 `NUMERIC`，时间使用 `TIMESTAMPTZ`；
- FK 访问路径显式建索引，关系数据保持 3NF，JSONB 只承载 durable action request/result 和审计扩展；
- 用户密码只保存 Argon2 PHC hash；session/activation 只保存不可逆 token hash；钱包 secret 只保存 AES-256-GCM ciphertext 和由环境 storage KEK 包裹的 DEK；
- 市场为全局 canonical 标的，策略、钱包、订单、持仓、收益均带 owner user；复合外键阻止跨用户钱包串接；
- 策略使用 `[active_from, active_until)` 生效窗口；到期生成 durable cancel intent，停止 place/replace 但不自动 SELL/merge；
- follower subscription 引用源策略并绑定自己的钱包，不复制源用户钱包、订单或资金；
- clean deploy 默认全局交易关闭且 kill switch 锁定。

## 初始化入口

| 文件 | 职责 |
|---|---|
| `packages/backend/migrations_v2/0001_manual_trading_schema.sql` | V4 唯一 migration baseline |
| `packages/backend/init.sql` | 与 baseline 字节一致的空库初始化快照 |
| `packages/backend/server/src/state.rs` | server 启动时连接 Postgres 并执行 V4 migration |

旧 `packages/backend/migrations/` 属于前一 schema epoch，不得与 `migrations_v2` 混用。V4 结构变化直接修改 clean baseline 与 `init.sql`；不编写旧数据升级脚本。

## Schema 分组

### 身份、钱包与风险

- `users`、`user_password_credentials`：环境管理员、管理员创建用户、`admin|market_editor|read_only` 角色和密码版本；
- `user_sessions`、`user_activation_tokens`：opaque HttpOnly session、CSRF hash、最近重新认证时间和一次性激活 token；
- `wallet_accounts`：owner、显示名、signer/funder、signature type、状态和钱包交易开关；
- `wallet_secret_envelopes`：每钱包 AES-256-GCM 密文、payload/wrapped-DEK nonce、wrapped DEK、storage `key_id` 和 secret 版本；
- `wallet_import_contexts`：一次性浏览器混合加密导入 context；签发使用 advisory-lock 串行容量检查并清理过期/已消费旧记录，消费按 owner + key id 原子完成；
- `wallet_risk_policies`：开放订单、开放 BUY、总仓位、单市场和单订单名义金额上限；
- `wallet_account_state`：余额、reserved、open BUY 与总持仓 snapshot，使用 monotonic version。

钱包状态非 `active` 时数据库约束禁止 `trading_enabled=true`。管理员可查看钱包运营数据，但 schema 不提供任何明文 secret 或导出结构。

### 人工市场、策略与跟随

- `managed_markets`：用户录入 condition、slug、question、URL 与状态，不存在全市场目录表；
- `managed_market_outcomes`：每个市场映射 YES/NO token，token 全局唯一；
- `market_strategies`：用户拥有的策略、`private|followable` 可见性和 `[active_from, active_until)` 有效期；
- `strategy_versions`：不可变版本，reward 条款随版本固化，每个策略最多一个 `published` 版本；
- `strategy_quote_slots`：稳定 desired-order identity，固化 outcome、quantity、fixed/book-rank、offset、价格边界、post-only 和 enabled；
- `strategy_subscriptions`、`strategy_subscription_wallets`：owner/follower subscription 与订阅者自己的钱包绑定；
- `strategy_commands`：publish/activate/pause/resume/expire/archive/force_cancel 的 durable command、lease 和重启恢复。

quote slot 约束 fixed price 与 positive book rank 二选一，价格严格位于 `(0,1)`，数量必须为正。订阅者与钱包 owner 通过复合外键一致。

### 批次与 durable execution

- `execution_batches`：固化 subscriber、source strategy/version、subscription、request source 和批次状态；
- `wallet_execution_jobs`：`batch + wallet` 唯一，支持 owner/epoch/expiry lease fencing；
- `execution_actions`：place/cancel/replace/reconcile action、全局 idempotency key、request/result JSON 与 action lease。

claim 索引只覆盖可执行 command/job/action，供 `FOR UPDATE SKIP LOCKED` 使用。running/executing 行必须同时具备 owner、positive epoch 和 expiry；terminal write 必须按 owner + epoch 条件更新。

### 订单、持仓与收益

- `managed_orders`：owner、subscription、钱包、市场、版本、quote slot、token、价格、数量、venue id、generation 和状态；
- `managed_orders_open_slot_uidx`：一个钱包同一 quote slot 最多一张 open-like 订单，`unknown` 也占用 slot；
- `order_transitions`：追加式订单状态时间线；
- `positions`：按 `(wallet_id, token_id)` 保存最新持仓，使用 version/observed time fencing；
- `venue_fills`、`external_cash_flows`：可审计成交和外部资金流；
- `position_valuation_snapshots`、`wallet_equity_snapshots`：按用户/钱包保存估值完整性、权益和 PnL snapshot。

这些表不恢复自动 SELL exit、merge 或 Funding runtime。`external_cash_flows` 只允许管理员通过 API 录入，并校验发生时间不早于钱包创建且不超过服务端当前时间五分钟；reward/fee 外部流是管理员汇总中此类调整的唯一来源。managed 累计成交差额和 position 同步产生的 fill/equity 数据仅用于操作性核算，尚无权威 venue fill ingestion 或完整估值生产者。行情缺失时必须标记 `stale|unavailable|partial`，不能以零代替未知值。

### 平台表

- `idempotency_keys`：actor-aware API request hash、owner lease、完整结果与 TTL；
- `audit_logs`：actor user/system、session、resource owner、action/result 追加日志；
- `system_runtime_state`：单行全局 trading/kill switch 状态。

## 数据一致性

- published strategy version 唯一，execution batch 固化 version，不受后续策略更新影响；
- open-like slot partial unique index 与 venue-first match 共同防止重复下单；
- submission/cancel 结果不明时使用 `unknown`，保留 slot ownership 并阻止自动重放；
- API 幂等按 `actor_type + actor_user_id + scope + idempotency_key` 唯一，并校验 request hash；
- 所有租户业务表通过 owner user 和复合 FK 约束钱包归属；管理员全局查询由 API 授权，不通过绕过约束实现；
- follower 的有效停止时间是 `min(source_strategy.active_until, subscription.active_until)`；源版本、订阅暂停和策略到期通过 durable command 驱动 cancel-only reconciliation；
- clean deploy 插入 `system_runtime_state(kill_switch_locked=true, trading_enabled=false)`，首次实盘必须显式重新认证并解锁。

## 当前状态与已知缺口

- V4 baseline 与 `init.sql` 已重写并保持字节一致；旧 V3 表 `wallet_credential_refs`、`market_reward_terms`、`strategy_wallet_targets` 和共享 actor 字段不再存在；
- migration 已包含用户、session、激活、owner 约束、策略有效期/跟随、加密凭证、fills/cashflows/equity 以及 actor-aware 幂等/审计结构；
- server identity、wallet envelope、actor-scoped API/store、subscription execution 和前端主流程已接入新 schema；
- `wallet_import_contexts` 已接入 owner-bound 持久化与原子消费；cash-flow 已有管理员录入链路，但 fills/valuation/equity 仍只有非权威的操作性生产路径，不能视为完整 PnL 账本；
- 数据库不能用普通 CHECK 自动验证“owner subscription 必须由源策略 owner 创建”等跨表业务规则，该规则由授权层和 subscription 创建事务保证；
- schema 不包含 events/news/evidences、Gamma/rewards catalog、candles、fair value、AI/info-risk、LLM calls 或 replay 表。

## 修改检查清单

- [ ] 直接同步修改 V4 baseline 与 `init.sql`，不增加旧 schema 兼容迁移。
- [ ] 两份 SQL 保持字节一致并在空 PostgreSQL 16 上执行。
- [ ] 新 FK 增加访问索引；不得弱化 open-slot、幂等和 lease fencing。
- [ ] 新公开字段同步 domain、contracts、server store/API 与前端 DTO。
- [ ] 运行 workspace 测试与相关数据库 smoke。
