# API App（HTTP API 服务）

最后更新：2026-06-12

## 概述

`polyedge-api` 是基于 Axum 的统一后端进程。它组装 HTTP 路由、认证中间件和原 `polyedge-worker` 后台 runtime，HTTP handler 与后台任务共享同一个 `AppState` / application service 实例。当前内网部署可通过 `POLYEDGE_AUTH__DISABLED=true` 关闭权限校验。

## 设计目标

- Handler 层尽量薄：只做请求解析、DTO 映射和响应构建
- 业务逻辑全部委托给 application 层的 Service
- 通过中间件栈统一处理认证/内网旁路、请求体限制、超时和追踪

## 架构与关键文件

| 文件 | 职责 |
|---|---|
| `main.rs` | 加载 Runtime、连接 orderbook 服务、启动 `WorkerRuntime` 和 Axum server，并统一优雅关闭 |
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

## 中间件栈

- `RequestBodyLimitLayer`（1MB）— 防止过大请求体
- `TraceLayer` — 请求追踪日志
- `TimeoutLayer`（30s）— 请求超时保护
- `CorsLayer::permissive()` — 允许前端和 API 分别部署在不同内网主机/端口
- 认证中间件：按路由组使用不同的认证级别；`POLYEDGE_AUTH__DISABLED=true` 时不校验 token、dev-auth 头或 step-up code，直接注入内部 admin `AuthContext`

Rewards Bot 的 `run` / `cancel-all` / `reset` handler 不直接执行策略、不读取 orderbook cache，也不直接修改托管订单。Handler 把控制命令写入 `reward_control_commands`，同时通过共享 `RewardBotService` revision 立即唤醒同进程 rewards loop；后台 runtime 领取命令并执行 live 逻辑。

所有 Rewards snapshot 响应只读取 `RewardBotService` / store；handler 不直接请求 CLOB/Data API。配置、账户、positions 和 heartbeat 在同进程 service 内有热缓存，分页 orders/plans、fills、events 等历史查询仍从 store 读取。外部 balance、positions、订单 scoring 和 UTC 当日账户级 maker rewards 聚合由内嵌后台 runtime 同步。

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
- SSE 流式端点已移除，前端通过 REST API 加载数据
- API 进程内嵌 worker runtime；`polyedge-worker` 二进制仅保留 CLI/运维兼容入口，不再单独部署常驻容器
- Rewards Bot 与 Copy Trading 控制端点只作为前端接口和命令入口，具体 live 策略、分析、撤单、重置由同进程后台 runtime 处理
- Rewards Bot snapshot 不承载全量 reward markets；配置、账户、positions、heartbeat 优先从共享内存读取
- 当前内网部署使用 `POLYEDGE_AUTH__DISABLED=true`，前端请求不需要权限头或 step-up code
- CORS 当前为 permissive，支持纯内网中 front/API 分别部署在不同服务器
- Step-up 认证代码路径仍保留；当 `POLYEDGE_AUTH__DISABLED=false` 时用于敏感操作（模式切换、kill switch、执行提交）

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
