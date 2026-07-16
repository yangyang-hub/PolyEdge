# polyedge-server（单后端应用）

最后更新：2026-07-15

## 概述

`packages/backend/server` 是 V3 唯一后端可执行程序。它在同一进程内组装 Axum API、Postgres store、targeted orderbook、environment wallet-secret resolver 和多钱包执行协调器；不再区分 API、provider、worker 或独立 orderbook 服务。

请求 handler 只做鉴权、DTO 解析、幂等、store 调用和响应封装。外部盘口由后台 supervisor 轮询，钱包 side effect 由执行 runtime 完成；请求路径不调用 Polymarket。

## 关键文件

| 文件/目录 | 职责 |
|---|---|
| `src/main.rs` | 加载配置/store，启动 orderbook 与 execution task，绑定 Axum，graceful shutdown |
| `src/lib.rs` | server crate 模块与 `app()` 入口 |
| `src/config.rs` | Postgres、CORS、Bearer token、targeted orderbook、多钱包和 secret prefix 配置 |
| `src/error.rs` | 结构化、脱敏的 HTTP 错误映射 |
| `src/api/mod.rs` | 路由、请求上下文、鉴权/step-up、幂等响应和 CORS |
| `src/state.rs` | Postgres pool、V3 migration 和进程内依赖组装 |
| `src/store/` | 钱包、策略、批次、交易账本、runtime state 与执行 lease SQL |
| `src/store/positions.rs` | Data API 钱包 positions 全量替换、零值回收和风险名义金额合计 |
| `src/store/order_reconciliation.rs` | managed order 缺失时的 venue 终态精确对账与 slot 释放 |
| `src/orderbook.rs` | 目标 token 集查询、CLOB REST poll、内存盘口 cache |
| `src/secrets.rs` | credential provider/locator 到每钱包 CLOB secret 的运行时解析 |
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
| 钱包 | `GET/POST /wallets`、`GET/PATCH /wallets/{id}` |
| 人工策略 | `GET/POST /market-strategies`、`GET/PATCH /market-strategies/{id}` |
| 执行批次 | `GET/POST /execution-batches`、`GET /execution-batches/{id}`、`POST /execution-batches/{id}/cancel` |
| 批量撤单 | `POST /cancellation-batches` |
| 订单与持仓 | `GET /orders`、`GET /positions` |
| 系统状态 | `GET/PATCH /system/runtime-state` |

所有写请求要求 `Idempotency-Key`，以 scope + request hash 持久化首次完整响应。危险写入使用：

- `wallet_trading_enable`
- `execution_submit`
- `order_cancel_force`
- `system_kill_switch_trigger`
- `system_kill_switch_release`

当 `POLYEDGE_AUTH__DISABLED=false` 时，当前实现要求至少 32 字符的 `POLYEDGE_AUTH__API_TOKEN`，并校验 `Authorization: Bearer <token>`。step-up scopes 来自 `x-polyedge-step-up-scopes`，code 来自 `x-polyedge-step-up-code`；production 启动时强制配置至少 16 字符的 `POLYEDGE_AUTH__STEP_UP_CODE`。该实现不是完整生产 session/JWT 身份系统。

## Targeted orderbook

`OrderbookSupervisor` 每轮从 Postgres 精确计算 token universe：

1. active/open 策略下 enabled wallet target 对应的 enabled quote slots；
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

## Secret 边界

数据库只保存 `provider`、`locator` 和可选 key version。当前 resolver 支持 environment provider：规范化 locator 后读取 `${POLYEDGE_WALLET_SECRETS__ENV_PREFIX}<LOCATOR>` JSON，解析 private key 与 CLOB API credentials。错误和 `Debug` 输出不得包含 secret 值。

## 当前状态

已实现：

- V3 migration 自动执行、Postgres CRUD、审计和 API 幂等；
- 钱包/策略/批次/撤单/账本/runtime-state API；
- 精确 token universe、targeted REST orderbook cache；
- 多钱包 job claim、钱包串行、place/cancel/replace、unknown fencing；
- CLOB 余额/开放订单读取、Data API positions 全量替换与持仓风险合计；
- exact-origin CORS、Bearer API token 骨架和危险操作 step-up。

已知缺口：

- targeted orderbook 还没有 WebSocket 增量流；
- 账户范围外部订单持续同步未完成；
- SELL exit、merge、Funding 与独立 fills 账本已从 schema/API/前端/connector/runtime 删除，不属于 V3 待办；
- 生产级 session/identity gateway 未实现；
- `src/api/mod.rs`、`execution.rs`、`store/execution.rs` 等超过 500 行软上限，后续触碰应继续按职责拆分；当前活动生产文件没有超过 800 行硬上限。

## 修改检查清单

- [ ] 路由变化同步 contracts、前端 API/DTO、根 AGENTS 和本文件。
- [ ] 新写端点定义幂等 scope，并评估最小 step-up scope。
- [ ] 外部调用不得进入 handler；新增后台数据源必须是人工目标集合。
- [ ] 修改执行状态机后覆盖 keep/place/cancel/replace/unknown/lease/risk 测试。
- [ ] 运行 `cargo fmt --all`、`cargo check --workspace --tests`、`cargo test --workspace` 和 clippy。
