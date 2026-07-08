# infrastructure（基础设施层）

最后更新：2026-07-08

## 概述

`polyedge_infrastructure` crate 提供运行时基础设施：配置加载、Postgres/内存 store、认证中间件、HTTP 错误映射、依赖注入和遥测。它实现 application 层定义的 store trait，并为 API、worker 和 orderbook 服务提供同一套 `AppState` / `Runtime` 构建逻辑。

旧钱包类与独立研究 store/settings/runtime 字段已删除；新部署使用当前迁移和 `init.sql` 直接初始化，不兼容旧表。

## 设计目标

- application 层只依赖 trait，基础设施层提供 Postgres 与 in-memory 双实现。
- 环境变量和 runtime config 统一进入强类型 `Settings`。
- 认证、幂等、审计、外部事件防重和依赖注入集中管理。
- orderbook 服务内部使用本 crate 的内存盘口 cache/registry；API/worker 正常通过 HTTP client 访问独立 orderbook 服务。

## 架构与关键文件

| 文件/目录 | 职责 |
|---|---|
| `lib.rs` | 模块声明 |
| `settings.rs` + `settings/` | `Settings`、默认值、环境变量解析和 runtime config 映射 |
| `runtime.rs` | `Runtime`、`AppState`、Postgres/Redis 连接和 store/service 注入 |
| `stores.rs` + `stores/` | Store trait 的 Postgres/内存实现 |
| `catalog/` | 市场、事件、新闻、执行历史等核心存储 |
| `auth.rs` + `auth/` | 认证/鉴权、内部 token、step-up scope |
| `http.rs` | `HttpError`、trace id、请求 hash 等 HTTP 辅助 |
| `telemetry.rs` | tracing 初始化和默认 filter |

## Settings

`Settings` 使用 `POLYEDGE_` 前缀环境变量加载，并对所有子结构启用 `#[serde(default)]`。

| 子配置 | 关键字段 |
|---|---|
| `ServerSettings` | API host、port（默认 38001） |
| `DatabaseSettings` | Postgres URL、连接池大小 |
| `RedisSettings` | Redis URL（当前默认 runtime 未强依赖） |
| `RuntimeSettings` | environment、initial_mode |
| `RiskSettings` | legacy 执行链路使用的全局风险/kill switch 状态初始值 |
| `PolymarketSettings` | CLOB/Gamma/Data API/WS/Polygon RPC host、账户、签名类型、私钥、API credentials、轮询限制 |
| `RewardsSettings` | rewards 进程级启用、poll 间隔、AI provider env-only key/base URL/model/timeout、信息风险扫描配置和可选备用 provider |
| `NewsSettings` | 新闻系统启用、间隔、超时、单源数量和 RSS/Atom source 列表 |
| `WorkerSettings` | news、rewards、execution、Polymarket 对账、orderbook 注册、database maintenance 等后台任务开关与间隔 |
| `OrderbookStreamSettings` | orderbook WS/poll/cache、registry token cap、candle history sync 和增量重订配置 |
| `OrderbookServiceSettings` | orderbook 服务端口、service URL、写 token |
| `AuthSettings` | auth disabled、issuer/audience、token TTL、step-up code、密钥和撤销 session |

未配置 `POLYEDGE_NEWS__SOURCES_JSON` 时，`NewsSettings.sources` 使用 `settings/defaults.rs` 中的默认 RSS/Atom 源。`POLYEDGE_POLYMARKET__PRIVATE_KEY` 只允许后端环境配置，既用于 CLOB live 签名，也用于 Funding API 广播 Polygon 转账。`POLYEDGE_POLYMARKET__FUNDER` 优先作为 Polymarket 入账钱包和 Deposit Wallet 地址，未配置时回退 `ACCOUNT_ID`。

进程级 rewards 默认关闭：`POLYEDGE_REWARDS__ENABLED=false`、`POLYEDGE_WORKER__POLL_REWARD_BOT=false`、`POLYEDGE_WORKER__POLL_REWARD_INFO_RISKS=false`。交易/对账类 worker 也默认关闭；即使开关打开，runtime 仍会在启动 live job 前检查 Polymarket account/private key/API credentials，不完整时只记录 warn 并跳过该 job。

Rewards provider 密钥只从 `RewardsSettings` 读取，不进入 `RewardBotConfig`、API snapshot 或前端 public env。业务层 provider、request format、并发上限、AI strategy hint、信息风险和事件窗口配置保存在 `reward_bot_config`。

## Stores

| 文件 | 实现内容 |
|---|---|
| `catalog/postgres/` | `MarketEventStore` 与 `NewsIngestionStore` 的 Postgres 实现 |
| `catalog/in_memory.rs` | 市场/事件/新闻内存实现 |
| `stores/runtime_config.rs` | runtime config key-value store |
| `stores/mode_state.rs` | 系统模式状态 |
| `stores/risk_state.rs` | 全局风险状态 |
| `stores/idempotency.rs` | 幂等请求状态 |
| `stores/external_event.rs` | 外部事件防重 |
| `stores/audit.rs` | 审计日志 |
| `stores/maintenance.rs` | 数据库 retention 清理 / no-op 实现 |
| `stores/rewards.rs` | `RewardBotStore` trait 实现聚合 |
| `stores/rewards/postgres.rs` | Rewards Postgres 主实现 |
| `stores/rewards/postgres_market_methods.rs` | rewards 市场候选查询、质量硬过滤和 row mapping |
| `stores/rewards/postgres_plans.rs` | quote plan 统计、分页、搜索和排序 SQL |
| `stores/rewards/postgres_orders.rs` | managed orders 分页与查询 |
| `stores/rewards/postgres_control_commands.rs` | rewards 控制命令队列 |
| `stores/rewards/postgres_heartbeat.rs` | rewards worker heartbeat |
| `stores/rewards/postgres_info_risk.rs` | 信息风险缓存 |
| `stores/rewards/postgres_event_windows.rs` | event-window 候选和 effective window 查询 |
| `stores/rewards/postgres_candles.rs` | rewards price-history candle 写入和查询 |
| `stores/rewards/postgres_writes.rs` | rewards 写入辅助，包括 quote plan、fair-value latest/history 和账本状态 |
| `stores/rewards/in_memory.rs` | Rewards 内存实现 |
| `stores/orderbook_cache.rs` | `InMemoryOrderbookCache`，供 orderbook 服务内部使用 |
| `stores/orderbook_registry.rs` | `InMemoryOrderbookSubscriptionRegistry`，供 orderbook 服务内部使用 |
| `stores/helpers/reward_config.rs` | `RewardBotConfig` 与 `reward_bot_config` key-value 行互转 |

### Store 模式

- `runtime_config` 启动时用环境变量值 bootstrap 数据库；环境变量始终优先。
- Postgres maintenance 使用 `WITH doomed AS (SELECT ctid ... LIMIT $n) DELETE ... USING doomed` 分批删除，避免超大事务。
- Rewards 配置以 key-value 保存，覆盖市场质量、quote/selection、dominant 单边、盘口集中度、偏好分类、机会评分、fair-value、adaptive post-fill 退出与 pending-exit 重评、AI advisory、信息风险、事件窗口、首单 gate、库存、requote、reconcile 和 BalancedMerge 参数。
- `RewardBotStore` 维护 `reward_control_commands`，API 入队 pending 命令，worker 使用 claim/complete/fail 处理；running 命令超过 5 分钟可重新领取。
- `RewardBotStore` 维护 `reward_worker_heartbeats`，API snapshot 只在配置启用且最近 2 分钟有 heartbeat 时标记 worker running。
- `reward_market_advisories`、`reward_market_info_risks` 和 `llm_calls` 已接入 Postgres/内存实现；外部 provider 调用按 UTC 日聚合展示。
- `reward_quote_plans` 统计在 Postgres 中直接 SQL 聚合 readiness 与 blocker counts，避免顶部概览反序列化全表 JSON。
- `reward_fair_values` 保存每个 condition 最新 fair-value 估计；`reward_fair_value_history` 追加保存历史估计用于审计/回测，数据库维护默认保留 90 天。
- `reward_positions` 可保存当前 rewards catalog 外的外部库存；外部账户持仓同步成功时会原子替换目标账户全部持仓，失败时保留上一版。
- `reward_merge_intents` 支持 BalancedMerge 配对库存合并、提交 tx hash、失败原因和 retry count。
- `InMemoryOrderbookCache` 写入时排序 bids/asks、裁剪深度、拒绝旧 `observed_at` 覆盖未过期条目，并合并更新的 `confirmed_at`。
- `InMemoryOrderbookSubscriptionRegistry` 对 source 做原子全量替换，最多 32 个 source，聚合优先级为 `rewards_active`、`exec_orders`、`rewards_eligible`、`rewards_ai_provider`、`rewards_candidates`。

## Auth

- `AuthContext`：请求级认证上下文。
- `IdempotencyKey`：幂等键解析。
- `InternalTokenVerifier`：内部 JWT 验证。
- `AuthSettings.disabled=true` 时跳过 console/connector/mode token 和 step-up 校验，注入内部 admin 上下文；仅用于纯内网部署。
- 当前公开敏感写路径主要是系统模式切换和 Funding 转账；关闭免鉴权后 Funding 仍要求 `funding_transfer` step-up scope。

中间件函数：

- `require_connector_write_auth`
- `require_console_read_auth`
- `require_console_write_auth`
- `require_mode_write_auth`

## Runtime

`AppState` 是 API handler、worker 和 orderbook 服务共享的依赖容器，当前包含：

- `settings`、`dependencies`、`runtime_config_store`
- `auth_verifier`、`idempotency_store`、`external_event_store`
- `system_mode_service`、`risk_service`、`execution_service`
- `market_event_service`、`news_ingestion_service`
- `database_maintenance_service`
- `reward_bot_service`
- `orderbook_cache`、`orderbook_registry`

Postgres 可用时使用 Postgres store；无数据库时使用 in-memory/no-op 实现，便于测试和本地最小运行。`PostgresAdvisoryLease` 使用 session advisory lock，rewards live loop 通过它避免多实例并发执行同一账户策略。

## 当前状态

- Postgres 与 in-memory 双实现已覆盖当前 active service。
- 默认 tracing filter 会让 API 内嵌 worker 的 info/warn 日志出现在 API 日志中。
- Funding API 复用 Polymarket settings 中的私钥、funder/account_id 和 Polygon RPC，不新增独立资金私钥配置。
- Orderbook cache/registry 默认仍由 `Runtime` 构建；API/worker 启动时会把它们替换为 `OrderbookHttpClient` 指向独立 orderbook 服务。
- Orderbook 写接口要求 `x-polyedge-orderbook-token` 与 `POLYEDGE_ORDERBOOK__WRITE_TOKEN` 匹配；未配置 token 时写接口关闭，读接口和健康检查仍可用。
- 数据库维护 runtime 只清理当前 schema 中可增长的历史/缓存/队列表，包括 fair-value history。

## 修改检查清单

- [ ] 新增 Store trait 方法后同步实现 Postgres 和 in-memory。
- [ ] 新增配置字段时同步更新 defaults、runtime config 映射、`.env.example` 和部署模板。
- [ ] 修改认证逻辑时检查所有中间件使用点。
- [ ] 修改 `AppState` 字段后检查 `runtime.rs` 构建逻辑和消费者。
- [ ] 运行 `cargo check --workspace --tests`。
