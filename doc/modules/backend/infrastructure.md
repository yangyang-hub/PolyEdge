# infrastructure（基础设施层）

最后更新：2026-06-01

## 概述

`polyedge_infrastructure` crate 提供所有跨切面的基础设施实现：持久化存储（Postgres/内存）、认证中间件、HTTP 工具、配置管理、运行时依赖注入容器和遥测。它是 application 层定义的 Store trait 的具体实现者。

## 设计目标

- 实现 application 层定义的所有 Store trait（端口适配）
- 提供 Postgres 和 in-memory 双实现，支持有/无数据库运行
- 统一配置加载（环境变量 + TOML）
- 封装认证和鉴权中间件

## 架构与关键文件

| 文件/目录 | 职责 |
|---|---|
| `lib.rs` | 模块声明（7 个 pub mod） |
| `settings.rs` + `settings/` | 配置管理：`Settings` 及所有子配置 |
| `stores.rs` + `stores/` | Store trait 的持久化实现 |
| `catalog/` | 核心市场/事件/信号/执行存储（最大子模块） |
| `auth.rs` + `auth/` | 认证中间件和令牌验证 |
| `runtime.rs` | `AppState`、`Runtime`、`RuntimeDependencies` |
| `http.rs` | `HttpError`、hash/trace 辅助函数 |
| `telemetry.rs` | 结构化日志初始化 |

## 核心数据结构

### Settings — 配置管理

**顶层结构 `Settings`**（通过 `config` crate 从环境变量加载）：

| 子配置 | 关键字段 |
|---|---|
| `ServerSettings` | host、port（默认 38001） |
| `DatabaseSettings` | url（Option）、max_connections |
| `RedisSettings` | url（Option） |
| `RuntimeSettings` | environment、initial_mode |
| `RiskSettings` | exposure_reference_nav、daily_pnl limits、gross/net exposure limits、kill_switch、min_signal_confidence 等 12 个字段 |
| `ArbitrageSettings` | book_source、scan_limit、scanner_version |
| `RewardsSettings` | enabled、poll_interval、capital、per_market_usd 等 |
| `NewsSettings` | enabled、sources（列表）、request_timeout_secs |
| `WorkerSettings` | 各 worker 的启用标志和轮询间隔 |
| `OrderbookStreamSettings` | WS 连接和轮询配置（默认 `max_tokens=20000`） |
| `AuthSettings` / `AuthKeySettings` | 认证配置和密钥 |
| `CopytradeSettings` | 跟单配置 |

所有字段使用 `#[serde(default)]`，通过 `POLYEDGE_` 前缀环境变量加载（如 `POLYEDGE_SERVER__PORT`）。

另有 `runtime_config` 子模块支持运行时动态配置。

### Stores — 持久化实现

**文件结构**（通过 `include!` 内联）：

| 实现文件 | 对应 trait | 存储 |
|---|---|---|
| `catalog/postgres/` | `MarketEventStore`、`ArbitrageStore`、`NewsIngestionStore` | PostgreSQL |
| `catalog/in_memory.rs` | 同上 | 内存（RwLock） |
| `stores/rewards/postgres.rs` | `RewardBotStore` | PostgreSQL（key-value config + 完整表） |
| `stores/rewards/postgres_control_commands.rs` / `postgres_orders.rs` / `postgres_writes.rs` | `RewardBotStore` 辅助 | 控制命令 SQL、订单分页 SQL、旧 reserved 释放辅助 |
| `stores/rewards/in_memory.rs` | 同上 | 内存 |
| `stores/copytrade/postgres.rs` | `CopyTradeStore` | PostgreSQL（key-value config + 完整表） |
| `stores/copytrade/postgres_control_commands.rs` / `postgres_rows.rs` / `postgres_writes.rs` | `CopyTradeStore` 辅助 | 控制命令 SQL、行映射、写入辅助 |
| `stores/copytrade/in_memory.rs` | 同上 | 内存 |
| `stores/mode_state.rs` | `ModeStateStore` | PostgreSQL/内存 |
| `stores/risk_state.rs` | `RiskStateStore` | PostgreSQL/内存 |
| `stores/idempotency.rs` | `IdempotencyStore` | PostgreSQL/内存 |
| `stores/audit.rs` | `AuditLogSink` | PostgreSQL/内存 |
| `stores/orderbook_cache.rs` | `OrderbookCache` | 内存（TTL + 定期清理）— 仅供 orderbook 服务内部使用；Worker/API 通过 `OrderbookHttpClient` 远程访问 |
| `stores/orderbook_registry.rs` | `OrderbookSubscriptionRegistry` | 内存（RwLock）— 仅供 orderbook 服务内部使用；Worker/API 通过 HTTP 注册 token |
| `stores/runtime_config.rs` | 运行时配置 | PostgreSQL key-value |
| `stores/helpers.rs` | DB 行映射辅助 | — |
| `stores/types.rs` | 共享类型 | — |

**关键模式：**
- Config 存储使用 key-value 表（`reward_bot_config`、`copytrade_config`、`runtime_config`）
- 常量：`SYSTEM_RUNTIME_STATE_ID = "global"`、`RISK_STATE_ID = "global"`
- `db_error(code, context)` 辅助函数统一创建 `dependency_unavailable` 错误
- `RewardBotStore.cancel_open_orders()` 在 Postgres/内存实现中兼容释放旧账本的 `reserved_usd`；新的 rewards 模拟开放买单不再逐单硬占用资金，订单列表优先返回 open-like 状态，避免大量历史成交/撤单淹没当前开放挂单。
- `RewardBotStore.list_orders_page()` 在 Postgres 实现中通过 count + limit/offset 做服务端分页，支持 outcome/condition/token 搜索、状态过滤和 price/size/status 排序；内存实现保持相同语义。
- `RewardBotStore.list_markets(limit)` 在 Postgres/内存实现中只返回 active reward markets，并按日奖励金额排序，用于 rewards tick candidate pool；`save_quote_plans()` 会替换当前 quote plan 快照，避免旧的全量计划继续出现在 `/rewards`。
- Postgres `RewardBotStore.apply_simulation_tick()` 会在同一事务中持久化 reward markets、quote plans、orders、fills、positions、account ledger 和 events，避免计划快照与账本/订单半更新。
- `RewardBotStore` 在 Postgres/内存实现中维护 `reward_control_commands` 队列；API 写入 pending 命令，worker 使用 claim/complete/fail 方法领取并更新执行状态。
- `CopyTradeStore` 在 Postgres/内存实现中维护 `copytrade_control_commands` 队列；API 写入 pending 命令，worker 使用 claim/complete/fail 方法领取并更新执行状态。

### Catalog — 核心数据存储

**Postgres 实现**（`catalog/postgres/`）：通过 `include!` 拆分为多个子文件
- `market_event/` — 最大的存储模块，包含 queries、execution_updates 等
- `arbitrage.rs` — 套利数据存储
- `helpers/` — 共享辅助文件：`fetch.rs`、`market_rows.rs`、`news_rows.rs`、`arbitrage_rows.rs`、`event_rows.rs`、`execution_rows.rs`、`calculations.rs`

**In-memory 实现**（`catalog/in_memory.rs`，~24KB）：用于测试和无数据库环境

### Auth — 认证中间件

- **`AuthContext`**：请求级认证上下文
- **`IdempotencyKey`**：幂等键解析
- **`InternalTokenVerifier`**：内部 JWT 令牌验证
- **`RequestKind`**：请求类型枚举
- **中间件函数：**
  - `require_connector_write_auth` — 连接器写入认证
  - `require_console_read_auth` — 控制台读取认证
  - `require_console_write_auth` — 控制台写入认证
  - `require_mode_write_auth` — 模式切换认证

### Runtime — 依赖注入

- **`AppState`**：所有服务实例的容器，被 API handler 和 worker 共享
  - 包含：`market_event_service`、`execution_service`、`risk_service`、`reward_bot_service`、`copytrade_service`、`arbitrage_service`、`news_ingestion_service`、`orderbook_cache` 等
- **`Runtime`**：应用运行时封装
- **`RuntimeDependencies`**：依赖项构建器

### HTTP 工具

- `HttpError`：HTTP 错误映射（AppError → HTTP status code）
- `hash_json(value)`：JSON 值哈希
- `new_trace_id()`：生成 trace ID
- `request_id_from_headers(headers)`：从请求头提取 request ID

## 依赖关系

- **上游**：`domain`（枚举、错误类型）、`application`（Store trait、Service struct）
- **下游**：`apps/api`（使用 AppState、auth middleware）、`apps/worker`（使用 AppState、Store 实现）

## 当前状态

- Postgres 和 in-memory 双实现均已就绪
- 配置通过环境变量加载，支持 `.env` 文件
- 认证中间件当前在 `off` 模式下运行
- Orderbook cache 当前 runtime 使用进程内 `InMemoryOrderbookCache`；Redis 实现保留但未接入默认 runtime

## 修改检查清单

- [ ] 新增 Store trait 方法后，在 postgres 和 in_memory 实现中同步添加
- [ ] 修改数据库查询后，运行 `cargo test --workspace`
- [ ] 新增配置字段时，同步更新 `deploy/.env.example`
- [ ] 修改认证逻辑时，检查所有中间件函数的使用点
- [ ] 修改 `AppState` 字段后，检查 `runtime.rs` 的构建逻辑和所有消费方
- [ ] 运行 `cargo check --workspace --tests`
