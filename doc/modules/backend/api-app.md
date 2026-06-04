# API App（HTTP API 服务）

最后更新：2026-06-04

## 概述

`polyedge-api` 是基于 Axum 的 HTTP API 服务。它组装所有路由、应用认证中间件，将 HTTP 请求映射到 application 层的服务调用。是用户和前端与后端交互的唯一入口；当前内网部署可通过 `POLYEDGE_AUTH__DISABLED=true` 关闭权限校验。

## 设计目标

- Handler 层尽量薄：只做请求解析、DTO 映射和响应构建
- 业务逻辑全部委托给 application 层的 Service
- 通过中间件栈统一处理认证/内网旁路、请求体限制、超时和追踪

## 架构与关键文件

| 文件 | 职责 |
|---|---|
| `lib.rs` | 路由组装入口：`build_app(state: AppState) -> Router`（~458 行） |
| `handlers/system.rs` | 健康检查、就绪检查、系统模式 |
| `handlers/market_handlers.rs` | 市场列表和详情 |
| `handlers/signal_actions.rs` | 信号 CRUD、重算、转换 |
| `handlers/execution_submit.rs` | 提交执行请求 |
| `handlers/execution_lists.rs` | 列表：orders、drafts、positions、trades、execution requests |
| `handlers/callbacks.rs` + `callback_helpers.rs` | 连接器回调（订单状态、成交回填） |
| `handlers/risk_handlers.rs` | 风控状态、kill switch |
| `handlers/console_risk.rs` | 控制台风控视图端点 |
| `handlers/mode_control.rs` | 系统模式转换 |
| `handlers/rewards.rs` | 奖励机器人管理；run/cancel/reset 只入队 worker 控制命令 |
| `handlers/copytrade.rs` | 跟单管理；run/analyze/cancel/reset 只入队 worker 控制命令 |
| `handlers/wallet_analysis.rs` | 钱包分析 |
| `handlers/streams.rs` | SSE 流式端点 |
| `handlers/health.rs` | 健康检查 |
| `handlers/runtime_config.rs` + `runtime_config_helpers.rs` | 运行时配置管理 |
| `handlers/list_helpers.rs` + `mappers.rs` | 通用分页和 DTO 映射辅助 |

## 路由结构

| 路由组 | 路径前缀 | 认证 |
|---|---|---|
| Health | `/healthz`、`/readyz` | 无 |
| Markets | `/api/v1/markets`、`/api/v1/market-categories` | console_read |
| Events/Evidence | `/api/v1/events`、`/api/v1/evidences` | console_read |
| News | `/api/v1/news/source-health`、`/api/v1/news/raw-events` | console_read |
| Signals | `/api/v1/signals`（含 transitions、recompute、execution-requests） | console_read/write |
| Orders/Trades | `/api/v1/orders/drafts`、`/api/v1/orders`、`/api/v1/trades` | console_read |
| Execution | `/api/v1/execution/requests`、`/api/v1/positions` | console_read |
| Callbacks | `/api/v1/connectors/callbacks/orders/status`、`trades/fill`、polymarket 变体 | connector_write |
| Pricing | `/api/v1/pricing/estimates` | console_read |
| Arbitrage | `/api/v1/arbitrage/scans`、`opportunities`、`analysis` | console_read |
| Rewards Bot | `/api/v1/rewards-bot`（snapshot/config/run/cancel-all/reset） | console_read/write |
| Copy Trading | `/api/v1/copy-trading`（snapshot/config/wallets/run/analyze/cancel-all/reset） | console_read/write |
| Wallet Analysis | `/api/v1/wallet-analysis` | console_read |
| Runtime Config | `/api/v1/runtime-config` | console_read/write |
| Risk | `/api/v1/risk/state`、`alerts`、`buckets` | console_read |
| System | `/api/v1/system/mode`、`kill-switch/trigger`、`kill-switch/release` | console_read/mode_write/console_write |
| Streaming | `/api/v1/stream/{channel}` | console_read |

## 中间件栈

- `RequestBodyLimitLayer`（1MB）— 防止过大请求体
- `TraceLayer` — 请求追踪日志
- `TimeoutLayer`（10s）— 请求超时保护
- `CorsLayer::permissive()` — 允许前端和 API 分别部署在不同内网主机/端口
- 认证中间件：按路由组使用不同的认证级别；`POLYEDGE_AUTH__DISABLED=true` 时不校验 token、dev-auth 头或 step-up code，直接注入内部 admin `AuthContext`

Rewards Bot 的 `run` / `cancel-all` / `reset` 端点不执行策略、不读取 orderbook cache，也不直接修改托管订单。API 只把控制命令写入 `reward_control_commands`，随后返回当前 snapshot；命令由 `polyedge-worker` 在 rewards tick 中领取并执行 live 逻辑。

所有 Rewards snapshot 响应都会在读取本地 service snapshot 后执行 live overlay：
- `LivePolymarketConnector.balance()` 覆盖 `account.available_usd`，并把 `reserved_usd` / `realized_pnl` 先清零。
- `LivePolymarketConnector.list_open_orders()` 用认证账户的全部 CLOB open orders 覆盖 `orders`，当前不按 rewards 管理范围过滤；API 使用进程级 connector cache，CLOB 请求失败时清空 cache，下一次请求重新认证。
- `PolymarketDataApiConnector.fetch_wallet_positions()` 覆盖 `positions`，并用外部 positions 的 realized PnL 合计覆盖账户 realized PnL。
- 缺少对应凭证、账户地址或外部调用失败时，相应字段保持零/空，不回退数据库本地账本。

`GET /api/v1/rewards-bot` 的订单查询参数仍会传给 `RewardBotService::snapshot_with_order_query()`，但 handler 随后会用全部 CLOB open orders 覆盖当前页 `orders`，且不会同步改写 `orders_page`。因此当前 `orders_page` 仍描述本地 managed-order 查询，不是 live open orders 的真实分页元数据；外部订单也不包含本地 `rew_*` ID、reason、scoring、pending-cancel 或 deferred-exit 等 worker 元数据。

Copy Trading 的 `run` / `analyze` / `cancel-all` / `reset` 端点同样不抓取 Polymarket Data API / CLOB，也不直接执行跟单循环；API 只写入 `copytrade_control_commands`，worker 负责领取并执行。

## 常量

- `CONNECTOR_ORDER_STATUS_SOURCE` — 连接器订单状态回调来源标识
- `CONNECTOR_TRADE_FILL_SOURCE` — 连接器成交回填回调来源标识
- `DEFAULT_CONSOLE_LIST_LIMIT` = 100
- `MAX_CONSOLE_LIST_LIMIT` = 200
- `MAX_STREAM_EMITTED_IDS` = 1024

## 数据流

```
HTTP Request
    ↓
Auth Middleware（认证 + 鉴权；内网 disabled 模式下注入 admin 上下文）
    ↓
Handler（解析请求、构建 Command/Query）
    ↓
Application Service（业务逻辑）
    ↓
Store（持久化）
    ↓
Handler（DTO 映射、构建响应）
    ↓
HTTP Response
```

## 当前状态

- ~40 个 REST 端点已实现
- SSE 流式端点已覆盖 signals、risk、events、arbitrage
- Rewards Bot 与 Copy Trading 控制端点只作为前端接口和命令入口，具体 live 策略、分析、撤单、重置由 worker 处理
- Rewards Bot snapshot 不承载全量 reward markets，市场数量从 `status.markets_tracked` 读取；账户余额、positions 和 orders 当前由 handler 直接从 Polymarket 覆盖
- 当前内网部署使用 `POLYEDGE_AUTH__DISABLED=true`，前端请求不需要权限头或 step-up code
- CORS 当前为 permissive，支持纯内网中 front/API 分别部署在不同服务器
- Step-up 认证代码路径仍保留；当 `POLYEDGE_AUTH__DISABLED=false` 时用于敏感操作（模式切换、kill switch、执行提交）

## 已知缺口

- Rewards live overlay 在 API handler 内直接调用 Polymarket CLOB/Data API，违反仓库约定的“外部数据由 worker 获取并写入 store/cache”架构；后续应迁移为 worker 同步 + API 只读 store。
- API 要展示 live balance/open orders 必须获得账户地址、私钥和签名配置（可选复用预配置 CLOB API credentials），这扩大了密钥暴露面；仅在 worker 配置私钥的推荐部署中，这两个字段会返回零/空。
- live open orders 覆盖后，`orders_page`、搜索、状态过滤和排序仍对应本地 managed orders，与响应 `orders` 不一致。
- live positions 使用 runtime `POLYEDGE_POLYMARKET__ACCOUNT_ID` 查询，open orders 是该认证账户的全部开放订单；它们当前不会校验或限定到 `RewardBotConfig.account_id` / rewards 托管订单。
- `status.open_orders` / `status.positions` 等 status 统计仍来自 overlay 前的本地 service snapshot，可能与最终 live `orders` / `positions` 数组不一致；account 也仍混合本地 capital/reward/tick 字段与外部 available/realized 字段。

## 修改检查清单

- [ ] 新增端点时在 `lib.rs` 的 `build_app()` 中注册路由
- [ ] 新增 handler 文件后在 `lib.rs` 中添加 `include!()`
- [ ] 选择正确的认证级别（console_read/console_write/connector_write/mode_write）
- [ ] 修改认证或 CORS 行为时同步更新 `AuthSettings`、部署模板和 `doc/modules/infra/deployment.md`
- [ ] DTO 类型从 `contracts` crate 引用，不在 handler 中内联定义
- [ ] 修改路由路径后同步更新前端 `src/lib/api/` 中的对应调用
- [ ] 运行 `cargo check --workspace --tests`
