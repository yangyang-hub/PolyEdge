# infrastructure（基础设施层）

最后更新：2026-06-04

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
| `PolymarketSettings` | account_id、chain_id、signature_type、funder、private_key、API credentials、CLOB/WS/Gamma/Data API host、poll limits |
| `ArbitrageSettings` | book_source、scan_limit、scanner_version |
| `RewardsSettings` | enabled、poll_interval、capital、per_market_usd 等 |
| `NewsSettings` | enabled、sources（列表）、request_timeout_secs |
| `WorkerSettings` | 各 worker 的启用标志和轮询间隔 |
| `OrderbookStreamSettings` | WS 连接和轮询配置（默认 `max_tokens=3000`、`max_levels_per_side=100`） |
| `OrderbookServiceSettings` | orderbook HTTP port/service_url；`write_token` 控制 register/ingest/delete 内部写认证 |
| `AuthSettings` / `AuthKeySettings` | 认证配置和密钥；`disabled` 可开启内网免鉴权模式 |
| `CopytradeSettings` | 跟单配置 |

所有字段使用 `#[serde(default)]`，通过 `POLYEDGE_` 前缀环境变量加载（如 `POLYEDGE_SERVER__PORT`）。`POLYEDGE_POLYMARKET__SIGNATURE_TYPE` 支持 `eoa`、`proxy`、`gnosis_safe`、`poly_1271`；Deposit Wallet 使用 `poly_1271` 并通过 `POLYEDGE_POLYMARKET__FUNDER` 配置 deposit wallet 地址。

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
| `stores/orderbook_cache.rs` | `OrderbookCache` | 内存（TTL + 定期清理 + 每侧盘口深度裁剪 + `entry_count` 真实条目统计）— 仅供 orderbook 服务内部使用；Worker/API 通过 `OrderbookHttpClient` 远程访问 |
| `stores/orderbook_registry.rs` | `OrderbookSubscriptionRegistry` | 内存（来源有序 token 原子替换 + 确定性优先级聚合 + 来源/去重总数统计）— 仅供 orderbook 服务内部使用；Worker 通过 HTTP 注册 token |
| `stores/orderbook_registry_tests.rs` | Registry 回归测试 | 原子 source 替换、优先级和跨 source 去重 |
| `stores/orderbook_cache_tests.rs` | Cache 回归测试 | 最优档排序/裁剪、批量写入和 stale threshold 语义 |
| `stores/rewards_tests.rs` | Rewards store 回归测试 | running 控制命令租约、账户持仓完整替换与失败保留 |
| `stores/runtime_config.rs` | 运行时配置 | PostgreSQL key-value |
| `stores/helpers.rs` | DB 行映射辅助 | — |
| `stores/types.rs` | 共享类型 | — |

**关键模式：**
- Config 存储使用 key-value 表（`reward_bot_config`、`copytrade_config`、`runtime_config`）
- 常量：`SYSTEM_RUNTIME_STATE_ID = "global"`、`RISK_STATE_ID = "global"`
- `db_error(code, context)` 辅助函数统一创建 `dependency_unavailable` 错误
- `RewardBotStore` 的 Postgres key-value 配置读写覆盖全部 rewards 风控配置字段（depth/rank/velocity/requote/reconcile 等）；`execution_mode` 键保留用于向后兼容但被忽略。
- `RewardBotStore` 支持按 external Polymarket order id 查询 rewards managed order，并支持通过 fill id 判断成交是否已入账；live worker 用这两个读路径完成 rewards 托管订单成交幂等同步。
- `RewardBotStore.cancel_open_orders()` 在 Postgres/内存实现中兼容释放旧账本的 `reserved_usd`；新的 rewards 开放买单不再逐单硬占用资金，订单列表优先返回 open-like 状态，避免大量历史成交/撤单淹没当前开放挂单。
- `RewardBotStore.list_orders_page()` 在 Postgres 实现中通过 count + limit/offset 做服务端分页，支持 outcome/condition/token 搜索、状态过滤和 price/size/status 排序；内存实现保持相同语义。
- `RewardBotStore.list_markets(limit)` 只返回 active reward markets；Postgres 实现会先关联 Gamma `markets`，只选择 open + tradable 市场，并按 `volume_24h`、日奖励金额、更新时间排序，用于 rewards tick candidate pool；内存实现按日奖励金额排序；`save_quote_plans()` 会替换当前 quote plan 快照，避免旧的全量计划继续出现在 `/rewards`。
- Postgres `RewardBotStore.apply_tick_outcome()` 会在同一事务中只持久化 orders、fills、positions、account ledger 和 events；reward market 全量目录只由 `upsert_markets()` 更新，quote plan 快照只由 `save_quote_plans()` 替换，避免增量 live tick 误停用全量奖励市场。
- Postgres/内存 `RewardBotStore.apply_account_sync()` 会更新账户状态；外部 positions 成功时原子替换目标账户全部 `reward_positions`，失败时通过 `None` 保留上一版持仓。外部持仓可来自当前 rewards catalog 之外的市场，因此 `reward_positions.condition_id` 不再依赖 `reward_markets` 外键。
- `RewardBotStore` 在 Postgres/内存实现中维护 `reward_control_commands` 队列；API 写入 pending 命令，worker 使用 claim/complete/fail 方法领取并更新执行状态；running 命令超过 5 分钟会重新进入可领取范围。
- `CopyTradeStore` 在 Postgres/内存实现中维护 `copytrade_control_commands` 队列；API 写入 pending 命令，worker 使用 claim/complete/fail 方法领取并更新执行状态。
- `InMemoryOrderbookCache` 在所有写入入口统一按 bids 降序、asks 升序排序后裁剪，确保无序 WS/poll/ingest 数据也保留 top-of-book；写入时间戳早于当前条目的盘口会被忽略；`get_stale_tokens(..., max_age_ms <= 0)` 只检查 TTL，不执行年龄 stale 检查。
- `InMemoryOrderbookSubscriptionRegistry.register_tokens()` 在持有写锁时原子执行 32-source 上限检查，关闭并发新 source 绕过 HTTP 预检查的竞态。

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
- **`AuthSettings.disabled`**：`POLYEDGE_AUTH__DISABLED=true` 时跳过 console/connector/mode token 和 step-up 校验，直接注入 admin `AuthContext`；仅用于纯内网部署
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
- **`PostgresAdvisoryLease`**：持有专用 Postgres session advisory lock；正常结束显式 unlock，异常 drop 时关闭连接释放锁，rewards worker 用它串行化 live 命令/tick/reconcile；使用时要求 `postgres.max_connections >= 2`

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
- 认证中间件支持 JWT/dev-auth 和内网免鉴权模式；当前部署模板默认 `POLYEDGE_AUTH__DISABLED=true`
- Orderbook cache 当前 runtime 使用进程内 `InMemoryOrderbookCache`；Redis 实现保留但未接入默认 runtime
- Orderbook 服务的 `/orderbook/stats` 现在区分真实 cache 条目数、registry 来源数和 registry 去重 token 总数，避免把订阅 token 数误报为缓存条目数
- Orderbook 进程内缓存会先保留最优价格顺序，再按 `POLYEDGE_ORDERBOOK_STREAM__MAX_LEVELS_PER_SIDE` 裁剪每侧 bids/asks 深度，默认 100 档；HTTP register/batch/ingest 入口使用 `max_tokens` 做请求规模上限，register 会原子替换对应 source 当前有序 token 集合，ingest 会先校验整批数据再批量写入并传播缓存错误，registry source 固定上限为 32 个
- Orderbook 缓存拒绝旧 `observed_at` 覆盖更新条目；rewards 控制命令具备 5 分钟 running lease，Postgres rewards live worker 通过 advisory lease 避免多实例并发执行
- Rewards managed order upsert 会更新后续实际提交的 `price` / `size`，保证 flatten 改价、CLOB 数量调整和未知提交恢复使用持久化后的真实参数
- Rewards store 已支持外部账户余额和完整持仓快照同步；成功空持仓快照会清空目标账户持仓，失败响应不会破坏上一版
- Orderbook register/ingest/delete 写接口要求 `x-polyedge-orderbook-token` 与 `POLYEDGE_ORDERBOOK__WRITE_TOKEN` 匹配；该密钥仅配置在 orderbook/worker 服务 env，未配置 token 时写接口关闭，读接口和健康检查仍可用

## 修改检查清单

- [ ] 新增 Store trait 方法后，在 postgres 和 in_memory 实现中同步添加
- [ ] 修改数据库查询后，运行 `cargo test --workspace`
- [ ] 新增配置字段时，同步更新对应的 `deploy/.env*.example`
- [ ] 修改认证逻辑时，检查所有中间件函数的使用点
- [ ] 修改 `AppState` 字段后，检查 `runtime.rs` 的构建逻辑和所有消费方
- [ ] 运行 `cargo check --workspace --tests`
