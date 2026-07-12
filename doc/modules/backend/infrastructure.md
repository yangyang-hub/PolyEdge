# infrastructure（基础设施层）

最后更新：2026-07-12

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
| `CorsSettings` | 浏览器 exact-origin allowlist；production 至少配置一项 |
| `DatabaseSettings` | Postgres URL、连接池大小 |
| `RedisSettings` | Redis URL（当前默认 runtime 未强依赖） |
| `RuntimeSettings` | environment、initial_mode |
| `RiskSettings` | legacy 执行链路使用的全局风险/kill switch 状态初始值 |
| `PolymarketSettings` | CLOB/Gamma/Data API/WS/Polygon RPC host、账户、签名类型、私钥、API credentials、轮询限制 |
| `RewardsSettings` | rewards 进程级启用、poll 间隔、AI provider env-only key/base URL/model/timeout、信息风险扫描配置和可选备用 provider |
| `NewsSettings` | 新闻系统启用、间隔、超时、单源数量和 RSS/Atom source 列表 |
| `WorkerSettings` | news、rewards、execution、Polymarket 对账、orderbook 注册、database maintenance 等后台任务开关与间隔 |
| `OrderbookStreamSettings` | orderbook WS/poll/cache、registry token cap、WS target chunk / 最大连接预算、candle history sync 和增量重订配置 |
| `OrderbookServiceSettings` | orderbook 服务端口、service URL、写 token |
| `AuthSettings` | auth disabled、issuer/audience、token TTL、step-up code、密钥和撤销 session |

未配置 `POLYEDGE_NEWS__SOURCES_JSON` 时，`NewsSettings.sources` 使用 `settings/defaults.rs` 中的默认 RSS/Atom 源。`POLYEDGE_POLYMARKET__PRIVATE_KEY` 只允许后端环境配置，既用于 CLOB live 签名，也用于 Funding API 广播 Polygon 转账。`POLYEDGE_POLYMARKET__FUNDER` 优先作为 Polymarket 入账钱包和 Deposit Wallet 地址，未配置时回退 `ACCOUNT_ID`。

进程级 rewards 默认关闭：`POLYEDGE_REWARDS__ENABLED=false`、`POLYEDGE_WORKER__POLL_REWARD_BOT=false`、`POLYEDGE_WORKER__POLL_REWARD_INFO_RISKS=false`。交易/对账类 worker 也默认关闭；即使开关打开，runtime 仍会在启动 live job 前检查 Polymarket account/private key/API credentials，不完整时只记录 warn 并跳过该 job。

Rewards provider 密钥只从 `RewardsSettings` 读取，不进入 `RewardBotConfig`、API snapshot 或前端 public env。业务层 provider、request format、并发上限、AI 风险动作、信息风险和事件窗口配置保存在 `reward_bot_config`；AI/info-risk 动作置信度只使用业务配置，不再有进程级 bps 阈值。

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
| `stores/rewards/postgres_run_ledger.rs` | strategy run / decision / action / order transition ledger 写入和查询 |
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
- 幂等请求把 24 小时结果保留窗口与 5 分钟执行租约分离：`started` 租约过期可由同 payload 新 request owner 原子接管，`complete` / `fail` 同时按 request hash、request id 和 active 状态 fencing；失败错误码持久化在 `error_code`，不会再因 schema 缺列掩盖原始业务失败。
- 外部 callback dedup 使用 5 分钟 `lease_expires_at` 和 trace owner。进程崩溃后未完成事件可重领；旧 owner 不能完成或 abandon 已由新请求接管的事件。
- Postgres maintenance 使用 `WITH doomed AS (SELECT ctid ... LIMIT $n) DELETE ... USING doomed` 分批删除，避免超大事务；strategy run ledger 按 run/transition 分开保留，删除旧 completed/failed/cancelled run 时由 FK 级联清理 decisions/actions。
- Rewards 配置以 key-value 保存，覆盖 `maker_market_budget_usd`、市场质量、动态 quote rank、机会评分、fair-value、adaptive 退出、AI/info-risk 动作阈值、事件窗口、库存偏斜、非对称 requote 和 BalancedMerge。旧 `per_market_usd`、`quote_size_usd`、`cancel_on_fill` key 不再解析或写入。
- Postgres `reward_bot_config` 为空时从 `production_live_drill_defaults()` 装配缺省值；已有 key 逐项覆盖该 profile。首次通过 API 保存配置后仍写入完整 key-value snapshot。In-memory store 保留通用 `Default`，用于测试和无数据库开发。
- `RewardBotStore` 维护 `reward_control_commands`，API 入队 pending 命令，worker 使用 claim/complete/fail 处理；claim 原子写入 5 分钟 `lease_owner` / `lease_version` / `lease_expires_at`，running 租约过期可重领，旧 owner/version 不能覆盖新 worker 的 terminal 结果。
- `RewardBotStore` 维护 `reward_worker_heartbeats`，API snapshot 只在配置启用且最近 2 分钟有 heartbeat 时标记 worker running。
- `reward_market_advisories` 只持久化 V2 action、size multiplier、edge buffer、confidence、reasons/metrics；`reward_market_info_risks` 额外持久化 evidence action 与 taxonomy。旧 AI suitability/quote mode/exit policy 列已从 clean baseline 删除。Provider 调用继续写 `llm_calls` 并按 UTC 日聚合。
- `reward_quote_plans` 按 `(condition_id, strategy_profile)` 保存当前计划，允许 standard 与 BalancedMerge 同市场并存；统计在 Postgres 中直接 SQL 聚合 readiness 与 blocker counts，避免顶部概览反序列化全表 JSON。
- Postgres rewards 候选预排序与 application 基础质量分使用同一 V2 方向：LP 日奖励与 rewards spread 合计最多 10 分，流动性、成交量、剩余时长、实时 spread 和中点质量占主导；最终资金排序仍由 application `selection_score` 完成。
- `reward_quote_plans` 额外持久化 `latest_run_id`、`quote_readiness`、`quote_mode`、`reason_code`、`blocker_codes` 和 provider/fair-value/event 摘要列，JSON 仍保存完整计划快照。
- `RewardBotStore` 维护 `reward_strategy_runs`、`reward_strategy_decisions`、`reward_strategy_actions` 和 `reward_order_transitions`。Postgres 与 in-memory 实现都支持创建/完成/失败 run、批量 upsert 决策和动作、记录订单状态变迁，以及按 run/order 分页查询。
- Strategy action 支持 account-scoped 原子 claim、owner-only lease renew 和 owner-fenced terminal finalize；Postgres 使用 `FOR UPDATE SKIP LOCKED`，仅恢复 planned 或租约明确过期的 executing。`unknown` 与没有租约的历史 executing 不参与自动领取，失去 lease 的 executor 不能覆盖 terminal 结果。
- `reward_strategy_replay_fixtures` 按 run 一对一保存受 8 MiB 上限和敏感字段检查保护的 replay fixture，记录 schema version、SHA-256、JSON 字节数和 captured time。默认写入 schema V2 紧凑 payload，读取保留 V1/缺省 schema 兼容；run 删除时级联清理。
- Merge intent store 支持 `create_merge_intent_if_absent` 和按 id 读取；Postgres 使用 `ON CONFLICT (id) DO NOTHING`，允许 executor 在 lease 恢复后安全重试 create 动作。链上调用前必须原子执行 `pending|unsupported -> broadcasting`，只有 `broadcasting -> submitted` 可写 tx hash；`broadcasting` 不再进入 executable 列表，未知广播结果不会自动重放。已提交链上 merge 只能用匹配的 `(intent_id, tx_hash)` receipt 更新为 completed/failed，防止陈旧 receipt 串写其他交易；`completed` 不再永久占用后续可配对库存。
- Provider cache 的 `expires_at` 只表示有效期边界，最新 advisory/info-risk 固定按 `created_at DESC, id DESC` 选择，避免旧长 TTL 结果覆盖新风险评估。
- 高频 rewards order、position、account、heartbeat、event-window、fair-value、strategy decision/action 和 merge-intent upsert 使用 `updated_at` / `observed_at` fencing；外部账户全量持仓替换先锁定账户版本，旧 snapshot 不会删除或覆盖新状态。Terminal strategy action 与 merge broadcast/receipt 状态也禁止被迟到的普通 upsert 回退。
- `reward_fair_values` 保存每个 condition 最新 fair-value 估计。Postgres 与 in-memory store 在写入前都按 condition 选择最新 latest，并按 `(condition_id, source, observed_at)` 对 history 幂等去重；同一 identity 却 payload 不同时拒绝整批写入。历史默认保留 90 天。
- `reward_positions` 可保存当前 rewards catalog 外的外部库存；外部账户持仓同步成功时会原子替换目标账户全部持仓，失败时保留上一版。
- `reward_merge_intents` 支持 BalancedMerge 配对库存合并、提交 tx hash、失败原因和 retry count。
- `InMemoryOrderbookCache` 写入时排序 bids/asks、裁剪深度、拒绝旧 `observed_at` 覆盖未过期条目；旧写入只有盘口内容完全一致时才可隐式合并 `confirmed_at`。poll reconcile 可在兼容性检查后用版本 fenced 的 `confirm_book_version` 只推进指定当前版本。
- Rewards candle store 支持批量 sample 写入；Postgres 通过 typed `UNNEST`、bucket 聚合与一次 upsert 持久化单 token history 响应，并过滤不晚于现有 close 的重叠点，避免重复累计 `sample_count`。
- `InMemoryOrderbookSubscriptionRegistry` 对 source 做原子全量替换，最多 32 个 source，聚合优先级为 `rewards_active`、`exec_orders`、`rewards_eligible`、`rewards_candidates`。Provider 不依赖 live orderbook，临时 `rewards_ai_provider` source 已删除。
- `OrderbookStreamSettings.ws_chunk_size` 默认 500，`ws_max_connections` 默认 8；orderbook 服务用两者计算有效 chunk，使旧的小 chunk runtime 值也不会突破 Polymarket market-channel 连接预算。

## Auth

- `AuthContext`：请求级认证上下文。
- `IdempotencyKey`：幂等键解析。
- `InternalTokenVerifier`：内部 JWT 验证。
- `AuthSettings.disabled=true` 时跳过 console/connector/mode token 和 step-up 校验，注入内部 admin 上下文；仅用于纯内网部署。
- Production 且 auth enabled 时必须至少配置一个 Ed25519 JWT verification key，否则 API 在 bind 前 fail-fast。`step_up_code` 只供 local dev bypass；production step-up 来自可信签发方写入短时 JWT 的 scope/expiry claims。
- 当前公开敏感写路径包括系统模式切换、Funding 转账和 Rewards 交易控制；关闭免鉴权后分别使用 system/funding/rewards 专用 step-up scope。Rewards run、启用 live trading、启用 BalancedMerge 自动执行和 reset 不能复用一个宽泛 scope。
- `AppState` 暴露统一 `audit_log_sink` 给薄 API handler，Rewards 配置/控制写入会持久化 actor、operator note、action 和结果。

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
- API CORS 只允许 `CorsSettings.allowed_origins` 中的精确 `http(s)://host[:port]`；`*`、路径、query 和 production 空列表都会 fail-fast。
- Funding API 复用 Polymarket settings 中的私钥、funder/account_id 和 Polygon RPC，不新增独立资金私钥配置。
- Orderbook cache/registry 默认仍由 `Runtime` 构建；API/worker 启动时会把它们替换为 `OrderbookHttpClient` 指向独立 orderbook 服务。
- Orderbook 写接口要求 `x-polyedge-orderbook-token` 与 `POLYEDGE_ORDERBOOK__WRITE_TOKEN` 匹配；未配置 token 时写接口关闭，读接口和健康检查仍可用。
- Orderbook WS 默认通过 8 连接硬预算收敛分片，配合 chunk 启动错峰和 SDK 长退避降低 Cloudflare 429/1015 风暴；`ws_max_connections` 已进入环境变量解析和 runtime config 映射。
- 数据库维护 runtime 只清理当前 schema 中可增长的历史/缓存/队列表，包括 fair-value history、strategy run ledger 和 order transitions。

## 修改检查清单

- [ ] 新增 Store trait 方法后同步实现 Postgres 和 in-memory。
- [ ] 新增配置字段时同步更新 defaults、runtime config 映射、`.env.example` 和部署模板。
- [ ] 修改认证逻辑时检查所有中间件使用点。
- [ ] 修改 `AppState` 字段后检查 `runtime.rs` 构建逻辑和消费者。
- [ ] 运行 `cargo check --workspace --tests`。
