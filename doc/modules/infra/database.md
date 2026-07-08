# 数据库（Migrations + Schema）

最后更新：2026-07-08

## 概述

数据库使用 PostgreSQL。当前基线包含 48 个 SQL 迁移文件，最新文件为 `0059_reward_adaptive_exit_reselection.sql`。schema 覆盖审计、幂等、LLM 调用、市场数据、事件/证据、执行历史、内部风险状态、新闻、LP rewards market maker、fair-value estimates、adaptive exit reselection、Funding/Polymarket 账户配套、orderbook price-history candles、runtime config 和 BalancedMerge。

本项目现在面向空库重新部署：`packages/backend/init.sql` 已按当前 `migrations/` 目录重新生成，不兼容已删除历史模块的旧表。运行时仍通过 `sqlx::migrate!` 使用 `packages/backend/migrations/` 做迁移校验。

## 初始化入口

- `packages/backend/init.sql`：空库初始化脚本，按当前迁移文件顺序展开。
- `packages/backend/migrations/*.sql`：Rust runtime 的内嵌迁移源。
- `init.sql` 只面向新数据库；已有数据库如需保留历史 `_sqlx_migrations`，需要单独规划迁移，不属于当前重新部署目标。

## 迁移文件列表

| 迁移 | 主题 | 核心表/改动 |
|---|---|---|
| `0001_support_tables.sql` | 支撑表 | `audit_logs`、`idempotency_keys`、`llm_calls` |
| `0002_market_event_core.sql` | 市场/事件核心 | `markets`、`market_resolution_rules`、`raw_events` |
| `0003_evidence_signal_core.sql` | 证据/legacy 信号 | `evidences`、`signals` |
| `0004_pricing_and_signal_transitions.sql` | 概率估计/legacy 转换 | `probability_estimates`、`signal_transitions` |
| `0005_risk_state.sql` | 内部风险状态 | `risk_state` |
| `0006_signal_rejection_metadata.sql` | legacy 信号元数据 | 修改 `signals` |
| `0007_execution_request_core.sql` | 执行请求 | `order_drafts`、`execution_requests` |
| `0008_execution_dispatch_metadata.sql` | 执行分发元数据 | 修改 `order_drafts`、`execution_requests` |
| `0009_orders_trades_positions.sql` | 订单/成交/持仓 | `orders`、`trades`、`positions` |
| `0010_market_connector_refs.sql` | Polymarket 引用 | 修改 `markets` |
| `0011_news_ingestion.sql` | 新闻采集 | 修改 `raw_events`、新增 `news_source_health` |
| `0012_news_source_health_list_index.sql` | 新闻源索引 | 修改 `news_source_health` |
| `0013_arbitrage_radar.sql` | 历史套利 schema | `arbitrage_scans`、`market_book_snapshots`、`arbitrage_opportunities` |
| `0014_arbitrage_validation_events.sql` | 历史套利验证 | `arbitrage_opportunity_validations`、`arbitrage_events` |
| `0015_reward_bot.sql` | Rewards 基础 | `reward_bot_config`、`reward_markets`、`reward_quote_plans`、`reward_managed_orders` 等 |
| `0016_runtime_config.sql` | 运行时配置 | `runtime_config` |
| `0017_market_slug.sql` | 市场 slug | 修改 `markets` |
| `0018_market_categories.sql` | 市场分类 | `market_categories` |
| `0019_reward_simulation.sql` | Rewards 账本扩展 | 修改 rewards orders/fills/account state |
| `0022_reward_bot_control_commands.sql` | Rewards 控制命令 | `reward_control_commands` |
| `0024_reward_markets_active_index.sql` | Rewards 市场索引 | active + daily rate 索引 |
| `0025_markets_active_volume_index.sql` | 市场活跃度索引 | open/tradable + 24h volume |
| `0026_reward_control_running_lease_index.sql` | 控制命令租约 | running + started_at 部分索引 |
| `0027_remove_paper_trade_manual_confirm.sql` | 运行模式收敛 | runtime/mode/execution request 约束 |
| `0028_reward_positions_external_inventory.sql` | Rewards 外部持仓 | 移除 `reward_positions.condition_id` 对 rewards 目录 FK |
| `0029_reward_account_wallet_address.sql` | Rewards 资金地址 | 修改 `reward_account_state` |
| `0030_rewards_snapshot_indexes.sql` | Rewards snapshot 索引 | `reward_fills`、`reward_positions` 查询索引 |
| `0031_worker_query_indexes.sql` | Worker 查询索引 | orders、raw_events 等查询索引 |
| `0032_reward_worker_heartbeats.sql` | Rewards worker 心跳 | `reward_worker_heartbeats` |
| `0033_reward_candidate_filter.sql` | Rewards 候选过滤 | 修改 `reward_bot_config` |
| `0034_reward_account_external_buy_notional.sql` | 外部 BUY notional | 修改 `reward_account_state` |
| `0035_auto_cancel_not_found_orders.sql` | 历史订单修复 | 调整历史 managed order 状态 |
| `0036_restore_not_found_reconciliation.sql` | 恢复 404 对账 | 修复待最终对账状态 |
| `0037_reward_market_quality.sql` | 市场质量 | `markets` 增加 liquidity/end/synced 字段和质量索引 |
| `0038_reward_market_advisories.sql` | AI advisory 缓存 | `reward_market_advisories` |
| `0039_reward_market_info_risks.sql` | 信息风险缓存 | `reward_market_info_risks` |
| `0040_markets_quality_index_no_synced_at.sql` | 索引写放大优化 | 重建 `idx_markets_reward_quality` |
| `0041_market_asset_id_lookup_indexes.sql` | asset id 反查 | YES/NO asset id 部分索引 |
| `0042_reward_order_strategy_bucket.sql` | Rewards bucket | `reward_managed_orders.strategy_bucket` |
| `0044_reward_market_candles.sql` | Rewards price-history K 线 | `reward_market_candles` |
| `0045_reward_control_command_dedupe.sql` | 控制命令去重 | pending/running partial unique indexes |
| `0048_reward_account_unmanaged_buy_notional.sql` | 外部买单占用 | 修改 `reward_account_state` |
| `0054_reward_market_event_windows.sql` | 事件窗口 | `reward_market_event_windows` |
| `0055_reward_balanced_merge_strategy.sql` | BalancedMerge | `strategy_profile`、`reward_merge_intents` |
| `0056_reward_managed_orders_external_inventory.sql` | 外部库存退出 | 移除 managed orders 对 rewards 目录 FK |
| `0057_reward_merge_intent_execution.sql` | merge intent 执行状态 | tx hash、提交/确认/失败时间、失败原因、retry_count |
| `0058_reward_fair_value.sql` | 做市 fair-value | `reward_fair_values`、`reward_fair_value_history` |
| `0059_reward_adaptive_exit_reselection.sql` | Adaptive 退出重评 | `reward_managed_orders` 增加退出策略来源、当前具体策略、floor、重选次数和最近重选时间 |

## Schema 领域分组

### 审计/幂等/LLM

- `audit_logs`：actor、action、resource、result、IP、user agent、payload JSON、version snapshot。
- `idempotency_keys`：scope + key + request_hash，跟踪 started/completed/failed 状态和 TTL。
- `llm_calls`：外部 provider 调用审计/统计，记录 task_type、provider/model、input_hash、raw/parsed output、validation_result、fallback_used、latency、cost_estimate、trace_id 和 created_at。Rewards combined provider 调用复用该表，snapshot 按 UTC 日聚合。

### 市场/事件/新闻

- `markets`：question、category、status、best_bid/ask/mid_price、`liquidity_usd`、volume_24h、`end_at`、`synced_at`、ambiguity_level、tradability_status、slug、Polymarket condition/YES/NO asset id 等。
- `market_resolution_rules`：resolution source 和 edge-case notes。
- `market_categories`：控制台市场分类。
- `raw_events`：source、hash、source_type、external_id、title、url、author、published_at、event_time、payload JSON。
- `news_source_health`：enabled、reliability、last_success/error、consecutive_failures、circuit breaker。
- `evidences`：market/event 关联证据、方向、strength、source reliability、novelty、resolution relevance、status、expiry。
- `probability_estimates`：prior/posterior/fair/market price、edge、confidence、model_version、reason_codes。

### Legacy 执行链路表

- `signals`、`signal_transitions`：随历史迁移保留的内部表，当前公开页面/API 不再使用。
- `risk_state`：kill switch、PnL、gross/net exposure、open alerts 等执行链路状态。
- `order_drafts`、`execution_requests`、`orders`、`trades`、`positions`：执行链路和 connector callback 使用的订单/成交/持仓历史。
- `arbitrage_*` 表：历史 schema 保留，当前不再有 active app/worker/API 写入新 scan/opportunity 数据。

### LP rewards

- `reward_bot_config`：key-value 配置，覆盖市场质量、quote/selection、dominant 单边、盘口集中度、偏好分类、统一机会评分、fair-value、adaptive post-fill 退出与 pending-exit 重评、AI advisory、信息风险、事件窗口、首单 gate、库存、requote、reconcile、BalancedMerge 等参数。
- `reward_markets`：condition、question、market_slug、rewards_max_spread/min_size、total_daily_rate、tokens JSON。
- `reward_quote_plans`：当前 quote plan snapshot，包含 market FK、score、`strategy_profile` 和 quote plan JSON。JSON 可携带 opportunity metrics、fair-value decision、event window、AI advisory、info-risk、readiness 和 blocker reasons。
- `reward_managed_orders`：托管订单，包含 account/condition/token、side、price、size、status、strategy bucket/profile、exit strategy source/selected/floor/reselect state、filled_size、reward_earned、external id 和对账锁等字段。外部库存补 SELL intent 可来自当前 rewards catalog 外的 condition；adaptive 本地 pending SELL 用这些字段在 worker 重启后继续持仓期重评。
- `reward_fills`：托管订单成交，保存 account/condition/token/outcome/side、price、size、notional、role、realized PnL。
- `reward_positions`：按 account + token 保存外部完整持仓，可包含当前 rewards catalog 外的市场。
- `reward_account_state`：capital、available、reserved 兼容字段、realized PnL、reward earned、fees、tick index、funding address、外部 BUY notional。
- `reward_control_commands`：run_once/cancel_all/reset 命令队列，支持 pending/running/completed/failed 和 running lease。
- `reward_worker_heartbeats`：worker running 状态来源。
- `reward_market_advisories`：AI advisory 缓存，按 condition/provider/request_format/model/input_hash 存储 suitability、推荐模式、confidence、reasons/metrics JSON 和 expires_at。
- `reward_market_info_risks`：信息风险缓存，按 condition/provider/request_format/model/input_hash 存储 risk level/type/direction、resolution_imminent、expected_event_at、confidence、summary、sources/metrics JSON 和 expires_at。
- `reward_market_candles`：orderbook 服务从 CLOB `/prices-history` 低频写入的 rewards token 5m source candles。provider price 同时写入 close、best bid close 和 best ask close，`spread_cents_close=0`，`sample_count` 表示同 bucket 内 provider history 点数量。
- `reward_fair_values`：每个 condition 最新 fair-value 估计，保存 fair_yes/fair_no、market midpoint、confidence、uncertainty、YES/NO 偏离、组件 JSON、拒绝原因和有效期。
- `reward_fair_value_history`：fair-value 历史追加表，用于审计和回测；数据库维护默认按 `created_at` 保留 90 天。
- `reward_market_event_windows`：按 condition/source 保存事件时间候选；effective 查询按 active、confidence、source 优先级和更新时间选一条。
- `reward_merge_intents`：BalancedMerge 配对库存合并意图，包含 YES/NO token、merge size、两侧库存均价、source fill、status、tx hash、submitted/confirmed/failed 时间、失败原因和 retry count。

## 数据保留与自动清理

内嵌 worker runtime 的 `database-maintenance` 任务调用 `DatabaseMaintenanceService`，Postgres 实现按表分批删除历史/缓存/队列数据。默认窗口：

- raw events：未关联 30 天，已关联 90 天。
- 过期 AI advisory / info-risk cache：额外保留 7 天。
- `reward_market_candles`：30 天。
- `reward_fair_value_history`：90 天。
- completed control commands：30 天；failed control commands：90 天。
- outbox/external dedup：30-90 天窗口。
- `llm_calls`：180 天。
- `audit_logs` / `mode_transitions`：365 天。

每个表每轮最多 20 批、每批 10,000 行，避免单次大事务。删除后物理文件不会立即缩小，需要依赖 autovacuum；如需归还磁盘空间，需规划 `VACUUM FULL` / `pg_repack` 等维护窗口。

## 当前状态

- 当前迁移文件数为 48，最新为 `0059_reward_adaptive_exit_reselection.sql`。
- `packages/backend/init.sql` 已由当前迁移重新生成。
- 已删除历史模块的迁移、表、store、handler 和前端 DTO 不在当前基线中。
- Rewards 竞争度相关数据只存在于 quote plan 的统一 opportunity metrics 中，不再有独立 observation 表或模块。
- Rewards 事件窗口、fair-value estimates、AI advisory、信息风险、price-history candles、worker heartbeat、控制命令去重和 BalancedMerge merge intent 已落地。
- 数据库维护任务生产模板默认开启；它不删除 rewards fills、positions、account state 等核心账本表。

## 修改检查清单

- [ ] 新增迁移时使用 `00XX_描述.sql` 命名格式。
- [ ] 新增表/列后同步更新 application store trait、infrastructure Postgres/in-memory 实现和前端 DTO（如对外暴露）。
- [ ] 新增枚举后同步更新 `domain` crate 和前端枚举翻译。
- [ ] 修改后运行 `cargo check --workspace --tests`；涉及查询行为时补充 `cargo test --workspace`。
- [ ] 更新本文档的迁移列表和 schema 说明。
