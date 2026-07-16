# polyedge-server（单后端应用）

最后更新：2026-07-16

## 概述

`packages/backend/server` 是 V4 唯一后端可执行程序。它在同一进程内组装 Axum API、Postgres store、targeted orderbook、wallet cryptography boundary 和多钱包执行协调器；不再区分 API、provider、worker 或独立 orderbook 服务。

请求 handler 只做鉴权、DTO 解析、幂等、store 调用和响应封装。外部盘口由后台 supervisor 轮询，钱包 side effect 由执行 runtime 完成；请求路径不调用 Polymarket。

## 关键文件

| 文件/目录 | 职责 |
|---|---|
| `src/main.rs` | 加载配置/store，启动 orderbook 与 execution task，绑定 Axum，graceful shutdown |
| `src/lib.rs` | server crate 模块与 `app()` 入口 |
| `src/config.rs` | Postgres、public origin、bootstrap admin、session TTL、targeted orderbook、execution 和 wallet crypto 配置 |
| `src/error.rs` | 结构化、脱敏的 HTTP 错误映射 |
| `src/api/mod.rs` + `src/api/{identity,finance}.rs` | 路由、Cookie session、RBAC、CSRF/Origin、recent-auth、幂等、管理员资金流和 CORS |
| `src/store/identity.rs` | 环境管理员 bootstrap、用户/激活/session 和管理员资金汇总 |
| `src/state.rs` | Postgres pool、V4 migration、环境管理员和进程内依赖组装 |
| `src/store/` | 钱包、策略、批次、交易账本、runtime state 与执行 lease SQL |
| `src/store/finance.rs` | actor-scoped cash-flow 查询、管理员录入时间校验与钱包 owner 审计 |
| `src/store/positions.rs` | Data API 钱包 positions 全量替换、零值回收和风险名义金额合计 |
| `src/store/strategies.rs` + `src/store/strategies/read.rs` | actor-scoped 策略 CRUD、有效期、版本 reward snapshot、owner subscription 与 durable commands |
| `src/store/subscriptions.rs` | follower subscription、用户钱包绑定、有效停止时间和租户隔离查询 |
| `src/api/subscriptions.rs` | 策略发现和 subscription HTTP handlers |
| `src/store/order_reconciliation.rs` | managed order 缺失时的 venue 终态精确对账与 slot 释放 |
| `src/orderbook.rs` | 目标 token 集查询、CLOB REST poll、内存盘口 cache |
| `src/secrets.rs` | 从数据库 envelope 解密每钱包 CLOB secret 的运行时解析 |
| `src/wallet_crypto.rs` | 浏览器 RSA-OAEP-256 + AES-256-GCM 一次性钱包导入上下文、数据库 AES-GCM envelope、密钥配置校验与脱敏 |
| `src/execution.rs` | 钱包 job claim、desired target 计算、风险校验、keep/place/cancel/replace |
| `src/execution/planning.rs` | fixed/book-rank target、重挂确认与钱包风险纯计算 |
| `src/execution/reconciliation.rs` | open-set 缺失订单的 external order id 查询、状态映射与 fail-closed 处理 |
| `src/execution/tests.rs` | desired-state、freshness、post-only、risk 和 reprice 单元测试 |

## HTTP API

健康检查：

- `GET /healthz`
- `GET /readyz`：检查 Postgres，并报告 targeted cache 条目数。

业务路由位于 `/api/v1`：

| 资源 | 路由 |
|---|---|
| 身份 | `POST /auth/login|logout|activate|reauth`、`GET /auth/me` |
| 管理员 | `GET/POST /admin/users`、`PATCH /admin/users/{id}`、`POST /admin/users/{id}/activation-token`、`GET /admin/finance` |
| 钱包导入 | `POST /security/wallet-import-contexts` |
| 钱包 | `GET/POST /wallets`、`GET/PATCH /wallets/{id}` |
| 人工策略 | `GET/POST /market-strategies`、`GET/PATCH /market-strategies/{id}` |
| 策略发现 | `GET /market-strategies/discover` |
| 策略跟随 | `GET/POST /strategy-subscriptions`、`PATCH /strategy-subscriptions/{id}` |
| 执行批次 | `GET/POST /execution-batches`、`GET /execution-batches/{id}`、`POST /execution-batches/{id}/cancel` |
| 批量撤单 | `POST /cancellation-batches` |
| 订单与持仓 | `GET /orders`、`GET /positions` |
| 外部资金流 | `GET /cash-flows`、管理员 `POST /cash-flows` |
| 系统状态 | `GET/PATCH /system/runtime-state` |

常规业务写请求要求 `Idempotency-Key`，以 actor user + scope + request hash 持久化首次完整响应；同时校验 CSRF token 和 `Origin == POLYEDGE_PUBLIC_ORIGIN`。登录、激活、登出、重新认证和当前管理员用户写接口尚未统一使用幂等层。危险写入类别为：

- `wallet_trading_enable`
- `execution_submit`
- `order_cancel_force`
- `system_kill_switch_trigger`
- `system_kill_switch_release`

身份使用数据库 opaque session。token/CSRF 只存 hash；用户名规范化为 3-64 位 ASCII 字母、数字、点、下划线或连字符，显示名拒绝控制字符。登录/reauth 失败计数持久化，所有 Argon2 hash/verify 还经过进程级有界并发，避免未认证请求耗尽 blocking pool。生产 session cookie 为 `__Host-polyedge_session`，带 `Secure`、`HttpOnly`、`SameSite=Strict`，CSRF cookie可由浏览器读取并回传 header。登录即记录 recent authentication；危险操作要求其未超过 `POLYEDGE_AUTH__RECENT_AUTH_TTL_SECONDS`，也可调用 `/auth/reauth` 刷新并轮换 session。环境管理员启动时在 advisory lock 事务中幂等创建/升级，不能经 API 禁用或降权。

## Targeted orderbook

`OrderbookSupervisor` 每轮从 Postgres 精确计算 token universe：

1. 当前有效且 active 的源策略及 subscription 下，enabled subscription wallet 对应的 enabled quote slots；
2. `planned|submitting|open|partially_filled|cancel_pending|unknown` managed orders；
3. quantity > 0 positions。

它只通过 CLOB REST 批量获取这些 token 的盘口，不调用 Gamma、不读取 rewards catalog、不扫描全市场、不预热候选市场。超过 max token 时返回冲突并拒绝截断。当前实现是 poll-only；没有 market-channel WS。

`CachedOrderBook` 保存排序后的 bids/asks、`observed_at` 和本地 `confirmed_at`。目标订单使用策略版本的 `book_freshness_ms` 检查 `confirmed_at`。

## 多钱包执行

- `ExecutionBatch` 固化 published strategy version；每个钱包对应一个 `WalletExecutionJob`。
- runtime 用全局 semaphore 限制钱包并发，用 wallet mutex 保证进程内单钱包串行，并用数据库 owner/epoch/expiry lease 防止多 owner 终态覆盖。
- 每个 quote slot 生成一个 desired BUY：fixed price 或 book rank + offset，随后检查价格边界、数量、freshness 和 post-only。
- 当前订单与 target 一致时 keep；target 消失或失效时 cancel；价格/数量变化满足确认与 cooldown 时 replace；无订单时通过风险检查后 place。
- managed open order 不在 venue open set 时，runtime 按 external order id 精确查询并持久化 partial/filled/cancelled/rejected/expired 终态；只有查询失败、标识不一致或未知状态才写 `unknown` 并占用 slot，禁止盲目补挂。
- 余额在钱包 job 执行前 best-effort 刷新；余额、kill switch、钱包/策略状态、风险预算和盘口校验失败均阻止新单。
- 每个允许新 BUY 的钱包 job 先从 Data API 拉取 funder 的完整 positions；store 只映射 `managed_market_outcomes` 已知 token，对上游缺失 token 写 quantity/average price/realized PnL 零，忽略未知 token，并刷新钱包总持仓与当前市场名义金额。Data API、解析或事务失败会终止 place/replace。kill switch、钱包禁用或操作员 force-cancel 路径跳过余额/持仓刷新，仍可执行保护性撤单。
- 操作员 cancellation batch 通过 durable marker 让目标钱包 job 进入 force-cancel-only，不生成新 target。
- runtime 每轮先持久化到期策略，再从 `strategy_subscriptions` + `strategy_subscription_wallets` 生成 desired-state job；源策略暂停/到期、subscription 暂停/停止/到期、钱包绑定禁用、市场关闭或版本非 published 均 fail closed 并只允许撤单。

## Secret 边界

数据库只保存加密 envelope（version、key id、nonce、wrapped DEK 和 ciphertext）及 ownership metadata；不得保存明文 private key、API secret 或 passphrase。浏览器钱包导入使用短期有界的一次性 RSA-OAEP-256 + AES-256-GCM context，签发容量、过期清理与原子消费统一由数据库管理，拒绝非当前 transport key id。钱包密文 JSON 只允许 `private_key` 与可选 CLOB `api_key/api_secret/api_passphrase`；account id、funder 和 chain id 始终取服务端钱包记录与配置，不接受密文中的隐藏覆盖。数据库 envelope 使用独立的 32 字节 AES storage KEK 包裹每个 payload 的随机 DEK，并以 AES-256-GCM 认证 payload 和 metadata。RSA 私钥与 base64 storage KEK 分别从 `TRANSPORT_PRIVATE_KEY_PEM_FILE`、`STORAGE_KEY_FILE` 的只读 secret 文件读取；Unix 下拒绝 group/world 可访问权限。密钥和解密结果使用 `secrecy`/`zeroize`。

storage KEK 由 secret manager/受控宿主文件挂载，不是 KMS/HSM；当前只支持单活动 key id，没有在线旧 keyring 轮换流程。

新增配置：

- `POLYEDGE_WALLET_CRYPTO__TRANSPORT_KEY_ID`（可选，默认 `wallet-import-rsa-v1`）；
- `POLYEDGE_WALLET_CRYPTO__STORAGE_KEY_ID`（可选，默认 `wallet-storage-aes-v1`）；
- `POLYEDGE_WALLET_CRYPTO__IMPORT_CONTEXT_TTL_SECONDS`（可选，默认 300，最大 3600）；
- `POLYEDGE_WALLET_CRYPTO__MAX_IMPORT_CONTEXTS`（可选，默认 1024，最大 100000）。

## 当前状态

已实现：

- V4 migration 自动执行、环境管理员 bootstrap、用户创建/激活、Cookie session、RBAC/CSRF/recent-auth；
- actor-aware Postgres CRUD、审计和业务 API 幂等；
- 钱包/策略/批次/撤单/账本/runtime-state API；
- 浏览器钱包导入 context、前端 ciphertext 解密、地址校验、数据库 envelope 写入与执行时解密；
- 精确 token universe、targeted REST orderbook cache；
- 多钱包 job claim、钱包串行、place/cancel/replace、unknown fencing；
- CLOB 余额/开放订单读取、Data API positions 全量替换与持仓风险合计；
- exact-origin CORS、写请求 Origin/CSRF 和危险操作 recent-auth；
- 外部 cash-flow 管理员录入、钱包归属审计，以及不早于钱包创建且不超过当前时间五分钟的时间边界校验；
- actor-scoped 策略读取与写入：管理员可全局查看，普通用户只能读取自有或 followable 策略，策略写入要求管理员或市场录入权限且只能修改自有资源（管理员除外）；
- `[active_from, active_until)` 策略窗口、不可恢复的 expired 状态、owner subscription 自动创建、follower subscription CRUD、钱包 owner 校验和 strategy commands 持久化。

已知缺口：

- targeted orderbook 还没有 WebSocket 增量流；
- 账户范围外部订单持续同步未完成；
- SELL exit、merge 与 Funding 不属于 V4；managed 累计成交差额和 position 同步产生的 fill/equity 数据仅供操作性核算，尚无权威 venue fill ingestion 或完整 valuation producer；管理员财务 API 将外部 reward/fee cash-flow 作为该类调整的唯一来源；
- session 与持久化 login/reauth 限流已实现，但没有 MFA、密码重置、session 管理或外部 identity gateway；
- import context 已持久化并原子消费；storage KEK 尚无 KMS/HSM 与在线 keyring 轮换；
- `expire_due_strategies` 已由 execution 周期 supervisor 每轮调用，execution/orderbook 已切换到 subscription wallet desired state；策略、订阅或钱包绑定失效时会生成 durable cancel batch/action 并在重启后继续撤单。strategy command 当前主要作为 durable 审计/批次关联，尚未接入 PostgreSQL NOTIFY 做低延迟唤醒，传播延迟受 reconcile interval 限制；
- `src/api/mod.rs`、`execution.rs`、`store/execution.rs` 等超过 500 行软上限，后续触碰应继续按职责拆分；当前活动生产文件没有超过 800 行硬上限。

## 修改检查清单

- [ ] 路由变化同步 contracts、前端 API/DTO、根 AGENTS 和本文件。
- [ ] 新写端点定义幂等 scope，并评估最小 role 与 recent-auth 要求。
- [ ] 外部调用不得进入 handler；新增后台数据源必须是人工目标集合。
- [ ] 修改执行状态机后覆盖 keep/place/cancel/replace/unknown/lease/risk 测试。
- [ ] 运行 `cargo fmt --all`、`cargo check --workspace --tests`、`cargo test --workspace` 和 clippy。
