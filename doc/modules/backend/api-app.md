# API App（HTTP API 服务）

最后更新：2026-06-26

## 概述

`polyedge-api` 是基于 Axum 的统一后端进程，crate 位于 `packages/api`。它组装 HTTP 路由、认证中间件和原 `polyedge-worker` 后台 runtime，HTTP handler 与后台任务共享同一个 `AppState` / application service 实例。当前内网部署可通过 `POLYEDGE_AUTH__DISABLED=true` 关闭权限校验。

## 设计目标

- Handler 层尽量薄：只做请求解析、DTO 映射和响应构建
- 业务逻辑全部委托给 application 层的 Service
- 通过中间件栈统一处理认证/内网旁路、请求体限制、超时和追踪

## 架构与关键文件

| 文件 | 职责 |
|---|---|
| `packages/api/src/main.rs` | 加载 Runtime、连接 orderbook 服务、启动 `WorkerRuntime` 和 Axum server；监听地址和 signal shutdown 复用 `polyedge-common` |
| `packages/api/src/lib.rs` | 路由组装入口：`build_app(state: AppState) -> Router`（~458 行） |
| `packages/api/src/handlers/system.rs` | 健康检查、就绪检查、系统模式 |
| `packages/api/src/handlers/market_handlers.rs` | 市场列表和详情 |
| `packages/api/src/handlers/funding.rs` | Polymarket 入金：读取后端资金配置、通过 Bridge 广播资金钱包 ERC-20 转账 |
| `packages/api/src/handlers/signal_actions.rs` | 信号 CRUD、重算、转换 |
| `packages/api/src/handlers/execution_submit.rs` | 提交执行请求 |
| `packages/api/src/handlers/execution_lists.rs` | 列表：orders、drafts、positions、trades、execution requests |
| `packages/api/src/handlers/callbacks.rs` + `callback_helpers.rs` | 连接器回调（订单状态、成交回填） |
| `packages/api/src/handlers/risk_handlers.rs` | 风控状态、kill switch |
| `packages/api/src/handlers/console_risk.rs` | 控制台风控视图端点 |
| `packages/api/src/handlers/mode_control.rs` | 系统模式转换 |
| `packages/api/src/handlers/rewards.rs` | 奖励机器人管理；run/cancel/reset 只入队 worker 控制命令 |
| `packages/api/src/handlers/copytrade.rs` | 跟单管理；run/analyze/cancel/reset 只入队 worker 控制命令 |
| `packages/api/src/handlers/wallet_analysis.rs` | 钱包分析 |
| `packages/api/src/handlers/health.rs` | 健康检查 |
| `packages/api/src/handlers/runtime_config.rs` + `runtime_config_helpers.rs` | 运行时配置管理 |
| `packages/api/src/handlers/list_helpers.rs` + `mappers.rs` | 通用分页和 DTO 映射辅助 |

## 路由结构

| 路由组 | 路径前缀 | 认证 |
|---|---|---|
| Health | `/healthz`、`/readyz` | 无 |
| Markets | `/api/v1/markets`、`/api/v1/market-categories` | console_read |
| Funding | `/api/v1/funding`、`/api/v1/funding/transfer` | console_read/write |
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

## 中间件栈

- `RequestBodyLimitLayer`（1MB）— 防止过大请求体
- `TraceLayer` — 请求追踪日志
- `TimeoutLayer`（30s）— 请求超时保护
- `CorsLayer::permissive()` — 允许前端和 API 分别部署在不同内网主机/端口
- 认证中间件：按路由组使用不同的认证级别；`POLYEDGE_AUTH__DISABLED=true` 时不校验 token、dev-auth 头或 step-up code，直接注入内部 admin `AuthContext`

Rewards Bot 的 `run` / `cancel-all` / `reset` handler 不直接执行策略、不读取 orderbook cache，也不直接修改托管订单。Handler 委托 `RewardBotService` 写入 `reward_control_commands`；同账户同动作已有 pending/running 命令时会合并重复请求，真正入队后通过共享 `RewardBotService` revision 立即唤醒同进程 rewards loop；后台 runtime 领取命令并执行 live 逻辑。

所有 Rewards snapshot 响应只读取 `RewardBotService` / store；handler 不直接请求 CLOB/Data API。配置、账户、positions 和 heartbeat 在同进程 service 内有热缓存，分页 orders/plans、fills、events、`llm_usage` 每日调用统计等历史查询仍从 store 读取。`orders_status=filled` 过滤会返回 `status=filled` 或 `filled_size > 0` 的本系统 managed orders，便于排查部分成交后已关闭的被吃订单。`status.open_orders` 只统计已有非内部 `external_order_id`、仍是 open-like 且未处于提交未知、取消未知、404 人工对账或 `awaiting final reconciliation` 锁定的 managed orders；本地 planned/exit intent 和已接受取消但仍等待最终对账的订单不会显示为当前 Polymarket 开放挂单。`status.error` 只报告当前开放订单上的活跃对账锁，不会因为历史 critical event 一直保持错误。外部 balance、positions、订单 scoring 和 UTC 当日账户级 maker rewards（聚合端点优先、明细端点 fallback）由内嵌后台 runtime 同步。
同进程 worker 成功读取 CLOB open-order snapshot 后，`status.open_orders` 优先使用该 snapshot 中仍存在的本系统 managed 外部订单数量；冷启动或尚未成功同步时才回退到本地 store 计数。

Copy Trading 的 `run` / `analyze` / `cancel-all` / `reset` 端点同样不抓取 Polymarket Data API / CLOB，也不直接执行跟单循环；API 只写入 `copytrade_control_commands`，worker 负责领取。当前产品只暴露 Analyze，`run` / `cancel-all` / `reset` 是历史兼容入口，worker 中不再触发模拟交易。

Funding 的 `GET /api/v1/funding` 只读取后端配置并返回派生出的付款钱包地址、Polymarket 入账钱包地址、支持资产和单笔上限；不会返回私钥。`POST /api/v1/funding/transfer` 是真实链上资金操作，当前内网免鉴权部署只要求 `Idempotency-Key` 和请求体确认；关闭 `POLYEDGE_AUTH__DISABLED` 后仍会走 console_write 与 `funding_transfer` step-up scope 校验。handler 不接收前端充值地址，入账钱包固定由 `POLYEDGE_POLYMARKET__FUNDER` 优先、`ACCOUNT_ID` 回退决定，随后委托 `PolymarketChainConnector` 调用 Polymarket Bridge `/deposit` 生成 EVM 入金地址并广播 Polygon ERC-20 转账。

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
- API crate 已从 `packages/backend/apps/api` 拆到顶层 `packages/api`，仍作为 `packages/Cargo.toml` Rust workspace member 构建
- SSE 流式端点已移除，前端通过 REST API 加载数据
- API 进程内嵌 worker runtime；`polyedge-worker` 二进制仅保留 CLI/运维兼容入口，不再单独部署常驻容器
- Rewards Bot 控制端点只作为前端接口和命令入口，具体 live 策略、撤单和重置由同进程后台 runtime 处理；Copy Trading 当前只保留钱包跟踪和 Analyze，旧 run/cancel/reset 入口不执行模拟交易
- Rewards Bot snapshot 不承载全量 reward markets；配置、账户、positions、heartbeat 优先从共享内存读取；`llm_usage` 统计来自 `llm_calls` 日聚合，不触发外部 provider 请求
- Markets DTO 返回 Gamma 同步的 `liquidity_usd` 与 `end_at`，供控制台和其他数据库消费者使用
- Console risk snapshot 先读取当前 positions，再通过 `MarketEventService.get_markets_by_ids()` 批量读取相关 markets 用于分类聚合，不再调用 markets 列表接口全量扫描市场表。
- 当前内网部署使用 `POLYEDGE_AUTH__DISABLED=true`，前端请求不需要权限头或 step-up code
- CORS 当前为 permissive，支持纯内网中 front/API 分别部署在不同服务器
- Step-up 认证代码路径仍保留；当 `POLYEDGE_AUTH__DISABLED=false` 时用于敏感操作（模式切换、kill switch、执行提交）
- Funding 入金端点已接入，支持后端配置资金钱包向配置的 Polymarket 钱包进行 Polygon USDC/USDT Bridge 入金；API 响应只暴露付款地址、入账钱包地址、Bridge 地址和交易 hash，不暴露私钥。

## 已知缺口

- Rewards snapshot 的外部 balance/positions/earnings 新鲜度取决于 rewards poll 周期和外部 API 成功率；失败时保留上一版状态。
- `orders` / `orders_page` 只覆盖本系统 managed orders，尚未提供账户范围全部外部开放订单视图。

## 修改检查清单

- [ ] 新增端点时在 `lib.rs` 的 `build_app()` 中注册路由
- [ ] 新增 handler 文件后在 `lib.rs` 中添加 `include!()`
- [ ] 选择正确的认证级别（console_read/console_write/connector_write/mode_write）
- [ ] 修改认证或 CORS 行为时同步更新 `AuthSettings`、部署模板和 `doc/modules/infra/deployment.md`
- [ ] DTO 类型从 `contracts` crate 引用，不在 handler 中内联定义
- [ ] 修改路由路径后同步更新前端 `src/lib/api/` 中的对应调用
- [ ] 运行 `cargo check --workspace --tests`
