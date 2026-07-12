# 数据库（Migrations + Schema）

最后更新：2026-07-12

## 概述

数据库使用 PostgreSQL。当前项目按尚未生产迁移、可从空库重新初始化的前提维护；数据库基线已压缩为单个初始化迁移 `0001_initial_schema.sql`，不保留历史增量迁移链。schema 覆盖审计、幂等、LLM 调用、市场数据、事件/证据、执行历史、内部风险状态、新闻、LP rewards market maker、strategy run ledger/replay fixtures、durable action executor、fair-value estimates、adaptive exit reselection、Funding/Polymarket 账户配套、orderbook price-history candles、runtime config 和 BalancedMerge。

本项目现在面向空库重新部署：`packages/backend/init.sql` 与 `packages/backend/migrations/0001_initial_schema.sql` 表达同一份当前 schema，不兼容已删除历史模块的旧表。运行时仍通过 `sqlx::migrate!` 使用单个 baseline 迁移做初始化/校验。

## 初始化入口

- `packages/backend/init.sql`：空库初始化脚本，直接表示当前完整 schema。
- `packages/backend/migrations/0001_initial_schema.sql`：Rust runtime 的单 baseline 内嵌迁移源，内容与初始化基线一致。
- 当前不维护增量迁移链；已有数据库如需保留历史 `_sqlx_migrations`，需要单独规划迁移，不属于当前重新部署目标。

## 迁移文件列表

| 迁移 | 主题 | 核心表/改动 |
|---|---|---|
| `0001_initial_schema.sql` | 当前完整 schema baseline | 全量创建当前所需表、约束和索引，包括 rewards market maker、strategy run ledger、fair-value、selection score、BalancedMerge、Funding、orderbook candles、worker/runtime 支撑表 |

## Schema 领域分组

### 审计/幂等/LLM

- `audit_logs`：actor、action、resource、result、IP、user agent、payload JSON、version snapshot。
- `idempotency_keys`：scope + key + request_hash，跟踪 started/completed/failed、`error_code`、5 分钟执行租约、terminal 时间和 24 小时结果 TTL；request id 充当 lease owner，旧执行者不能在租约被接管后写 terminal 状态。
- `external_event_dedup`：connector callback payload hash、处理结果、trace owner 与 5 分钟租约；未完成事件可在崩溃后重领，完成/abandon 均 owner-fenced。
- `llm_calls`：外部 provider 调用审计/统计，记录 task_type、provider/model、input_hash、raw/parsed output、validation_result、fallback_used、latency、cost_estimate、trace_id 和 created_at。Rewards combined provider 调用复用该表，snapshot 按 UTC 日聚合。

### 市场/事件/新闻

- `markets`：question、category、status、best_bid/ask/mid_price、`liquidity_usd`、volume_24h、`end_at`、`synced_at`、ambiguity_level、tradability_status、slug、Polymarket condition/YES/NO asset id 等。
- `market_resolution_rules`：resolution source 和 edge-case notes。
- `market_categories`：控制台市场分类。
- `raw_events`：source、hash、source_type、external_id、title、url、author、published_at、event_time、payload JSON。
- `news_source_health`：enabled、reliability、last_success/error、consecutive_failures、circuit breaker。
- `evidences`：market/event 关联证据、方向、strength、source reliability、novelty、resolution relevance、status、expiry。
- `probability_estimates`：prior/posterior/fair/market price、edge、confidence、model_version、reason_codes。

### Legacy / 通用执行链路表

- `signals`、`signal_evidence_links`、`signal_transitions`：baseline 仍保留的内部表，当前公开页面/API/worker 不再提供历史 signal 控制台流程。
- `risk_state`：kill switch、PnL、gross/net exposure、open alerts 等执行链路状态。
- `order_drafts`、`execution_requests`、`orders`、`trades`、`positions`：执行链路和 connector callback 使用的订单/成交/持仓历史。
- `arbitrage_*` 与 `market_book_snapshots`：历史 schema 保留，当前不再有 active app/worker/API 写入新 scan/opportunity 数据。

### LP rewards

- `reward_bot_config`：key-value 配置，覆盖 `maker_market_budget_usd`、动态 rank、交易 edge、机会评分、adaptive 退出、AI/info-risk 动作阈值、事件窗口、库存偏斜、非对称 requote 和 BalancedMerge；不再保存旧双预算或成交后整组撤单 key。
- 空表读取使用 application `production_live_drill_defaults()`；无需在 baseline SQL 中硬编码配置行。用户首次保存后会写入完整当前 snapshot，因此重新部署空库会获得最新保守 profile。
- `reward_markets`：condition、question、market_slug、rewards_max_spread/min_size、total_daily_rate、tokens JSON。
- `reward_quote_plans`：当前 quote plan snapshot，主键为 `(condition_id, strategy_profile)`，包含基础/selection score、readiness/mode/reason/blocker、fair-value、`ai_action`、`info_risk_action`/level 摘要和完整 JSON。预算、provider size 与库存 headroom blocker 有独立 reason code。
- `reward_strategy_runs`：每轮 full tick 的 run header，记录 account、trace、trigger、status、config hash/json、输入摘要、指标、开始/完成时间和错误。
- `reward_strategy_runs_started_idx` 支撑不带 account/status 过滤的全局最近运行列表；AI advisory/info-risk 另有 `expires_at` 单列索引支撑全局过期清理。
- `reward_strategy_decisions`：每个 run 下按 condition + strategy profile 记录 quote plan 决策快照、排序、readiness、reason/blocker、planned notional、fair-value/opportunity/event、`ai_action`、`info_risk_action`/level 和 decision JSON。
- `reward_strategy_actions`：从 tick outcome 派生的动作账本，记录 place/cancel/exit/fill/merge/skip 等动作、状态、幂等键、请求/结果 JSON 和关联订单。
- `reward_strategy_actions` 还保存 `lease_owner`、`lease_expires_at` 和 `execution_attempts`，支持多实例 executor 原子 claim/续租和超时恢复；partial index 加速 planned/expired-executing 领取。
- `reward_strategy_replay_fixtures`：与 strategy run 一对一的确定性回放 fixture，保存 schema version、SHA-256、JSON 字节数、JSONB payload 和 captured time，随 run 级联删除。当前默认写 schema V2 紧凑 payload，同表保留 V1/缺省 schema 读取兼容。
- `reward_order_transitions`：托管订单状态追加式时间线，记录 managed/external order、from/to status、reason、metadata，并可关联 run/action。
- `reward_managed_orders`：托管订单，包含 account/condition/token、side、price、size、status、strategy bucket/profile、exit strategy source/selected/floor/reselect state、filled_size、reward_earned、external id 和对账锁等字段。外部库存补 SELL intent 可来自当前 rewards catalog 外的 condition；adaptive 本地 pending SELL 用这些字段在 worker 重启后继续持仓期重评。
- `reward_fills`：托管订单成交，保存 account/condition/token/outcome/side、price、size、notional、role、realized PnL。
- `reward_positions`：按 account + token 保存外部完整持仓，可包含当前 rewards catalog 外的市场。
- 外部持仓替换使用单条 typed `UNNEST`/批量 upsert，而不是事务内逐仓位往返；position、managed fill、merge retry/avg price 和 fair-value/confidence/JSON 均有数据库 CHECK 防线。
- `reward_account_state`：capital、available、reserved 兼容字段、realized PnL、reward earned、fees、tick index、funding address、外部 BUY notional。
- `reward_control_commands`：run_once/cancel_all/reset 命令队列，支持 pending/running/completed/failed；running claim 写入 `lease_owner`、单调 `lease_version` 和 `lease_expires_at`，terminal update 必须匹配 owner/version。
- `reward_worker_heartbeats`：worker running 状态来源。
- `reward_market_advisories`：AI advisory V2 缓存，按 condition/provider/request_format/model/input_hash 存储 `action=allow|reduce|stop_new`、size multiplier、edge buffer、confidence、reasons/metrics JSON 和 expires_at；有效记录固定按 `created_at DESC, id DESC` 取最新，不能按较长 TTL 覆盖新评估；旧 suitability/quote mode/exit policy 列已删除。
- `reward_market_info_risks`：信息风险 V2 缓存，存储 evidence action（含定向 cancel）、risk level/type/direction、resolution_imminent、expected_event_at、confidence、summary、sources/metrics JSON 和 expires_at；单条与 condition 批量读取均按 `created_at DESC, id DESC` 选择最新有效评估。
- `reward_market_candles`：orderbook 服务从 CLOB `/prices-history` 低频写入的 rewards token 5m source candles。provider price 同时写入 close、best bid close 和 best ask close，`spread_cents_close=0`，`sample_count` 表示同 bucket 内新持久化点数量；batch upsert 会排除不晚于现有 close 的重叠点，避免增量回看重复计数。
- `reward_fair_values`：每个 condition 最新 fair-value 估计，保存 fair_yes/fair_no、market midpoint、confidence、uncertainty、YES/NO 偏离、组件 JSON、拒绝原因和有效期。
- `reward_fair_value_history`：fair-value 历史追加表，用于审计和回测；`(condition_id, source, observed_at)` 唯一索引使同一估计重试幂等，数据库维护默认按 `created_at` 保留 90 天。
- `reward_market_event_windows`：按 condition/source 保存事件时间候选；effective 查询按 active、confidence、source 优先级和更新时间选一条。
- `reward_merge_intents`：BalancedMerge 配对库存合并意图，包含 YES/NO token、merge size、两侧库存均价、source fill、status、tx hash、submitted/confirmed/failed 时间、失败原因和 retry count；广播前原子进入不可自动重领的 `broadcasting`，只有该状态能写 `submitted + tx_hash`，链上 receipt 解析再以 intent id + tx hash 双重 fencing 更新 completed/failed。Active paired size 只计算 pending/unsupported/broadcasting/submitted，completed 不抵扣未来新库存。
- 高频 mutable snapshot 使用版本 fencing：managed order、position、account state、heartbeat、event-window、fair-value 和 strategy ledger 不接受较旧时间版本；账户全量持仓替换会先锁定 account state 版本，避免乱序同步先删除新仓位再写回旧仓位。

## 数据保留与自动清理

内嵌 worker runtime 的 `database-maintenance` 任务调用 `DatabaseMaintenanceService`，Postgres 实现按表分批删除历史/缓存/队列数据。默认窗口：

- raw events：未关联 30 天，已关联 90 天。
- 过期 AI advisory / info-risk cache：额外保留 7 天。
- `reward_market_candles`：30 天。
- `reward_fair_value_history`：90 天。
- `reward_strategy_runs`：90 天，删除 completed/failed/cancelled run 时级联删除对应 decisions/actions。
- `reward_order_transitions`：180 天。
- `reward_risk_events`：180 天。
- completed control commands：30 天；failed control commands：90 天。
- outbox/external dedup：30-90 天窗口。
- `llm_calls`：180 天。
- `audit_logs` / `mode_transitions`：365 天。

每个表每轮最多 20 批、每批 10,000 行，避免单次大事务。删除后物理文件不会立即缩小，需要依赖 autovacuum；如需归还磁盘空间，需规划 `VACUUM FULL` / `pg_repack` 等维护窗口。

## 当前状态

- 当前迁移目录只保留单个 baseline：`0001_initial_schema.sql`。
- `packages/backend/init.sql` 与 runtime baseline 表达同一当前 schema。
- 已移除的钱包类和独立研究模块不再有前端路由、API、worker、application store、DTO 或专属 schema；baseline 仍明确保留上述 legacy signal/arbitrage 通用表，不能据此推断旧控制台流程仍可用。
- Rewards 竞争度相关数据只存在于 quote plan 的统一 opportunity metrics 中，不再有独立 observation 表或模块；最终市场选择优先级存于 quote plan 的 `selection_score` / `selection_metrics`。
- Rewards 事件窗口、strategy run ledger、Replay V2 fixture、condition 级 fair-value estimates/幂等 history、AI advisory、信息风险、price-history candles、worker heartbeat、控制命令去重和 BalancedMerge merge intent 已落地。
- 新建数据库直接包含 `reward_fair_value_history_identity_uidx`。保留数据的现场实例在部署新二进制时应先去重同 `(condition_id, source, observed_at)` 历史行，再以独立运维 DDL 创建唯一索引。二进制使用无目标 `ON CONFLICT DO NOTHING`，因此索引未建立时不会中断 live tick，但重试幂等只在索引存在后完整生效。
- Strategy run/decision ledger 是审计和 replay 输入，不作为 live 定价输入；`reward_strategy_actions` 同时是 Postgres-only durable executor 的执行队列。executor 随 rewards poll loop 启动，使用 account-scoped lease、owner fencing 和当前风险/venue 状态复核后执行或恢复受支持动作。
- 数据库维护任务生产模板默认开启；它不删除 rewards fills、positions、account state 等核心账本表。

## 修改检查清单

- [ ] 修改 schema 时优先更新 `init.sql` 和 `migrations/0001_initial_schema.sql` 的当前基线；生产部署前不新增增量迁移。
- [ ] 新增表/列后同步更新 application store trait、infrastructure Postgres/in-memory 实现和前端 DTO（如对外暴露）。
- [ ] 新增枚举后同步更新 `domain` crate 和前端枚举翻译。
- [ ] 修改后运行 `cargo check --workspace --tests`；涉及查询行为时补充 `cargo test --workspace`。
- [ ] 更新本文档的迁移列表和 schema 说明。
