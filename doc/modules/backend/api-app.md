# API App（HTTP API 服务）

最后更新：2026-07-07

## 概述

`polyedge-api` 是基于 Axum 的统一后端进程，crate 位于 `packages/backend/api`。它组装 HTTP 路由、认证中间件、runtime 依赖和内嵌 `WorkerRuntime`。API handler 保持薄层，只做认证、请求解析、DTO 映射、命令入队和 store/service 读取；市场数据和策略执行由 orderbook 服务与 worker 负责。

当前 API 聚焦市场/事件/新闻基础设施、订单/执行查询、pricing 估计、LP rewards、Funding、runtime config 和系统模式。旧钱包类与独立研究路由已删除。

## 设计目标

- Handler 不直接抓 Polymarket/Gamma/CLOB 外部数据。
- 业务逻辑委托 application service；持久化通过 infrastructure store。
- 写操作使用明确的认证层级和幂等/step-up 校验（当 auth 未关闭时）。
- Rewards 控制端点只入队命令，由后台 runtime 执行 live 策略。

## 架构与关键文件

| 文件 | 职责 |
|---|---|
| `packages/backend/api/src/main.rs` | 加载 Runtime、连接 orderbook 服务、启动内嵌 worker runtime 和 Axum server |
| `packages/backend/api/src/lib.rs` | 路由组装入口、handler include 和共享常量 |
| `handlers/system.rs` | 健康、就绪和系统状态 |
| `handlers/market_handlers.rs` | 市场列表、详情和分类 |
| `handlers/funding.rs` | 后端资金钱包状态和 Polygon Bridge 入金转账 |
| `handlers/execution_lists.rs` | orders、drafts、trades、execution requests 查询 |
| `handlers/callbacks.rs` + `callback_helpers.rs` | connector 订单状态/成交回调 |
| `handlers/mode_control.rs` | 系统模式转换 |
| `handlers/rewards.rs` | Rewards snapshot、配置保存和 run/cancel/reset 命令入队 |
| `handlers/health.rs` | 健康检查 helper |
| `handlers/runtime_config.rs` + `runtime_config_helpers.rs` | 运行时配置读写 |
| `handlers/mappers.rs` | DTO 映射辅助 |

## 路由结构

| 路由组 | 路径 | 认证 |
|---|---|---|
| Health | `GET /healthz`、`GET /readyz` | 无 |
| Markets | `GET /api/v1/markets`、`GET /api/v1/markets/{market_id}`、`GET /api/v1/market-categories` | `console_read` |
| Orderbook | `GET /api/v1/orderbook/{token_id}` | `console_read` |
| Funding | `GET /api/v1/funding`、`POST /api/v1/funding/transfer` | `console_read` / `console_write` |
| Events/Evidences | `GET /api/v1/events`、`GET /api/v1/evidences` | `console_read` |
| News | `GET /api/v1/news/source-health`、`GET /api/v1/news/raw-events` | `console_read` |
| Orders/Trades | `GET /api/v1/orders/drafts`、`GET /api/v1/orders`、`GET /api/v1/trades` | `console_read` |
| Execution | `GET /api/v1/execution/requests` | `console_read` |
| Callbacks | `POST /api/v1/connectors/callbacks/orders/status`、`POST /api/v1/connectors/callbacks/trades/fill`、Polymarket 同名变体 | `connector_write` |
| Pricing | `GET /api/v1/pricing/estimates` | `console_read` |
| Rewards Bot | `GET /api/v1/rewards-bot`、`POST /api/v1/rewards-bot/config`、`POST /api/v1/rewards-bot/run`、`POST /api/v1/rewards-bot/cancel-all`、`POST /api/v1/rewards-bot/reset` | `console_read` / `console_write` |
| Runtime Config | `GET /api/v1/runtime-config`、`POST /api/v1/runtime-config` | `console_read` / `console_write` |
| System | `GET /api/v1/system/mode`、`POST /api/v1/system/mode` | `console_read` / `mode_write` |

## 中间件栈

- `RequestBodyLimitLayer`：1MB 请求体限制。
- `TraceLayer`：HTTP 请求追踪。
- `TimeoutLayer`：30 秒超时。
- `CorsLayer::permissive()`：支持前端和 API 分别部署在内网不同主机/端口。
- 认证中间件：按路由组使用 `console_read`、`console_write`、`connector_write` 或 `mode_write`；`POLYEDGE_AUTH__DISABLED=true` 时直接注入内部 admin `AuthContext`。

## 关键行为

Rewards Bot 的 `run`、`cancel-all` 和 `reset` handler 不直接执行策略、不读取外部盘口、不修改 live 订单。handler 委托 `RewardBotService` 写入 `reward_control_commands`，同账户同动作 pending/running 命令会合并；成功入队后通过 service revision 唤醒同进程 rewards loop。

Rewards snapshot 只读取 `RewardBotService` / store。配置、账户、positions 和 heartbeat 有同进程热缓存；分页 orders/plans、fills、events、`llm_usage` 每日调用统计从 store 读取。snapshot handler 会 best-effort 通过 `OrderbookHttpClient` 批量读取当前页 orders/positions 的 token 盘口并注入 `token_quotes`；orderbook 服务不可用时不阻断响应。

Funding 状态接口只返回派生付款地址、Polymarket 入账钱包、支持资产、单笔上限和 USDC/USDT 链上余额，不返回私钥。转账接口是真实链上操作：验证幂等键和确认字段后，委托 `PolymarketChainConnector` 调用 Bridge 生成入金地址并广播 Polygon ERC-20 转账。

Connector callback 路由用于订单状态和成交回填。回调通过 external-event/idempotency 存储防重，并委托 execution service 做状态更新和审计。

## 数据流

```text
HTTP Request
    -> Auth Middleware
    -> Handler
    -> Application Service
    -> Store / connector callback helper
    -> DTO response
```

## 当前状态

- 当前 REST 端点覆盖市场、事件/证据、新闻、订单/成交、执行请求、pricing、rewards、funding、runtime config、system mode、connector callback 和单 token orderbook 代理。
- SSE 流式端点已移除；前端通过 REST API 加载数据。
- API 进程内嵌 worker runtime；`polyedge-worker` 只作为 CLI/运维兼容入口保留。
- 当前内网部署常用 `POLYEDGE_AUTH__DISABLED=true`；关闭该开关后，写路径仍使用 console/mode 权限和 Funding step-up scope。
- CORS 当前为 permissive，适配内网前后端分开部署。

## 已知缺口

- Rewards 外部 balance、positions 和 earnings 新鲜度取决于 rewards poll 周期和外部 API 成功率。
- `orders` / `orders_page` 只覆盖本系统 managed orders，尚未提供账户范围全部外部开放订单视图。

## 修改检查清单

- [ ] 新增端点时在 `build_app()` 中注册路由并选择正确认证层级。
- [ ] 新增 handler 文件后在 `lib.rs` 中添加 `include!()`。
- [ ] DTO 从 `contracts` crate 引用，不在 handler 中内联定义。
- [ ] 修改路由后同步更新前端 `src/lib/api/`、模块文档和 API 合约文档。
- [ ] 运行 `cargo check --workspace --tests`。
