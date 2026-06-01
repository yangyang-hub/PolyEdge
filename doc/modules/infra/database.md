# 数据库（Migrations + Schema）

最后更新：2026-06-01

## 概述

数据库使用 PostgreSQL，通过 23 个 SQL 迁移文件管理 schema。覆盖审计、市场数据、事件/信号、执行管道、风控、套利、奖励、跟单等领域。

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
| `0020_copy_trading.sql` | 跟单 | `copytrade_config`、`copytrade_wallets`、`copytrade_source_trades`、`copytrade_copy_orders`、`copytrade_positions`、`copytrade_account_state`、`copytrade_events` |
| `0021_copytrade_daily_pnl.sql` | 跟单日 PnL | 修改 `copytrade_account_state`（`daily_realized_pnl`） |
| `0022_reward_bot_control_commands.sql` | 奖励机器人控制命令 | `reward_control_commands` |
| `0023_copytrade_control_commands.sql` | 跟单控制命令 | `copytrade_control_commands` |

## Schema 领域分组

### 1. 审计/幂等

- **`audit_logs`**：完整审计追踪 — actor（user/session/roles）、action、resource、result（accepted/succeeded/rejected/failed）、IP、user agent、payload JSON、version snapshot
- **`idempotency_keys`**：scope + key + request_hash，跟踪 started/completed/failed 状态，有 TTL

### 2. 市场数据

- **`markets`**：question、category、status、best_bid/ask/mid_price（NUMERIC(12,6) 约束 0-1）、volume_24h、ambiguity_level、tradability_status、version、slug、polymarket_condition_id/yes_asset_id/no_asset_id
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

- **`reward_bot_config`**：key-value 配置
- **`reward_markets`**：condition_id、question、market_slug、rewards_max_spread/min_size、total_daily_rate、tokens JSON
- **`reward_quote_plans`**：market FK、scoring、quote plan
- **`reward_managed_orders`**：account_id、condition_id、token_id、filled_size、reward_earned、last_scored_at
- **`reward_fills`**：order_id、account_id、condition_id、token_id、outcome、side、price、size、notional_usd、role、realized_pnl
- **`reward_account_state`**：capital_usd、available_usd、reserved_usd（旧硬占用兼容字段，下一次 rewards tick 自动释放）、realized_pnl、reward_earned_usd、fees_paid、tick_index
- **`reward_control_commands`**：API 入队给 worker 的 rewards 控制命令（run_once/cancel_all/reset）及 pending/running/completed/failed 状态

### 9. 跟单

- **`copytrade_config`**：key-value 配置
- **`copytrade_wallets`**：address、label、status（active/paused）、sizing overrides、rolling stats（trades/volume/PnL/win_rate/ROI）
- **`copytrade_source_trades`**：检测到的源交易（deterministic ID 去重）
- **`copytrade_copy_orders`**：镜像的跟单订单
- **`copytrade_positions`**：按 wallet+market 聚合
- **`copytrade_account_state`**：资金池账本（同 reward_account_state 结构 + daily_realized_pnl）
- **`copytrade_events`**：活动/风险事件日志
- **`copytrade_control_commands`**：API 入队给 worker 的 copytrade 控制命令（run_once/analyze_wallets/cancel_all/reset）及 pending/running/completed/failed 状态

### 10. 运行时配置

- **`runtime_config`**：key TEXT PK、value TEXT、updated_at

## 当前状态

- 23 个迁移文件，最新为 `0023_copytrade_control_commands.sql`
- 所有表使用 PostgreSQL 特性（JSONB、NUMERIC 约束、BIGSERIAL、部分索引等）
- 迁移使用 `sqlx` 管理

## 修改检查清单

- [ ] 新增迁移时使用 `00XX_描述.sql` 命名格式
- [ ] 新增表后在对应的 application Store trait 中添加 CRUD 方法
- [ ] 新增列后同步更新 infrastructure 的 Postgres 实现和 in-memory 实现
- [ ] 新增枚举类型后同步更新 `domain` crate 的对应枚举
- [ ] 修改后运行 `cargo test --workspace` 验证迁移兼容性
- [ ] 更新本文档的迁移列表和 schema 说明
