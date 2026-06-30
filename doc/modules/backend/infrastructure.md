# infrastructure（基础设施层）

最后更新：2026-06-30

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
| `catalog/` | 核心市场/事件/执行历史存储（最大子模块，保留 legacy signal/position 表访问） |
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
| `RiskSettings` | exposure_reference_nav、daily_pnl limits、gross/net exposure limits、kill_switch、min_signal_confidence 等 legacy 执行风险字段；旧前端风控配置面板/API 已移除 |
| `PolymarketSettings` | account_id、chain_id、signature_type、funder、private_key、API credentials、CLOB/WS/Gamma/Data API/Polygon RPC host、poll limits |
| `RewardsSettings` | enabled、poll_interval、OpenAI-compatible/Anthropic API key 与 base URL、AI model/timeout/min confidence、可选备用 provider（第二个独立 provider/model/key/URL；GLM/DeepSeek/Agnes 通过模型名识别）、信息风险扫描间隔、每轮 provider 上限、置信度和 web search 开关等 |
| `SmartMoneySettings` | Smart Money signal advisory 独立 OpenAI-compatible/Anthropic API key、base URL 和请求超时；provider/request format/model 保存在 Smart Money config |
| `NewsSettings` | enabled、poll_interval_secs、request_timeout_secs、max_items_per_source、sources（RSS/Atom 源列表） |
| `WorkerSettings` | 各 worker 的启用标志和轮询间隔，包含数据库维护开关、Smart Money scan 开关与间隔 |
| `OrderbookStreamSettings` | WS 连接、轮询和 rewards candle history 配置（默认 `max_tokens=3000`、`reward_candidate_token_cap=50`、`ws_chunk_size=100`、`poll_reconcile_interval_secs=10`、`max_levels_per_side=100`、candle history enabled、interval=300s、request_delay=500ms、max_tokens_per_cycle=600） |
| `OrderbookServiceSettings` | orderbook HTTP port/service_url；`write_token` 控制 register/ingest/delete 内部写认证 |
| `AuthSettings` / `AuthKeySettings` | 认证配置和密钥；`disabled` 可开启内网免鉴权模式 |
| `CopytradeSettings` | 跟单配置 |

所有字段使用 `#[serde(default)]`，通过 `POLYEDGE_` 前缀环境变量加载（如 `POLYEDGE_SERVER__PORT`）。`packages/backend/.env.example` 只保留本地运行常用项和安全关闭 worker 循环的必要覆盖；完整默认值以 `settings/defaults.rs` 为准，业务阈值优先通过 runtime_config/Settings 调整。未配置 `POLYEDGE_NEWS__SOURCES_JSON` 时，`NewsSettings.sources` 默认包含 8 个已验证可访问的 RSS/Atom 源：`fed_press`、`sec_press`、`nasa_news`、`bbc_world`、`npr_news`、`coindesk`、`cointelegraph`、`decrypt`；设置该变量或 runtime config `news.sources_json` 会覆盖整个列表。`POLYEDGE_POLYMARKET__SIGNATURE_TYPE` 支持 `eoa`、`proxy`、`gnosis_safe`、`poly_1271`；Deposit Wallet 使用 `poly_1271` 并通过 `POLYEDGE_POLYMARKET__FUNDER` 配置 deposit wallet 地址。`POLYEDGE_POLYMARKET__PRIVATE_KEY` 只允许配置在后端/API 环境，除了 CLOB live 签名外，也被 Funding API 用于从后端资金钱包广播 Polygon ERC-20 入金交易；`POLYEDGE_POLYMARKET__FUNDER` 优先作为 Polymarket 入账钱包，未配置时 Funding API 回退 `ACCOUNT_ID`。`POLYEDGE_POLYMARKET__POLYGON_RPC_URL` 用于 worker 读取资金钱包链上 pUSD 余额，也用于 Funding API 广播 Polygon 转账，默认 `https://polygon-bor-rpc.publicnode.com`。

进程级 rewards 默认关闭：`RewardsSettings.enabled=false`、`WorkerSettings.poll_reward_bot=false`、`WorkerSettings.poll_reward_info_risks=false`。execution drain、paper 对账、Polymarket 私有订单/成交/用户 WS worker 的代码默认值也为关闭，避免缺实盘账户配置时 API 内嵌 runtime 自动启动交易/对账循环。Smart Money 定时扫描也默认关闭：`WorkerSettings.poll_smart_money=false`，启用后还要求数据库中的 `SmartMoneyConfig.enabled=true` 才会抓 Polymarket Data API；扫描间隔 `smart_money_interval_secs` 默认 900 秒。本地模板和部署侧 `deploy/.env.api.example` 显式关闭除新闻采集和数据库维护之外的后台循环，防止 `polyedge-api` 内嵌 runtime 意外启动交易/分析任务。旧 signal recompute 与 arbitrage worker 配置字段已移除。只有部署环境显式同时开启对应 worker 开关时才启动 live poll loop、异步信息风险扫描或 Smart Money scan；即使开关被打开，worker runtime 也会在启动 live Polymarket 常驻任务前检查 `POLYEDGE_POLYMARKET__ACCOUNT_ID`、`POLYEDGE_POLYMARKET__PRIVATE_KEY` 和可选三项 API credential 的完整性，不完整时只记录一次 warn 并跳过对应 job。Rewards AI provider 密钥只在 `RewardsSettings` 中读取（`POLYEDGE_REWARDS__AI_OPENAI_API_KEY` / `POLYEDGE_REWARDS__AI_ANTHROPIC_API_KEY`，以及可选 fallback key），不会进入 `RewardBotConfig`、API snapshot 或前端 public env；Rewards 主/备 provider 并发限制保存在 `RewardBotConfig` key-value 配置中，不是环境变量。Smart Money signal advisory 密钥只在 `SmartMoneySettings` 中读取（`POLYEDGE_SMART_MONEY__SIGNAL_ADVISORY_OPENAI_API_KEY` / `POLYEDGE_SMART_MONEY__SIGNAL_ADVISORY_ANTHROPIC_API_KEY`），base URL 和超时同样为 env-only，provider/request-format/model 和 signal advisory 并发限制则保存在 `SmartMoneyConfig` 并随 API snapshot 返回。Rewards 默认 base URL 包含 OpenAI-compatible `/v1` 和 Anthropic 根地址；默认模型 `gpt-4.1-mini`。Smart Money signal advisory 默认 provider/model 也是 `openai` / `gpt-4.1-mini`，默认 OpenAI-compatible base URL 为 `https://api.openai.com/v1`、Anthropic 为 `https://api.anthropic.com`、超时 180 秒。GLM/DeepSeek/Agnes 沿用 OpenAI-compatible 配置：分别设置对应 OpenAI-compatible base URL 与模型名；模型名包含 `glm` 或 `deepseek` 时请求格式会在 worker/connector 层强制归一为 Chat Completions 并使用兼容 JSON mode，模型名包含 `agnes` 时也强制归一为 Chat Completions 但保留 strict JSON schema。Agnes 可配置 `POLYEDGE_REWARDS__AI_OPENAI_BASE_URL=https://apihub.agnes-ai.com/v1`、`POLYEDGE_REWARDS__AI_MODEL=agnes-2.0-flash`、`POLYEDGE_REWARDS__AI_OPENAI_API_KEY=<secret>`；Smart Money signal advisory 使用对应 `POLYEDGE_SMART_MONEY__SIGNAL_ADVISORY_OPENAI_*` 环境变量和页面中的模型名配置。AI advisory 最低置信度 `5500` bps、信息风险最低置信度 `7000` bps、单次超时 180 秒。`ai_advisory_enabled=true` 时，缺少可用 provider 配置或未达到最低置信度的 advisory 会阻断新增 rewards 挂单。`POLYEDGE_REWARDS__INFO_RISK_MAX_MARKETS_PER_CYCLE` 默认 50，0 表示本轮不发 provider 请求；它在 full-tick combined provider refresh 中表示真实外部 provider 请求上限且缓存命中不占额度，在 AI advisory 未启用时的独立 info-risk worker 中仍表示每轮候选 condition 裁剪上限。AI advisory 事件驱动批量通道和 AI/info-risk batch-size 配置已移除。信息风险 OpenAI web search 默认关闭。可选第二个完全独立的 LLM 备用接口通过 `POLYEDGE_REWARDS__AI_FALLBACK_PROVIDER` / `_REQUEST_FORMAT` / `_API_KEY` / `_BASE_URL` / `_MODEL` 五项同时配置启用，备用密钥同样只在 `RewardsSettings` 读取、不进入 `RewardBotConfig`、API snapshot 或前端；主接口（`ai_provider` 选定）调用任意失败（网络/超时、HTTP 4xx/5xx、或返回无法解析的响应）时用同一请求重试备用接口（可不同 provider/模型），主备两次调用都写入 `llm_calls`（`fallback_used` 区分），advisory/info-risk 缓存按 `(provider,request_format,model,input_hash)` 各自独立存储，备用 provider 仍只配置 `openai` 或 `anthropic`；备用模型名包含 `glm`/`deepseek`/`agnes` 时同样强制归一为 `openai_chat_completions`。

Orderbook candle history 默认由 orderbook 服务独立启用：`POLYEDGE_ORDERBOOK_STREAM__REWARD_CANDLE_HISTORY_ENABLED=true`、`SYNC_INTERVAL_SECS=300`、`REQUEST_DELAY_MS=500`、`MAX_TOKENS_PER_CYCLE=600`、`BACKFILL_SECS=7200`、`INCREMENTAL_SECS=900`。这些字段控制 CLOB `/prices-history` 请求节奏，用于替代原先从本地高频 orderbook 更新派生 candles 的路径；max tokens 设为 0 会跳过本轮外部请求。

另有 `runtime_config` 子模块支持运行时动态配置。

### Stores — 持久化实现

**文件结构**（通过 `include!` 内联）：

| 实现文件 | 对应 trait | 存储 |
|---|---|---|
| `catalog/postgres/` | `MarketEventStore`、`NewsIngestionStore` | PostgreSQL |
| `catalog/postgres/market_event/market_queries.rs` | `MarketEventStore` 市场查询 helper | 市场列表/计数/分类/详情/按 id 批量读取 SQL；从通用 `queries.rs` 拆分 |
| `catalog/in_memory.rs` | 同上 | 内存（RwLock） |
| `stores/rewards/postgres.rs` | `RewardBotStore` | PostgreSQL（key-value config + managed orders/merge intents/完整账本表 + `llm_calls` 记录/每日统计） |
| `stores/rewards/postgres_market_methods.rs` | Rewards Postgres 市场查询 helper | 质量硬过滤、综合排序、row mapping |
| `stores/rewards/postgres_plans.rs` | Rewards Postgres quote plan helper | quote plan 统计、分页、搜索和排序 SQL |
| `stores/rewards/postgres_control_commands.rs` / `postgres_heartbeat.rs` / `postgres_orders.rs` / `postgres_writes.rs` | `RewardBotStore` 辅助 | 控制命令、worker heartbeat、订单分页 SQL、旧 reserved 释放辅助 |
| `stores/rewards/postgres_info_risk.rs` | `RewardBotStore` 辅助 | 信息风险缓存查询和写入 SQL |
| `stores/rewards/postgres_event_windows.rs` | `RewardBotStore` 辅助 | reward market 事件窗口候选 upsert 和 effective window 查询 SQL |
| `stores/rewards/postgres_low_competition.rs` | `RewardBotStore` 辅助 | 低竞争 legacy observation 写入和最近窗口查询 SQL；当前统一机会评分运行路径不再写入新的独立 observation |
| `stores/rewards/postgres_candles.rs` | `RewardBotStore` 辅助 | rewards price-history candle upsert 和最近 K 线查询 SQL |
| `stores/rewards/in_memory.rs` | 同上 | 内存（含 LLM 调用记录测试路径） |
| `stores/copytrade/postgres.rs` | `CopyTradeStore` | PostgreSQL（key-value config、tracked wallets、source trades、events、控制命令；旧模拟表兼容） |
| `stores/copytrade/postgres_control_commands.rs` / `postgres_rows.rs` / `postgres_writes.rs` | `CopyTradeStore` 辅助 | 控制命令 SQL、行映射、写入辅助 |
| `stores/copytrade/in_memory.rs` | 同上 | 内存 |
| `stores/smart_money/postgres.rs` | `SmartMoneyStore` | PostgreSQL（key-value config、候选钱包、画像、评分、源交易、未处理信号候选查询、信号写入/列表、signal decision 写入/列表和 signal advisory 缓存读写） |
| `stores/smart_money/postgres_rows.rs` / `postgres_config.rs` / `in_memory.rs` | `SmartMoneyStore` 辅助 | 行映射、配置解析 helper 和内存实现；内存实现同样按 `source_trade_id` 防重生成信号，按 `(signal_id, stage)` 防重写 decision，并按 `(signal_id, provider, request_format, model, input_hash)` upsert advisory |
| `stores/high_probability/postgres.rs` | `HighProbabilityStore` | PostgreSQL（key-value config、market outcome 标签、rewards candle sample input/observe candidate 查询、历史样本、分桶统计、baseline 回测 run/trade、退出规则摘要、观察记录） |
| `stores/high_probability/postgres_rows.rs` / `in_memory.rs` | `HighProbabilityStore` 辅助 | 行映射、配置解析和内存实现 |
| `stores/mode_state.rs` | `ModeStateStore` | PostgreSQL/内存 |
| `stores/risk_state.rs` | `RiskStateStore` | PostgreSQL/内存 |
| `stores/idempotency.rs` | `IdempotencyStore` | PostgreSQL/内存 |
| `stores/audit.rs` | `AuditLogSink` | PostgreSQL/内存 |
| `stores/maintenance.rs` | `DatabaseMaintenanceStore` | PostgreSQL 分批清理 / 无数据库 no-op |
| `stores/orderbook_cache.rs` | `OrderbookCache` | 内存（TTL + 定期清理 + 每侧盘口深度裁剪 + `entry_count` 真实条目统计）— 仅供 orderbook 服务内部使用；Worker/API 通过 `OrderbookHttpClient` 远程访问 |
| `stores/orderbook_registry.rs` | `OrderbookSubscriptionRegistry` | 内存（来源有序 token 原子替换 + 确定性优先级聚合 + 来源/去重总数统计）— 仅供 orderbook 服务内部使用；Worker 通过 HTTP 注册 token |
| `stores/orderbook_registry_tests.rs` | Registry 回归测试 | 原子 source 替换、优先级和跨 source 去重 |
| `stores/orderbook_cache_tests.rs` | Cache 回归测试 | 最优档排序/裁剪、批量写入、确认时间合并和 stale threshold 语义 |
| `stores/rewards_tests.rs` | Rewards store 回归测试 | running 控制命令租约、重复命令合并、历史清理边界、账户持仓完整替换与失败保留 |
| `stores/runtime_config.rs` | 运行时配置 | PostgreSQL key-value |
| `stores/helpers.rs` | DB 行映射辅助 | — |
| `stores/helpers/reward_config.rs` | Rewards key-value 配置读写 helper | `RewardBotConfig` 与 `reward_bot_config` key-value 行互转 |
| `stores/types.rs` | 共享类型 | — |

**关键模式：**
- Config 存储使用 key-value 表（`reward_bot_config`、`copytrade_config`、`smart_money_config`、`high_probability_config`、`runtime_config`）
- `runtime_config` bootstrap 在每次启动时用环境变量值覆盖数据库值（`ON CONFLICT ... DO UPDATE ... WHERE value IS DISTINCT FROM EXCLUDED.value`），确保环境变量始终优先；API 运行时修改仅在当前进程生命周期内生效，重启后恢复为环境变量值
- 常量：`SYSTEM_RUNTIME_STATE_ID = "global"`、`RISK_STATE_ID = "global"`
- `db_error(code, context)` 辅助函数统一创建 `dependency_unavailable` 错误
- `PostgresDatabaseMaintenanceStore` 使用 `WITH doomed AS (SELECT ctid ... LIMIT $n) DELETE ... USING doomed` 模式分批删除，每个表每轮最多 20 批、每批 10,000 行，避免单次超大事务；无 Postgres 环境使用 `NoopDatabaseMaintenanceStore`
- `RewardBotStore` 的 Postgres key-value 配置读写覆盖全部 rewards 报价/风控配置字段（市场质量门槛、`quote_bid_rank`、quote/selection mode、dominant probability/depth/concentration、preferred categories、统一机会评分 `opportunity_*` 竞争/奖励/退出/稳定性/资金占比阈值与权重、AI advisory 开关/provider/request format/TTL、AI provider 主/备并发开关和上限、AI strategy hint 开关与最低置信度、信息风险开关/mode/过滤等级/TTL、事件窗口开关/最低置信度/停止新增/撤买单/恢复冷却/未知时间/Gamma 未审核日期处理、首单信息风险要求/观察窗口、BalancedMerge `balanced_merge_*` 独立市场/订单/edge/质量/未配对库存阈值、depth/rank/velocity/requote/reconcile，以及 `requote_drift_confirm_sec` / `requote_drift_cooldown_sec` / `requote_drift_max_cancels_per_cycle` 换价 guard）。`low_competition_*` 旧键仍可兼容读取旧库/旧 API payload，但 `RewardBotConfig::normalized()` 会强制独立低竞争 sleeve 关闭（mode/off、独立市场/订单/全局占比为 0，旧 liquidity/volume 过滤关闭并清零），运行时不再使用低竞争专属报价、候选 profile、撤单阈值或 provider gate。`execution_mode`、`quote_edge_cents`、`reward_competition_factor`、`single_sided_divisor_c`、`fill_rate_per_tick`、`max_fill_ratio`、`auto_cancel_stale_minutes`、旧 AI/info-risk batch-size 键保留用于向后兼容但被忽略。`quote_bid_rank` 保存为 1–3，默认 1；`balanced_merge_enabled` 默认 false，开启后 profile 配置会把合并策略固定为双边买单并用 `balanced_merge_min_edge_cents` 映射到 safety margin；`exit_markup_cents` 默认 0，表示 `exit_at_markup` 成交后按被吃买单原价挂卖；`event_window_enabled` 默认 true，`event_window_min_confidence` 默认 high，stop-new/cancel/resume 默认 10800/3600/3600 秒，未知事件时间默认 observe，Gamma 未审核日期默认 ignore；`ai_provider_concurrency_enabled` 默认 false，主/备最大并发默认 1 并钳制到 1–10；`ai_strategy_hint_enabled` 默认 true，`ai_strategy_hint_min_confidence` 默认 0.75 并钳制到 0–1；`first_quote_quarantine_sec` 保存为 0–86400，默认 600。
- `RewardBotStore.latest_market_advisory()` 和 `save_market_advisory()` 在 Postgres 与内存实现中读写 `reward_market_advisories`；缓存 key 为 condition/provider/request_format/model/input_hash，且只返回 `expires_at > now` 的记录。Postgres 行映射 helper 解析 suitability、quote mode、exit policy、reasons JSON 和 metrics JSON；AI strategy hint 不新增列，存放在 `metrics_json.strategy_hint` 中，供 live gate、live materializer 和 placement budget cap 读取。
- `RewardBotStore.latest_market_info_risk(s)` 和 `save_market_info_risk()` 在 Postgres 与内存实现中读写 `reward_market_info_risks`；请求缓存 key 为 condition/provider/request_format/model/input_hash，批量读取按 condition 返回最新未过期记录。Postgres 行映射 helper 解析 risk level/type/direction、sources JSON 和 metrics JSON。
- `RewardBotStore.record_llm_call()` 和 `list_llm_call_daily_stats()` 在 Postgres 与内存实现中读写已有 `llm_calls` 表；worker 只在实际外部 AI advisory / info-risk provider HTTP 调用后写入，combined provider 请求按一条外部调用计数，snapshot 查询按 UTC 日聚合 AI、info-risk、总调用和失败调用。
- `RewardBotStore.record_low_competition_observations()` 和 `list_low_competition_observations()` 在 Postgres 与内存实现中保留对 `reward_low_competition_observations` 的历史兼容读写；当前 unified rewards 运行路径不再生成独立低竞争 observation，API snapshot 的 `low_competition_report` 返回 `None`。
- `RewardBotStore.prune_history(cutoff)` 在 Postgres 中使用单事务清理 cutoff 之前的终态 managed orders（`cancelled`/`filled`/`error`）、`reward_risk_events` 和 `reward_low_competition_observations`；内存实现保持相同语义。该清理不会删除 `planned`/`open`/`exit_pending` 订单、`reward_fills`、`reward_positions` 或 `reward_account_state`，避免破坏 live 对账和账本。
- `RewardBotStore.record_market_candle_sample()` 和 `list_recent_market_candles()` 在 Postgres 与内存实现中读写 `reward_market_candles`；Postgres 根据 `reward_markets.tokens_json` 把 token 映射到 active reward market，按 `(token_id, interval_sec, bucket_start)` upsert price-history OHLC，同一 close timestamp 的重复写入不增加 `sample_count`，AI advisory 按 condition/interval/window 读取最近 K 线。
- `RewardBotStore.upsert_market_event_windows()` 和 `list_effective_market_event_windows()` 在 Postgres 与内存实现中读写 `reward_market_event_windows`；Postgres upsert 使用 `INSERT ... SELECT ... WHERE EXISTS (reward_markets)`，避免 Gamma 全量同步遇到非 rewards condition 时触发外键错误；effective 查询按 confidence、source 优先级和 `updated_at` 为每个 condition 选一条 active window。
- `RewardBotStore` 支持按 external Polymarket order id 查询 rewards managed order、通过 fill id 判断成交是否已入账，并通过 `latest_fill_at(account_id)` 查询最近 confirmed fill；live worker 用这些读路径完成托管订单成交幂等同步和外部账户快照保护。Postgres 与内存实现还维护 `reward_merge_intents` / `RewardMergeIntent`，`active_merge_intent_size(account_id, condition_id)` 汇总 pending/unsupported/submitted/completed size，避免 BalancedMerge 配对库存重复创建合并 intent。
- `RewardBotStore.cancel_open_orders()` 在 Postgres/内存实现中兼容释放旧账本的 `reserved_usd`；新的 rewards 开放买单不再逐单硬占用资金，订单列表优先返回 open-like 状态，避免大量历史成交/撤单淹没当前开放挂单。worker 的 `list_open_orders()` 仍包含本地 planned/exit intent；控制台 `status.open_orders` 使用独立的 external count，优先读取同进程 worker 最近一次成功 CLOB open-order snapshot 里仍存在、外部剩余量为正且状态非终态的本系统 managed 外部订单数量，冷启动时回退到 store 中已有非内部 `external_order_id`、仍是 open-like、本地剩余量为正且未处于提交未知、取消未知、404 人工对账或 `awaiting final reconciliation` 锁定的订单计数。
- `RewardBotStore.list_orders_page()` 在 Postgres 实现中通过 count + limit/offset 做服务端分页，支持 outcome/condition/token 搜索、状态过滤和 price/size/status 排序；`orders_status=filled` 会匹配 `status='filled'` 或 `filled_size > 0` 的部分成交 managed orders；内存实现保持相同语义。
- `RewardBotStore.list_markets(limit)` 只返回 active reward markets；Postgres candidate query 关联 Gamma `markets`，统一 profile 硬过滤非 open/tradable、高歧义、liquidity 与 `volume_24h` 两个活跃度代理均低于阈值、临近 `end_at`、Gamma spread 过宽、`synced_at` 过期或异常超前、奖励不足、奖励 spread 无效及非唯一 YES/NO token 市场；若只配置 liquidity 或 volume 单项阈值，则只检查该项，两项都为 0 时关闭该活跃度预筛。默认 midpoint 仍限制在常规双边区间；auto 单边允许时，查询会额外允许 `dominant_min_probability..dominant_max_probability` 及反向区间进入候选。候选查询不再用 `rewards_min_size <= per_market_usd` 预筛预算，高最小份额市场会保留到 live materializer 和实际钱包余额准入层处理。综合排序按 CLOB 原始 cents 直接使用 rewards spread，并结合奖励、流动性、成交量、剩余时长和 Gamma `category` 偏好；真实盘口层面的竞争资金、奖励密度、退出能力和稳定性由 application 的 `opportunity_metrics` 在 quote plan 阶段统一调整评分与可挂资格。Gamma 同行的有效、未交叉 best bid/ask 可在 reward token 缺 price 时注入 YES midpoint 和 NO complement 作为候选规划回退；内存实现同步校验唯一 YES/NO、midpoint/dominant 区间、活跃度代理和同步时间。`upsert_markets()` 对 reward catalog 先 upsert 当前快照、再只停用缺失的 active rows，且 unchanged rows 最多每小时 touch 一次，避免每轮全量 active=false/true 写放大；`save_quote_plans()` 会替换当前 quote plan 快照，避免旧的全量计划继续出现在 `/rewards`；`count_quote_plans()` 在 Postgres 中用 SQL 直接聚合 readiness 与 blocker counts（包含等待盘口、AI/info-risk pending、live 盘口验证等），不再读取并反序列化全表 `quote_plan_json`。Postgres 查询实现拆分到 `postgres_market_methods.rs` 和 `postgres_plans.rs`。
- Postgres `RewardBotStore.apply_tick_outcome()` 会在同一事务中只持久化 orders、fills、positions、merge intents、account ledger 和 events；reward market 全量目录只由 `upsert_markets()` 更新，quote plan 快照只由 `save_quote_plans()` 替换，避免增量 live tick 误停用全量奖励市场。
- Postgres/内存 `RewardBotStore.apply_account_sync()` 会更新账户状态；外部 positions 成功时原子替换目标账户全部 `reward_positions`，失败时通过 `None` 保留上一版持仓。worker 会结合 `latest_fill_at` 在 confirmed fill 后 120 秒内跳过整次外部账户替换，避免最终一致性响应回滚本地现金或库存。外部持仓可来自当前 rewards catalog 之外的市场，因此 `reward_positions.condition_id` 和外部库存补退出写入的 `reward_managed_orders.condition_id` 都不再依赖 `reward_markets` 外键。
- `RewardBotStore` 在 Postgres/内存实现中维护 `reward_control_commands` 队列；API 写入 pending 命令时，store 会合并同账户同动作且仍为 pending/running 的重复命令，Postgres 侧由 partial unique index 兜底防止并发重复入队；worker 使用 claim/complete/fail 方法领取并更新执行状态；running 命令超过 5 分钟会重新进入可领取范围。
- `RewardBotStore` 在 Postgres/内存实现中按 account 维护 rewards worker heartbeat；API snapshot 只把配置已启用且最近 2 分钟有 heartbeat 的 worker 标记为 running。Postgres 表由迁移 `0032_reward_worker_heartbeats.sql` 创建。
- `CopyTradeStore` 在 Postgres/内存实现中维护 `copytrade_control_commands` 队列；API 写入 pending 命令，worker 使用 claim/complete/fail 方法领取并更新执行状态。当前 copytrade worker 只执行 source trade 检测和 Analyze 钱包分析；run/cancel/reset 兼容命令不再触发模拟交易。
- `SmartMoneyStore` 在 Postgres/内存实现中维护 Smart Money foundation 数据：配置（含 signal advisory provider/request-format/model 和并发限制）、候选钱包、候选状态、画像、评分、源交易、信号列表、signal decision 审计列表和 `smart_signal_advisories` 缓存；advisory cache key 为 signal/provider/request_format/model/input_hash，读取只返回 `expires_at > now` 的记录。当前不包含控制命令队列。`scan-smart-money-once` CLI 和可选 `POLYEDGE_WORKER__POLL_SMART_MONEY` 定时任务会从 Data API leaderboard 与 active copytrade tracked wallets 写入候选，并扫描候选钱包生成画像、评分和 Data API activity 源交易；随后通过 `list_unprocessed_signal_trades()` 读取尚未有 `smart_signals.source_trade_id` 的源交易，通过 `record_signals()` 按 source trade 防重插入 deterministic observe/rejected 信号，再通过 `record_signal_decisions()` 按 `(signal_id, stage)` 防重写入 `deterministic_gate` decision；开启 `signal_advisory_enabled` 后，worker 会使用 Smart Money 独立 provider 配置、独立 signal advisory 并发限制和 `SmartMoneySettings` 中的 env-only key/base URL 刷新 `smart_signal_advisories`。API 可把候选更新为 `candidate/watch/tracked/blocked/rejected`，未入库钱包会创建 `manual` 来源记录。recent trades/链上完整 discovery、wallet advisory、paper/live 写路径尚未接入。
- `HighProbabilityStore` 在 Postgres/内存实现中维护动态高概率市场定价研究 foundation 数据：配置、market outcome 标签、已构建样本、分桶统计、baseline backtest run/trade、退出规则摘要和 observation。Postgres 实现会从 `reward_market_candles` + `high_probability_market_outcomes` + `markets` 读取 rewards candle sample inputs；observe candidate 查询会读取活跃 rewards 最新 5m candle，并排除已有 resolved/voided/ambiguous outcome 标签的 condition。当前支持 `build-high-probability-samples-once` 构建 rewards first-touch 样本，`refresh-high-probability-buckets-once` 从已有已结算样本刷新 bucket stats，`run-high-probability-backtest-once` 持久化 70/30 baseline walk-forward 回测结果、`exit_rule_reports` 和入场交易明细，以及 `observe-high-probability-once` 或默认关闭的 `POLYEDGE_WORKER__POLL_HIGH_PROBABILITY_OBSERVE` runtime loop 写入只读 observations。全市场 candles/outcomes、paper/live 写路径尚未接入。
- `InMemoryOrderbookCache` 在所有写入入口统一按 bids 降序、asks 升序排序后裁剪，确保无序 WS/poll/ingest 数据也保留 top-of-book；写入 `observed_at` 早于当前未过期条目的盘口内容会被忽略，相同内容时间戳下 WS 优先于 poll，但若被拒绝的 poll/ingest 携带更新的 `confirmed_at`，缓存会合并确认时间并刷新 TTL，避免安静市场因盘口版本不变而过期；已过期条目不会阻挡后续较旧 `observed_at` 的 poll/ingest 快照恢复；`get_books()` 在一次读锁内返回多个未过期盘口；`get_stale_tokens(..., max_age_ms <= 0)` 只检查 TTL，不执行年龄 stale 检查，年龄 stale 检查使用 `confirmed_at`。
- `InMemoryOrderbookSubscriptionRegistry.register_tokens()` 在持有写锁时原子执行 32-source 上限检查，关闭并发新 source 绕过 HTTP 预检查的竞态；空 token 集合会删除 source，聚合优先级为 `rewards_active`、`exec_orders`、`rewards_eligible`、`rewards_ai_provider`、`rewards_candidates`、`copytrade`。

### Catalog — 核心数据存储

**Postgres 实现**（`catalog/postgres/`）：通过 `include!` 拆分为多个子文件
- `market_event/` — 最大的存储模块，包含 queries、execution_updates 等
- `news.rs` — 新闻源健康和 raw news event 存储
- `helpers/` — 共享辅助文件：`fetch.rs`、`market_rows.rs`、`news_rows.rs`、`event_rows.rs`、`execution_rows.rs`、`calculations.rs`

**In-memory 实现**（`catalog/in_memory.rs`，~24KB）：用于测试和无数据库环境

旧 arbitrage store 和 helper 已删除；迁移创建的历史套利表仍保留给既有数据库兼容，不再有 application/infrastructure 读写实现。

### Auth — 认证中间件

- **`AuthContext`**：请求级认证上下文
- **`IdempotencyKey`**：幂等键解析
- **`InternalTokenVerifier`**：内部 JWT 令牌验证
- **`RequestKind`**：请求类型枚举
- **`AuthSettings.disabled`**：`POLYEDGE_AUTH__DISABLED=true` 时跳过 console/connector/mode token 和 step-up 校验，直接注入 admin `AuthContext`；仅用于纯内网部署
- **Step-up scopes**：当前公开写路径主要使用系统模式切换和 `funding_transfer`；内网免鉴权模式会注入全部 scope，真实鉴权模式下 Funding API 转账必须携带该 scope。旧信号执行、熔断和风险阈值 scope 仍可被解析用于兼容旧 token，但对应公开 API 已移除
- **中间件函数：**
  - `require_connector_write_auth` — 连接器写入认证
  - `require_console_read_auth` — 控制台读取认证
  - `require_console_write_auth` — 控制台写入认证
  - `require_mode_write_auth` — 模式切换认证

### Runtime — 依赖注入

- **`AppState`**：所有服务实例的容器，被 API handler 和 worker 共享
  - 包含：`market_event_service`、`execution_service`、`risk_service`、`reward_bot_service`、`copytrade_service`、`smart_money_service`、`high_probability_service`、`news_ingestion_service`、`orderbook_cache`、`orderbook_registry` 等
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
- **下游**：`packages/backend/api`（使用 AppState、auth middleware）、`packages/backend/order`（使用 AppState、Runtime 和 orderbook stores）、`packages/backend/apps/worker`（使用 AppState、Store 实现）

## 当前状态

- Postgres 和 in-memory 双实现均已就绪
- 配置通过环境变量加载，支持 `.env` 文件
- 未设置 `RUST_LOG` 时，默认 tracing filter 为 `{service_name}=debug,polyedge_worker=info,tower_http=info,sqlx=info`，因此 `polyedge-api` 内嵌 worker runtime 的 info/warn 日志会出现在 API 服务日志中；显式设置 `RUST_LOG` 会覆盖该默认值
- 新闻源默认值已内置在 `settings/defaults.rs`；部署模板默认开启 news 子系统和 worker poll loop，会抓取模板中显式配置的默认 RSS/Atom 源，新闻提升为 events/evidences 仍默认关闭；代码默认关闭 execution drain、paper 对账和 Polymarket 私有对账/WS worker，需显式开启才运行
- 认证中间件支持 JWT/dev-auth 和内网免鉴权模式；当前部署模板默认 `POLYEDGE_AUTH__DISABLED=true`
- Funding API 复用 Polymarket settings 中的私钥、funder/account_id 和 Polygon RPC，不新增单独资金私钥配置；API 状态响应只返回派生付款地址和入账钱包地址，不暴露私钥
- Orderbook cache 当前 runtime 使用进程内 `InMemoryOrderbookCache`；Redis 实现保留但未接入默认 runtime
- Orderbook 服务的 `/orderbook/stats` 现在区分真实 cache 条目数、registry 来源数和 registry 去重 token 总数，避免把订阅 token 数误报为缓存条目数；worker 注册 rewards 候选预热 token 受 `POLYEDGE_ORDERBOOK_STREAM__REWARD_CANDIDATE_TOKEN_CAP` 限制，默认 50，设为 0 后周期注册任务会按空结果防抖清空候选预热 source
- Orderbook 进程内缓存会先保留最优价格顺序，再按 `POLYEDGE_ORDERBOOK_STREAM__MAX_LEVELS_PER_SIDE` 裁剪每侧 bids/asks 深度，默认 100 档；HTTP register/batch/ingest 入口使用 `max_tokens` 做请求规模上限，Polymarket WS 订阅使用 `POLYEDGE_ORDERBOOK_STREAM__WS_CHUNK_SIZE` 控制每条连接承载的 token 数（默认 100），poll reconcile 默认 10 秒，register 会原子替换对应 source 当前有序 token 集合，worker 对周期注册空集合做 active/exec 2 轮、eligible/candidates 3 轮防抖后才发送空集合清源，ingest 会先校验整批数据再批量写入并传播缓存错误，registry source 固定上限为 32 个
- Orderbook 缓存拒绝旧 `observed_at` 覆盖未过期条目，但会合并更新的 `confirmed_at` 作为最近确认时间；已过期条目可被后续写入恢复；rewards 控制命令具备 5 分钟 running lease，并会合并 pending/running 重复命令；Postgres rewards live worker 通过 advisory lease 避免多实例并发执行
- Rewards managed order upsert 会更新后续实际提交的 `price` / `size` / `strategy_bucket` / `strategy_profile`，保证 flatten 改价、CLOB 数量调整、profile 分流和未知提交恢复使用持久化后的真实参数；运行时新订单统一使用 standard bucket，历史 low_competition bucket 仅兼容既有数据，profile 可为 `standard` 或 `balanced_merge`。
- Rewards store 已持久化 quote/selection mode、dominant 单边阈值、盘口集中度阈值、偏好分类、统一 `opportunity_*` 机会评分配置、BalancedMerge `balanced_merge_*` 配置、AI advisory 配置（不含 batch size，含主/备 provider 并发设置和 strategy hint 开关/置信度）、信息风险配置（不含 batch size）和首单入场 gate 配置；`reward_market_advisories`、`reward_market_info_risks`、`reward_low_competition_observations`、`reward_market_candles` 与 `reward_merge_intents` 表已由迁移创建，并已接入 Postgres/内存读写，其中低竞争 observation 表仅保留历史兼容，当前 snapshot 不再生成低竞争 shadow report；AI strategy hint 保存在 `reward_market_advisories.metrics_json.strategy_hint` 而不是新列；AI advisory / info-risk 的实际 provider 调用会写入 `llm_calls` 并在 Rewards snapshot 中按日聚合展示。
- Rewards quote plan snapshot 统计在 Postgres 中直接用 SQL 聚合 readiness 与 blocker counts，不再为顶部概览读取并反序列化全表 `quote_plan_json`；blocker 分类包含等待盘口、AI/info-risk pending、信息风险、资金不足、live 盘口验证和其它原因，历史低竞争 blocker 计数固定为 0。
- 数据库维护 store 已接入 runtime：Postgres 环境定期清理 raw events、AI/info-risk cache、reward candles、控制命令、copytrade 历史、outbox/external dedup、LLM call、audit 和 mode transition 历史；in-memory/test runtime 使用 no-op，避免测试状态被后台任务改变。
- Rewards store 已支持外部账户余额和完整持仓快照同步；成功空持仓快照会清空目标账户持仓，失败响应不会破坏上一版，最近 confirmed fill 时间用于 worker 的 120 秒账户快照保护；worker 写入的资金钱包地址优先使用 `FUNDER`，CLOB 余额为 0/失败时可用 Polygon pUSD 链上余额回填 snapshot
- `markets` 保存 Gamma `liquidity_usd`、`end_at` 和本地 `synced_at`；Postgres market upsert 使用单条 `INSERT .. ON CONFLICT DO UPDATE WHERE` 表达新增、真实数据变化更新和 freshness-only 刷新，返回实际写入行数，并在每批事务内设置短 `lock_timeout` / `statement_timeout`。默认调用仍刷新 `synced_at`，orderbook full sync 通过 `MarketUpsertOptions` 只刷新超过新鲜度阈值的安静市场，priority sync 继续强制刷新重点市场，避免 rewards 关键市场因目录新鲜度过低被误判。
- MarketEventStore 的 Postgres 实现支持 `get_markets_by_ids()` 通过 `m.id = ANY($1)` 批量读取少量相关市场，供少量关联市场信息查询避免全量 markets 列表扫描。
- `idx_markets_reward_quality` 不包含高频变化的 `synced_at`，降低 freshness-only 刷新对索引和 WAL 的写放大；`idx_markets_polymarket_yes_asset_id` / `idx_markets_polymarket_no_asset_id` 支撑 orderbook priority sync 的注册 token 到 condition id 反查；rewards 候选查询仍在关联 Gamma `markets` 后按 `synced_at` 做新鲜度过滤。
- Orderbook register/ingest/delete 写接口要求 `x-polyedge-orderbook-token` 与 `POLYEDGE_ORDERBOOK__WRITE_TOKEN` 匹配；该密钥仅配置在 `deploy/.env.orderbook` 和 `deploy/.env.api`，未配置 token 时写接口关闭，读接口和健康检查仍可用

## 修改检查清单

- [ ] 新增 Store trait 方法后，在 postgres 和 in_memory 实现中同步添加
- [ ] 修改数据库查询后，运行 `cargo test --workspace`
- [ ] 新增配置字段时，同步更新对应的 `deploy/.env.{api,orderbook,front}.example`
- [ ] 修改认证逻辑时，检查所有中间件函数的使用点
- [ ] 修改 `AppState` 字段后，检查 `runtime.rs` 的构建逻辑和所有消费方
- [ ] 运行 `cargo check --workspace --tests`
