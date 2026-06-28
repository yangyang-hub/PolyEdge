# Worker App（后台任务服务）

最后更新：2026-06-28

## 概述

Worker 代码现在同时提供共享库和兼容 CLI。`polyedge-api` 在同一进程内启动 `WorkerRuntime`，运行数据库维护、新闻采集/提升、执行分发、订单对账、奖励机器人、copytrade 钱包跟踪/分析、Smart Money 定时画像扫描和 orderbook token 注册；Smart Money runtime 默认关闭，启用后从 Polymarket Data API leaderboard 和 active copytrade tracked wallets 生成候选，再扫描候选钱包画像、评分和源交易，并基于 orderbook 服务缓存为未处理源交易生成 deterministic observe/rejected 信号与 deterministic gate decision 审计；当 `signal_advisory_enabled=true` 时，worker 还会为近期 observe 信号构造 signal advisory payload/input_hash、检查未过期缓存，并在 provider 环境密钥存在时调用 LLM provider 保存三态 advisory；动态高概率市场定价目前提供本地 outcome JSON 导入、一次性样本构建、bucket stats 刷新、带基础退出规则摘要的 baseline 回测持久化和一次性只读 observe 扫描 CLI，不在 runtime 中自动调度；旧信号重算和套利雷达 worker 已移除。`polyedge-worker` 二进制继续提供维护/调试子命令，但 Docker 不再部署独立常驻 worker 容器。

## 设计目标

- 每个 worker 任务是独立的函数，可通过 CLI 子命令单独运行或组合运行
- 支持优雅关闭（`tokio::signal::ctrl_c()`）
- 每次运行生成结构化 Report 用于监控和日志
- 通过 `AppState` 共享所有服务实例

## 架构与关键文件

| 文件 | 职责 |
|---|---|
| `lib.rs` | 共享 runtime：CLI 参数解析、任务实现聚合，对 API 暴露 `WorkerRuntime` |
| `main.rs` | 薄 CLI 入口，调用 `polyedge_worker::run_cli()` |
| `worker/service.rs` | `WorkerRuntime` 生命周期与后台任务编排 |
| `worker/service_info_risk.rs` | WorkerRuntime 中 rewards 信息风险扫描任务接线 |
| `worker/database_maintenance.rs` | 数据库维护任务：定期调用 `DatabaseMaintenanceService` 并记录逐表清理统计 |
| `worker/orderbook_registration.rs` | Worker orderbook token 注册：周期注册 rewards/执行订单 token，并在 rewards 新买单持久化后即时刷新 `rewards_active` source |
| `worker/market_sync.rs` | 市场同步 CLI 兼容入口：Gamma liquidity/end time → `markets` + Rewards API → `reward_markets` |
| `worker/news.rs` | 新闻采集入口 |
| `worker/news_helpers.rs` | 新闻采集辅助函数 |
| `worker/news_promotion.rs` | 新闻提升为 events/evidence |
| `worker/execution_dispatch.rs` | 执行请求分发与 confirmed live trade 对账 |
| `worker/execution_queue.rs` | 执行队列管理 |
| `worker/execution_reconcile.rs` | 订单/成交对账 |
| `worker/orderbook_stream.rs` | Orderbook stream — 仅保留 CLI 子命令兼容，核心逻辑已迁移到独立 `polyedge-orderbook` 服务 |
| `worker/rewards.rs` | 奖励机器人 tick；消费 API 入队的 run/cancel/reset 控制命令 |
| `worker/rewards/account_sync.rs` | rewards 外部余额、CLOB open-order 反查/BUY 收养重开、完整持仓、检测库存原价卖出 intent、订单 scoring 与 UTC 当日账户级 rewards 聚合同步 |
| `worker/rewards/live_sync.rs` | rewards live 托管订单成交/状态同步、单订单失败隔离、Reset cancel-all 语义 |
| `worker/rewards/live_orders.rs` | rewards live 撤单、同一 external order in-flight 去重、成交入账、post-fill exit/flatten intent |
| `worker/rewards/live_submission.rs` | rewards live 单笔提交、post-only 接受状态处理和 submission marker |
| `worker/rewards/live_pending.rs` | rewards live 持久化 intent 提交、BUY 提交前 last-look（含事件窗口新增 BUY 阻断）、SELL 退出持仓裁剪/无仓位关闭、开放订单匹配恢复和未知结果锁定 |
| `worker/rewards/live_helpers.rs` | rewards live 价格 tick、fill id、退出重试与订单状态转换辅助函数 |
| `worker/rewards/live_orderbook_risk.rs` | rewards live orderbook 可用性/新鲜度 helper：新增挂单 stale 余量、近期 BUY stale-only 撤单 grace、等待原因 |
| `worker/rewards/live_requote.rs` | rewards live 换价 guard：报价漂移识别、历史盘口稳定确认、冷却和单轮限速 |
| `worker/rewards/live_placement_limits.rs` | rewards live placement 资金预算、AI condition notional cap、provider 前 funding precheck、同 condition BUY 缺口名义金额 helper |
| `worker/rewards/live_cancel.rs` | rewards live 撤单候选与撤单原因分流：事件窗口 BUY 撤单、通用硬风控、depth/rank/history/requote 规则 |
| `worker/rewards/live_risk.rs` | rewards live placement 风控：事件窗口停止新增、资金/库存 cap、订单创建准入、depth/rank/history/requote helper |
| `worker/rewards/orderbook_events.rs` | rewards worker 本地盘口 cache、orderbook 内部 WS 消费、HTTP bootstrap、活跃 token 更新 channel 和 condition 盘口首次就绪检测（驱动撤单快路径与 AI advisory 批量 worker） |
| `worker/rewards/event_cancel.rs` | rewards orderbook 事件驱动 hard-risk 撤单快路径：独立 task 消费活跃 token 更新，只做 cancel-only 风控，不跑重型同步/重挂/换价 |
| `worker/rewards/polling.rs` | rewards poll loop、盘口读取、独立事件撤单 worker 接线、fast reconcile、外部同步节流、5 天历史清理、进程内盘口历史和独立后台盘口预热 task（`run_reward_managed_orderbook_cache_prewarm`，每 5 秒刷新活跃/eligible/候选 token 本地 cache，不阻塞 fast reconcile） |
| `worker/rewards/provider_advisory.rs` | rewards AI advisory cache gate、pre-LLM 候选硬过滤、provider connector/permit helper、provider LLM 调用记录 helper |
| `worker/rewards/provider_refresh_batch.rs` | rewards 主 provider refresh 的批量请求 helper：按配置批量请求 AI advisory / info-risk，AI 请求上限按缓存过滤后的实际 provider request 消耗，逐项保存，记录实际 provider 调用，漏项或错配回退单市场 |
| `worker/rewards/provider_refresh_orderbook.rs` | rewards 主 provider refresh 的临时 orderbook source helper：按临时批次订阅 AI 所需盘口，允许挂单 advisory 后合并提升 `rewards_eligible` |
| `worker/rewards/provider_refresh.rs` | rewards AI advisory / 信息风险 provider refresh：full tick 后分别启动 AI advisory 与 info-risk 后台 task，各自单实例、各自 pre-LLM gate；AI refresh 使用 cache gate 前 deterministic cycle 构建稳定 input hash，AI provider request cap 在缓存过滤后消耗；AI 盘口使用 `rewards_ai_provider` 临时 source 每批最多 10 个市场，info-risk 不再占用 AI 临时盘口批次；实际外部调用经 connectors 全局 LLM 单并发闸门并写入 `llm_calls` |
| `worker/rewards/provider_refresh_candidates.rs` | rewards provider refresh 候选 condition 归一化、开放订单/持仓优先级和统一 standard 候选排序 helper |
| `worker/rewards/provider_fallback.rs` | rewards LLM provider 备用接口：解析可选第二个独立 provider/model 接口；主接口任意失败（网络/超时、HTTP 4xx/5xx、或无法解析的空响应）时用同一请求重试备用接口（`advise_with_fallback`/`assess_with_fallback` 单飞 permit、主备两次 `llm_calls` 记录、合并 overload 判定），advisory/info-risk 缓存读取与 live tick gate 同时识别 primary 和 fallback 来源 |
| `worker/rewards/provider_batch.rs` | rewards AI advisory orderbook 事件驱动批量 worker：盘口首次就绪入队、攒批 `advise_batch`、缺失回退单请求、记录实际 provider 调用、info-risk 同步推进（默认关闭，与 provider refresh 并存） |
| `worker/rewards/info_risk.rs` | rewards 信息风险异步扫描、provider 缓存命中、pre-LLM 候选硬过滤、每轮扫描 cap、quote plan 风险应用、独立扫描 provider 调用记录 |
| `tests/rewards.rs` / `tests/rewards_orderbook_risk.rs` / `tests/rewards_reconciliation.rs` | rewards live 下单、orderbook stale 防抖风控、成交、对账、退出重试与增量持久化回归测试 |
| `worker/copytrade.rs` | copytrade 钱包跟踪与分析；消费 API 入队的 run/analyze/cancel/reset 兼容控制命令 |
| `worker/smart_money.rs` | Smart Money Intelligence scan：从 Polymarket Data API leaderboard 和 active copytrade tracked wallets seed 候选，再按 tracked/watch/candidate 顺序读取候选钱包 activity/positions/closed positions/trades，写入画像、评分和源交易；随后从 orderbook 服务缓存读取 token 盘口，为未处理源交易生成 observe/rejected 信号和 deterministic gate decision；开启 signal advisory 时构造请求 payload/input_hash、查缓存，并在 provider key 存在时保存三态 advisory；不执行 paper/live 交易 |
| `worker/smart_money/advisory.rs` | Smart Money signal advisory refresh：选择近期 observe 信号，补齐源交易/profile/score 上下文，按 Smart Money 独立 provider/request-format/model 配置构造 provider payload/input_hash，统计缓存命中与待请求数；有独立 provider key 时调用 `SmartSignalAdvisoryConnector`（与 rewards provider 共享 connectors 全局 LLM 单并发闸门）保存 advisory 并记录 `llm_calls` |
| `worker/smart_money/profile.rs` | Smart Money wallet profile 和 source trade 构造 helper：从 Data API activity/positions/closed positions/trades 近端样本生成画像和去重源交易 |
| `worker/high_probability.rs` | 动态高概率市场定价研究：导入本地 outcome JSON 标签，从本地 outcome 标签 + rewards candles 构建 first-touch 样本，从已入库历史样本刷新 bucket stats，持久化 baseline 回测 run/trade/退出规则摘要，并用 orderbook 服务缓存执行一次性只读 observe 扫描；不抓外部 API、不执行交易 |
| `worker/polymarket_config.rs` | Polymarket 配置刷新 |
| `worker/polymarket_events.rs` | Polymarket 用户事件 WebSocket |
| `worker/shared.rs` | 共享辅助函数 |

## CLI 子命令

| 命令 | 函数 | 描述 |
|---|---|---|
| `run`（默认） | `run_worker_service` | 兼容长期 worker 循环；正常部署由 API 内嵌 runtime 代替 |
| `sync-markets-once` | `sync_markets_once` | 一次性市场同步 |
| `ingest-news-once` | `ingest_news_once` | 一次性新闻采集 |
| `run-database-maintenance-once` | `run_database_maintenance_once` | 一次性执行数据库历史/缓存/队列表 retention 清理 |
| `poll-news` | `poll_news` | 持续新闻轮询 |
| `promote-news-events` | `promote_news_events` | 新闻提升为 events |
| `scan-rewards-once` | `run_reward_bot_once` | 一次性消费 rewards 控制命令或执行 live 策略 tick；仅适合诊断，不维持长期订单 heartbeat |
| `poll-reward-bot` | `poll_reward_bot` | 持续消费 rewards 控制命令和 live 策略轮询 |
| `scan-reward-info-risks-once` | `scan_reward_info_risks_once` | 一次性异步扫描 rewards 候选、开放订单和持仓市场信息风险并写入缓存 |
| `poll-reward-info-risks` | `poll_reward_info_risks` | 持续异步扫描 rewards 候选、开放订单和持仓市场信息风险 |
| `scan-copytrade-once` | `run_copytrade_once` | 一次性消费 copytrade 控制命令，或扫描 active tracked wallets 并记录 source trades |
| `poll-copytrade` | `poll_copytrade` | 持续消费 copytrade 控制命令并轮询 tracked wallets |
| `analyze-wallets-once` | `analyze_wallets_once` | 一次性钱包分析 |
| `scan-smart-money-once` | `run_smart_money_once` | 一次性 seed leaderboard/copytrade 候选并扫描 Smart Money 候选钱包，持久化候选、画像、评分、源交易、deterministic observe/rejected 信号和 gate decision；开启 signal advisory 时构造 observe 信号 advisory 请求、查缓存，并在 provider key 存在时刷新 advisory 缓存 |
| `poll-smart-money` | `poll_smart_money` | 持续轮询 Smart Money scan；每轮先检查 Smart Money config enabled，未启用时跳过外部请求 |
| `import-high-probability-outcomes <path>` | `import_high_probability_outcomes_once` | 从本地 JSON 文件导入/更新 `high_probability_market_outcomes` 标签；支持顶层数组或 `{ "outcomes": [...] }`，不调用外部 API |
| `build-high-probability-samples-once [limit]` | `build_high_probability_samples_once` | 一次性读取 `reward_market_candles` + `high_probability_market_outcomes`，构建 first-touch `high_probability_samples` |
| `refresh-high-probability-buckets-once` | `refresh_high_probability_buckets_once` | 一次性读取已入库 `high_probability_samples`，按当前配置刷新 `high_probability_bucket_stats` |
| `run-high-probability-backtest-once` | `run_high_probability_backtest_once` | 一次性按当前配置运行 70/30 baseline walk-forward 回测，并持久化 run、退出规则摘要与交易明细 |
| `observe-high-probability-once [limit]` | `observe_high_probability_once` | 一次性读取活跃 rewards 最新 candle 候选和 orderbook 服务缓存，按当前 bucket/edge gate 写入只读 observations；同一流程也可由默认关闭的 runtime poll loop 周期触发；不抓外部 API、不下单 |
| `drain-execution-queue` | `drain_execution_queue` | 处理排队的执行请求 |
| `reconcile-paper-fills` | `reconcile_paper_fills` | Paper 交易对账 |
| `poll-paper-order-statuses` | `poll_paper_order_statuses` | Paper 订单状态轮询 |
| `poll-polymarket-order-statuses` | `poll_polymarket_order_statuses` | Live Polymarket 订单状态轮询 |
| `reconcile-polymarket-fills` | `reconcile_polymarket_fills` | Live Polymarket 成交对账 |
| `consume-polymarket-user-events` | `consume_polymarket_user_events` | 消费 Polymarket WS 事件 |

## 核心 Worker 数据流

### database-maintenance — 数据库维护

```
run_database_maintenance_once()
    → DatabaseMaintenanceService.prune_history(now)
    → PostgresDatabaseMaintenanceStore 按 retention cutoff 分批 DELETE
    → 日志输出逐表 deleted 计数和 total_deleted
```

内嵌 worker runtime 默认通过 `POLYEDGE_WORKER__DATABASE_MAINTENANCE=true` 每 3600 秒执行一次；本地 `.env.example` 默认关闭，避免开发进程意外清历史。当前覆盖 raw events、AI/info-risk 过期缓存、reward price-history candles、rewards/copytrade 控制命令、copytrade events/source trades、outbox/external dedup、LLM calls、audit logs 和 mode transitions。每个表每轮最多 20 批、每批 10,000 行；清理失败只记录 warn，不影响其他 worker 循环。

### market_sync — 市场同步（已迁移到 orderbook 服务）

市场同步逻辑已迁移到 `polyedge-orderbook` 服务（`packages/backend/order/src/market_sync.rs`）。Orderbook 服务启动时先暴露 HTTP `/healthz`，再由后台任务执行 initial/periodic market sync，避免外部市场 API 延迟阻塞容器健康检查。Worker 中保留 `sync_markets_once` 函数供 CLI 子命令 `sync-markets-once` 使用，但 daemon 模式不再调度此任务。

### register-orderbook-tokens — 盘口 token 注册

```
register_orderbook_tokens()
    → 遍历活跃执行订单（Submitted/Open/PartiallyFilled）→ 解析市场 YES/NO asset_id
    → reward_bot_service.list_active_reward_book_token_ids() → rewards 活跃订单/持仓 token
    → reward_bot_service.list_eligible_reward_book_token_ids() → 当前最终可挂单 eligible quote plan token
    → reward_bot_service.list_all_reward_candidate_token_ids() → rewards 候选 token 填充剩余额度
    → orderbook_registry.register_tokens("rewards_active", ...)
    → orderbook_registry.register_tokens("exec_orders", ...)
    → orderbook_registry.register_tokens("rewards_eligible", ...)
    → orderbook_registry.register_tokens("rewards_candidates", ...)
    // 通过 OrderbookHttpClient → HTTP POST /orderbook/register 注册到 orderbook 服务
```

此任务替代了原来的 `consume-orderbook-stream` 和 `sync-markets` 任务。Worker 不再直接运行盘口流或市场同步，而是通过 HTTP 告知 orderbook 服务需要订阅哪些 token。
注册任务最长每 60 秒执行一次，orderbook 服务重启后可自动恢复订阅；rewards live tick 在新买单 intent 持久化并并入 open_orders 后，还会立即重新注册 `rewards_active` source，避免刚落库的实盘订单等待下一个周期注册才被 orderbook 订阅覆盖。每个 source（rewards 活跃订单/持仓 token、活跃 execution token、当前最终 eligible quote plan token、AI provider 临时批次 token、其余 rewards 候选 token）独立收集并各自去重、截断后注册；跨 source 去重和总量上限由 orderbook registry 聚合层负责（按固定优先级 `rewards_active > exec_orders > rewards_eligible > rewards_ai_provider > rewards_candidates` 合并去重后 `take(POLYEDGE_ORDERBOOK_STREAM__MAX_TOKENS)`）。`rewards_eligible` source 只覆盖最终可挂单市场；AI advisory/info-risk gate 前的 deterministic eligible token 不再长期注册到该 source，而是由后台 provider refresh 临时注册 `rewards_ai_provider` source，每批最多 10 个市场，下一批原子替换上一批，完成后清空。provider refresh 的候选顺序保留开放订单/持仓最高优先级，其后按统一 standard 候选顺序处理，不再按低竞争类型混排。provider 返回允许挂单 advisory 后会把该市场 token 即时合并到 `rewards_eligible` source，后续 full tick 再用持久 quote plan 校正；真正下单仍要经过 live 盘口、资金和订单风控。候选来源只用于给尚未产生 quote plan 的市场提前预热盘口，受 `POLYEDGE_ORDERBOOK_STREAM__REWARD_CANDIDATE_TOKEN_CAP` 限制，默认只预热 50 个候选 token，设为 0 会清空 `rewards_candidates` source 但不影响 active/final-eligible token 或 AI provider 临时订阅。每个成功查询的 source 使用一次原子替换注册；周期注册任务对空集合做防抖，`rewards_active`/`exec_orders` 连续 2 轮成功为空才清远端 source，`rewards_eligible`/`rewards_candidates` 连续 3 轮成功为空才清远端 source，即时 `rewards_active` 刷新若读到空集合会保留上一版等待周期任务确认；任一 source 的数据库查询失败时保留远端上一版集合，不会用空集合误删订阅。

### copytrade — 钱包跟踪与分析

```
copytrade_service.claim_next_control_command()
    → worker 执行 queued run_once / analyze_wallets / cancel_all / reset
    → copytrade_service.complete_control_command() 或 fail_control_command()

无待处理控制命令时：
    fetch_wallet_analysis_inputs() // 获取 active tracked wallets 的 Data API activity + positions
        → copytrade_service.detect_and_record_source_trades()
```

Report: `CopyTradeRunReport { wallets_scanned, trades_detected, orders_placed, orders_filled, orders_skipped }`

约束：worker 是 copytrade Data API 抓取、source trade 检测和钱包分析命令的唯一执行者。API 只把 `run_once` / `analyze_wallets` / `cancel_all` / `reset` 写入 `copytrade_control_commands`；worker 每轮先领取并处理待执行命令，处理到命令时跳过本轮自动 scan。
当前 copytrade 不下单、不撤单，也不维护模拟资金账本。`AnalyzeWallets` 会读取钱包 activity/positions 并更新分析统计；`RunOnce`、`CancelAll`、`Reset` 保留为历史兼容命令，在 worker 中是 no-op。自动轮询只检测并记录 active wallets 的 source trades。

### smart_money — Smart Money Intelligence foundation

```
poll_smart_money()
    → 每 `POLYEDGE_WORKER__SMART_MONEY_INTERVAL_SECS` 秒触发
    → SmartMoneyConfig.enabled=false 时跳过本轮，不抓外部 API
    → run_smart_money_once()

run_smart_money_once()
    → Data API leaderboard seed(source="polymarket_leaderboard")
    → copytrade_service.snapshot() 读取 active tracked wallets
    → smart_money_service.upsert_candidate(source="copytrade_tracked")
    → smart_money_service.list_candidates(status=tracked/watch/candidate)
    → 按 task_limit/max 50 去重选择待扫描钱包
    → PolymarketDataApiConnector 读取 activity / positions / closed-positions / trades
    → smart_money_service.save_profile_and_score()
    → smart_money_service.record_trades()
    → smart_money_service.list_signal_candidate_trades()
    → state.orderbook_cache.get_books_with_max_age()
    → smart_money_service.generate_signals_from_trades()
    → 写入 smart_signals 和 deterministic smart_signal_decisions
    → signal_advisory_enabled=true 时按 Smart Money provider 配置为近期 observe 信号构造 advisory payload/input_hash
    → provider key 存在时调用 SmartSignalAdvisoryConnector（connectors 层全局 LLM 单并发）并保存 smart_signal_advisories
    → 输出本轮 wallets/candidates/profiles/scores/trades/signals/decisions/advisories 与 snapshot 统计
```

该任务是 Phase 1 数据生产入口：先从 Polymarket Data API leaderboard 读取最多 50 个总体榜单条目，过滤正 PnL 且成交量不低于 Smart Money 配置 `min_total_volume_usd` 的钱包并写入 `polymarket_leaderboard` 来源候选；再从已有 active copytrade tracked wallets 写入 `copytrade_tracked` 来源候选；最后按 tracked、watch、candidate 状态补充扫描目标，单轮最多扫描 `task_limit` 指定数量且硬上限 50。任一来源已标记为 `blocked` 或 `rejected` 的钱包会被本轮 seed 和扫描全局跳过。profile 指标基于 Data API 近端样本，`/trades` 用于交易数、成交量、活跃天数和市场集中度，closed positions 前 3 页样本用于 realized PnL/胜率，activity 用于源交易落库。外部请求失败按钱包隔离记录 warn，不用空样本覆盖旧画像；leaderboard 读取失败只跳过本轮 seed，不影响已有候选扫描。源交易落库后，worker 会读取尚未生成信号的 source trades（每轮最多 200 条），按 token 通过 `OrderbookCache` 远程读取 orderbook 服务缓存，缺 token、缺盘口、缺方向价格、过期、滑点超限或最优档深度不足都会生成 `rejected` 信号，通过 gate 的源交易生成 `observe` 信号；信号落库后会回读 id，并写入 `stage=deterministic_gate` 的 `smart_signal_decisions`，重复运行按 `(signal_id, stage)` 防重。orderbook 服务整体读取失败时本轮只跳过信号生成并记录 warn，不直接抓外部 CLOB。若 `signal_advisory_enabled=true`，本轮还会读取最近最多 100 条 observe 信号和最近最多 500 条源交易/profile/score 上下文，按 Smart Money 配置中的 `signal_advisory_provider`、`signal_advisory_request_format` 和 `signal_advisory_model` 构造 signal advisory provider payload 与稳定 input_hash，并按 provider/request_format/model/input_hash 检查已有缓存；provider key/base URL/timeout 来自 `POLYEDGE_SMART_MONEY__SIGNAL_ADVISORY_*` 环境变量，有 key 时调用 provider 保存三态 `smart_signal_advisories` 并记录 `llm_calls(task_type=smart_signal_advisory)`，无 key 时只输出 `signal_advisory_candidates/cache_hits/requests_built` 统计。provider 失败只记录 warn 和失败调用，不影响 deterministic 信号生成或已有缓存。内嵌 runtime 通过 `POLYEDGE_WORKER__POLL_SMART_MONEY=true` 启动定时任务，间隔由 `POLYEDGE_WORKER__SMART_MONEY_INTERVAL_SECS` 控制（默认 900 秒，runtime 中下限 60 秒）；每轮还要求 Smart Money 配置 `enabled=true`，否则跳过外部 discovery/profile 请求，但仍会尝试为已入库未处理源交易生成信号、decision 和 advisory refresh。当前不接链上 RPC；不会生成 paper/live 执行或实盘订单。后续 recent-trades discovery/profiler/scorer/wallet advisory/paper/live worker 应在该模块内逐步补齐。

### high_probability — 动态高概率市场定价研究

```
refresh_high_probability_buckets_once()
    → high_probability_service.refresh_bucket_stats()
    → 读取 high_probability_samples 中已结算样本
    → build_high_probability_bucket_stats()
    → 替换当前 model_version 的 high_probability_bucket_stats
    → 输出 samples_scanned / settled_samples / buckets_computed / buckets_saved
```

`import_high_probability_outcomes_once()` 会读取本地 JSON 文件并通过 `HighProbabilityService.upsert_market_outcome()` 写入 outcome 标签；每行要求 `condition_id` 和 `status`，`status=resolved` 时必须提供 `winning_token_id` 与 RFC3339 `resolved_at`。`build_high_probability_samples_once()` 会读取本地 `reward_market_candles`、`high_probability_market_outcomes` 和 `markets`，只对已有 outcome 标签的 condition 构建 first-touch 样本；resolved 标签会转成 win/loss，voided/ambiguous 不参与 bucket 胜率统计。`run_high_probability_backtest_once()` 会使用当前配置、较早 70% 已结算样本训练 bucket、较晚 30% 样本测试 edge gate，并写入 `high_probability_backtest_runs.exit_rule_reports` 与 `high_probability_backtest_trades`。`observe_high_probability_once()` 会读取活跃 reward token 的最新本地 candle 候选，排除已有 resolved/voided/ambiguous outcome 标签的 condition，再通过 `OrderbookCache.get_books_with_max_age()` 从 orderbook 服务缓存读取当前盘口；service 按当前模型版本 bucket stats、net edge、最低置信度、推荐最高入场价、spread、ask depth 和排除风险标签写入 `allow/reject/skip` observation。该模块不调用 Polymarket/Gamma/CLOB/Data API，不直接订阅 orderbook，也不产生订单。内嵌 runtime 可通过 `POLYEDGE_WORKER__POLL_HIGH_PROBABILITY_OBSERVE=true` 启动自动 observe poll loop，间隔由 `POLYEDGE_WORKER__HIGH_PROBABILITY_OBSERVE_INTERVAL_SECS` 控制（默认 300 秒，runtime 下限 60 秒），单轮候选上限复用 `POLYEDGE_WORKER__TASK_LIMIT`；默认关闭。后续全市场 candle/outcome producer、完整执行成本/多阶段退出回测、paper/live guarded worker 应在该模块内逐步补齐。

### rewards — 奖励策略与控制命令

```
reward_bot_service.claim_next_control_command()
    → worker 执行 queued run_once / cancel_all / reset
    → reward_bot_service.complete_control_command() 或 fail_control_command()

无待处理控制命令时：
    fetch_reward_bot_inputs() // 获取奖励市场 + 盘口
        → prepare_live_cycle()
        → 批量读取 effective event window 并写入 quote plan；StopNewQuotes 阻断新 BUY，CancelOpenBuys/InEventWindow/PostEventCooldown 触发 BUY 撤单，SELL exit 不阻断
        → provider refresh 前执行 live funding precheck 和 pre-LLM 候选硬过滤：无开放订单/持仓的新 condition 若当前可用资金放不下最低 rewards size 待补腿，先标记 funding reason；已有订单/持仓 condition 仍保留 provider 覆盖
    → 只应用已缓存 AI advisory，并在后台分别单实例刷新缺失 AI advisory 与信息风险缓存（两条队列各自开放订单/持仓优先，其后按统一 standard 候选顺序处理；AI advisory 使用 `rewards_ai_provider` 临时 orderbook source，每批最多 10 个市场，切换下一批前取消上一批；AI advisory / info-risk provider 请求分别受 rewards 配置 `ai_advisory_batch_size` / `info_risk_batch_size` 控制，1 为逐市场，>1 批量请求并对漏项/错配回退单市场；每次实际外部 provider HTTP 调用先经过 connectors 层全局 LLM 单并发闸门再写入 `llm_calls`，批量请求计为 1 次调用；仅在该市场所有报价 token 盘口都已发布后才请求 AI 并写缓存，请求 payload 会读取最近最多 24 小时 5m price-history source candles 并聚合为最多 24 根 1h candles，同时带当前盘口定价合理性和 TTL/cache policy，cache key 只看已完成小时级摘要和统一策略上下文；缺盘口的市场本轮跳过请求、不写缓存，等盘口到达后再评估，避免缓存“无盘口不允许挂单”结果长期卡住市场）
        → 读取已缓存的信息风险并按配置标记/过滤 quote plan
        → info-risk enforce 开启时应用首单 gate：新 condition 首次 BUY 需先有信息风险缓存并满足观察窗口，已有订单/持仓 condition 跳过
        → 应用统一 opportunity metrics：竞争度、奖励密度、退出能力、盘口稳定性和资金占用共同调整 quote plan score
        → sync managed rewards order trades/statuses
        → 批量同步 managed order scoring 状态与 CLOB open-order snapshot（收养/重开 active rewards BUY，关闭缺失 managed BUY）
        → 同步 UTC 当日账户级 maker rewards（`/rewards/user/total?sponsored=true` 聚合优先，对齐官网 native+sponsored 口径；明细 fallback 合并 native 与 sponsored-only）
        → 无近期 confirmed fill 时同步外部 balance + 链上 pUSD 余额回退 + 完整 positions 快照
        → LivePolymarketConnector.submit_token_order()
        → orderbook stream active-token event：独立 cancel worker 只读更新 token 的活跃盘口，随后直接执行 hard-risk cancel-only 检查
        → orderbook stream wake 或 reconcile_interval_sec 兜底：普通 fast reconcile 读取活跃盘口并做成交同步、退出提交、完整撤单检查和节流外部同步
```

Report: `RewardBotRunReport { markets_scanned, books_fetched, plans_built, eligible_plans, placed_orders, cancelled_orders, filled_orders, risk_cancelled_orders, reward_accrued }`

约束：后台 runtime 是 rewards 策略和控制命令的唯一执行者。rewards poll loop 在整个生命周期持有 Postgres advisory lease，多实例中只有 lease owner 会认证 CLOB、执行命令/full tick/fast reconcile 并维护 5 秒 heartbeat id 链；standby 实例不发心跳也不执行。API handler 只写控制命令；同一账户同一动作已有 `pending/running` 命令时会合并重复入队，避免重复点击制造多轮 full tick 或 cancel-all；共享 `RewardBotService` 会在真正入队时立即唤醒同进程 loop，配置 revision 变化还会立即触发 full cycle。`POLYEDGE_WORKER__POLL_REWARD_BOT=true` 控制 API 内嵌 runtime 是否启动 rewards loop。

自动 tick 只从 Postgres 的 `reward_markets` 读取奖励市场。长期 `poll-reward-bot` 启动后会通过 `OrderbookStreamClient` 连接 `polyedge-orderbook` 的内部 `/orderbook/stream`，维护 worker 进程内本地盘口 cache；本地 cache 使用 `POLYEDGE_ORDERBOOK_STREAM__MAX_LEVELS_PER_SIDE` 和 `POLYEDGE_ORDERBOOK_STREAM__BOOK_TTL_MS` 限制深度与过期读取。启动和重连时通过 `OrderbookHttpClient` / `POST /orderbook/batch` bootstrap 当前 rewards 活跃、eligible 和候选 token，后续缺失 token 也会按需 HTTP 补齐；周期注册任务会把全部最终 eligible quote plan token 注册到 orderbook `rewards_eligible` source，pre-AI deterministic eligible 市场由 provider refresh 的 `rewards_ai_provider` 临时 source 按批订阅准备 AI 评估。内部 WS 连接建立最多等待 5 秒，已连接后若约 3 个 poll reconcile 周期没有收到任何事件，worker 会主动重连并重新 HTTP bootstrap。Postgres 候选 market pool 关联 Gamma `markets`，硬过滤非 open/tradable、高歧义、低流动性、低 24h 成交量、临近结算、Gamma spread 过宽、市场同步过期、奖励不足以及 FDV/launch/token/official-result 等高事件跳变风险市场；不再按 `per_market_usd` 预筛双边最小份额预算，高最小份额市场会保留到 live materializer 和实际钱包余额准入层处理。full tick 会在 AI/info-risk provider refresh 之前先对无开放订单/持仓的新 condition 做 live funding precheck，当前余额无法补齐最低 rewards size 待补腿的计划会先标为不可挂并跳出 provider 候选；已有 open-like 订单或持仓的 condition 跳过该前置资金门槛，仍进入 provider 队列以继续风险管理。仅唯一且明确的 YES/NO token 会进入候选与订阅。通过门槛后按奖励、流动性、成交量、剩余时长和奖励 spread 综合排序；随后统一机会评分会把竞争资金、奖励密度、退出能力、盘口稳定性和资金占用纳入 quote plan score 调整。worker 使用本地 cache 读取候选和活跃 token 盘口，遇到本地缺失、过期或接近新挂单 freshness headroom 的 token 会提前回源 orderbook HTTP batch 并回填本地 cache；这些 batch 请求会携带 `refresh_if_stale_ms`，默认 `stale_book_ms=45000` 时 placement 最大盘口年龄约 35 秒、HTTP 预刷新阈值约 25 秒，orderbook 服务若自身缓存也超过约 25 秒会同步 `/books` 刷新后再返回；若本 tick 没有新鲜缓存盘口，不会提交新 post-only 订单。

full tick 会在开头读取候选盘口；`prepare_live_cycle()` 会先把 effective event window 评估挂到 quote plan：`StopNewQuotes` 和未知时间 block 模式只阻断新增 BUY，不把已有 live BUY 立即撤掉；`CancelOpenBuys`、`InEventWindow` 和 `PostEventCooldown` 会把计划 hard-block，并由 fast reconcile / event cancel 共用的 `live_cancel_reason()` 撤已有 BUY。随后先执行 live funding precheck、标记 `pre_ai_eligible`，再应用已有 AI/info-risk cache gate，之后才启动后台 AI/info-risk provider refresh，因此明显资金不足、事件窗口阻断新增或已有未过期 provider 缓存的无敞口新 condition 不会反复消耗模型 token。provider refresh 使用 cache gate 前的 deterministic cycle 构建 request/input hash，避免把 live gate fail-closed 后的 `quote_mode=None`/空腿状态写入缓存键；provider refresh 仍会在请求前应用 pre-LLM gate：已有订单/持仓 condition 始终保留 provider 风险覆盖，无敞口计划必须仍 eligible 或 pre-AI eligible 且未被事件窗口阻断新增；gate 返回 active-exposure / standard 候选类型，legacy low-competition bucket 会按 standard 处理。完成 AI/info-risk cache gate 后会立即用本轮内存 quote plan 原子替换 `rewards_eligible` orderbook source，让新放行的 token 不必等待周期注册任务；订单同步和账户同步完成后，进入撤单、待提交 intent 和新挂单前，会对当前 open-like 订单与 eligible quote plan token 再执行一次同样的本地 cache / orderbook HTTP batch 回填并合并到本轮 books；该 batch 会要求 orderbook 服务按 placement 预刷新阈值补齐自身 stale 缓存，而不是只返回可能已经超过 live 窗口的旧缓存。随后 worker 用当前 books materialize quote readiness，应用统一 opportunity metrics 并保存 quote plan 快照，避免 tick 前半段 I/O 耗时让盘口落入 placement stale 窗口，也避免控制台读到尚未 live 验证的 eligible 中间态。

Worker 本地盘口 cache 的 TTL 按本地接收/写入时间计算，避免上游未来 `observed_at` 延长旧盘口寿命；上游 `observed_at` 仍保留给盘口内容版本和历史记录，planner/live materializer 与 live 风控判断盘口新鲜度都使用 `confirmed_at`。

低竞争 rewards sleeve 已合并到统一机会评分，不再有 `LowCompetitionProbeState`、`rewards_low_competition_probe` source、独立候选 profile、独立 live placement cap 或专用撤单 gate。`low_competition_*` 配置、legacy bucket、metrics 和 observation 表仍可反序列化/清理，但运行时会归一化为 off/0，quote plan 最终保持 standard bucket 并清空 legacy low-competition metrics。当前统一 `opportunity_metrics` 会基于 orderbook、账户、开放订单和本地盘口历史计算计划 notional、竞争资金、竞争倍数、预估 100U 日奖、账户与单市场资金占比、退出深度/滑点、坏成交恢复天数、盘口样本数、中点波动、top-of-book 跳变、四个组件分数和综合机会分，并用 `score_adjustment` 调整 quote plan score；该调整可能让计划跨过 `min_market_score` 门槛，但不会绕过市场质量、AI/info-risk、资金、盘口和 kill-switch 硬风控。full tick 会在 provider gate 前先用机会评分影响候选资格，随后在订单/账户同步、live action 盘口刷新和 readiness materialize 后刷新同一指标；后置刷新只允许降级或更新展示，不把 provider/资金/盘口等 gate 已阻塞的计划重新放行。

报价计划构建阶段只应用市场质量、概率区间、配置和非盘口依赖过滤，不再因为 `quote_bid_rank` 缺档、目标价格超出 rewards spread、auto 单边所需的退出深度/top1/top3 买盘集中度/HHI、实际盘口价格预算、`per_market_usd` 或 `quote_size_usd` 而淘汰市场；quote plan 的腿可以只是 YES/NO token 占位元数据。live placement 准备创建订单时才用当前 orderbook materialize 真实腿：报价价格由 `quote_bid_rank=1|2|3` 选择 YES/NO 目标买盘价，粗 tick 盘口按买一/买二/买三（不同买价）选择，细 tick 盘口会从买一回退 `rank-1` 个 0.01 价格步长后选择不高于目标价的当前买盘档位，避免 0.001 tick 下买三只退两个细档；随后验证目标档位、rewards spread、touch ask、安全边际、auto/enforce 盘口指标和实际 size/notional。缺少、空、过期或已进入 placement freshness headroom 的盘口不创建新订单、不写 12 小时 skip，而是保持 quote plan eligible 并写入等待 orderbook 订阅数据的 reason；持久化 quote plan 时会刷新 `quote_readiness`，其中 `eligible=true` 但没有真实 price/size/notional 报价腿的计划标记为 `waiting_orderbook`，只有已有真实报价腿的计划标记为 `ready_to_quote` 并计入 API snapshot 的 `status.ready_quote_markets`。worker 会先对超过新挂单 freshness headroom 的本地盘口通过 orderbook HTTP batch 刷新，默认保留 10 秒 stale 余量（短 stale 窗口保留半窗），后续 full tick 拿到带足够新鲜度余量的盘口后会重新 materialize 并继续挂单流程。非 transient live 验证不通过时不下单，并把该 quote plan 标记 `live_skip_until` / `live_skip_reason`，跳过标记默认 12 小时后失效以便奖励范围或盘口变化后重新评估。开放订单目标价漂移超过 `requote_drift_cents` 不再单轮全量撤单；worker 会要求 `requote_drift_confirm_sec` 秒前的历史盘口同方向仍超阈值、订单年龄超过 `requote_drift_cooldown_sec`，并且每个 reconcile 周期最多撤 `requote_drift_max_cancels_per_cycle` 个 drift 候选。报价大小不再使用 `per_market_usd` 或 `quote_size_usd` 作为构造上限；live materializer 只把报价腿按 CLOB 成本精度向上对齐到 `rewards_min_size`，并满足 Polymarket 1 美元最小名义金额。实际能否新增同一 condition 的 YES/NO 腿由 `available_usd` 扣除未归属外部 BUY notional 后的余额决定；full tick 会在 provider refresh 之前先用同一预算逻辑拦截无 active exposure 且待补最低 rewards size 腿放不下的新 condition，live placement 阶段仍会在下单前复核资金并写入 funding reason，等待下一轮余额或开放订单同步后重新评估。默认 `quote_mode=double` 且 `selection_mode=observe`，仍生成既有双边计划；当配置为 `quote_mode=auto`、`selection_mode=enforce` 且启用 dominant single-side 时，planner 只基于 YES/NO 概率生成初步单边或双边模式，盘口指标和双边不可行后的单腿回退都在 live materializer 中完成。`observe` 模式只把推荐模式和 `book_metrics` 写入 quote plan，不改变实际挂单。

AI advisory 启用后只在 full tick 的 `prepare_live_cycle()` 之后参与 live gate，不参与 fast reconcile。`prepare_live_cycle()` 构建新计划时会记录 AI 过滤前的 deterministic eligible condition 集合，并继承上一版 quote plan 中未过期且 provider/request_format/model 匹配的 advisory；继承阶段只应用已有决策，不因缺少缓存提前 fail closed。live tick 随后只查询 `reward_market_advisories` 缓存并应用 gate：缺少未过期 advisory 或模型为空仍会把原本 eligible 的计划置为不可挂等待 provider；新 provider 输出 `allow_quote=true|false` 二值和 conservative `strategy_hint`，允许结果保留 deterministic 报价腿继续进入 live 盘口、资金和订单风控，不允许结果硬拦（内部兼容映射为 `avoid`）；旧 `suitability` 响应按二值 fail-closed 兼容，仅 `allow` 视为允许，`watch`/`avoid`/其它非 allow 值一律映射为 `avoid` 硬拦（旧 watch 放行行为已移除，对应 advisory cache key 已升到 `schema_version` 10 失效旧缓存）。AI 和 info-risk gate 都完成后才启动后台 provider refresh，并继续等待 live action 盘口刷新与 readiness materialize 后统一保存 quote plan 快照，避免预过滤 eligible 状态或尚未 live 验证的中间态被周期性 orderbook 注册任务和控制台读取；live tick 不等待外部 AI provider，因此不会因为全量 advisory 请求拖住下单主循环。full tick 后台 provider refresh 拆成 AI advisory 与 info-risk 两个独立 Tokio task，分别用进程内 `AtomicBool` 保证各自单实例；后台 AI refresh 使用 gate 前 deterministic quote plan 构建 request/input hash，避免 live gate 把缺缓存计划改成 `quote_mode=None`、清空 legs 后污染 provider 缓存键。后台 refresh 仍把 gate 前的 quote plan、DB reward market、账户/仓位/开放订单、orderbook top levels、当前盘口定价合理性摘要、provider TTL/cache policy，以及由最近最多 24 小时 5m price-history source candles 聚合出的最多 24 根 1h candles 和摘要放入 provider payload，但 funding precheck、pre-LLM gate 已拦截的无 active exposure 新 condition，以及已有未过期且未进入提前刷新窗口的缓存命中 condition 不进入本轮 provider 请求队列，已有开放订单或持仓的 condition 仍保留最高优先级以继续覆盖风险管理。AI `input_hash` 只由市场身份/问题、奖励参数、计划 quote mode、统一策略配置和已完成小时级 candle summary 组成的稳定 cache-key payload 计算；info-risk `input_hash` 只由搜索 query、市场身份/问题/事件、计划 quote mode 和风险策略配置计算。账户余额、开放订单、持仓、即时盘口档位、盘口时间戳、quote plan reason/score、`market_synced_at`、当前小时内 5m source 更新和旧低竞争专属策略字段不会进入缓存键，避免 provider refresh 已保存记录却在后续 full tick `cache_hits=0`。新写入的 AI advisory / info-risk 缓存会按稳定键增加最多 20% 且最多 15 分钟的正向 TTL jitter；provider refresh 和事件驱动批量通道看到缓存仍未过期但已进入较小的提前刷新窗口（`min(TTL/20, 60s)`）时，会继续把旧缓存用于当前 cycle，同时发起后台续期，避免等到过期后集中出现 provider pending，也避免 1 小时 TTL 在过期前 12 分钟就滚动重打。AI 与 info-risk 各自保留开放订单、持仓最高优先级，剩余候选按统一 standard 顺序处理，并分别受 `POLYEDGE_REWARDS__INFO_RISK_MAX_MARKETS_PER_CYCLE` 约束（默认 50）；AI 侧按缓存过滤后的实际 provider request 数消耗该上限，缓存命中只应用到 refresh 内存 plan 和 eligible token 推进，不占请求名额；info-risk 侧仍按 selected condition cap 截断。AI refresh 对 AI 盘口按最多 10 个市场一批注册 `rewards_ai_provider` 临时 source，批次切换会取消上一批，结束后清空，并按 `ai_advisory_batch_size` 批量补 AI advisory、把命中或新写入的 advisory 挂到本轮内存 plan；info-risk refresh 不再等待 AI 临时盘口批次，独立按 `info_risk_batch_size` 批量补 info-risk，默认值 1 保持逐市场请求，漏项、错配或整体解析失败会回退单市场。provider 成功后只写入 `reward_market_advisories` / `reward_market_info_risks` 缓存，供后续 full tick 使用，不用旧 cycle 覆盖当前 quote plan snapshot；允许挂单 advisory 会即时把该市场 token 合并注册到 `rewards_eligible` source，后续 full tick 再用持久 quote plan 校正。AI advisory 和 info-risk 分别使用独立进程内 `Semaphore(1)`，同一 worker/API 进程内各自单飞但可彼此并行。OpenAI-compatible/Anthropic API key、base URL、模型、超时和原有 advisory allow 最低置信度来自 worker 环境变量；GLM/DeepSeek 通过 OpenAI-compatible base URL 和模型名配置，不再作为独立 provider；strategy hint 应用阈值来自 rewards 配置 `ai_strategy_hint_min_confidence`，可在前端调整；OpenAI-compatible base URL 可以是根地址、`/v1` 地址或 GLM 这类 `/v4` versioned base，connector 会对 root 自动请求 `/v1/...`、保留 `/vN/...` 并兼容 Bearer / `api-key` 认证头。OpenAI provider 可用 Responses 或 Chat Completions；模型名包含 `glm` 或 `deepseek` 时会强制使用 `openai_chat_completions`，并发送 `response_format=json_object` + `max_tokens`；MiMo provider 使用 `openai_chat_completions`，`openai_responses` 会返回 provider 未实现错误。API 内嵌 worker 启动时会记录 rewards poll loop 是否启用、AI key 是否配置、模型名和 interval；每轮 full tick 会记录 markets/books/plans/pre_ai_eligible_plans/eligible/open_orders/positions 以及 AI/info-risk 配置；缓存 gate 会记录 pre_ai_eligible_plans/ai_existing_advisories/ai_request_candidates/ai_pending_plans/cache_hits/skipped_missing_market/applied，后台 provider refresh 会分别记录 AI 与 info-risk 的 candidates/cache_hits/requested/saved/failures/skipped_missing_market 汇总和逐个 requesting/saved 进度。provider HTTP 传输失败，或明确返回限流、认证失败、服务端不可用（如 HTTP 429/401/403/5xx、`system_cpu_overloaded`、`overloaded`）时，对应 refresh 会停止本轮剩余 provider 请求，避免继续压垮 provider 或在错误配置下扫完整候选池。connector 会把 provider 返回的 confidence 钳制到 `0..=1`。AI advisory 不能绕过市场质量、盘口和风控硬过滤；成交后退出策略由 `post_fill_strategy` 配置决定，不再由 AI advisory 的 `exit_policy` 覆盖。

信息风险扫描仍有独立异步任务入口，但当 `ai_advisory_enabled=true` 时，独立 `scan-reward-info-risks-once` / `poll-reward-info-risks` 不再连续请求全量 info-risk provider，而是记录跳过，交给 full tick 启动的专用 info-risk provider refresh task。AI advisory 未启用时，独立 info-risk 任务会读取当前 rewards 配置、candidate markets、quote plans、开放订单和持仓，按开放订单、持仓、通过 pre-LLM gate 的 quote plan 顺序构建结构化风险请求；market-only 候选若没有 active exposure 且没有通过 quote-plan gate，不再触发 provider 请求。市场详情从 active rewards catalog 补齐，因此已持仓或已挂单市场即使不再适合新增报价也会被评估。请求按 input hash 查询 `reward_market_info_risks`；缓存未命中或已进入提前刷新窗口时调用 provider，每轮最多处理 `POLYEDGE_REWARDS__INFO_RISK_MAX_MARKETS_PER_CYCLE` 个 condition（默认 50，0 表示本轮不发 provider 请求）；每个 provider 请求都会先获取 info-risk 专用单飞 permit。OpenAI-compatible/Anthropic API key、base URL、模型和超时复用 AI provider 环境变量；GLM/DeepSeek 通过 OpenAI-compatible base URL 和模型名配置；OpenAI-compatible 路径同样会规范化 root base URL 到 `/v1` 并保留 `/vN` provider base，OpenAI Responses 可通过 `POLYEDGE_REWARDS__INFO_RISK_WEB_SEARCH_ENABLED=true` 启用 provider-native web search（模型名包含 `glm`/`deepseek` 的 Chat Completions 不发送 web search tool）。info-risk task 启动会记录 interval、首轮延迟和 web search 开关；API 内嵌 runtime 中 info-risk 首轮会延迟一个 info-risk interval，避免启动时抢占 provider 通道；每轮扫描会记录开始、逐个 provider 请求进度、跳过原因、candidates/selected_conditions/cache_hits/requested/saved/failures/skipped_missing_market/applied_plans 汇总；provider 失败只记录 warning，若 provider HTTP 传输失败或明确返回限流、认证失败、服务端不可用（如 HTTP 429/401/403/5xx、`system_cpu_overloaded`），本轮会停止剩余外部请求并保留已有缓存。live full tick 只读取最新未过期缓存，并在 `info_risk_enabled=true` 时把结果附加到 quote plan；connector 会把 provider 返回的 confidence 钳制到 `0..=1`。当 `info_risk_mode=enforce` 时，缺少未过期风险缓存会 fail closed；已有风险置信度达到 `POLYEDGE_REWARDS__INFO_RISK_MIN_CONFIDENCE_BPS` 后，`critical`、官方结果、`resolution_imminent=true` 或配置为 `low/medium` 避免等级时命中的风险等级会把计划置为不可挂；普通 `high` 风险以及仅 `risk_type=imminent_resolution` 但 `resolution_imminent=false` 的结果只作为信息提示保留并继续走 live 盘口、资金和订单风控。既有 buy 会沿用“计划不可挂即撤单”的 live 风控路径。

live 模式会用 `LivePolymarketConnector::submit_token_order()` 提交 post-only GTC token 买单，用 `cancel_order()` 撤销本系统托管订单；未成交 post-only maker 买单不在本地按全局 notional 硬锁资金。不同 condition 的本系统未成交订单可复用资金，但同一 condition 会累计已有 managed BUY 剩余 notional 与待补 YES/NO 腿；账户开放 BUY 总额会同步到 `external_buy_notional`，其中无法归属到本系统 managed order 的外部 BUY notional 会先从 `available_usd` 中保守扣除，再做同 condition 准入，符合 CLOB 同市场余额有效性规则并降低人工/其它机器人挂单叠加风险。

所有新报价和 post-fill exit/flatten 会先持久化本地 intent，再记录 submission attempt 后调用 CLOB；瞬时明确拒单会持久化回 Planned 并停止本轮后续买单，响应丢失则锁住本地订单并只通过严格开放订单匹配恢复 external order id。新建/恢复订单先保持 `scoring=false`，仅权威 scoring 查询可以置 true；`min_depth_usd` 会扣除自身剩余挂单，只统计外部 bid 深度。live placement materialize quote plan 时要求目标报价腿都有非空新鲜盘口，默认 `stale_book_ms=45000`、placement 最大盘口年龄约 35 秒，配置归一化会把低于 5000ms 的值抬到 5000ms；盘口 freshness 使用 orderbook `confirmed_at` 而不是内容版本 `observed_at`，安静市场只要最近被 poll/WS 确认过就不会因内容未变化被误判 stale。

新建挂单路径遇到盘口缺失、空盘口、超过 `stale_book_ms`、已进入 placement freshness headroom 或事件窗口阻断新增 BUY 时等待下一轮条件恢复，不写长期 skip；新建 quote intent 与已落库待提交 BUY 在提交前都会复用 live 撤单风控，并在真正 POST 前用 orderbook 服务做 1 秒 max-age last-look，风险不通过的本地 intent 会在提交前取消，last-look 缺盘口或刷新失败则 fail closed 等下轮重试。BUY last-look 只要求当前提交 token 由 orderbook 服务以 1 秒 max-age 返回。BUY 仍检查事件窗口、计划 eligible、报价漂移、min depth、bid rank、depth drop、fill velocity、mass cancel、best ask touch、requote age 和 kill switch。live full tick、fast reconcile 和独立事件撤单 worker 都会读取开放订单/持仓活跃 token 的盘口。事件撤单 worker 只处理事件窗口 BUY 撤单、缺盘口、空盘口、SELL 盘口过期、BUY 非 stale 硬风险、外部 bid 深度不足、bid rank 过高和盘口历史窗口风险。该路径不做订单/账户同步、重挂、退出提交、报价漂移换价或定期 requote；full/fast 路径仍覆盖完整检查。

即使 `enabled=false` 已停止新增报价，硬风险仍会触发撤单；价格漂移属于策略性换价，只在历史同向确认、订单冷却后按单轮上限进入撤单；近期已有 external order id 的 BUY 若仅因盘口刚超过 `stale_book_ms` 会短暂延迟 stale-only 撤单，给即时 `rewards_active` 注册、WS/poll reconcile 和 HTTP bootstrap 恢复盘口的时间，缺盘口/空盘口和其他硬风险仍立即撤单。每笔外部下单、撤单、已确认成交和状态变化会立即落库；所有 live 撤单请求共享进程内 in-flight 去重，同一 external order id 在一次 `cancel_order()` 未返回前不会被事件撤单 worker 和普通 reconcile 重复发送；撤单/成交同步会跳过本地 synthetic ID。

外部单订单查询返回 404 时，worker 会按 token 和下单时间窗口分页查询认证账户 trades，并按 external order id 精确匹配 confirmed fill；仍无法确认时持久化 critical 对账锁并暂停新买单，后续成功查询会自动解除；若该 404 锁超过 5 分钟且仍无 CLOB/Data API 成交证据，worker 会把本地订单标记为 `cancelled` 以释放开放挂单计数。提交结果未知订单现在会在恢复查询确认 CLOB 无对应 open 订单后经过 `LIVE_SUBMISSION_UNKNOWN_CLOSE_AFTER_SECS`（默认 600 秒）宽限自动本地关闭（与 404 锁一致），但如果后续 positions 快照显示该 BUY token 在提交后出现库存，则继续保留对账锁等待确认；取消结果未知订单仍不会仅因本地超时 force-cancel；旧 `auto_cancel_stale_minutes` 配置已忽略。撤单接受后本地订单保留为待最终对账，下一轮先同步成交再确认取消，避免 cancel/fill 竞态丢成交；post-only violation 的 cancel rejected/unknown 会按最小 15 秒间隔重试，cancel accepted 但超过 30 秒仍未完成最终对账时会再次尝试撤单。

worker 每轮还会读取 CLOB 账户开放订单 snapshot：未归属但 token 可唯一映射到 active reward market 的开放 BUY 会被收养为 managed order，已有同 external id 的非 open 本地 BUY 会在 CLOB 仍 open 时重开；已提交、open-like、普通 BUY managed order 若不在 snapshot 中且不处于提交未知、404、pending cancel、post-only violation 或其他对账锁状态，会本地标记为 `cancelled`，释放开放挂单计数；sell exit 仍走单订单/成交对账和 retry 逻辑。worker 仅在 trade 达到 `CONFIRMED` 后按 external trade id + external order id 幂等写入 fills、现金、库存和 PnL；买入 fill 与对应 exit intent 同事务落库，之后只撤同 condition 对侧仍开放的 buy sibling。

撤单已被 CLOB 接受并处于 `cancel accepted; awaiting final reconciliation` 的 BUY 不再无限保留全局新买单锁：如果后续 CLOB open-order snapshot 已确认该 external order 不存在，worker 会把本地剩余量关闭为 `cancelled` 并释放新增 BUY；提交结果未知、外部 404 人工对账、cancel result unknown 和 post-only violation 等更高风险锁仍按各自严格对账路径处理。

`ExitAtMarkup` 价格以被吃买单原价加 `exit_markup_cents` 为基准并向上取整到 0.01 tick，默认加价为 0；`HoldAndRequote` 按被吃买单原价持久化 post-only SELL intent，之后继续正常报价；外部 positions 快照检测到尚无 open-like SELL 的非零库存时，也会按该持仓 `avg_price` 向上对齐 tick 后创建原价 post-only SELL intent，避免已有库存无人接管退出；退出 floor 始终用 intent price 与当前持仓 `avg_price` 的较高值，不会用 midpoint 或低于 floor 的买一价覆盖；提交前会把 SELL size 裁剪到当前同 token 持仓，若没有对应持仓或剩余 size 为 0，会关闭本地 stale exit 并记录 warning，避免余额不足拒单反复刷屏；提交前低于 1 美元最小名义金额的退出单会进入短 reason 的 dust deferred 状态，每 300 秒重新评估但不重复拼接历史原因。同 token 有未完成 sell exit 时暂停新增 buy。

`ExitAtMarkup`、`HoldAndRequote` 和外部库存补退出按 post-only maker SELL 提交 floor；如果当前买一已大于等于 maker 价格，原价 post-only 会穿盘口无法 resting，worker 会保留 `exit_pending` 并按 30 秒退避等待可作为 maker 挂出。`FlattenImmediately` 会持久化非 post-only flatten intent，提交前读取当前 best bid；best bid 不低于 floor 时按 best bid 用 FAK/taker SELL 尝试非亏损平仓，best bid 缺失或低于 floor 时按 30 秒退避保留 deferred exit。post-only 明确退出拒单使用有界退避，达到最大拒绝次数后停止盲目重试。

每轮先同步 managed order 的 confirmed fills；CLOB open-order snapshot 和账户开放 buy notional 观测不受 confirmed fill 保护期影响。本轮新增 fill，或最新 confirmed fill 距今不足 120 秒时，只跳过外部 balance/positions 替换，防止最终一致性延迟回滚本地现金和库存。保护期结束后，成功 positions 快照原子替换该 rewards 账户全部持仓，并为没有退出单的库存补原价 SELL intent；失败时保留上一版。该同步在 `enabled=false` 且没有开放订单时也会尝试运行。SELL、非 rewards 市场和无法唯一映射 token 的外部开放订单明细，以及奖励结算对账仍是缺口，worker 仍需要独立维护组合风险。
每次成功读取 CLOB 账户开放订单 snapshot 后，worker 还会把仍出现在该 snapshot 中且外部剩余量为正、状态非 filled/matched/cancelled/expired 的本系统 managed 外部订单数量写入 `RewardBotService` 热缓存，供控制台 `status.open_orders` 优先展示，避免本地仍 open-like 但外部已不开放或已完全成交的订单短时间抬高开放挂单数。

SELL 退出 intent 持久化的是非亏损退出 floor（被吃买单原价、markup 后价格或外部持仓 `avg_price`，并在提交前再与当前持仓 `avg_price` 取较高值）；提交前不会使用 midpoint 或页面“当前价”降价。`ExitAtMarkup`、`HoldAndRequote` 和外部库存补退出按该 floor 提交 post-only maker SELL 来保留反向流动性奖励机会；若当前 orderbook 买一已大于等于 floor，原价卖单会穿盘口，因此只记录 `reward_live_exit_post_only_crossing_deferred` 并按 30 秒退避等待可 resting 的盘口。`FlattenImmediately` 只有在 best bid 不低于 floor 时才用非 post-only FAK/taker SELL 按 best bid 尝试非亏损平仓；best bid 缺失或低于 floor 时保留 deferred exit 并按 30 秒退避。SELL 提交前会用当前 positions 校验并裁剪 size；无对应 token 持仓时关闭 stale exit。

poll loop 每轮读取持久化 rewards 配置；读取失败时不会使用默认配置冒险执行，也不会永久退出任务，而是等待 1 秒后重试。控制命令 wake、配置 revision 变化、活跃 rewards token 的 orderbook stream 更新和周期 timer 都会唤醒 loop；活跃盘口事件还会进入独立事件撤单 channel，由 `RewardEventCancelGuard` 常驻 task 立即执行 hard-risk cancel-only 检查，不等待 poll loop 当前 full tick、控制命令或外部同步结束。普通 fast reconcile 仍会被活跃盘口 wake 或周期 timer 唤醒，用于成交同步、退出 intent、完整撤单检查和节流外部同步。持有 live advisory lease 的 poll loop 启动后会立即尝试一次 rewards 历史清理，之后每 5 天清理一次 5 天前的终态 managed orders（`cancelled`/`filled`/`error`）、risk events 和 legacy 低竞争 observations；清理失败只记录 warn，不退出交易循环，并且不会删除 `planned`/`open`/`exit_pending`、fills、positions 或 account state。fast reconcile 每轮仍会用活跃盘口做风险撤单和退出 intent 处理，但外部重型同步独立节流：托管订单状态最小 5 秒间隔，CLOB open-order snapshot 最小 15 秒间隔，managed scoring 按 `min_scoring_check_sec` 且归一化下限 15 秒，账户级 rewards earnings 与 balance/positions snapshot 最小 60 秒间隔；full tick 和 `run_once` 完整同步后会刷新这些节流时间戳，避免紧随其后的盘口事件重复打外部 API。full tick 仍由 `POLYEDGE_REWARDS__POLL_INTERVAL_SECS` 作为全量候选发现和计划重建兜底，fast reconcile 仍由 `reconcile_interval_sec` 作为兜底 sweep；内部 WS 空闲重连只恢复事件源，不能替代这两个周期兜底。数据库 worker heartbeat 写入失败只记录告警；CLOB 订单 heartbeat 独立每 5 秒发送并串联 server 返回的 heartbeat id，单次请求 4 秒超时，失败或超时后清空 id 并按 5-60 秒退避重建链；首个失败和连续失败每 6 次记录 warn，其余连续失败降为 debug，恢复时记录 info。生产环境必须保持 poll loop 运行；一次性命令或有限循环退出后不再维护订单 heartbeat。

旧的未提交 quote intent 会先经过当前计划、盘口、kill switch 和撤单风险检查，再允许提交；BUY intent 在真正 POST 前还会用 orderbook 服务做当前提交 token 的 1 秒 max-age last-look，若盘口缺失、刷新失败或出现 best ask touch 等硬风险则不提交并等下轮。任一提交结果未知、待最终对账或外部订单 404 会暂停全部新增买单，但不会阻断订单同步、风险撤单或卖出退出；外部订单 404 锁超过 5 分钟且仍无成交证据时会自动本地关闭；提交结果未知订单在恢复查询确认 CLOB 无对应 open 订单后经过 `LIVE_SUBMISSION_UNKNOWN_CLOSE_AFTER_SECS`（默认 600 秒）宽限也会自动本地关闭，不再需要手动改库，但若 positions 快照显示该 BUY token 在提交后出现库存则继续保留对账锁。同一批次第一笔 POST 结果未知后也不会继续发送后续买单。CLOB 已明确返回的 HTTP 4xx 拒单会将当前 intent 标记为 error，不会误进入提交未知锁。managed order 会持久化实际提交数量；SELL intent 的 price 保留非亏损退出 floor，post-only exit 被取消后的 replacement 会保留退出 floor 并在后续按 maker 规则重试，flatten replacement 保留退出 floor 并在后续按 best bid 非亏损 FAK 或继续等待。

关联 trade 按 ID 单独查询失败时，connector 会按该订单 token 和下单时间窗口扫描认证账户 trades，并按 external order id 精确匹配；只有所有预期关联 trade 都达到终态后才关闭订单。若认证 CLOB 已明确给出 matched size，但认证 trade 明细与历史页仍无法解码，worker 会再读取 Data API 钱包活动，并且只在 token、BUY、价格、时间窗口、累计数量与唯一 managed order 全部严格匹配时生成补账 fill。单订单已返回 404 时，无论认证账户 trade 扫描报错，还是扫描成功但没有返回该 external order id 的成交，都会继续执行 Data API 回退；此时还必须要求累计数量恰好等于本地订单剩余量，并且完整外部持仓快照已覆盖该数量。外部账户/持仓快照时间已覆盖该成交时，补账不会再次扣减现金或增加库存，但仍会关闭本地订单并创建退出 intent。任一订单的全部回退都失败时，worker 只跳过该订单并继续处理其余订单、账户快照和 stale 清理；如果同一外部订单 404 锁已持续超过 5 分钟，worker 会本地关闭该订单，不再中止整轮 reconcile。

### orderbook_stream — 盘口流（已迁移到 orderbook 服务）

盘口流逻辑已迁移到 `polyedge-orderbook` 服务（`packages/backend/order/src/stream.rs`）。Worker 中 `worker/orderbook_stream.rs` 仅保留 `consume-orderbook-stream` CLI 子命令兼容（daemon 模式不再调度），兼容路径同样消费完整 `book` 快照和 `price_change` 增量，并周期性全量 poll 当前 token。Worker 通过 `OrderbookHttpClient`（HTTP）读取 orderbook 服务的缓存数据，通过携带 `POLYEDGE_ORDERBOOK__WRITE_TOKEN` 的 `register-orderbook-tokens` 任务注册订阅 token；rewards poll loop 还会通过 `OrderbookStreamClient` 订阅 orderbook 服务内部 `/orderbook/stream`，将 WS/poll/ingest 更新写入 worker 本地 cache，并把活跃 rewards token 更新同时用于唤醒普通 fast reconcile 和投递到独立 hard-risk 撤单 worker。Standalone orderbook 服务遵守 `POLYEDGE_ORDERBOOK_STREAM__RESTART_INTERVAL_SECS`，HTTP `/orderbook/register` 原子替换对应 source 的 token 集合，缓存每侧盘口深度受 `POLYEDGE_ORDERBOOK_STREAM__MAX_LEVELS_PER_SIDE` 限制。

### news — 新闻采集

```
settings.news.sources (enabled 过滤；未配置 POLYEDGE_NEWS__SOURCES_JSON 时使用内置默认 RSS/Atom 源)
    → RssNewsConnector.fetch() per source
    → news_ingestion_service.ingest_source_items()
    → SHA-256 去重 → insert_raw_news_event()
```

Report: `NewsIngestionRunReport { sources_scanned/succeeded/failed, fetched, inserted, deduped }`

## 依赖关系

- **上游**：所有 crate（domain、application、connectors、infrastructure）
- **下游**：无（终端执行者）
- **配置来源**：`infrastructure::Settings` 中的 worker、rewards、copytrade、news 配置段；盘口数据通过 `POLYEDGE_ORDERBOOK__SERVICE_URL` 连接 orderbook 服务

## 当前状态

- 常用维护/调试子命令已实现，`polyedge-worker` 仍作为 CLI 兼容入口保留
- `run` 主循环包含 database-maintenance、register-orderbook-tokens、rewards、copytrade、Smart Money、news 和 execution/对账等任务；Smart Money runtime 默认关闭，需同时开启 worker 开关和 Smart Money config；旧 arbitrage radar 与 signal recompute 循环已移除
- database-maintenance 默认生产模板开启、本地模板关闭；它集中清理可增长历史/缓存/队列表，避免 `reward_market_candles`、AI/info-risk cache、raw events、copytrade/source trade、控制命令、outbox/dedup、LLM/audit 等表无限膨胀。
- news worker 当前只抓取 RSS/Atom XML feed；未配置 `POLYEDGE_NEWS__SOURCES_JSON` 时会读取内置默认源列表，部署模板显式写入默认源并默认设置 `POLYEDGE_NEWS__ENABLED=true`、`POLYEDGE_WORKER__POLL_NEWS=true`
- rewards worker 会通过数据库命令队列接收前端 Run / Cancel / Reset 请求，API 进程不再执行 rewards 策略；仅支持 live 实盘模式，策略配置不依赖全局 system mode，但新买单和现有买单撤单遵守全局 kill switch
- copytrade worker 会通过数据库命令队列接收前端兼容控制命令；当前前端只暴露 Analyze，Run/Cancel/Reset 不再作为产品入口。API 进程不抓取 copytrade 输入，worker 负责 Data API 抓取、source trades 检测和钱包分析
- register-orderbook-tokens 每个 source 独立注册全量 token，由 orderbook registry 聚合层按固定优先级 `rewards_active`、`exec_orders`、`rewards_eligible`、`rewards_ai_provider`、`rewards_candidates` 跨 source 去重并 `take(POLYEDGE_ORDERBOOK_STREAM__MAX_TOKENS)` 截断总量；`rewards_eligible` 由周期任务统一注册全部最终 eligible quote plan token，AI gate 前的 deterministic eligible token 由 AI advisory provider refresh 以最多 10 个市场一批注册到临时 `rewards_ai_provider` source；AI/info-risk 两条 refresh 队列各自保留开放订单/持仓优先，其余按统一 standard 候选顺序处理；rewards live 新买单落库后会即时刷新 `rewards_active` source，候选 token 优先来自 open/tradable 且 `volume_24h` 高的市场并受 `POLYEDGE_ORDERBOOK_STREAM__REWARD_CANDIDATE_TOKEN_CAP` 限制；空集合成功结果会防抖后再清 source，active/exec 连续 2 轮、eligible/candidates 连续 3 轮才清理
- rewards poll loop 在 Postgres 路径全程持有 advisory lease，统一覆盖 CLOB heartbeat、命令、orderbook 内部 WS 本地 cache、独立事件撤单 worker、full tick、fast reconcile 和 5 天历史清理；本地盘口 cache 按本地接收 TTL 过期，避免上游未来时间戳延长缓存寿命；控制命令具备 5 分钟 running lease
- 活跃 rewards token 的 orderbook 更新会进入独立事件撤单 worker，按更新 token 过滤开放订单并立即执行 hard-risk cancel-only 检查；BUY 硬风险包含计划不可挂、盘口缺失/为空、stale grace 后过期、best ask touch、外部 bid 深度不足、bid rank 过高、盘口历史窗口风险和全局 kill switch；该路径不跑订单/账户同步、重挂、退出提交、报价漂移换价或定期 requote，普通 fast reconcile 仍作为完整检查和周期兜底
- rewards orderbook 内部 WS client 建连最多等待 5 秒；已连接后若约 3 个 orderbook poll reconcile 周期无事件，会主动重连并重新 HTTP bootstrap 本地盘口 cache
- scheduled full tick 不再二次消费控制命令；拿不到 advisory lease 时保留到期状态并在后续轮询重试，不会把 command-only 周期记作已完成 full tick
- rewards poll loop 按账户写入 `reward_worker_heartbeats`；snapshot 的 `status.running` 仅在配置启用且最近 2 分钟存在 heartbeat 时为 true
- rewards SELL 退出 intent 按非亏损 floor 执行：`ExitAtMarkup`、`HoldAndRequote` 和外部库存补退出走 post-only maker SELL；`FlattenImmediately` 在 best bid 不低于 floor 时走非 post-only FAK/taker SELL，否则按 30 秒退避等待非亏损 bid。提交前会按当前 token 持仓裁剪 size，无持仓 stale exit 会关闭，不使用 midpoint 或页面“当前价”降价卖出。
- 低竞争市场 sleeve 已合并进统一机会评分，不再构建独立低竞争候选 profile、probe source、shadow report、低竞争 open-order 占比 cap 或专用撤单 gate；legacy `low_competition_*` 配置和 observation 表仅兼容历史数据。full tick 会为所有 quote plan 计算 `opportunity_metrics`，把竞争倍数、100U 日奖、账户/单市场资金占比、退出深度/滑点、坏成交恢复天数和盘口稳定性转为四个组件分数与综合机会分，并通过 `score_adjustment` 调整 score 与 `min_market_score` 资格判断。
- rewards full tick 已读取 Gamma `markets.category` 作为候选评分输入；命中 `preferred_categories` 时只增加候选排序分，不绕过市场质量、盘口和风控硬过滤。AI advisory 已接入 full tick：live tick 先应用已有 AI/info-risk cache gate，再启动后台 AI advisory provider refresh 与 info-risk provider refresh 两个 task 分别异步填充 `reward_market_advisories` 和 `reward_market_info_risks`，两条队列各自保留开放订单/持仓最高优先级，其余按统一 standard 候选顺序处理，并跳过 provider/request_format/model 匹配且未到提前刷新窗口的缓存命中 condition；新写入的 AI/info-risk cache 使用确定性 TTL jitter 打散过期时间，provider refresh 会在未过期缓存进入较小刷新窗口（`min(TTL/20, 60s)`）时提前续期；AI advisory 使用临时盘口批次补缓存，info-risk 独立批量/逐市场补缓存，`ai_advisory_batch_size` 和 `info_risk_batch_size` 分别控制主 refresh 单次 provider 请求包含的市场数，默认 1 保持逐市场，批量解析按 condition 拆分保存，漏项/错配或整体解析失败会回退单市场，provider 过载则停止对应 task 本轮剩余请求；每次实际外部 AI advisory / info-risk provider 调用会记录到 `llm_calls`，失败的 HTTP/解析结果也计入失败调用，供 snapshot 按日展示；AI 请求候选限定为 deterministic planner 原本 eligible 且仍缺少有效 advisory 的 condition，payload 将 5m price-history source candles 聚合为最多 24 根 1h candles 和摘要，cache key schema 10 只包含已完成小时级摘要和统一策略上下文，不包含当前小时内 source 更新或旧低竞争专属策略字段；AI 开启后 provider 未配置、失败或缺缓存仍 fail closed，`avoid` 硬拦截；旧 `suitability` 响应按二值 fail-closed 兼容，`watch`/`avoid`/其它非 allow 值一律映射为 `avoid` 硬拦（旧 watch 放行行为已移除）。`allow` 且 confidence 达到 `ai_strategy_hint_min_confidence` 时，worker 会直接应用 `strategy_hint`：方向只能收窄或跳过，bid rank 只能更保守，同 condition 新增 BUY 预算会被 `max_condition_notional_usd` 压低；低置信度 `allow` 仍回退 deterministic 计划继续进入 live 盘口、资金和订单风控。provider confidence 会被钳制到 `0..=1`。信息风险已接入缓存过滤：AI 开启时由 full tick 专用 info-risk provider refresh 推进，AI 未开启时仍可由独立 info-risk worker 扫描；live tick 只读缓存，enforce 模式下缺缓存仍 fail closed；新 condition 首次 BUY 报价还会按配置等待信息风险缓存和首单观察窗口，已有订单/持仓 condition 不受该首单 gate 限制；已有风险中只有 `critical`、官方结果、`resolution_imminent=true` 或配置为 `low/medium` 避免等级时命中的风险等级会硬拦截，普通 `high` 风险和仅 `risk_type=imminent_resolution` 但 `resolution_imminent=false` 的结果保留为信息提示并继续进入 live 盘口、资金和订单风控；新增买单会保守扣除未归属到本系统 managed order 的外部 BUY notional。
- rewards full tick 现在在标记 `pre_ai_eligible` 和启动 AI/info-risk provider refresh 前执行 live funding precheck；无开放订单/持仓的新 condition 若当前可用资金放不下最低 rewards size 待补腿，会先写入 funding reason 并退出普通 provider 队列，从而减少大模型请求，已有订单/持仓 condition 仍保留 provider 优先级。
- rewards full tick 和 fast reconcile 在 managed order 同步后总会读取 CLOB open-order snapshot，收养/重开可映射到 active reward market 的开放 BUY，关闭缺失或 snapshot 中已非 active 的普通 managed BUY，并刷新账户开放 buy notional；资金钱包地址优先使用 `POLYEDGE_POLYMARKET__FUNDER`，未配置时使用 `ACCOUNT_ID`；CLOB balance 为 0 或失败但链上 pUSD 余额大于 0 时，账户 snapshot 用 Polygon pUSD 余额回填；新确认成交所在周期及其后 120 秒只延后外部 balance/positions 替换，避免 CLOB/Data API 最终一致性回滚本地账本
- 默认大部分 worker 通过配置开关控制启用/禁用
- Polymarket live 任务需要真实凭证；Deposit Wallet 使用 `POLYEDGE_POLYMARKET__SIGNATURE_TYPE=poly_1271` + `POLYEDGE_POLYMARKET__FUNDER=<deposit_wallet>`，worker 会通过 connector 走 CLOB V2 `POLY_1271` 下单/撤单路径。
- Rewards 生产与测试入口均已移除 `RewardSimulationOutcome` / `simulated_orders` 旧命名，统一使用 `RewardTickOutcome` / `placed_orders`。

## 修改检查清单

- [ ] 新增 worker 任务时：(1) 在 `worker/` 中创建文件 (2) 在 `main.rs` 中添加 CLI 子命令 (3) 在 `run_worker_service()` 中添加到主循环
- [ ] 修改 worker 逻辑后检查对应的 application service 是否需要更新
- [ ] 新增 Report 类型时确保使用 `Default` derive 并包含有用的统计字段
- [ ] 运行 `cargo check --workspace --tests`
- [ ] 更新根目录 `AGENTS.md` 中的常用 worker 子命令列表
