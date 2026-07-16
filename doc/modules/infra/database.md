# 数据库（V3 Clean Deploy Schema）

最后更新：2026-07-15

## 概述

V3 schema 服务于人工市场策略、稳定 quote slots 和多钱包批量执行。项目明确不兼容旧数据：不迁移旧 markets/events/news/evidences/rewards catalog/candles/fair-value/AI/provider/replay 表，部署时必须创建空数据库。

核心原则：

- 主键使用 `BIGINT GENERATED ALWAYS AS IDENTITY`；
- 价格、数量、余额和名义金额使用 `NUMERIC`，时间使用 `TIMESTAMPTZ`；
- FK 访问路径显式建索引；
- 关系数据保持 3NF，JSONB 只承载 durable action request/result 和审计扩展；
- credential 表只保存 provider + locator，不保存明文 secret；
- clean deploy 默认全局交易关闭且 kill switch 锁定。

## 初始化入口

| 文件 | 职责 |
|---|---|
| `packages/backend/migrations_v2/0001_manual_trading_schema.sql` | V3 唯一 migration baseline |
| `packages/backend/init.sql` | 与 baseline 字节一致的空库初始化快照 |
| `packages/backend/server/src/state.rs` | server 启动时连接 Postgres 并执行 V3 migration |

旧 `packages/backend/migrations/` 属于前一 schema epoch，不得与 `migrations_v2` 混用。V3 结构变化直接修改 clean baseline 与 `init.sql`；不编写旧数据升级脚本。

## Schema 分组

### 钱包与风险

- `wallet_credential_refs`：`environment|vault|kms` locator 与可选 key version。
- `wallet_accounts`：显示名、signer/funder、signature type、状态和钱包交易开关。
- `wallet_risk_policies`：开放订单、开放 BUY、总仓位、单市场和单订单名义金额上限。
- `wallet_account_state`：余额、reserved、open BUY 与总持仓 snapshot，使用 monotonic version。

钱包状态非 `active` 时数据库约束禁止 `trading_enabled=true`。真实私钥与 CLOB secret 只存在于外部 secret provider。

### 人工市场与策略

- `managed_markets`：operator 录入 condition、slug、question、URL 与状态；不存在全市场目录表。
- `managed_market_outcomes`：每个市场恰当映射 YES/NO token，token 全局唯一。
- `market_reward_terms`：人工录入 minimum size、maximum spread 与可选 daily rate。
- `market_strategies`：市场级统一策略资源。
- `strategy_versions`：不可变版本；每个策略最多一个 `published` 版本。
- `strategy_quote_slots`：每行是一张持续维护的目标订单，固化 outcome、quantity、fixed/book-rank、offset、价格边界、post-only 和 enabled。
- `strategy_wallet_targets`：把一个统一策略分配到多个钱包，不复制策略参数。

quote slot 约束 fixed price 与 positive book rank 二选一，价格严格位于 `(0,1)`，数量必须为正。

### 批次与 durable execution

- `execution_batches`：固化 strategy version、请求人、操作备注和批次状态。
- `wallet_execution_jobs`：`batch + wallet` 唯一；支持 owner/epoch/expiry lease fencing。
- `execution_actions`：place/cancel/replace/reconcile action、全局 idempotency key、request/result JSON 与 action lease。

claim 索引只覆盖可执行 job/action，供 `FOR UPDATE SKIP LOCKED` 使用。running/executing 行必须同时具备 owner、positive epoch 和 expiry；terminal write 必须按 owner + epoch 条件更新。

### 订单与持仓

- `managed_orders`：钱包、市场、版本、quote slot、token、价格、数量、venue id、generation 和状态。
- `managed_orders_open_slot_uidx`：一个钱包同一 quote slot 最多一张 open-like 订单；`unknown` 也占用 slot。
- `order_transitions`：追加式订单状态时间线。
- `positions`：按 `(wallet_id, token_id)` 保存最新持仓，使用 version/observed time fencing。

### 平台表

- `idempotency_keys`：API request hash、owner lease、完整结果与 TTL。
- `audit_logs`：actor/action/resource/result/operator note 追加日志。
- `system_runtime_state`：单行全局 trading/kill switch 状态。

## 数据一致性

- published strategy version 唯一，execution batch 固化 version，不受后续策略更新影响。
- open-like slot partial unique index 与 venue-first match 共同防止重复下单。
- submission/cancel 结果不明时使用 `unknown`，保留 slot ownership 并阻止自动重放。
- API 幂等以 `scope + idempotency_key` 唯一，并校验 request hash；相同 payload 重放首次完整响应，不同 payload 冲突。
- clean deploy 插入 `system_runtime_state(kill_switch_locked=true, trading_enabled=false)`，首次实盘必须显式提权解锁。

## 当前状态

- V3 baseline 与 `init.sql` 已建立并保持字节一致。
- schema 已在 PostgreSQL 16 空库执行验证。
- `polyedge-server` 启动时使用 V3 migration，并已接入钱包、策略、批次、撤单、账本、runtime state、幂等、审计与 execution lease SQL。
- schema 不包含 events/news/evidences、Gamma/rewards catalog、candles、fair value、AI/info-risk、LLM calls 或 replay 表。
- Data API positions 全量替换已接入钱包 execution job。独立 fills、SELL exit、merge 与 Funding 表和运行能力均不属于 V3 schema。

## 修改检查清单

- [ ] 直接同步修改 V3 baseline 与 `init.sql`，不增加旧 schema 兼容迁移。
- [ ] 两份 SQL 保持字节一致并在空 PostgreSQL 16 上执行。
- [ ] 新 FK 增加访问索引；不得弱化 open-slot、幂等和 lease fencing。
- [ ] 新公开字段同步 domain、contracts、server store/API 与前端 DTO。
- [ ] 运行 workspace 测试与相关数据库 smoke。
