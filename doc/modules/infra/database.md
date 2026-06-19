# 数据库（Migrations + Schema）

最后更新：2026-06-19

## 概述

数据库使用 PostgreSQL，通过 44 个 SQL 迁移文件管理 schema。覆盖审计、市场数据、事件/信号、执行管道、风控、套利、奖励、跟单等领域。为了空库一次性初始化，`packages/backend/init.sql` 机械合并了当前全部迁移内容；运行时仍通过 `sqlx` 使用 `packages/backend/migrations/` 做迁移校验和增量升级。

## 初始化入口

- `packages/backend/init.sql`：完整空库初始化脚本，按 `0001` 到 `0044` 顺序展开所有迁移，适合新环境人工执行一次，例如 `psql "$POLYEDGE_POSTGRES__URL" -v ON_ERROR_STOP=1 -f packages/backend/init.sql`。
- `packages/backend/migrations/*.sql`：保留给 Rust runtime 的 `sqlx::migrate!` 使用。不要删除或重命名已应用的迁移文件，否则现有数据库的 `_sqlx_migrations` 历史可能与二进制内嵌迁移不一致。
- `init.sql` 只面向空数据库；已存在 schema 的数据库继续使用 runtime 自动迁移或按需执行新增迁移。

## 迁移文件列表

| 迁移 | 主题 | 核心表 |
|---|---|---|
| `0001_support_tables.sql` | 支撑表 | `audit_logs`、`idempotency_keys` |
| `0002_market_event_core.sql` | 市场/事件核心 | `markets`、`market_resolution_rules`、`raw_events` |
| `0003_evidence_signal_core.sql` | 证据/信号 | `evidences`、`signals` |
| `0004_pricing_and_signal_transitions.sql` | 概率估计/信号转换 | `probability_estimates`、`signal_transitions` |
| `0005_risk_state.sql` | 风控状态 | `risk_state` |
| `0006_signal_rejection_metadata.sql` | 信号拒绝元数据 | 修改 `signals` |
| `0007_execution_request_core.sql` | 执行请求 | `order_drafts`、`execution_requests` |
| `0008_execution_dispatch_metadata.sql` | 执行分发元数据 | 修改 `order_drafts`、`execution_requests` |
| `0009_orders_trades_positions.sql` | 订单/成交/持仓 | `orders`、`trades`、`positions` |
| `0010_market_connector_refs.sql` | Polymarket 引用 | 修改 `markets`（condition_id、asset_id） |
| `0011_news_ingestion.sql` | 新闻采集 | 修改 `raw_events`、`news_source_health` |
| `0012_news_source_health_list_index.sql` | 新闻源索引 | 修改 `news_source_health` |
| `0013_arbitrage_radar.sql` | 套利雷达 | `arbitrage_scans`、`market_book_snapshots`、`arbitrage_opportunities` |
| `0014_arbitrage_validation_events.sql` | 套利验证 | `arbitrage_opportunity_validations`、`arbitrage_events` |
| `0015_reward_bot.sql` | 奖励机器人 | `reward_bot_config`、`reward_markets`、`reward_quote_plans`、`reward_managed_orders` |
| `0016_runtime_config.sql` | 运行时配置 | `runtime_config` |
| `0017_market_slug.sql` | 市场 slug | 修改 `markets` |
| `0018_market_categories.sql` | 市场分类 | `market_categories`（预置 13 个分类） |
| `0019_reward_simulation.sql` | 奖励模拟 | 修改 `reward_managed_orders`、`reward_fills`、`reward_account_state` |
| `0020_copy_trading.sql` | 跟单历史 schema | `copytrade_config`、`copytrade_wallets`、`copytrade_source_trades`、旧模拟表 `copytrade_copy_orders` / `copytrade_positions` / `copytrade_account_state`、`copytrade_events` |
| `0021_copytrade_daily_pnl.sql` | 旧跟单日 PnL | 修改旧模拟表 `copytrade_account_state`（`daily_realized_pnl`） |
| `0022_reward_bot_control_commands.sql` | 奖励机器人控制命令 | `reward_control_commands` |
| `0023_copytrade_control_commands.sql` | 跟单控制命令 | `copytrade_control_commands` |
| `0024_reward_markets_active_index.sql` | 奖励市场查询索引 | `reward_markets` active + daily rate 索引 |
| `0025_markets_active_volume_index.sql` | 市场活跃度索引 | `markets` open/tradable + 24h volume 索引 |
| `0026_reward_control_running_lease_index.sql` | Rewards 控制命令租约索引 | `reward_control_commands` running + started_at 部分索引 |
| `0027_remove_paper_trade_manual_confirm.sql` | 收敛系统运行模式 | 修改 runtime/mode transition/execution request mode 约束 |
| `0028_reward_positions_external_inventory.sql` | Rewards 外部账户持仓 | 移除 `reward_positions.condition_id` 到奖励目录的外键 |
| `0029_reward_account_wallet_address.sql` | Rewards 资金钱包 | 为 `reward_account_state` 增加 wallet address |
| `0030_rewards_snapshot_indexes.sql` | Rewards snapshot 索引 | `reward_fills`、`reward_positions` 查询索引 |
| `0031_worker_query_indexes.sql` | Worker 查询索引 | orders、raw_events、copytrade source trades 索引 |
| `0032_reward_worker_heartbeats.sql` | Rewards worker 心跳 | `reward_worker_heartbeats` |
| `0033_reward_candidate_filter.sql` | Rewards 候选过滤 | 修改 `reward_bot_config` |
| `0034_reward_account_external_buy_notional.sql` | Rewards 外部买单观测 | 修改 `reward_account_state`；不作为开放 maker 买单的硬资金占用 |
| `0035_auto_cancel_not_found_orders.sql` | 历史订单修复 | 调整历史 rewards managed order 状态 |
| `0036_restore_not_found_reconciliation.sql` | 恢复 404 对账 | 将被错误自动取消的外部订单恢复为待成交对账状态 |
| `0037_reward_market_quality.sql` | Rewards 市场质量与安全修复 | `markets` 增加 liquidity/end/synced 字段和质量索引；恢复旧 stale auto-cancel 订单的对账锁 |
| `0038_reward_market_advisories.sql` | Rewards AI advisory 缓存 | `reward_market_advisories` |
| `0039_reward_market_info_risks.sql` | Rewards 信息风险缓存 | `reward_market_info_risks` |
| `0040_markets_quality_index_no_synced_at.sql` | Rewards 市场质量索引写放大优化 | 重建 `idx_markets_reward_quality`，移除高频变化的 `markets.synced_at` |
| `0041_market_asset_id_lookup_indexes.sql` | 市场 asset id 反查索引 | `markets.polymarket_yes_asset_id` / `polymarket_no_asset_id` 部分索引 |
| `0042_reward_order_strategy_bucket.sql` | Rewards 订单策略 bucket | `reward_managed_orders.strategy_bucket` 及按 bucket/account/status 查询索引 |
| `0043_reward_low_competition_observations.sql` | Rewards 低竞争观测 | `reward_low_competition_observations` 及 account/condition 最近观察索引 |
| `0044_reward_market_candles.sql` | Rewards 盘口派生 K 线 | `reward_market_candles` 及 condition/token 最近 K 线索引 |

## Schema 领域分组

### 1. 审计/幂等

- **`audit_logs`**：完整审计追踪 — actor（user/session/roles）、action、resource、result（accepted/succeeded/rejected/failed）、IP、user agent、payload JSON、version snapshot
- **`idempotency_keys`**：scope + key + request_hash，跟踪 started/completed/failed 状态，有 TTL

### 2. 市场数据

- **`markets`**：question、category、status、best_bid/ask/mid_price（NUMERIC(12,6) 约束 0-1）、`liquidity_usd`、volume_24h、`end_at`、`synced_at`、ambiguity_level、tradability_status、version、slug、polymarket_condition_id/yes_asset_id/no_asset_id；condition_id 有唯一索引，yes/no asset id 有非空部分索引用于 orderbook priority sync 反查
- **`market_resolution_rules`**：resolution_source、edge_case_notes（text 数组）
- **`market_categories`**：id、label、sort_order（预置 13 个分类：Sports、Politics、Crypto 等）

### 3. 事件/新闻

- **`raw_events`**：source、hash（SHA-256 去重）、source_type（news/social/official/calendar/market）、external_id、title、url、author、published_at、event_time、payload JSON
- **`news_source_health`**：enabled、reliability、last_success/error、consecutive_failures、circuit_breaker_until

### 4. 证据/信号

- **`evidences`**：market FK、event FK、direction（supports_yes/supports_no/background）、strength、source_reliability、novelty、resolution_relevance（均 0-1）、status、expiry
- **`signals`**：action（buy/sell）、side（yes/no）、market_price、fair_price、edge、confidence、lifecycle_state（7 状态）、estimate_id FK、rejected_by_user_id、rejected_at
- **`signal_transitions`**：from_state、to_state、reason、actor
- **`probability_estimates`**：prior/posterior/fair/market price、edge、confidence、time_horizon、model_version、reason_codes（JSONB）

### 5. 风控

- **`risk_state`**：kill_switch、daily_pnl、gross_exposure（0-10）、net_exposure（0-10）、open_alerts、notes 数组

### 6. 执行管道

- **`order_drafts`**：connector_name、side、limit_price、quantity、notional、status、external_order_id、submitted_at、failure_code/message
- **`execution_requests`**：mode、risk_state_version、requested_by_user_id、status、external_order_id、submitted_at、failure_code/message
- **`orders`**：external_order_id、side、limit_price、quantity、filled_quantity、avg_fill_price、status（8 状态）
- **`trades`**：order_id FK、price、quantity、fee、side、role（maker/taker）
- **`positions`**：market/account/connector 聚合 — net_quantity、avg_entry_price、unrealized_pnl、realized_pnl

### 7. 套利

- **`arbitrage_scans`**：market_count、snapshot_count、opportunity_count、scanner_version
- **`market_book_snapshots`**：scan FK、market FK、yes/no bid/ask price + size
- **`arbitrage_opportunities`**：buy/sell 引用、gross_edge、net_edge、capacity、status（5 状态）
- **`arbitrage_opportunity_validations`**：validation_status（9 状态）、gross/net edge、fee_estimate、slippage_buffer、validated_capacity、book_age
- **`arbitrage_events`**：BIGSERIAL PK、event_type

### 8. 奖励机器人

- **`reward_bot_config`**：key-value 配置，包含报价/风控、市场质量、quote/selection mode、dominant 单边阈值、盘口集中度阈值、偏好分类、低竞争 sleeve mode/额度/竞争/退出/稳定性阈值、AI advisory 开关/provider/request format/TTL 和信息风险开关/mode/过滤等级/TTL
- **`reward_markets`**：condition_id、question、market_slug、rewards_max_spread/min_size、total_daily_rate、tokens JSON
- **`reward_quote_plans`**：market FK、scoring、quote plan
- **`reward_managed_orders`**：account_id、condition_id、token_id、strategy_bucket（standard/low_competition/none）、filled_size、reward_earned、last_scored_at
- **`reward_fills`**：order_id、account_id、condition_id、token_id、outcome、side、price、size、notional_usd、role、realized_pnl
- **`reward_positions`**：按 account_id + token_id 保存外部完整持仓；可包含当前 rewards catalog 之外的市场，不再依赖 `reward_markets` 外键
- **`reward_account_state`**：capital_usd、available_usd、reserved_usd（旧硬占用兼容字段，下一次 rewards tick 自动释放）、realized_pnl、reward_earned_usd、fees_paid、tick_index
- **`reward_control_commands`**：API 入队给 worker 的 rewards 控制命令（run_once/cancel_all/reset）及 pending/running/completed/failed 状态；running 超过 5 分钟可重新领取
- **`reward_market_advisories`**：AI advisory 缓存表，按 condition/provider/request_format/model/input_hash 保存 suitability、推荐 quote mode、exit policy、confidence、reasons/metrics JSON 和 expires_at；`input_hash` 使用稳定 cache-key payload（市场身份/问题、奖励参数、计划 quote mode 和相关策略配置），不包含每轮变化的账户、开放订单、持仓或盘口实时字段；worker 只读取未过期记录，缓存未命中时调用 provider 后写入
- **`reward_market_info_risks`**：信息风险缓存表，按 condition/provider/request_format/model/input_hash 保存 query_hash、risk_level、risk_type、directional_risk、resolution_imminent、expected_event_at、confidence、summary、sources/metrics JSON 和 expires_at；`input_hash` 使用稳定 cache-key payload（搜索 query、市场身份/问题/事件、计划 quote mode 和风险策略配置），不包含账户、开放订单、持仓、quote plan reason/score 或 market_synced_at 等动态字段；异步 worker 写入，live rewards tick 只读取未过期缓存
- **`reward_low_competition_observations`**：低竞争 sleeve 跨周期 observation，按 account/condition/observed_at 记录模式、计划 notional、竞争资金、预估 reward/100/day、退出深度/滑点、midpoint 波动、样本不足、低竞争 gate、最终可挂、AI/信息风险拦截、主策略重叠和拒绝原因 JSON；snapshot shadow report 读取最近窗口聚合，不会自动改配置。
- **`reward_market_candles`**：orderbook 服务从内部盘口更新派生的 rewards token K 线，按 token/interval/bucket 保存 midpoint OHLC、收盘 bid/ask、收盘 spread、样本数和 close observed_at；当前用于 AI advisory payload 和摘要 cache key，不包含真实成交量。
- **低竞争 rewards sleeve**：v2 已实现，配置复用 `reward_bot_config` key-value，quote plan 指标复用 `reward_quote_plans` JSON，managed order bucket 使用 `reward_managed_orders.strategy_bucket`，跨周期观察使用 `reward_low_competition_observations`。

### 9. Copytrade 钱包跟踪与分析

- **`copytrade_config`**：key-value 配置
- **`copytrade_wallets`**：address、label、status（active/paused）、sizing overrides、rolling stats（trades/volume/PnL/win_rate/ROI）
- **`copytrade_source_trades`**：检测到的源交易（deterministic ID 去重）
- **`copytrade_events`**：活动/风险事件日志
- **`copytrade_control_commands`**：API 入队给 worker 的 copytrade 控制命令（run_once/analyze_wallets/cancel_all/reset）及 pending/running/completed/failed 状态；当前只有 analyze_wallets 有实际分析语义，run/cancel/reset 为历史兼容 no-op
- **旧模拟表**：`copytrade_copy_orders`、`copytrade_positions`、`copytrade_account_state` 仍随迁移存在，用于历史兼容和避免破坏旧数据；当前前端/API snapshot 不再展示模拟账户、订单或持仓，worker 也不会写入新的模拟订单。

### 10. 运行时配置

- **`runtime_config`**：key TEXT PK、value TEXT、updated_at

## 当前状态

- 44 个迁移文件，最新为 `0044_reward_market_candles.sql`
- `packages/backend/init.sql` 已合并 `0001`–`0044`，作为完整空库初始化脚本
- 所有表使用 PostgreSQL 特性（JSONB、NUMERIC 约束、BIGSERIAL、部分索引等）
- 迁移使用 `sqlx` 管理
- Rewards 低竞争市场 sleeve v2 已落地，新增 schema 包括 managed order 的 `strategy_bucket` 和 `reward_low_competition_observations` 观测表；shadow report 是 snapshot 派生结果，不单独落表。Rewards AI advisory 已接入 `reward_market_candles`，K 线由 orderbook 内部盘口更新派生，不直接从外部 API 请求。

## 修改检查清单

- [ ] 新增迁移时使用 `00XX_描述.sql` 命名格式
- [ ] 新增表后在对应的 application Store trait 中添加 CRUD 方法
- [ ] 新增列后同步更新 infrastructure 的 Postgres 实现和 in-memory 实现
- [ ] 新增枚举类型后同步更新 `domain` crate 的对应枚举
- [ ] 修改后运行 `cargo test --workspace` 验证迁移兼容性
- [ ] 更新本文档的迁移列表和 schema 说明
