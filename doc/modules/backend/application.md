# application（应用/服务层）

最后更新：2026-06-24

## 概述

`polyedge_application` crate 是系统最大的 crate，包含所有业务逻辑服务、Store trait（端口定义）、命令/查询类型和领域视图模型。它定义了系统**做什么**，但不关心**怎么做**（由 infrastructure/connectors 实现）。

## 设计目标

- 遵循六边形架构：定义 Store trait 作为端口，不依赖具体实现
- 所有业务编排逻辑集中在本层
- 通过 `Arc<dyn XxxStore>` 和 `Arc<dyn XxxService>` 实现依赖注入

## 架构与关键文件

| 文件/目录 | 职责 |
|---|---|
| `lib.rs` | 模块声明 + `pub use` 收敛对外 API（~86 行） |
| `system_mode.rs` | 系统模式管理：`SystemModeService`、`ModeStateStore`、`IdempotencyStore`、`AuditLogSink`、`AuthenticatedActor` |
| `market_event/` | 核心市场/事件/信号：`MarketEventService`、`MarketEventStore`（最大 Store trait） |
| `execution/` | 执行管道：`ExecutionService`（组合 MarketEventService + RiskService） |
| `risk.rs` | 风控：`RiskService`、`RiskStateStore`、`RiskPolicy`、kill-switch 命令 |
| `rewards/` | 做市奖励：`RewardBotService`、`RewardBotStore`、质量过滤/排序、静态事件风险过滤、首单 info-risk/quarantine gate、盘口指标/单边报价推荐、price-history candles、低竞争 sleeve profile/指标 gate/shadow report、AI advisory 输入/决策/执行约束、异步信息风险缓存、priority condition 列表、live-only 状态与订单分页查询、历史清理、events/fills/open_order_count 内存缓存、in-process command wake channel；`RewardBotStore` trait 拆在 `service/store.rs`，service 单元测试拆在 `service/tests.rs`，配置默认值/归一化/patch 逻辑拆在 `config_impl.rs`，运行时模型拆在 `runtime_models.rs`，quote/selection/AI 枚举拆在 `quote_selection_models.rs`，AI 模型拆在 `ai_advisory_models.rs`，信息风险模型拆在 `info_risk_models.rs`，deterministic 盘口选择 helper 拆在 `planner_selection.rs`，live 盘口 materializer 拆在 `planner_live.rs`，低竞争指标拆在 `low_competition.rs`，低竞争 observation/report 聚合拆在 `low_competition_report.rs`，snapshot 聚合拆在 `service_snapshot.rs` |
| `copytrade/` | 钱包跟踪与分析：`CopyTradeService`、`CopyTradeStore`、tracked wallets、source trades、钱包分析和控制命令队列；旧模拟引擎已移除 |
| `arbitrage/` | 套利：`ArbitrageService`、`ArbitrageStore`、机会检测/验证、扫描历史清理 |
| `news_ingestion.rs` | 新闻采集：`NewsIngestionService`、`NewsIngestionStore` |
| `maintenance.rs` | 数据库维护：`DatabaseMaintenanceService`、`DatabaseMaintenanceStore`、集中 retention cutoffs 与清理统计 |
| `orderbook_cache.rs` | 盘口缓存：`OrderbookCache` trait、`CachedOrderBook` 和内部推送事件 `OrderbookStreamEvent` |
| `orderbook_registry.rs` | 盘口订阅注册中心：`OrderbookSubscriptionRegistry` trait，多来源 token 聚合 |
| `wallet_analysis/` | 钱包分析：纯计算（无 I/O），`build_wallet_analysis_report` |
| `list_filters.rs` | 通用分页/过滤辅助 |

## 核心数据结构与服务

### system_mode — 系统治理骨架

**Store Traits（3 个）：**
- `ModeStateStore`：`current() -> ModeSnapshot`、`transition(command) -> ModeSnapshot`
- `IdempotencyStore`：`begin(request) -> IdempotencyBegin`、`complete(request, response_json)`、`fail(request, error_code)`
- `AuditLogSink`：`append(entry)`

**关键类型：**
- `AuthenticatedActor`：user_id、session_id、roles、request_id、ip、user_agent — 被几乎所有模块引用
- `ModeSnapshot`：当前 SystemMode、环境、版本、更新时间
- `ModeTransitionCommand`：目标模式、原因、幂等键、请求哈希、actor、required_scope
- `AuditLogEntry`：完整审计日志条目

**服务：** `SystemModeService` — 管理模式转换（含幂等性检查和审计日志）

### market_event — 核心数据管理

**Store Trait：** `MarketEventStore` — 系统最大的 Store trait
- Markets：`list_markets`、`count_markets`、`get_market`、`get_markets_by_ids`、`upsert_markets`、`upsert_markets_with_options`、`list_market_categories`
- Signals：`get_signal`、`list_signals`、`recompute_signal`、`approve_signal`、`reject_signal`
- Events/Evidence：`list_events`、`list_evidences`、`list_probability_estimates`
- Execution：`list_order_drafts`、`list_execution_requests`、`submit_execution_request`
- Orders/Trades/Positions：`list_orders`、`list_trades`、`get_order_by_external_ref`、`list_positions`
- Dispatch/Reconciliation：`list_dispatch_candidates`、`list_reconciliation_candidates`、`mark_execution_submitted`、`reconcile_execution_fill` 等

**服务：** `MarketEventService` — 所有方法是对 Store 的薄代理；`get_markets_by_ids()` 会规范化、去重 market id 并批量读取相关市场，供需要少量关联市场信息的 API 避免全量列表扫描；`MarketUpsertOptions` 可控制 market upsert 对 `markets.synced_at` 是每次刷新还是仅在超过指定秒数后刷新，供 orderbook full sync 降低无变化市场的写放大。

### execution — 执行管道

**服务：** `ExecutionService`
- 依赖：`Arc<MarketEventService>` + `Arc<RiskService>` + `Arc<dyn AuditLogSink>`
- 不定义自己的 Store trait，复用 `MarketEventStore`
- 关键方法：`submit_execution_request`（校验信号 ID 和原因，委托到 MarketEventStore）

### risk — 风控

**关键类型：**
- `RiskStateSnapshot`：kill_switch、daily_pnl、gross/net_exposure、open_alerts、version
- `RiskPolicy`：可配置阈值（exposure_reference_nav、min_signal_confidence、max_daily_loss 等）
- `ApproveSignalCommand`/`RejectSignalCommand`：乐观并发控制（expected_version）

**服务：** `RiskService` — 信号审批/拒绝、kill-switch 触发/释放

### rewards — 做市奖励

**Store Trait：** `RewardBotStore`（定义在 `rewards/service/store.rs`）
- Config：`load_config`、`save_config`（key-value 模式）
- Markets：`upsert_markets`、`list_markets`、`list_all_active_markets`、`active_market_summary`
- Quote Plans：`save_quote_plans`（替换当前计划快照）、`list_quote_plans`
- Low Competition Observations：`record_low_competition_observations`、`list_low_competition_observations`，用于 snapshot shadow report 聚合，不改变配置或自动启用 enforce
- Market Candles：`record_market_candle_sample`、`list_recent_market_candles`，保存 orderbook 服务低频 `/prices-history` 同步的 5m price OHLC，供 AI advisory 读取最近 K 线和摘要
- AI Advisory / Info Risk：`latest_market_advisory`、`save_market_advisory`、`latest_market_info_risk(s)`、`save_market_info_risk`，按 condition/provider/request_format/model/input_hash 缓存未过期模型判断；信息风险结果还保存 query hash、风险等级、风险类型、方向性、来源和有效期
- Orders/Positions/Events：完整 CRUD；订单支持 `RewardOrderListQuery` 后端分页并在 snapshot 中返回 `orders_page`，其中 `orders_status=filled` 会同时匹配 `status=filled` 和 `filled_size > 0` 的部分成交订单；live worker 可按 external Polymarket order id 查 managed order、用 fill id 做成交幂等，并通过 `latest_fill_at(account_id)` 查询账户最近 confirmed fill 时间；`prune_history(cutoff)` 只清理 cutoff 之前的终态订单（`cancelled`/`filled`/`error`）、risk events 和低竞争 observations，保留 `planned`/`open`/`exit_pending`、fills、positions 和 account state
- State Tick：`apply_tick_outcome`（原子持久化 orders/fills/positions/ledger/events，不修改奖励市场目录或 quote plan 快照）、`apply_account_sync`（更新账户；`Some(positions)` 原子替换该账户全部持仓，`None` 保留持仓）、`reset_state`（重置账户状态、清空 orders/fills/positions）
- Control Commands：`enqueue_control_command`、`claim_next_control_command`、`complete_control_command`、`fail_control_command`

**服务：** `RewardBotService` — 读写配置、市场管理、快照聚合、订单分页快照、live tick 计划准备、轻量 live state 读取、priority rewards condition 列表、rewards 控制命令入队/领取/完成状态管理，以及受控历史清理。服务内部缓存 config、account、positions、events（最新 200 条）、fills（最新 200 条）、external_open_order_count 和 worker heartbeat，API 与内嵌后台 runtime 共享实例时直接读写这些热状态；缓存为空时回退到数据库；清理历史时会同步裁掉内存中的旧 events。`status.open_orders` 只统计已有非内部 `external_order_id`、仍是 open-like、本地剩余量为正且未处于提交未知、取消未知、404 人工对账或 `awaiting final reconciliation` 锁定的 managed orders；本地尚未提交的 planned/exit intent、已完全成交但状态尚未终态化的 open-like 行、已接受取消但仍等待最终对账的订单仍留在订单列表/同步队列中，但不作为当前 Polymarket 开放挂单展示；`status.error` 只从当前 open-like 订单的活跃对账锁推导，不会把短暂 `awaiting final reconciliation` 当作错误，也不会被历史 critical event 永久污染；`status.blocker_counts` 按 quote plan reason 聚合等待盘口、AI pending/低置信度/watch/avoid、信息风险、低竞争 observe-only、资金不足、live 盘口验证和其它拦截，供控制台解释可挂数量变化。配置保存会推进 runtime revision；控制命令入队会合并同账户同动作的 `pending/running` 重复命令，Postgres 侧还有 partial unique index 防并发重复，只有真正入队时才推进 revision 并通过 command_wake channel 立即唤醒 worker poll loop。live tick 准备新 quote plans 时会记录 AI 过滤前的 deterministic eligible condition 集合，把该状态持久化为 quote plan 的 `pre_ai_eligible`，并保留 `orderbook_token_ids` 供后台 AI provider refresh 临时订阅使用；最终 `rewards_eligible` 订阅只覆盖通过 AI/info-risk gate 后仍可挂单的 quote plan token；同时从上一版 quote plan 继承未过期且 provider/request_format/model 匹配的 AI advisory，也会继承尚未过期且非 transient 的 live 盘口验证跳过标记；worker 在 AI/info-risk cache gate 后记录低竞争 observation，但会等订单/账户同步和 live action 盘口刷新完成，再用当前 books materialize quote readiness 后保存 quote plan snapshot，避免控制台读到“eligible 但尚未 live 验证”的中间态；缺少/过期 orderbook 或 quote plan missing token 的旧跳过标记不再继承，后续 tick 等待订阅缓存恢复后重新 materialize；worker 读取盘口时会优先使用内部 WS 维护的本地缓存，但对本地缺失、超过 `stale_book_ms` 或接近新挂单 freshness headroom 的 token 会提前通过 orderbook HTTP batch 读取服务端缓存并回填本地缓存，默认 45 秒 stale 窗口下新挂单允许约 35 秒内的确认盘口并在本地年龄超过约 25 秒时预刷新，避免 worker 本地缓存断流、临界跨线或落在不可下单年龄窗口后长期卡住；live tick 只读取已有缓存，缺 provider 缓存仍 fail closed，但 AI watch 或低置信度 allow/watch 会回退 deterministic 计划继续进入 live 盘口/资金风控，不等待外部 provider；worker 会在后台用独立 AI advisory task 刷新缺失 advisory 缓存，并用独立 info-risk task 刷新信息风险缓存，供后续 tick 使用。奖励市场目录替换拒绝空 snapshot，修改 `account_id` 前会检查旧账户状态。面向控制台的 snapshot 不携带全量 active reward markets，但会返回最近 24 小时低竞争 shadow report。缓存辅助方法拆分在 `service_cache.rs`，store trait 拆在 `service/store.rs`，service 单元测试拆在 `service/tests.rs`，账户/订单/成交/事件/report/snapshot/历史清理统计类型拆分在 `runtime_models.rs`。
worker 成功读取 CLOB open-order snapshot 后，会把仍出现在该 snapshot 中且外部剩余量为正、状态非 filled/matched/cancelled/expired 的本系统 managed 外部订单数量写入 `external_open_order_count` 热缓存，snapshot 优先展示该观测值；冷启动或尚未成功同步时才回退到 store 的本地 managed order 计数。

**执行模式：**
- `RewardExecutionMode` 枚举仅保留 `Live` 变体，`FromStr` 仍把旧字符串（`validation`、`dry_run`、`paper`、`simulation`）归一为 live；`execution_mode` 字段已从 `RewardBotConfig` / patch 中移除，Store 读取旧 `execution_mode` 配置键时直接忽略。
- `RewardBotConfig.quote_bid_rank` 仅允许 1–3，默认 1（买一）；粗 tick 盘口按第 1/2/3 个不同买价选择，细 tick 盘口会先按从买一回退 `rank-1` 个 0.01 价格步长计算目标价，再选择不高于该目标价的当前买盘档位，避免 0.001 tick 下买三只退两个细档；旧的中间价偏移字段 `quote_edge_cents` 已移除。
- `RewardBotConfig.quote_mode=double|auto` 与 `selection_mode=observe|enforce` 控制确定性盘口选择。默认 `double + observe` 保持既有双边报价；`auto + enforce + dominant_single_side_enabled=true` 时，planner 只用 YES/NO 概率区间生成 `double` / `single_yes` / `single_no` / `none` 初步计划，不在计划构建阶段因盘口档位、退出深度、top1/top3 深度占比、HHI、`per_market_usd` 或 `quote_size_usd` 淘汰市场。live placement 通过 `materialize_reward_quote_plan_for_live_orderbook()` 读取当前盘口后再验证目标档位、rewards spread、盘口集中度、退出深度、安全边际和实际 size/notional；双边目标档位、rewards spread、touch ask 或安全边际不满足时，也在此阶段按实际目标档位价格尝试通过同一校验的单腿回退。`observe` 只在 quote plan 记录推荐模式和 `book_metrics`，不改变实际挂单。
- AI advisory 定义低频模型判断的输入、输出和执行约束：`build_reward_ai_advisory_request()` 从开放订单、持仓和已通过初步 planner 筛选的 eligible 奖励市场、确定性计划、账户/仓位/开放订单、盘口 top levels、最近 24 根 5m price-history candles 和 candle summary 构建完整 provider payload；`input_hash` 不再使用完整动态 payload，而是用市场身份/问题、奖励参数、计划 quote mode、相关策略配置和 candle summary 组成的稳定 cache-key payload 计算 SHA-256，避免账户余额、开放订单、盘口时间戳或盘口档位抖动导致后台刚保存的 advisory 下一轮无法命中，同时让趋势/波动摘要变化能触发重新评估。新保存的 provider cache 会按 condition/provider/request_format/model/input hash 计算确定性正向 TTL jitter（最多 TTL 的 20%，且最多 15 分钟），避免同批写入的 AI/info-risk 记录在同一秒集中失效；同一窗口也作为后台提前刷新阈值，缓存仍有效时 live gate 继续使用旧记录。`apply_existing_reward_ai_advisories()` 只把已有 advisory 挂到 quote plan，不把缺失 advisory 的计划改为 pending；`apply_reward_ai_advisories()` 用于 live tick 的缓存 gate，并在 `ai_advisory_enabled=true` 时只把缺少 advisory 或 `avoid` 决策的原 eligible 计划改为不可挂，AI `watch`、低置信度 `allow/watch` 或非 avoid 的 `quote_mode=none` 会保留 deterministic 报价腿并继续进入 live 盘口/资金风控；`allow` 的 `single_yes/single_no` 收窄只在置信度达到阈值且 `selection_mode=enforce` + `quote_mode=auto` 时应用。后台 AI advisory provider refresh 只写入 `reward_market_advisories` 缓存，不阻塞 live tick，也不使用旧 cycle 覆盖 quote plan snapshot；AI 请求按 worker 设置的每轮 condition cap 扫描，并按最多 10 个市场一批注册 `rewards_ai_provider` 临时 orderbook source，下一批会取消上一批，结束后清空；AI 请求可由配置 `ai_advisory_batch_size` 控制单次 provider 请求包含的市场数（默认 1 保持逐市场，最大 12），批量响应按 condition 拆分保存，漏项/错配市场回退到单市场请求；非 `avoid` advisory 后会把该市场 token 即时合并到 `rewards_eligible` source，后续 full tick 再用持久 quote plan 校正。遇到 HTTP 传输失败、429、认证失败或常见 5xx 会停止本轮请求。`reward_market_books_available()` 在后台 refresh 请求前校验该市场所有报价 token 盘口都已发布（非空 bids 与 asks），盘口缺失/为空的市场本轮跳过请求且不写缓存，等 orderbook 订阅/缓存返回盘口后再评估，避免缓存键不含即时盘口的设计下被一条空 watch/advise 卡住整个 TTL；AI advisory 与 info-risk provider refresh 分别使用独立候选队列、独立 running flag 和独立 provider permit，info-risk 不再挤占 AI advisory 的 selected condition 名额。advisory cache key `schema_version` 已升到 5，使从 orderbook-derived candles 切换到 price-history candles 前的 advisory 失效并重新评估。`reward_ai_advisories_from_quote_plans()` 用于 full tick 新计划保存前从上一版 snapshot 搬运未过期且 provider/request_format/model 匹配的 advisory。默认 `ai_advisory_enabled=false`；配置 wire value 使用 `ai_provider=openai|anthropic` 和 `ai_request_format=openai_responses|openai_chat_completions|anthropic_messages`，后端兼容读取旧 `open_ai*` 拼写但序列化始终输出无下划线 `openai*`。provider confidence 在 connector 解析时钳制到 `0..=1`；`avoid` 仍不能绕过市场质量、盘口和风控硬过滤；成交后退出策略由 `RewardBotConfig.post_fill_strategy` 决定，不再由 advisory `exit_policy` 覆盖。
- Info risk 定义异步信息流风险判断的输入、输出和执行约束：`build_reward_info_risk_assessment_request()` 从奖励市场、当前 quote plan、账户/仓位/开放订单和策略配置构建完整 provider payload 与搜索 query，并在 provider payload 中加入 `evaluation_time_utc` 与 imminent 判定策略，要求模型按该 UTC 时间判断近期/临近风险；`query_hash` 仍按 query 计算，`input_hash` 改为用搜索 query、市场身份/问题/事件、计划 quote mode 和风险策略配置组成的稳定 cache-key payload 计算 SHA-256，不包含每轮变动的账户、开放订单、持仓、quote plan reason/score、`market_synced_at` 或 `evaluation_time_utc`。info-risk cache key `schema_version=3` 会让未携带评估时间策略的旧缓存失效并重新评估；新保存的 info-risk 记录也使用与 AI advisory 相同的确定性 TTL jitter / 提前刷新窗口。`apply_reward_info_risks()` 会把最新未过期风险挂到 quote plan。默认 `info_risk_enabled=false`；provider confidence 在 connector 解析时钳制到 `0..=1`。主 info-risk provider refresh 的请求可由配置 `info_risk_batch_size` 控制单次请求包含的市场数（默认 1，最大 12），批量响应按 condition 拆分保存，漏项/错配市场回退到单市场请求；该 refresh 与 AI advisory refresh 独立运行、独立 cap、独立 provider permit。当 `info_risk_mode=enforce` 时，缺少未过期风险缓存会 fail closed，eligible 计划会被置为不可挂；`require_info_risk_before_first_quote`（默认 true）还会要求新 condition 首次 BUY 报价前已有未过期风险缓存，`first_quote_quarantine_sec`（默认 300，0 可关闭）要求新 condition 至少观察对应秒数；已有 open-like 订单或持仓的 condition 跳过该首单 gate。已有风险达到置信度阈值后，`critical`、官方结果、`resolution_imminent=true` 或配置为 `low/medium` 避免等级时命中的风险等级会硬拦截，普通 `high` 风险以及仅 `risk_type=imminent_resolution` 但 `resolution_imminent=false` 的结果保留为 quote plan 上的信息提示并继续进入 live 盘口/资金风控。独立 info-risk worker 与 full-tick info-risk provider refresh 都受 `POLYEDGE_REWARDS__INFO_RISK_MAX_MARKETS_PER_CYCLE` 每轮 condition cap 约束（默认 50）。该结果不能绕过市场质量、盘口和风控硬过滤。
- 市场质量默认门槛为 `min_market_liquidity_usd=1000`、`min_market_volume_24h_usd=1000`、`min_hours_to_end=48`、`max_market_spread_cents=10`、`max_market_data_age_minutes=15`。候选 prefilter 还会拒绝 FDV/launch-day、token launch/airdrop、official-result/listing 这类高跳变事件风险市场。旧 `reward_competition_factor`、`single_sided_divisor_c`、`fill_rate_per_tick`、`max_fill_ratio` 和 `auto_cancel_stale_minutes` 配置键读取时忽略。
- 低竞争 rewards sleeve 已实现 v2，默认 `low_competition_mode=off`。它使用独立 `standard` / `low_competition` candidate profile，不全局降低主策略 `min_market_liquidity_usd` / `min_market_volume_24h_usd`；低竞争 profile 只放宽自身流动性/24h 成交量门槛，仍共享 open/tradable、高歧义、唯一 YES/NO、结算时间、Gamma spread、数据新鲜度和 FDV/launch/token/official-result 事件风险硬过滤。`low_competition_mode=observe` 只写入 `strategy_bucket` 和 `low_competition_metrics`，不会实盘挂单；`enforce` 需要竞争资金、预估 reward/100/day、退出深度、盘口历史样本和 midpoint 波动全部达标，并要求 `ai_advisory_enabled=true` 且 `info_risk_enabled=true/info_risk_mode=enforce`，之后仍由既有 AI/info-risk cache gate fail closed。Worker 在 AI/info-risk gate 后写入低竞争 observation，snapshot 汇总最近 24 小时 shadow report（样本数、通过率、reward 中位/P90、退出深度倍数、中点波动 P95、退出滑点 P95、AI/信息风险拦截、主策略重叠和小额 enforce 建议）。独立 sleeve 市场数、开放订单和库存 cap 会在 live placement 使用，但仍受全局 `max_markets`、`max_open_orders`、kill switch、盘口和账户外部 BUY notional 风控约束；report 不会自动切换配置。
- Worker 使用当前 quote plan 通过 `LivePolymarketConnector` 提交 post-only token 买单，并对本系统托管的 live 订单执行撤单；该模式由 rewards 配置控制，与全局 system mode 解耦，但遵守 `RiskService` 全局 kill switch。Polymarket 返回 `matched` / `delayed` 等非 live 接受状态时，worker 会把它视为 post-only 安全违规并立即尝试撤单，并保留为待最终成交/取消对账状态。

**控制命令类型：**
- `RewardControlAction`：`run_once`、`cancel_all`、`reset`
- `RewardControlCommandStatus`：`pending`、`running`、`completed`、`failed`
- `RewardControlCommand`：API 与 worker 之间的数据库命令消息

**订单分页类型：**
- `RewardOrderListQuery`：orders search/status/sort/page/page_size 查询
- `RewardListPage`：`page`、`page_size`、`total_items`、`total_pages`
- `RewardBotSnapshot.orders_page`：service 层当前本地 managed `orders` 数组对应的分页元数据

**Quote plan 类型：**
- `RewardQuotePlan`：包含市场、得分、eligible、`pre_ai_eligible`、`quote_readiness`、`strategy_bucket`、quote mode、推荐 mode、`book_metrics`、`low_competition_metrics`、AI advisory、info risk、midpoint、`orderbook_token_ids`、报价腿和 rewards 参数；`eligible=true` 表示策略候选仍值得订阅/跟踪，`quote_readiness=ready_to_quote|waiting_orderbook|provider_pending|blocked` 表示面向 UI 细分和 snapshot 就绪计数的真实报价就绪状态。`ready_to_quote` 要求计划 eligible、quote mode 非 none 且报价腿已有真实 price/size/notional；等待盘口的计划仍可保持 eligible 以进入长期 `rewards_eligible` 订阅，但不会计入 `status.ready_quote_markets`。`orderbook_token_ids` 保存 AI/info-risk gate 前的 YES/NO token，供后台 provider refresh 临时订阅盘口；live placement 缺少、过期或已接近 stale 边界的新鲜盘口时会保持 eligible 并写入等待 orderbook 订阅数据的 reason，不写 `live_skip_until`；非 transient 盘口验证失败才写入 `live_skip_until` / `live_skip_reason`，后续计划准备阶段在有效期内继承该跳过标记。
- `RewardLiveQuoteMaterialization`：live placement 用当前 orderbook materialize 后的 quote mode、推荐 mode、盘口指标、midpoint 和真实报价腿。

**Tick 结果类型：** `RewardTickOutcome`（在 `rewards/engine.rs` 中定义），包含 account、markets、plans、orders、positions、fills、events 和 report。模拟引擎已移除；生产 live 路径通过 worker 的 `LivePolymarketConnector` 直接执行。

**资金与盘口约束：**
- 未成交 post-only maker 买单不在本地按全局 notional 硬锁同一笔 USDC，同一资金池可同时在多个不同市场报价。
- 买单计划不再把 `per_market_usd` 或 `quote_size_usd` 作为报价腿构造额度。live materializer 只把 `rewards_min_size` 向上对齐到 CLOB 两位小数成本精度，并同时满足 Polymarket 1 美元最小名义金额；`rewards_min_size` 是份额数量，单腿成本按 `price * size` 计算。是否能新增报价由 live placement 使用实际 `account.available_usd` 扣除未归属外部 BUY notional 后的余额判断：同一 condition 的已有 managed BUY 剩余 notional 与待补 YES/NO 腿总 notional 必须合计不超过该余额；若待补最低 rewards size 腿已经放不下，worker 会把该 quote plan 标为不可挂并写入 funding reason，等后续余额/开放订单同步后由下一轮重新评估。
- 报价计划构建阶段不再用 `quote_bid_rank` 缺档、目标价 rewards spread、盘口集中度、盘口价格预算、`per_market_usd` 或 `quote_size_usd` 过滤候选；计划腿可只是携带 YES/NO token 的占位元数据。planner 和 live materializer 读取盘口 midpoint/档位时按 `RewardOrderBook.confirmed_at` 判断新鲜度，`observed_at` 只表示盘口内容版本。live placement 准备创建订单时才用当前 orderbook materialize 真实腿：报价价格按 `quote_bid_rank` 选择目标盘口价，粗 tick 使用第 N 个不同买价，细 tick 使用从买一回退 `rank-1` 个 0.01 价格步长后的不高于目标价档位；双边计划优先要求 YES/NO 两腿都存在目标档位且通过 rewards spread、touch ask 和安全边际校验，auto/enforce/dominant 下若双边的档位、spread、touch ask 或安全边际不可行则尝试只挂通过同一校验的一条腿；单边计划只要求目标侧存在。缺少/过期新鲜盘口时不提交订单、保持计划 eligible 并等待 orderbook 订阅/缓存返回；没有可行单腿的目标档位价格距离各自中间价超过 `min(market rewards_max_spread, config.max_spread_cents)` 等非 transient 验证失败时不下单且写入 `live_skip_until`/`live_skip_reason`，跳过标记默认 12 小时后失效以便奖励范围或盘口变化后重新评估；开放订单与最新目标档位价格相差超过 `requote_drift_cents` 时不会立即全量撤单，而是先经过 `requote_drift_confirm_sec` 历史盘口同向确认、`requote_drift_cooldown_sec` 最小挂单年龄和 `requote_drift_max_cancels_per_cycle` 单轮限速后才作为换价撤单候选。
- `max_spread_cents` 归一化范围是 `0.1..=99`，与前端校验及二元概率价格有效范围一致；市场 `rewards_max_spread` 按 CLOB 原始 cents 直接使用，不做百分比换算。
- `max_markets=0` 或 `max_open_orders=0` 表示不再新挂单；`quote_size_usd=0` 不再禁用报价。
- 缺少新鲜缓存盘口时不会提交新 post-only 订单，也不会把市场写入长期 live skip；placement 会保持候选等待 orderbook 订阅数据返回，并在本地盘口缺失、超过 `stale_book_ms` 或超过新挂单 freshness headroom 时从 orderbook 服务 HTTP batch 尝试刷新，batch 会携带预刷新确认年龄，orderbook 服务若自身缓存也缺失或超龄会同步 CLOB `/books` 后再返回；必须看到 YES/NO 两腿非空且距离 stale 边界仍有余量的新鲜盘口后才会创建 intent；默认 `stale_book_ms=45000` 时 placement 最大盘口年龄约 35 秒，HTTP 预刷新阈值约 25 秒，避免 intent 刚落库就因下一轮 reconcile 判定盘口过期而撤单。
- live tick 在候选盘口初读之后还会执行 AI/info-risk gate；gate 完成后 worker 会立即用本轮内存 quote plan 注册 `rewards_eligible` orderbook source，避免新 eligible token 等待周期注册任务。订单同步和账户同步完成后，进入撤单、待提交 intent 和新挂单前，worker 会针对当前 open-like 订单与 eligible quote plan token 再做一次本地 cache / orderbook HTTP batch 刷新并合并到本轮 books，随后先 materialize quote readiness 并保存快照，避免 tick 内 I/O 耗时让初读盘口在 placement 阶段变旧，也避免控制台读到未 live 验证的中间态。
- 全局敞口门槛使用「已有库存 notional + 当前候选单腿 notional」做准入。
- 单次 rewards tick 使用 `list_reward_run_candidate_markets()` 从 `reward_markets` 读取候选池；Postgres 路径关联 Gamma `markets`，硬过滤非 open/tradable、高歧义、低流动性、低 24h 成交量、临近结算、Gamma spread 过宽、同步过期或异常超前、奖励不足、奖励 spread 无效、不具备唯一 YES/NO token 以及 FDV/launch/token/official-result 等高事件跳变风险市场。默认 midpoint 仍受 `min_midpoint..max_midpoint` 限制；auto 单边开启后 SQL 会额外允许 `dominant_min_probability..dominant_max_probability` 及其反向区间进入候选。SQL 不再用 `rewards_min_size <= per_market_usd` 做预算预筛，高最小份额市场会保留到 live materializer 和实际钱包余额准入层处理；live placement 会按当前账户资金把无法补齐最低 rewards size 的计划标为不可挂。SQL 按 CLOB 原始 cents 使用 rewards spread，再按奖励、流动性、成交量、剩余时长和有效奖励 spread 的综合质量分排序；planner 可对 `preferred_categories` 命中的 Gamma 分类追加评分。
- `list_priority_reward_condition_ids()` 为 orderbook priority sync 返回重点 condition：当前开放/持仓市场优先，其次最终 eligible 或 pre-AI deterministic eligible quote plans，最后使用放宽新鲜度窗口的 rewards 候选，以便 stale catalog 仍能恢复重点市场 `markets.synced_at`。

**live 资金模型：**
- Rewards live maker 下单沿用跨市场软资金复用语义：不同 condition 的本系统未成交 post-only/GTC 买单可复用同一资金池；但 Polymarket 会对同一 condition 的全部开放 BUY 订单累计做余额有效性检查，因此 placement 会先计算该 condition 已有 managed BUY 剩余 notional 与待补 YES/NO 腿总 notional。账户开放 BUY 总额会同步到 `external_buy_notional`；worker 会先把 CLOB open-order snapshot 中可唯一映射到 active reward market YES/NO token 的开放 BUY 收养/重开为 managed order，其余无法归属到本系统 managed order 的外部 BUY notional 才会从 `available_usd` 中保守扣除，再做同 condition 准入，避免人工/其它机器人挂单与本系统新单叠加。SELL、非 rewards 市场和无法唯一映射 token 的外部开放订单明细仍未按 condition 映射。
- Live 新挂单仍要求目标 YES/NO 两腿都有非空盘口；`stale_book_ms` 默认 45000，orderbook 默认 10 秒 poll reconcile 会配合 WS 让盘口保持在新挂单新鲜度窗口内，配置归一化下限为 5000ms，不再允许生产配置把盘口年龄检查降到 0。quote plan midpoint/materialize、新挂单和撤单 stale 判断使用 orderbook `confirmed_at`，因此安静市场只要最近被 poll/WS 确认过，不会因为 `observed_at` 内容版本长期不变而被判过期。新挂单路径遇到盘口缺失、空盘口、超过 `stale_book_ms` 或已进入 placement freshness headroom 时，会先对缺失、过期或超过新挂单 freshness headroom 的 token 通过 orderbook 服务 HTTP batch 尝试刷新；默认保留 10 秒 stale 余量（短 stale 窗口保留半窗），仍无足够新鲜度余量的盘口时保持计划等待 orderbook 缓存恢复，而不是写入 12 小时 skip；新建 quote intent 与已落库待提交 BUY 在提交前都会复用 live 撤单风控（计划仍 eligible、报价漂移、min depth、bid rank、depth drop、fill velocity、mass cancel、best ask touch、kill switch 等），并在真正 POST 前用 orderbook 服务做 1 秒 max-age last-look，风险不通过的本地 intent 会在提交前取消，last-look 缺盘口或刷新失败则 fail closed 等下轮重试。live reconcile 会对本系统托管的开放订单读取活跃 token 盘口；盘口缺失/空盘口、SELL 盘口过期、BUY 的非 stale 硬风险或超过短暂 grace 的 BUY stale-only 风险会触发撤单，即使 `enabled=false` 已停止新增报价；近期已有 external order id 的 BUY 只在单纯 stale 且仍处于 grace 窗口内时延迟撤单，价格漂移只在 reprice guard 确认后按单轮上限撤单，资格、深度、best ask touch 和 kill switch 等硬风险仍不延迟。
- full tick 的 live action 阶段会重新刷新当前 open-like 订单与 eligible quote plan token 的盘口后再做撤单/提交/新挂单判断；这次刷新复用 worker 本地 cache 与 orderbook HTTP batch，只拉取本地缺失、过期或接近 placement headroom 的 token（默认在 placement 最大年龄前预留刷新余量）。
- Live `reset` 不清空本地账本或删除托管订单；worker 会先按 cancel-all 语义撤销本系统托管 live 订单，若任一 Polymarket 撤单被拒绝，则命令失败并保留本地状态以避免孤儿实盘订单。
- 风险控制重点放在成交后：trade 达到 `CONFIRMED` 后，worker 对本系统托管 rewards 订单按 external trade id 幂等更新现金、库存、fills 和 PnL，并撤掉 sibling legs；新挂单的 per-token 和全局库存门槛都使用「已有库存 notional + 当前候选订单 notional」准入。
- 本地仍需保留 `max_open_orders`、`max_markets`、per-token/全局库存和 kill-switch；这些限制控制操作风险和订单风暴，而不是把所有开放买单当作已消耗资金。
- 当前 live 已具备 post-only token 买单提交、订单撤单、本系统托管订单 confirmed 成交同步、同轮多笔 trade 累计入账、成交后对侧 buy sibling 撤单以及 `ExitAtMarkup` / `HoldAndRequote` / `FlattenImmediately` sell 下单；既有 sell exit 不属于 sibling cancel 目标。报价与退出订单先持久化 intent，买入 fill 与退出 intent 同事务写入，提交结果未知时保持 open-like 锁定状态等待开放订单匹配恢复或人工对账，不自动重复提交；外部订单 404 会先保持 open-like 对账锁，若超过 5 分钟仍无 CLOB/Data API 成交证据则自动本地标记为 `cancelled`；提交结果未知订单在恢复查询确认 CLOB 无对应 open 订单后经过 `LIVE_SUBMISSION_UNKNOWN_CLOSE_AFTER_SECS`（默认 600 秒）宽限也会自动本地关闭（与 404 锁一致），但若完整 positions 快照显示提交后出现了对应 BUY 库存，则保留对账锁等待确认，避免把可能已成交订单误关。worker 还会用 CLOB open orders snapshot 反查普通 managed BUY：未归属但 token 可唯一映射到 active reward market 的开放 BUY 会被收养为 managed order，已有同 external id 的非 open 本地 BUY 会在 CLOB 仍 open 时重开；已提交、open-like 且无提交未知、404、pending cancel、post-only violation 等对账锁的 BUY 若已不在外部开放订单列表中，会本地关闭为 `cancelled`，释放开放挂单计数；sell exit 不走该快速关闭路径。`ExitAtMarkup` 价格以被吃买单原价加 `exit_markup_cents` 为基准并向上取整到 0.01 tick，默认加价为 0；`HoldAndRequote` 按被吃买单原价持久化 post-only SELL 退出 floor intent，之后继续正常报价；外部 positions 快照检测到尚无 open-like SELL 的非零库存时，也会按该持仓 `avg_price` 创建 post-only 原价 SELL 退出 floor intent；明确退出拒单使用有界退避并在达到最大拒绝次数后停止自动重试，提交前低于 1 美元最小名义金额的退出单会进入短 reason 的 dust deferred 状态，每 300 秒重新评估但不重复拼接历史原因；`FlattenImmediately` 会持久化非 post-only flatten intent，提交前读取当前 best bid，best bid 不低于 floor 时按 best bid 用 FAK/taker SELL 尝试非亏损平仓，best bid 缺失或低于 floor 时按 30 秒退避保留本地 deferred exit 并重试。同 token 退出未完成时暂停新增买单。worker 会把外部 balance、账户开放买单总 notional 观测和完整 positions 快照写入 store，资金钱包地址优先使用 `FUNDER`，且 CLOB balance 为 0/失败时会用链上 pUSD 余额回填账户 snapshot；账户开放买单总 notional 与 managed BUY open-list 反查不受 confirmed fill 保护期影响，并用于估算未归属到本系统的外部 BUY notional 以保守限制新增买单；balance/positions 替换仍会根据 `latest_fill_at` 在 confirmed fill 后保护本地账户状态 120 秒。保护期结束后，成功 positions 快照原子替换该账户全部持仓，失败时保留上一版。SELL、非 rewards 市场和无法唯一映射 token 的外部开放订单明细以及奖励结算对账仍是缺口。
- SELL 退出 intent 的持久化价格是非亏损退出 floor（策略期望价与当前持仓 `avg_price` 的较高值）；提交前不使用 midpoint 或页面“当前价”降价。`ExitAtMarkup`、`HoldAndRequote` 和外部库存补退出按该 floor 提交 post-only maker SELL；若当前 orderbook 买一已大于等于 floor，原价卖单会穿盘口，因此只记录 `reward_live_exit_post_only_crossing_deferred` 并按 30 秒退避等待可 resting 的盘口。`FlattenImmediately` 只在 best bid 不低于 floor 时用非 post-only FAK/taker SELL 按 best bid 提交；best bid 低于 floor 或盘口缺失时递延，不提交亏损卖单。
- 未决提交、待最终对账或外部订单 404 会暂停新增 live 买单但继续卖出退出；外部订单 404 锁超过 5 分钟且仍无成交证据时会自动本地关闭；post-only exit 被取消后的 replacement 保留退出 floor 并按 maker 规则重试，flatten replacement 保留退出 floor 并在后续按 best bid 非亏损 FAK 或继续等待。
- 新建或恢复的 buy order 初始 `scoring=false`，只有 CLOB `orders_scoring` 权威查询可以置为 true；`min_depth_usd` 检查会从聚合盘口中扣除本系统订单自身的剩余 notional，只把外部支撑深度计入阈值。
- 仍需要用真实小额账户验证跨市场资金复用和账户范围外开放订单的组合影响，不能依赖 venue 替系统做组合风险管理。
- 参考官方文档：Order Lifecycle / Requirements 和 Orders Overview / Validity Checks，后续实现时需要复核最新文档。

### copytrade — 钱包跟踪与分析

**Store Trait：** `CopyTradeStore`
- Config/Wallets/SourceTrades/Events/AccountState 读写，以及旧 Orders/Positions 表的兼容读写路径
- Source trades：记录 Data API 检测到的钱包成交，按 deterministic id 去重
- Wallet analysis：保存钱包滚动统计（trades、volume、PnL、win_rate、ROI 等）
- Control Commands：`enqueue_control_command`、`claim_next_control_command`、`complete_control_command`、`fail_control_command`

**服务：** `CopyTradeService` — 配置管理、钱包管理、source trade 检测与记录、钱包分析统计、控制命令入队/领取/完成状态管理。

**控制命令类型：** `CopyControlAction`（run_once/analyze_wallets/cancel_all/reset）、`CopyControlCommandStatus`、`CopyControlCommand`

**当前语义：**
- Copytrade 不下单、不撤单，也不维护模拟资金账本、模拟订单、模拟持仓或 PnL 面板。
- Worker 从 Polymarket Data API 读取 active tracked wallets 的 activity/positions，用 `detect_and_record_source_trades()` 写入 source trades。
- `AnalyzeWallets` 控制命令和 `analyze-wallets-once` 会读取同一批钱包输入并更新钱包分析统计。
- `RunOnce`、`CancelAll`、`Reset` 仍作为数据库控制命令兼容值存在；当前 worker 中这些动作是 no-op，不应在产品文案里描述成真实跟单或模拟交易。
- 旧 `copytrade_copy_orders`、`copytrade_positions`、`copytrade_account_state` 表仍存在用于迁移兼容和历史数据，但当前前端/API snapshot 不再展示模拟账户、订单或持仓。

### arbitrage — 套利

**Store Trait：** `ArbitrageStore`
- Scan lifecycle、market book snapshots、opportunities、validations、analysis runs、events、history prune

**核心函数：** `detect_arbitrage_opportunities`、`validate_arbitrage_opportunity`、`build_arbitrage_analysis`

**历史清理：** `ArbitrageService.prune_scan_history(started_before)` 返回 `ArbitrageHistoryPruneReport`，删除 cutoff 前的 `arbitrage_scans`，依赖数据库 FK cascade 清理 `market_book_snapshots`、`arbitrage_opportunities` 和 validations；`prune_events()` 继续单独清理 `arbitrage_events`。

### news_ingestion — 新闻采集

**Store Trait：** `NewsIngestionStore`
- `insert_raw_news_event`（SHA-256 去重）、`record_news_source_success/failure`、`list_news_source_health`

**服务：** `NewsIngestionService` — 批量采集、去重、健康追踪

### maintenance — 数据库维护

**Store Trait：** `DatabaseMaintenanceStore`
- `prune_database_history(cutoffs)`：按统一 retention cutoff 清理数据库历史/缓存/队列表。

**关键类型：**
- `DatabaseMaintenanceCutoffs`：集中定义各类表的保留窗口。当前默认包括 raw events 未关联 30 天、已关联 90 天，AI/info-risk 过期缓存额外 7 天 grace，rewards candles 30 天，completed control commands 30 天、failed control commands 90 天，copytrade events 90 天、source trades 180 天，outbox published 30 天、failed/dead_letter 90 天，external dedup processed 90 天、stale unprocessed 7 天，LLM calls 180 天，audit/mode transitions 365 天。
- `DatabaseMaintenanceReport`：逐表返回删除行数，并提供 `total_deleted()` 汇总。

**服务：** `DatabaseMaintenanceService` — 由 worker 定期调用，application 层只定义策略和端口，不直接依赖 Postgres。

### orderbook_cache — 盘口缓存

**Trait：** `OrderbookCache`
- `get_book(token_id)`、`get_books(token_ids)`、`get_books_with_max_age(token_ids, max_age_ms)`、`set_book(book)`、`set_books(books)`、`get_stale_tokens(token_ids, max_age_ms)`、`entry_count()`
- `max_age_ms <= 0` 表示关闭年龄 stale 检查，但具体实现仍可按 TTL 判定过期。
- `get_books_with_max_age()` 默认退化为 `get_books()`；`OrderbookHttpClient` 会把正数 `max_age_ms` 作为 `refresh_if_stale_ms` 传给 orderbook 服务，由服务端在自身缓存缺失或超龄时同步刷新后返回。

**类型：**
- `CachedOrderBook`：token_id、bids、asks、observed_at、confirmed_at、source；`observed_at` 表示盘口内容版本时间，`confirmed_at` 表示服务最近确认该 token 盘口仍可用的时间，旧消息缺失 `confirmed_at` 时回退使用 `observed_at`
- `OrderbookStreamReason`：`book`、`price_change`、`poll_reconcile`、`ingest`
- `OrderbookStreamEvent`：orderbook 服务内部 WS 推送事件，包含单调 sequence、reason 和规范化 `CachedOrderBook`

**实现：**
- `InMemoryOrderbookCache`（infrastructure crate）— 进程内缓存，仅供 orderbook 服务使用
- `OrderbookHttpClient`（connectors crate）— HTTP 客户端，Worker 和 API 通过此实现调用 orderbook 服务

### orderbook_registry — 盘口订阅注册中心

**Trait：** `OrderbookSubscriptionRegistry`
- `register_tokens(source, token_ids) -> Result<()>` 原子替换来源有序 token 集合；另有 `unregister_source`、`unregister_tokens`、`list_all_tokens()`、`total_token_count()`、`source_count()`、`has_source()`、`changed_since()`

**实现：**
- `InMemoryOrderbookSubscriptionRegistry`（infrastructure crate）— 进程内注册中心，仅供 orderbook 服务使用，保留来源内顺序并按 live rewards / execution / candidates / copytrade 优先级聚合
- `OrderbookHttpClient`（connectors crate）— HTTP 客户端，Worker 通过此实现携带共享写 token 注册 token 到 orderbook 服务；注册失败返回错误

### wallet_analysis — 钱包分析

**纯计算模块**，无 Store trait、无 Service struct
- 输入：`ClosedPositionInput`、`TradeInput`、`OpenPositionInput`、`ActivityInput`
- 输出：`WalletAnalysisReport`（profile/pnl/activity/categories/style/risk/top_markets/recent_trades 等）
- 入口：`build_wallet_analysis_report()` — 同步纯函数

## 模块依赖关系

```
system_mode ← (基础，无依赖)
    ↑
market_event ← (核心数据)
    ↑
execution ← (组合 market_event + risk + audit)
    ↑
risk ← (依赖 market_event + system_mode)
    ↑
rewards ← (独立；仅支持 live 实盘模式)
copytrade ← (独立，集成 wallet_analysis)
arbitrage ← (可能使用 orderbook_cache)
news_ingestion ← (独立，输出供 signal pipeline 使用)
maintenance ← (独立，集中数据库 retention 策略)
orderbook_cache ← (共享基础设施 trait)
```

## 当前状态

- 所有模块已实现完整的 Store trait 和 Service struct
- Rewards 已移除旧 validation/simulation tick 引擎，仅保留 live-only 配置、quote planner、live orderbook materializer、确定性盘口指标/单边 quote mode、AI advisory 输入/决策/缓存端口、信息风险输入/决策/缓存端口、首单入场 gate、状态类型和增量持久化端口。
- Rewards AI advisory 已读取 orderbook 服务持久化的 5m price-history candles；payload 包含最近 candle 序列和摘要，cache key 只包含摘要，不包含完整 K 线数组；AI advisory 与 info-risk 新缓存保存时会加入确定性 TTL jitter 并在同一窗口内提前续期，降低大批缓存同步过期导致的 provider pending。
- Rewards live 模式已接入质量硬过滤与综合排序、post-only token 买单、撤单、本系统托管订单成交同步、成交后现金/库存/PnL 更新、可持续重试的 exit/flatten sell、CLOB open-order 反查、可映射 active rewards BUY 的收养/重开、外部余额/完整持仓快照、managed order scoring 和 UTC 当日账户级 maker rewards 同步（聚合端点优先、明细端点 fallback）；新增买单会把未归属到本系统 managed order 的外部 BUY notional 从可用资金中保守扣除，并要求 orderbook 盘口距离 stale 边界仍有余量，近期 BUY 的 stale-only 撤单有短暂 grace 以吸收 registry/poll 抖动。SELL、非 rewards 市场和无法唯一映射 token 的外部开放订单明细与奖励结算对账仍待完成。
- Rewards SELL 退出单提交前会保留非亏损 floor：`ExitAtMarkup`、`HoldAndRequote` 和外部库存补退出走 post-only maker SELL，当前买一穿过 floor 时递延等待可 resting 的盘口；`FlattenImmediately` 只在 best bid 不低于 floor 时走非 post-only FAK/taker SELL，best bid 低于 floor 时递延，避免亏损卖出；信息风险 enforce 不再仅凭 `risk_type=imminent_resolution` 拦截，只有 `resolution_imminent=true`、官方结果或命中的风险等级才会阻断。
- Rewards 低竞争市场 sleeve v2 已落地并默认关闭；系统会在启用 `observe/enforce` 后计算 `qualified_competition_usd`、`estimated_reward_per_100_usd_day`、退出深度和盘口稳定性窗口，quote plan 会携带 `strategy_bucket` 与指标；full tick 会持久化低竞争 observation，snapshot 会返回最近 24 小时 shadow report 和保守的小额 enforce 建议。只有 `enforce` 且低竞争指标、AI advisory、info-risk enforce 和既有 live 风控全部通过时才会按低竞争标签小额实盘下单；report 不会自动启用 enforce。
- Rewards 保留数据库控制命令队列用于持久恢复；API 入队时会合并同账户同动作的 pending/running 重复命令，真正入队后通过共享 runtime revision 立即唤醒后台执行。
- Copytrade 已精简为只读钱包跟踪和分析：API 负责钱包配置和控制命令入队，worker 负责检测 source trades 与执行 Analyze；Run/Cancel/Reset 兼容命令当前不执行交易逻辑。
- Wallet analysis 是纯计算，已完全实现
- Arbitrage 是只读链路（发现/记录/校验/分析/展示），不会创建执行请求
- Database maintenance 已集中覆盖非核心长期账本的高增长历史/缓存/队列表；live 账本类表（如 rewards fills/positions/account state）不在通用维护任务中删除，避免破坏对账。

## 修改检查清单

- [ ] 新增 Store trait 方法后，同步更新 `infrastructure` 中的 Postgres 和 in-memory 实现
- [ ] 修改 Service 方法后，同步更新 `packages/api` 中的 handler 和 `packages/backend/apps/worker` 中的 worker
- [ ] 修改视图/命令类型后，同步更新 `contracts` crate 中的 DTO
- [ ] 新增模块后在 `lib.rs` 中添加 `mod` 声明和 `pub use` 导出
- [ ] 使用 `include!` 拆分时，被 include 文件不写自己的 `use`
- [ ] 文件行数不超过 500（软上限）/ 800（硬上限）
- [ ] 运行 `cargo check --workspace --tests`
