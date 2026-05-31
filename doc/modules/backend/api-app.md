# API App（HTTP API 服务）

最后更新：2026-05-31

## 概述

`polyedge-api` 是基于 Axum 的 HTTP API 服务。它组装所有路由、应用认证中间件，将 HTTP 请求映射到 application 层的服务调用。是用户和前端与后端交互的唯一入口。

## 设计目标

- Handler 层尽量薄：只做请求解析、DTO 映射和响应构建
- 业务逻辑全部委托给 application 层的 Service
- 通过中间件栈统一处理认证、请求体限制、超时和追踪

## 架构与关键文件

| 文件 | 职责 |
|---|---|
| `lib.rs` | 路由组装入口：`build_app(state: AppState) -> Router`（~461 行） |
| `handlers/system.rs` | 健康检查、就绪检查、系统模式 |
| `handlers/market_handlers.rs` | 市场列表和详情 |
| `handlers/signal_actions.rs` | 信号 CRUD、重算、转换 |
| `handlers/execution_submit.rs` | 提交执行请求 |
| `handlers/execution_lists.rs` | 列表：orders、drafts、positions、trades、execution requests |
| `handlers/callbacks.rs` + `callback_helpers.rs` | 连接器回调（订单状态、成交回填） |
| `handlers/risk_handlers.rs` | 风控状态、kill switch |
| `handlers/console_risk.rs` | 控制台风控视图端点 |
| `handlers/mode_control.rs` | 系统模式转换 |
| `handlers/rewards.rs` + `reward_inputs.rs` + `reward_mappers.rs` | 奖励机器人管理 |
| `handlers/copytrade.rs` | 跟单管理 |
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
- 认证中间件：按路由组使用不同的认证级别

Rewards Bot `run` 端点只从 `reward_markets` 表读取 bounded candidate pool，先做无需盘口的奖励市场预过滤，再并发读取候选 token 的 Redis orderbook cache，避免全量 reward market / orderbook 扫描触发 10s 请求超时。

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
Auth Middleware（认证 + 鉴权）
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
- 认证当前为 `off` 模式，使用 dev-bypass 头
- Step-up 认证用于敏感操作（模式切换、kill switch、执行提交）

## 修改检查清单

- [ ] 新增端点时在 `lib.rs` 的 `build_app()` 中注册路由
- [ ] 新增 handler 文件后在 `lib.rs` 中添加 `include!()`
- [ ] 选择正确的认证级别（console_read/console_write/connector_write/mode_write）
- [ ] DTO 类型从 `contracts` crate 引用，不在 handler 中内联定义
- [ ] 修改路由路径后同步更新前端 `src/lib/api/` 中的对应调用
- [ ] 运行 `cargo check --workspace --tests`
