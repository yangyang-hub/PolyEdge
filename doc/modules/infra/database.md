# 数据库（Migrations + Schema）

最后更新：2026-07-07

## 概述

数据库使用 PostgreSQL，通过 58 个 SQL 迁移文件管理 schema。覆盖审计、市场数据、事件/证据、执行历史、内部风险状态、奖励、跟单、Smart Money Intelligence、动态高概率市场定价研究等领域。为了空库一次性初始化，`packages/backend/init.sql` 机械合并了当前全部迁移内容；运行时仍通过 `sqlx` 使用 `packages/backend/migrations/` 做迁移校验和增量升级。旧 signals、risk_state、positions 和 arbitrage 表随已应用迁移保留，用于历史/内部兼容，不再对应前端页面或公开控制台 API。

## 初始化入口

- `packages/backend/init.sql`：完整空库初始化脚本，当前按 `0001` 到 `0058` 顺序展开。
- `packages/backend/migrations/*.sql`：保留给 Rust runtime 的 `sqlx::migrate!` 使用。不要删除或重命名已应用的迁移文件，否则现有数据库的 `_sqlx_migrations` 历史可能与二进制内嵌迁移不一致。
- `init.sql` 只面向空数据库；已存在 schema 的数据库继续使用 runtime 自动迁移或按需执行新增迁移。

## 迁移文件列表

| 迁移 | 主题 | 核心表 |
|---|---|---|
| `0001_support_tables.sql` | 支撑表 | `audit_logs`、`idempotency_keys`、`llm_calls` |
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
| `0044_reward_market_candles.sql` | Rewards price-history K 线 | `reward_market_candles` 及 condition/token 最近 K 线索引 |
| `0045_reward_control_command_dedupe.sql` | Rewards 控制命令去重 | `reward_control_commands` pending/running partial unique indexes |
| `0046_reward_low_competition_competition_share.sql` | Rewards 低竞争竞争份额和资金占比观测 | 修改 `reward_low_competition_observations` |
| `0047_reward_low_competition_not_low_competition.sql` | Rewards 低竞争高竞争混入标签 | 修改 `reward_low_competition_observations` 增加 `not_low_competition` |
| `0048_reward_account_unmanaged_buy_notional.sql` | Rewards 非本系统外部买单占用冻结值 | 修改 `reward_account_state` 增加 unmanaged external buy notional |
| `0049_smart_money_intelligence.sql` | Smart Money Intelligence 基础 schema | `smart_money_config`、`smart_wallet_candidates`、`smart_wallet_profiles`、`smart_wallet_scores`、`smart_wallet_trades`、`smart_signals`、`smart_signal_decisions`、`smart_wallet_advisories`、`smart_signal_advisories`、`smart_paper_executions` |
| `0050_high_probability_pricing_strategy.sql` | 动态高概率市场定价研究基础 schema | `high_probability_config`、`high_probability_samples`、`high_probability_bucket_stats`、`high_probability_observations` |
| `0051_high_probability_market_outcomes.sql` | 动态高概率市场 outcome 标签 | `high_probability_market_outcomes` |
| `0052_high_probability_backtests.sql` | 动态高概率 baseline 回测持久化 | `high_probability_backtest_runs`、`high_probability_backtest_trades` |
| `0053_high_probability_backtest_exit_rules.sql` | 动态高概率 baseline 回测退出规则摘要 | 修改 `high_probability_backtest_runs.exit_rule_reports` |
| `0054_reward_market_event_windows.sql` | Rewards 事件窗口候选 | `reward_market_event_windows` |
| `0055_reward_balanced_merge_strategy.sql` | Rewards 成交后合并策略 | `reward_managed_orders.strategy_profile`、`reward_quote_plans.strategy_profile`、`reward_merge_intents` |
| `0056_reward_managed_orders_external_inventory.sql` | Rewards 外部库存退出 intent | 移除 `reward_managed_orders.condition_id` 到奖励目录的外键 |
| `0057_reward_merge_intent_execution.sql` | Rewards 合并 intent 执行状态 | 修改 `reward_merge_intents` 增加 tx hash、提交/确认/失败时间、失败原因和 retry_count |
| `0058_reward_market_fair_values.sql` | High Probability fair value 快照 | `reward_market_fair_values`，保存保守 fair_yes 区间、置信度、不确定性、reason_codes 和 live_eligible |

## Schema 领域分组

### 1. 审计/幂等/LLM 调用

- **`audit_logs`**：完整审计追踪 — actor（user/session/roles）、action、resource、result（accepted/succeeded/rejected/failed）、IP、user agent、payload JSON、version snapshot
- **`idempotency_keys`**：scope + key + request_hash，跟踪 started/completed/failed 状态，有 TTL
- **`llm_calls`**：外部大模型调用审计/统计表，记录 task_type、model/prompt version、input_hash、raw/parsed output、validation_result、fallback_used、latency、cost_estimate、trace_id 和 created_at；Rewards AI advisory / info-risk provider 调用会复用该表，snapshot 按 UTC 日聚合调用次数

### 2. 市场数据

- **`markets`**：question、category、status、best_bid/ask/mid_price（NUMERIC(12,6) 约束 0-1）、`liquidity_usd`、volume_24h、`end_at`、`synced_at`、ambiguity_level、tradability_status、version、slug、polymarket_condition_id/yes_asset_id/no_asset_id；condition_id 有唯一索引，yes/no asset id 有非空部分索引用于 orderbook priority sync 反查
- **`market_resolution_rules`**：resolution_source、edge_case_notes（text 数组）
- **`market_categories`**：id、label、sort_order（预置 13 个分类：Sports、Politics、Crypto 等）

### 3. 事件/新闻

- **`raw_events`**：source、hash（SHA-256 去重）、source_type（news/social/official/calendar/market）、external_id、title、url、author、published_at、event_time、payload JSON
- **`news_source_health`**：enabled、reliability、last_success/error、consecutive_failures、circuit_breaker_until

### 4. 证据/legacy 信号

- **`evidences`**：market FK、event FK、direction（supports_yes/supports_no/background）、strength、source_reliability、novelty、resolution_relevance（均 0-1）、status、expiry
- **`signals`** / **`signal_transitions`**：随历史迁移保留的 legacy 表；公开 `/signals` 页面/API 和 signal recompute worker 已移除
- **`probability_estimates`**：prior/posterior/fair/market price、edge、confidence、time_horizon、model_version、reason_codes（JSONB）；当前通过 pricing API 读取，不再挂载 signals 页面

### 5. 内部风险状态

- **`risk_state`**：kill_switch、daily_pnl、gross_exposure（0-10）、net_exposure（0-10）、open_alerts、notes 数组；当前仅作为执行链路和 connector callback 兼容状态，不再有前端风控页面或 `/api/v1/risk/*` API

### 6. 执行历史

- **`order_drafts`**：connector_name、side、limit_price、quantity、notional、status、external_order_id、submitted_at、failure_code/message
- **`execution_requests`**：mode、risk_state_version、requested_by_user_id、status、external_order_id、submitted_at、failure_code/message
- **`orders`**：external_order_id、side、limit_price、quantity、filled_quantity、avg_fill_price、status（8 状态）
- **`trades`**：order_id FK、price、quantity、fee、side、role（maker/taker）
- **`positions`**：market/account/connector 聚合 — net_quantity、avg_entry_price、unrealized_pnl、realized_pnl；公开 `/positions` 页面/API 已移除，表保留给执行链路/历史兼容

### 7. 历史套利表

- **`arbitrage_scans`**、**`market_book_snapshots`**、**`arbitrage_opportunities`**、**`arbitrage_opportunity_validations`**、**`arbitrage_events`**：历史 schema 保留，当前 arbitrage application/store/worker/API/frontend 已移除，不再写入新 scan/opportunity 数据

### 8. 奖励机器人

- **`reward_bot_config`**：key-value 配置，包含报价/风控、市场质量、quote/selection mode、dominant 单边阈值、盘口集中度阈值、偏好分类、统一机会评分 `opportunity_*` 竞争/奖励/退出/稳定性/资金占比阈值与权重、BalancedMerge `balanced_merge_*` 独立策略阈值、AI advisory 开关/provider/request format/TTL 和信息风险开关/mode/过滤等级/TTL；`low_competition_*` 旧键仅兼容历史配置，运行时归一化为独立低竞争 sleeve 关闭
- **`reward_markets`**：condition_id、question、market_slug、rewards_max_spread/min_size、total_daily_rate、tokens JSON
- **`reward_quote_plans`**：market FK、scoring、`strategy_profile`（standard/balanced_merge）和 quote plan JSON；当前仍按 condition 替换快照，同一 condition 只保留一个 active profile，standard 优先避免同市场双 profile 自我竞争
- **`reward_managed_orders`**：account_id、condition_id、token_id、strategy_bucket（standard/low_competition/none，当前运行时新订单统一写 standard，low_competition 仅历史兼容）、strategy_profile（standard/balanced_merge）、filled_size、reward_earned、last_scored_at；外部库存补 SELL 退出可能来自当前 rewards catalog 之外的市场，因此 `condition_id` 不再依赖 `reward_markets` 外键
- **`reward_fills`**：order_id、account_id、condition_id、token_id、outcome、side、price、size、notional_usd、role、realized_pnl
- **`reward_positions`**：按 account_id + token_id 保存外部完整持仓；可包含当前 rewards catalog 之外的市场，不再依赖 `reward_markets` 外键
- **`reward_account_state`**：capital_usd、available_usd、reserved_usd（旧硬占用兼容字段，下一次 rewards tick 自动释放）、realized_pnl、reward_earned_usd、fees_paid、tick_index
- **`reward_control_commands`**：API 入队给 worker 的 rewards 控制命令（run_once/cancel_all/reset）及 pending/running/completed/failed 状态；running 超过 5 分钟可重新领取
- **`reward_market_advisories`**：AI advisory 缓存表，按 condition/provider/request_format/model/input_hash 保存 suitability、推荐 quote mode、exit policy、confidence、reasons/metrics JSON 和 expires_at；`input_hash` 使用稳定 cache-key payload（市场身份/问题、奖励参数、计划 quote mode 和相关策略配置），不包含每轮变化的账户、开放订单、持仓或盘口实时字段；worker 只读取未过期记录，缓存未命中时调用 provider 后写入
- **`reward_market_info_risks`**：信息风险缓存表，按 condition/provider/request_format/model/input_hash 保存 query_hash、risk_level、risk_type、directional_risk、resolution_imminent、expected_event_at、confidence、summary、sources/metrics JSON 和 expires_at；`input_hash` 使用稳定 cache-key payload（搜索 query、市场身份/问题/事件、计划 quote mode 和风险策略配置），不包含账户、开放订单、持仓、quote plan reason/score 或 market_synced_at 等动态字段；异步 worker 写入，live rewards tick 只读取未过期缓存
- **`reward_low_competition_observations`**：历史低竞争 sleeve observation 表，按 account/condition/observed_at 记录旧模式、计划 notional、探测 notional、竞争资金、竞争份额 bps、竞争倍数、账户有效可用资金、低竞争开放 BUY notional、加上当前计划后的低竞争/condition 挂单 notional 与 bps 占比、预估 reward/100/day、退出深度/滑点、midpoint 波动、样本不足、低竞争 gate、最终可挂、AI/信息风险拦截、主策略重叠、not_low_competition（高竞争混入标签）和拒绝原因 JSON；当前统一机会评分运行路径不再写入新 observation，snapshot 不再生成低竞争 shadow report。
- **`reward_market_candles`**：orderbook 服务从 CLOB `/prices-history` 低频同步的 rewards token K 线，按 token/interval/bucket 保存 price OHLC、close observed_at 和兼容字段；当前 price-history 行的 `best_bid_close` / `best_ask_close` 等于 provider price，`spread_cents_close=0`，`sample_count` 表示同 bucket 持久化的 provider history 点数量，不包含真实成交量。
- **`reward_market_event_windows`**：按 condition/source 保存 rewards 市场真实事件时间候选，包含 event type、start/end、confidence、source URL/payload、active、review metadata 和 updated_at；有效窗口查询按 active、confidence、source 优先级和更新时间为每个 condition 选一条。Gamma 日期候选默认低/中置信，不会在默认 high hard-gate 阈值下直接触发停挂。
- **`reward_merge_intents`**：BalancedMerge profile 在 YES/NO 库存可配对后写入的合并意图，包含 account/condition、YES/NO token、merge size、两侧持仓 size/均价、source fill、trace、status（pending/unsupported/submitted/completed/failed）、tx hash、submitted/confirmed 时间、failed_reason 和 retry_count；自动执行开关关闭时 worker 创建 `unsupported` intent，开启时创建/读取 `pending` 或 legacy `unsupported` intent 并提交链上 CTF merge，active size 查询会把 non-failed intent 计入防重。
- **统一机会评分**：当前 rewards bot 已把普通市场和原低竞争市场合并到同一 quote plan 流，竞争资金、奖励密度、退出深度/滑点、盘口稳定性和资金占比作为 `opportunity_metrics` 写入 `reward_quote_plans` JSON 并参与评分/可挂资格；旧低竞争 schema（`strategy_bucket=low_competition`、`reward_low_competition_observations`、`low_competition_*` 配置键）仅保留历史/API/DB 兼容。

### 9. Copytrade 钱包跟踪与分析

- **`copytrade_config`**：key-value 配置
- **`copytrade_wallets`**：address、label、status（active/paused）、sizing overrides、rolling stats（trades/volume/PnL/win_rate/ROI）
- **`copytrade_source_trades`**：检测到的源交易（deterministic ID 去重）
- **`copytrade_events`**：活动/风险事件日志
- **`copytrade_control_commands`**：API 入队给 worker 的 copytrade 控制命令（run_once/analyze_wallets/cancel_all/reset）及 pending/running/completed/failed 状态；当前只有 analyze_wallets 有实际分析语义，run/cancel/reset 为历史兼容 no-op
- **旧模拟表**：`copytrade_copy_orders`、`copytrade_positions`、`copytrade_account_state` 仍随迁移存在，用于历史兼容和避免破坏旧数据；当前前端/API snapshot 不再展示模拟账户、订单或持仓，worker 也不会写入新的模拟订单。

### 10. 运行时配置

- **`runtime_config`**：key TEXT PK、value TEXT、updated_at

### 11. Smart Money Intelligence

- **`smart_money_config`**：key-value 配置，当前用于 observe/paper/approval/live_guarded 模式、发现/LLM advisory 开关、样本量、滑点、盘口深度和敞口阈值。
- **`smart_wallet_candidates`**：自动发现或后续导入的钱包候选池，记录 wallet address、source、candidate/watch/tracked/blocked/rejected 状态、最近发现时间、分析时间、晋级/拒绝时间、reason 和 raw payload。
- **`smart_wallet_profiles`**：钱包滚动画像，包含交易数、已结算样本、成交额、realized PnL、ROI、胜率、回撤、平均/中位交易额、活跃天数、市场数、集中度、低流动性交易占比、可跟窗口 stale 占比和最近交易时间。
- **`smart_wallet_scores`**：确定性评分结果，按钱包保存 total/profit/consistency/risk/liquidity/recency/copyability score、tier、解释 JSON 和 scoring version。
- **`smart_wallet_trades`**：标准化源钱包交易，按 deterministic id 去重；第一阶段由 worker 后续接入 Data API 写入，第二阶段可补链上 source 校验。
- **`smart_signals`** / **`smart_signal_decisions`**：源交易转化出的跟随信号和确定性/LLM gate 决策记录；当前 schema 已建立，后续 worker 才会生成信号。
- **`smart_wallet_advisories`** / **`smart_signal_advisories`**：LLM advisory 缓存；只保存模型对结构化 payload 的 allow/observe/reject 建议、confidence、risk tags、reasons 和 raw output，不作为唯一执行依据。
- **`smart_paper_executions`**：纸面跟随执行记录，用于验证信号在延迟和滑点后的真实可跟性；实盘执行不复用该表。

### 12. 动态高概率市场定价研究

- **`high_probability_config`**：key-value 配置，当前用于 observe/paper/live_guarded 兼容模式、市场范围、模型版本、最小 edge、手续费 buffer、风险 margin、最小样本数、spread/depth、研究用仓位上限和 fair value provider 配置（enabled、TTL、市场/历史权重、目标样本量、最大不确定性、盘口 stale 阈值）。paper/live_guarded 仍不是独立执行路径。
- **`high_probability_market_outcomes`**：本地 outcome 标签表，记录 condition 的 resolved/voided/ambiguous 状态、winning token、resolved_at、market type、risk tags 和标签来源；当前由人工/脚本/后续 producer 写入，样本构建不会从 `markets.status` 猜 winning token。
- **`high_probability_samples`**：已构建的 token 时点样本，记录 condition/token、side、sampled_at、trigger kind、可执行价格、价格 bucket、市场类型、剩余时间/liquidity/spread bucket、路径特征、风险标签、最终 outcome、settlement PnL、最大回撤和持仓时间。当前由 `build-high-probability-samples-once` 从本地 outcome 标签和 rewards candles 构建。
- **`high_probability_bucket_stats`**：按 model_version + bucket_key 保存分桶统计，包含样本数、胜场数、胜率、保守 fair probability、期望 PnL、最大回撤、跌破阈值比例、平均持仓时间和推荐最高入场价。
- **`high_probability_backtest_runs`**：一次 baseline walk-forward 回测的可复现运行记录，保存模型版本、市场范围、训练/测试样本数、候选/交易/跳过计数、胜率、PnL、ROI、最大回撤、窗口时间、`exit_rule_reports`、notes 和配置 snapshot。
- **`high_probability_backtest_trades`**：baseline 回测中实际通过 edge gate 的模拟入场明细，关联 run 和 sample，保存 bucket、可执行价格、fair probability、net edge、推荐最高入场价、最终 outcome、单笔/累计 PnL 和 drawdown。
- **`high_probability_observations`**：observe/paper/live guarded 决策记录；当前 `observe-high-probability-once` 和默认关闭的 `POLYEDGE_WORKER__POLL_HIGH_PROBABILITY_OBSERVE` runtime loop 会写入基于本地 rewards candle 候选和 orderbook 服务缓存计算出的只读 `allow/reject/skip` observation，paper/live 仍未实现。
- **`reward_market_fair_values`**：High Probability fair value provider 输出表，按 `(condition_id, model_version)` upsert 当前快照，保存 token、YES/NO 补数来源、可成交价、`fair_yes_low/mid/high`、market implied、历史 base rate、confidence、uncertainty_cents、sample_count、bucket_key、fallback_level、input_hash、reason_codes、live_eligible、computed/expires 时间。API 只读未过期当前 model_version 快照；Rewards 做市商尚未接入该输出。

## 数据保留与自动清理

数据库当前主要依赖通用自动清理链路：

- **通用数据库维护**：内嵌 worker runtime 的 `database-maintenance` 周期任务调用 `DatabaseMaintenanceService`，Postgres 实现按表分批删除历史/缓存/队列数据。默认窗口为 raw events 未关联 30 天、已关联 90 天；过期 AI advisory / info-risk cache 额外保留 7 天；`reward_market_candles` 30 天；completed control commands 30 天、failed control commands 90 天；`copytrade_events` 90 天、`copytrade_source_trades` 180 天；published outbox 30 天、failed/dead_letter outbox 90 天；processed external dedup 90 天、stale unprocessed dedup 7 天；`llm_calls` 180 天；`audit_logs` / `mode_transitions` 365 天。

维护任务每个表每轮最多删除 20 批、每批 10,000 行，避免单次大事务；删除后 PostgreSQL 物理文件不会立即缩小，需要依赖 autovacuum 回收可复用空间，若要把 9GB 这类已膨胀文件还给操作系统，需要计划 `VACUUM FULL` / `pg_repack` 等维护窗口。

## 当前状态

- 58 个迁移文件，最新为 `0058_reward_market_fair_values.sql`
- `packages/backend/init.sql` 已合并 `0001`–`0058`
- 所有表使用 PostgreSQL 特性（JSONB、NUMERIC 约束、BIGSERIAL、部分索引等）
- 迁移使用 `sqlx` 管理
- 旧套利表仍随迁移存在，但 arbitrage application/store/worker/API/frontend 已移除，当前不会继续写入新 scan/opportunity 数据。
- 通用数据库维护已接入 API 内嵌 worker runtime，生产模板默认开启 `POLYEDGE_WORKER__DATABASE_MAINTENANCE=true`，用于防止缓存、日志、队列和低频 price-history 表持续增长；它不删除 rewards fills/positions/account state 等核心账本表。
- Rewards 低竞争市场 sleeve 已合并到统一机会评分，现有低竞争 schema 仅作为历史兼容保留；当前 active 指标为 `reward_quote_plans.quote_plan_json.opportunity_metrics`，包含 competition-share/multiple、100U 日奖、资金占比、退出深度/滑点、坏成交恢复天数、盘口样本/波动/跳变和机会分。Rewards 事件窗口已落地 `reward_market_event_windows`，live tick 会把 effective window 写入 `reward_quote_plans.quote_plan_json.event_window` 并用于阻断新增 BUY 或撤已有 BUY。BalancedMerge 成交后合并 profile 已落地 `strategy_profile` 列、`balanced_merge_*` 配置和 `reward_merge_intents` 表；自动执行默认关闭，开启 `balanced_merge_auto_execute_enabled` 后 worker 会读取 `pending/unsupported` intent 并通过 Safe proxy wallet 广播 CTF merge，提交结果写回 tx hash/submitted/failed 字段。Rewards AI advisory 已接入 `reward_market_candles`，5m source K 线由 orderbook 服务统一低频限速调用 CLOB `/prices-history` 写入，不由 worker/API 直接请求外部接口；AI advisory 在 application 层把这些 source candles 聚合为 1h 输入。Rewards AI advisory / info-risk 的实际 provider 调用复用 `llm_calls` 做每日调用统计，通用数据库维护保留 180 天。
- Smart Money Intelligence 当前已落地基础 schema 和后端 service/store/API；`scan-smart-money-once` 会从 active copytrade tracked wallets 写入候选、近端样本画像、确定性评分和 Data API activity 源交易。自动全网发现、信号生成、LLM advisory refresh、纸面模拟和实盘 guarded execution 仍是待实现阶段。
- 动态高概率市场定价研究当前已落地基础 schema 和后端 service/store；`build-high-probability-samples-once` 会从本地 outcome 标签 + rewards candles 构建 first-touch 样本，`refresh-high-probability-buckets-once` 会从已入库 `high_probability_samples` 计算并替换当前模型版本的 bucket stats，`run-high-probability-backtest-once` 会持久化 baseline walk-forward backtest runs/trades 和基础退出规则摘要，`observe-high-probability-once` 和默认关闭的自动 observe runtime loop 会把只读扫描结果写入 `high_probability_observations`，`refresh-high-probability-fair-values-once` 和默认关闭的自动 fair value loop 会在 `fair_value_enabled=true` 时 upsert `reward_market_fair_values`。全市场 price-history/outcome producer、完整执行成本/多阶段退出回测、Phase 4 校准与漂移监控、Rewards 做市商引用关系仍未实现；High Probability 不再作为独立 paper/live guarded execution 推进。

## 修改检查清单

- [ ] 新增迁移时使用 `00XX_描述.sql` 命名格式
- [ ] 新增表后在对应的 application Store trait 中添加 CRUD 方法
- [ ] 新增列后同步更新 infrastructure 的 Postgres 实现和 in-memory 实现
- [ ] 新增枚举类型后同步更新 `domain` crate 的对应枚举
- [ ] 修改后运行 `cargo test --workspace` 验证迁移兼容性
- [ ] 更新本文档的迁移列表和 schema 说明
