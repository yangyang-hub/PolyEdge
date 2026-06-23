# infrastructure（基础设施层）

最后更新：2026-06-23

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
| `settings.rs` + `settings/` | 配置管理：`Settings`、所有子配置、默认值和 runtime config 条目 |
| `stores.rs` + `stores/` | Store trait 的持久化实现 |
| `catalog/` | 核心市场/事件/信号/执行存储（最大子模块） |
| `auth.rs` + `auth/` | 认证中间件和令牌验证 |
| `runtime.rs` | `AppState`、`Runtime`、`RuntimeDependencies` |
| `http.rs` | `HttpError`、hash/trace 辅助函数 |
| `telemetry.rs` | 结构化日志初始化；默认 filter 覆盖 API 内嵌 worker info 日志 |

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
| `PolymarketSettings` | account_id、chain_id、signature_type、funder、private_key、API credentials、CLOB/WS/Gamma/Data API/Polygon RPC host、poll limits |
| `ArbitrageSettings` | book_source、scan_limit、scanner_version |
| `RewardsSettings` | enabled、poll_interval、AI provider API key/base URL/model/timeout/min confidence、信息风险扫描间隔/每轮 condition cap/置信度/web search 开关等 |
| `NewsSettings` | enabled、poll_interval_secs、request_timeout_secs、max_items_per_source、sources（RSS/Atom 源列表） |
| `WorkerSettings` | 各 worker 的启用标志和轮询间隔，包含数据库维护开关 `database_maintenance` 与 `database_maintenance_interval_secs` |
| `OrderbookStreamSettings` | WS 连接、轮询和 rewards candle history 配置（默认 `max_tokens=3000`、`reward_candidate_token_cap=50`、`ws_chunk_size=100`、`poll_reconcile_interval_secs=10`、`max_levels_per_side=100`、candle history enabled、interval=300s、request_delay=500ms、max_tokens_per_cycle=600） |
| `OrderbookServiceSettings` | orderbook HTTP port/service_url；`write_token` 控制 register/ingest/delete 内部写认证 |
| `AuthSettings` / `AuthKeySettings` | 认证配置和密钥；`disabled` 可开启内网免鉴权模式 |
| `CopytradeSettings` | 跟单配置 |

所有字段使用 `#[serde(default)]`，通过 `POLYEDGE_` 前缀环境变量加载（如 `POLYEDGE_SERVER__PORT`）。`packages/backend/.env.example` 只保留本地运行常用项和安全关闭 worker 循环的必要覆盖；完整默认值以 `settings/defaults.rs` 为准，业务阈值优先通过 runtime_config/Settings 调整。未配置 `POLYEDGE_NEWS__SOURCES_JSON` 时，`NewsSettings.sources` 默认包含 8 个已验证可访问的 RSS/Atom 源：`fed_press`、`sec_press`、`nasa_news`、`bbc_world`、`npr_news`、`coindesk`、`cointelegraph`、`decrypt`；设置该变量或 runtime config `news.sources_json` 会覆盖整个列表。`POLYEDGE_POLYMARKET__SIGNATURE_TYPE` 支持 `eoa`、`proxy`、`gnosis_safe`、`poly_1271`；Deposit Wallet 使用 `poly_1271` 并通过 `POLYEDGE_POLYMARKET__FUNDER` 配置 deposit wallet 地址。`POLYEDGE_POLYMARKET__POLYGON_RPC_URL` 用于 worker 读取资金钱包链上 pUSD 余额，默认 `https://polygon-bor-rpc.publicnode.com`。

进程级 rewards 默认关闭：`RewardsSettings.enabled=false`、`WorkerSettings.poll_reward_bot=false`、`WorkerSettings.poll_reward_info_risks=false`。其他历史 worker 循环在代码默认值中仍可能为 true，因此本地模板和部署侧 `deploy/.env.api.example` 会显式写入 `false` 防止 `polyedge-api` 内嵌 runtime 意外启动任务。只有部署环境显式同时开启对应 worker 开关时才启动 live poll loop 或异步信息风险扫描。AI provider 密钥只在 `RewardsSettings` 中读取（`POLYEDGE_REWARDS__AI_OPENAI_API_KEY` / `POLYEDGE_REWARDS__AI_ANTHROPIC_API_KEY`），不会进入 `RewardBotConfig`、API snapshot 或前端 public env；默认模型 `gpt-4.1-mini`、AI advisory 最低置信度 `5500` bps、信息风险最低置信度 `7000` bps、单次超时 180 秒。`ai_advisory_enabled=true` 时，缺少可用 provider 配置或未达到最低置信度的 advisory 会阻断新增 rewards 挂单。AI advisory 每轮最大市场数环境变量已移除；`POLYEDGE_REWARDS__INFO_RISK_MAX_MARKETS_PER_CYCLE` 现在作为 AI advisory/info-risk 统一 provider refresh 与独立 info-risk worker 的每轮 condition cap，默认 50，0 表示本轮不发 provider 请求。信息风险 OpenAI web search 默认关闭。

Orderbook candle history 默认由 orderbook 服务独立启用：`POLYEDGE_ORDERBOOK_STREAM__REWARD_CANDLE_HISTORY_ENABLED=true`、`SYNC_INTERVAL_SECS=300`、`REQUEST_DELAY_MS=500`、`MAX_TOKENS_PER_CYCLE=600`、`BACKFILL_SECS=7200`、`INCREMENTAL_SECS=900`。这些字段控制 CLOB `/prices-history` 请求节奏，用于替代原先从本地高频 orderbook 更新派生 candles 的路径；max tokens 设为 0 会跳过本轮外部请求。

另有 `runtime_config` 子模块支持运行时动态配置。

### Stores — 持久化实现

**文件结构**（通过 `include!` 内联）：

| 实现文件 | 对应 trait | 存储 |
|---|---|---|
| `catalog/postgres/` | `MarketEventStore`、`ArbitrageStore`、`NewsIngestionStore` | PostgreSQL |
| `catalog/postgres/market_event/market_queries.rs` | `MarketEventStore` 市场查询 helper | 市场列表/计数/分类/详情/按 id 批量读取 SQL；从通用 `queries.rs` 拆分 |
| `catalog/in_memory.rs` | 同上 | 内存（RwLock） |
| `stores/rewards/postgres.rs` | `RewardBotStore` | PostgreSQL（key-value config + 完整表） |
| `stores/rewards/postgres_market_methods.rs` | Rewards Postgres 市场查询 helper | 质量硬过滤、综合排序、row mapping |
| `stores/rewards/postgres_control_commands.rs` / `postgres_heartbeat.rs` / `postgres_orders.rs` / `postgres_writes.rs` | `RewardBotStore` 辅助 | 控制命令、worker heartbeat、订单分页 SQL、旧 reserved 释放辅助 |
| `stores/rewards/postgres_info_risk.rs` | `RewardBotStore` 辅助 | 信息风险缓存查询和写入 SQL |
| `stores/rewards/postgres_low_competition.rs` | `RewardBotStore` 辅助 | 低竞争 sleeve observation 写入和最近窗口查询 SQL |
| `stores/rewards/postgres_candles.rs` | `RewardBotStore` 辅助 | rewards price-history candle upsert 和最近 K 线查询 SQL |
| `stores/rewards/in_memory.rs` | 同上 | 内存 |
| `stores/copytrade/postgres.rs` | `CopyTradeStore` | PostgreSQL（key-value config、tracked wallets、source trades、events、控制命令；旧模拟表兼容） |
| `stores/copytrade/postgres_control_commands.rs` / `postgres_rows.rs` / `postgres_writes.rs` | `CopyTradeStore` 辅助 | 控制命令 SQL、行映射、写入辅助 |
| `stores/copytrade/in_memory.rs` | 同上 | 内存 |
| `stores/mode_state.rs` | `ModeStateStore` | PostgreSQL/内存 |
| `stores/risk_state.rs` | `RiskStateStore` | PostgreSQL/内存 |
| `stores/idempotency.rs` | `IdempotencyStore` | PostgreSQL/内存 |
| `stores/audit.rs` | `AuditLogSink` | PostgreSQL/内存 |
| `stores/maintenance.rs` | `DatabaseMaintenanceStore` | PostgreSQL 分批清理 / 无数据库 no-op |
| `stores/orderbook_cache.rs` | `OrderbookCache` | 内存（TTL + 定期清理 + 每侧盘口深度裁剪 + `entry_count` 真实条目统计）— 仅供 orderbook 服务内部使用；Worker/API 通过 `OrderbookHttpClient` 远程访问 |
| `stores/orderbook_registry.rs` | `OrderbookSubscriptionRegistry` | 内存（来源有序 token 原子替换 + 确定性优先级聚合 + 来源/去重总数统计）— 仅供 orderbook 服务内部使用；Worker 通过 HTTP 注册 token |
| `stores/orderbook_registry_tests.rs` | Registry 回归测试 | 原子 source 替换、优先级和跨 source 去重 |
| `stores/orderbook_cache_tests.rs` | Cache 回归测试 | 最优档排序/裁剪、批量写入和 stale threshold 语义 |
| `stores/rewards_tests.rs` | Rewards store 回归测试 | running 控制命令租约、重复命令合并、历史清理边界、账户持仓完整替换与失败保留 |
| `stores/runtime_config.rs` | 运行时配置 | PostgreSQL key-value |
| `stores/helpers.rs` | DB 行映射辅助 | — |
| `stores/helpers/reward_config.rs` | Rewards key-value 配置读写 helper | `RewardBotConfig` 与 `reward_bot_config` key-value 行互转 |
| `stores/types.rs` | 共享类型 | — |

**关键模式：**
- Config 存储使用 key-value 表（`reward_bot_config`、`copytrade_config`、`runtime_config`）
- `runtime_config` bootstrap 在每次启动时用环境变量值覆盖数据库值（`ON CONFLICT ... DO UPDATE ... WHERE value IS DISTINCT FROM EXCLUDED.value`），确保环境变量始终优先；API 运行时修改仅在当前进程生命周期内生效，重启后恢复为环境变量值
- 常量：`SYSTEM_RUNTIME_STATE_ID = "global"`、`RISK_STATE_ID = "global"`
- `db_error(code, context)` 辅助函数统一创建 `dependency_unavailable` 错误
- `PostgresDatabaseMaintenanceStore` 使用 `WITH doomed AS (SELECT ctid ... LIMIT $n) DELETE ... USING doomed` 模式分批删除，每个表每轮最多 20 批、每批 10,000 行，避免单次超大事务；无 Postgres 环境使用 `NoopDatabaseMaintenanceStore`
- `RewardBotStore` 的 Postgres key-value 配置读写覆盖全部 rewards 报价/风控配置字段（市场质量门槛、`quote_bid_rank`、quote/selection mode、dominant probability/depth/concentration、preferred categories、低竞争 sleeve mode/额度/竞争/退出/稳定性阈值、AI advisory 开关/provider/request format/TTL/批量大小、信息风险开关/mode/过滤等级/TTL/批量大小、depth/rank/velocity/requote/reconcile，以及 `requote_drift_confirm_sec` / `requote_drift_cooldown_sec` / `requote_drift_max_cancels_per_cycle` 换价 guard）；`execution_mode`、`quote_edge_cents`、`reward_competition_factor`、`single_sided_divisor_c`、`fill_rate_per_tick`、`max_fill_ratio` 和 `auto_cancel_stale_minutes` 旧键保留用于向后兼容但被忽略。`quote_bid_rank` 保存为 1–3，默认 1；`exit_markup_cents` 默认 0，表示 `exit_at_markup` 成交后按被吃买单原价挂卖；`ai_advisory_batch_size` / `info_risk_batch_size` 保存为 1–12，默认 1 表示逐市场 provider 请求。
- `RewardBotStore.latest_market_advisory()` 和 `save_market_advisory()` 在 Postgres 与内存实现中读写 `reward_market_advisories`；缓存 key 为 condition/provider/request_format/model/input_hash，且只返回 `expires_at > now` 的记录。Postgres 行映射 helper 解析 suitability、quote mode、exit policy、reasons JSON 和 metrics JSON。
- `RewardBotStore.latest_market_info_risk(s)` 和 `save_market_info_risk()` 在 Postgres 与内存实现中读写 `reward_market_info_risks`；请求缓存 key 为 condition/provider/request_format/model/input_hash，批量读取按 condition 返回最新未过期记录。Postgres 行映射 helper 解析 risk level/type/direction、sources JSON 和 metrics JSON。
- `RewardBotStore.record_low_competition_observations()` 和 `list_low_competition_observations()` 在 Postgres 与内存实现中读写 `reward_low_competition_observations`；Postgres 通过 `(account_id, observed_at DESC)` 索引读取最近窗口，用于 API snapshot 生成低竞争 shadow report，不会自动修改配置。
- `RewardBotStore.prune_history(cutoff)` 在 Postgres 中使用单事务清理 cutoff 之前的终态 managed orders（`cancelled`/`filled`/`error`）、`reward_risk_events` 和 `reward_low_competition_observations`；内存实现保持相同语义。该清理不会删除 `planned`/`open`/`exit_pending` 订单、`reward_fills`、`reward_positions` 或 `reward_account_state`，避免破坏 live 对账和账本。
- `RewardBotStore.record_market_candle_sample()` 和 `list_recent_market_candles()` 在 Postgres 与内存实现中读写 `reward_market_candles`；Postgres 根据 `reward_markets.tokens_json` 把 token 映射到 active reward market，按 `(token_id, interval_sec, bucket_start)` upsert price-history OHLC，同一 close timestamp 的重复写入不增加 `sample_count`，AI advisory 按 condition/interval/window 读取最近 K 线。
- `RewardBotStore` 支持按 external Polymarket order id 查询 rewards managed order、通过 fill id 判断成交是否已入账，并通过 `latest_fill_at(account_id)` 查询最近 confirmed fill；live worker 用这些读路径完成托管订单成交幂等同步和外部账户快照保护。
- `RewardBotStore.cancel_open_orders()` 在 Postgres/内存实现中兼容释放旧账本的 `reserved_usd`；新的 rewards 开放买单不再逐单硬占用资金，订单列表优先返回 open-like 状态，避免大量历史成交/撤单淹没当前开放挂单。worker 的 `list_open_orders()` 仍包含本地 planned/exit intent；控制台 `status.open_orders` 使用独立的 external count，只统计已有 `external_order_id` 的 open-like 订单。
- `RewardBotStore.list_orders_page()` 在 Postgres 实现中通过 count + limit/offset 做服务端分页，支持 outcome/condition/token 搜索、状态过滤和 price/size/status 排序；内存实现保持相同语义。
- `RewardBotStore.list_markets(limit)` 只返回 active reward markets；Postgres candidate query 关联 Gamma `markets`，硬过滤非 open/tradable、高歧义、低 liquidity、低 `volume_24h`、临近 `end_at`、Gamma spread 过宽、`synced_at` 过期或异常超前、奖励不足、奖励 spread 无效及非唯一 YES/NO token 市场。默认 midpoint 仍限制在常规双边区间；auto 单边允许时，查询会额外允许 `dominant_min_probability..dominant_max_probability` 及反向区间进入候选。候选查询不再用 `rewards_min_size <= per_market_usd` 预筛预算，高最小份额市场会保留到 live materializer 和实际钱包余额准入层处理。综合排序按 CLOB 原始 cents 直接使用 rewards spread，不做会缩小 99c 等合法值的单位换算，并把 Gamma `category` 映射到 `RewardMarket.category` 供 planner 做偏好分类加分。Gamma 同行的有效、未交叉 best bid/ask 可在 reward token 缺 price 时注入 YES midpoint 和 NO complement 作为候选规划回退；内存实现同时校验唯一 YES/NO、midpoint/dominant 区间和同步时间。`upsert_markets()` 对 reward catalog 先 upsert 当前快照、再只停用缺失的 active rows，且 unchanged rows 最多每小时 touch 一次，避免每轮全量 active=false/true 写放大；`save_quote_plans()` 会替换当前 quote plan 快照，避免旧的全量计划继续出现在 `/rewards`。Postgres 查询实现拆分到 `postgres_market_methods.rs`。
- Postgres `RewardBotStore.apply_tick_outcome()` 会在同一事务中只持久化 orders、fills、positions、account ledger 和 events；reward market 全量目录只由 `upsert_markets()` 更新，quote plan 快照只由 `save_quote_plans()` 替换，避免增量 live tick 误停用全量奖励市场。
- Postgres/内存 `RewardBotStore.apply_account_sync()` 会更新账户状态；外部 positions 成功时原子替换目标账户全部 `reward_positions`，失败时通过 `None` 保留上一版持仓。worker 会结合 `latest_fill_at` 在 confirmed fill 后 120 秒内跳过整次外部账户替换，避免最终一致性响应回滚本地现金或库存。外部持仓可来自当前 rewards catalog 之外的市场，因此 `reward_positions.condition_id` 不再依赖 `reward_markets` 外键。
- `RewardBotStore` 在 Postgres/内存实现中维护 `reward_control_commands` 队列；API 写入 pending 命令时，store 会合并同账户同动作且仍为 pending/running 的重复命令，Postgres 侧由 partial unique index 兜底防止并发重复入队；worker 使用 claim/complete/fail 方法领取并更新执行状态；running 命令超过 5 分钟会重新进入可领取范围。
- `RewardBotStore` 在 Postgres/内存实现中按 account 维护 rewards worker heartbeat；API snapshot 只把配置已启用且最近 2 分钟有 heartbeat 的 worker 标记为 running。Postgres 表由迁移 `0032_reward_worker_heartbeats.sql` 创建。
- `CopyTradeStore` 在 Postgres/内存实现中维护 `copytrade_control_commands` 队列；API 写入 pending 命令，worker 使用 claim/complete/fail 方法领取并更新执行状态。当前 copytrade worker 只执行 source trade 检测和 Analyze 钱包分析；run/cancel/reset 兼容命令不再触发模拟交易。
- `InMemoryOrderbookCache` 在所有写入入口统一按 bids 降序、asks 升序排序后裁剪，确保无序 WS/poll/ingest 数据也保留 top-of-book；写入时间戳早于当前未过期条目的盘口会被忽略，相同时间戳下 WS 优先于 poll，已过期条目不会阻挡后续较旧 `observed_at` 的 poll/ingest 快照恢复；`get_books()` 在一次读锁内返回多个未过期盘口；`get_stale_tokens(..., max_age_ms <= 0)` 只检查 TTL，不执行年龄 stale 检查。
- `InMemoryOrderbookSubscriptionRegistry.register_tokens()` 在持有写锁时原子执行 32-source 上限检查，关闭并发新 source 绕过 HTTP 预检查的竞态；空 token 集合会删除 source，聚合优先级为 `rewards_active`、`exec_orders`、`rewards_eligible`、`rewards_ai_provider`、`rewards_low_competition_probe`、`rewards_candidates`、`copytrade`。

### Catalog — 核心数据存储

**Postgres 实现**（`catalog/postgres/`）：通过 `include!` 拆分为多个子文件
- `market_event/` — 最大的存储模块，包含 queries、execution_updates 等
- `arbitrage.rs` — 套利数据存储；扫描历史按 retention 分批删除旧 `arbitrage_scans`，通过 FK cascade 清理 `market_book_snapshots` / opportunities
- `helpers/` — 共享辅助文件：`fetch.rs`、`market_rows.rs`、`news_rows.rs`、`arbitrage_rows.rs`、`event_rows.rs`、`execution_rows.rs`、`calculations.rs`

**In-memory 实现**（`catalog/in_memory.rs`，~24KB）：用于测试和无数据库环境

Arbitrage store 的 Postgres 和 in-memory 实现均支持 `prune_arbitrage_scan_history()`；Postgres 每次最多执行 20 批、每批 250 个旧 scan 的删除，先统计将被级联删除的 snapshots/opportunities，再删除 scan，避免单次超大事务；in-memory 实现同步移除对应 snapshots、opportunities 和 validations。

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
- **`PostgresAdvisoryLease`**：持有专用 Postgres session advisory lock；正常结束显式 unlock，异常 drop 时关闭连接释放锁。rewards poll loop 在整个生命周期持有同一 live lease，保证多实例中只有一个执行者和一条 CLOB heartbeat 链；一次性命令使用同一 lease，使用时要求 `postgres.max_connections >= 2`

### HTTP 工具

- `HttpError`：HTTP 错误映射（AppError → HTTP status code）
- `hash_json(value)`：JSON 值哈希
- `new_trace_id()`：生成 trace ID
- `request_id_from_headers(headers)`：从请求头提取 request ID

## 依赖关系

- **上游**：`domain`（枚举、错误类型）、`application`（Store trait、Service struct）
- **下游**：`packages/api`（使用 AppState、auth middleware）、`packages/orderbook`（使用 AppState、Runtime 和 orderbook stores）、`packages/backend/apps/worker`（使用 AppState、Store 实现）

## 当前状态

- Postgres 和 in-memory 双实现均已就绪
- 配置通过环境变量加载，支持 `.env` 文件
- 未设置 `RUST_LOG` 时，默认 tracing filter 为 `{service_name}=debug,polyedge_worker=info,tower_http=info,sqlx=info`，因此 `polyedge-api` 内嵌 worker runtime 的 info/warn 日志会出现在 API 服务日志中；显式设置 `RUST_LOG` 会覆盖该默认值
- 新闻源默认值已内置在 `settings/defaults.rs`；部署模板默认开启 news 子系统和 worker poll loop，会抓取模板中显式配置的默认 RSS/Atom 源，新闻提升为 events/evidences 仍默认关闭
- 认证中间件支持 JWT/dev-auth 和内网免鉴权模式；当前部署模板默认 `POLYEDGE_AUTH__DISABLED=true`
- Orderbook cache 当前 runtime 使用进程内 `InMemoryOrderbookCache`；Redis 实现保留但未接入默认 runtime
- Orderbook 服务的 `/orderbook/stats` 现在区分真实 cache 条目数、registry 来源数和 registry 去重 token 总数，避免把订阅 token 数误报为缓存条目数；worker 注册 rewards 候选预热 token 受 `POLYEDGE_ORDERBOOK_STREAM__REWARD_CANDIDATE_TOKEN_CAP` 限制，默认 50，设为 0 后周期注册任务会按空结果防抖清空候选预热 source
- Orderbook 进程内缓存会先保留最优价格顺序，再按 `POLYEDGE_ORDERBOOK_STREAM__MAX_LEVELS_PER_SIDE` 裁剪每侧 bids/asks 深度，默认 100 档；HTTP register/batch/ingest 入口使用 `max_tokens` 做请求规模上限，Polymarket WS 订阅使用 `POLYEDGE_ORDERBOOK_STREAM__WS_CHUNK_SIZE` 控制每条连接承载的 token 数（默认 100），poll reconcile 默认 10 秒，register 会原子替换对应 source 当前有序 token 集合，worker 对周期注册空集合做 active/exec 2 轮、eligible/candidates 3 轮防抖后才发送空集合清源，ingest 会先校验整批数据再批量写入并传播缓存错误，registry source 固定上限为 32 个
- Orderbook 缓存拒绝旧 `observed_at` 覆盖未过期条目，但已过期条目可被后续写入恢复；rewards 控制命令具备 5 分钟 running lease，并会合并 pending/running 重复命令；Postgres rewards live worker 通过 advisory lease 避免多实例并发执行
- Rewards managed order upsert 会更新后续实际提交的 `price` / `size` / `strategy_bucket`，保证 flatten 改价、CLOB 数量调整、未知提交恢复和低竞争 bucket 统计使用持久化后的真实参数
- Rewards store 已持久化 quote/selection mode、dominant 单边阈值、盘口集中度阈值、偏好分类、低竞争 sleeve 配置、AI advisory 配置和信息风险配置；`reward_market_advisories`、`reward_market_info_risks`、`reward_low_competition_observations` 与 `reward_market_candles` 表已由迁移创建，并已接入 Postgres/内存读写，供 worker 跳过重复模型判断、生成低竞争 shadow report，并向 AI advisory 提供 price-history K 线。
- 数据库维护 store 已接入 runtime：Postgres 环境定期清理 raw events、AI/info-risk cache、reward candles、控制命令、copytrade 历史、outbox/external dedup、LLM call、audit 和 mode transition 历史；in-memory/test runtime 使用 no-op，避免测试状态被后台任务改变。
- Rewards store 已支持外部账户余额和完整持仓快照同步；成功空持仓快照会清空目标账户持仓，失败响应不会破坏上一版，最近 confirmed fill 时间用于 worker 的 120 秒账户快照保护；worker 写入的资金钱包地址优先使用 `FUNDER`，CLOB 余额为 0/失败时可用 Polygon pUSD 链上余额回填 snapshot
- `markets` 保存 Gamma `liquidity_usd`、`end_at` 和本地 `synced_at`；Postgres market upsert 使用单条 `INSERT .. ON CONFLICT DO UPDATE WHERE` 表达新增、真实数据变化更新和 freshness-only 刷新，返回实际写入行数，并在每批事务内设置短 `lock_timeout` / `statement_timeout`。默认调用仍刷新 `synced_at`，orderbook full sync 通过 `MarketUpsertOptions` 只刷新超过新鲜度阈值的安静市场，priority sync 继续强制刷新重点市场，避免 rewards 关键市场因目录新鲜度过低被误判。
- MarketEventStore 的 Postgres 实现支持 `get_markets_by_ids()` 通过 `m.id = ANY($1)` 批量读取少量相关市场，API 风险快照用它替代全量 markets 列表，避免控制台风险页在大市场表上触发 `LIMIT 65535` 的慢查询。
- `idx_markets_reward_quality` 不包含高频变化的 `synced_at`，降低 freshness-only 刷新对索引和 WAL 的写放大；`idx_markets_polymarket_yes_asset_id` / `idx_markets_polymarket_no_asset_id` 支撑 orderbook priority sync 的注册 token 到 condition id 反查；rewards 候选查询仍在关联 Gamma `markets` 后按 `synced_at` 做新鲜度过滤。
- Orderbook register/ingest/delete 写接口要求 `x-polyedge-orderbook-token` 与 `POLYEDGE_ORDERBOOK__WRITE_TOKEN` 匹配；该密钥仅配置在 `deploy/.env.orderbook` 和 `deploy/.env.api`，未配置 token 时写接口关闭，读接口和健康检查仍可用

## 修改检查清单

- [ ] 新增 Store trait 方法后，在 postgres 和 in_memory 实现中同步添加
- [ ] 修改数据库查询后，运行 `cargo test --workspace`
- [ ] 新增配置字段时，同步更新对应的 `deploy/.env.{api,orderbook,front}.example`
- [ ] 修改认证逻辑时，检查所有中间件函数的使用点
- [ ] 修改 `AppState` 字段后，检查 `runtime.rs` 的构建逻辑和所有消费方
- [ ] 运行 `cargo check --workspace --tests`
