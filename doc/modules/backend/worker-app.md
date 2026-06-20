# Worker App（后台任务服务）

最后更新：2026-06-20

## 概述

Worker 代码现在同时提供共享库和兼容 CLI。`polyedge-api` 在同一进程内启动 `WorkerRuntime`，运行新闻采集、信号重算、执行分发、订单对账、奖励机器人、套利扫描、copytrade 钱包跟踪/分析和 orderbook token 注册；`polyedge-worker` 二进制继续提供维护/调试子命令，但 Docker 不再部署独立常驻 worker 容器。

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
| `worker/market_sync.rs` | 市场同步 CLI 兼容入口：Gamma liquidity/end time → `markets` + Rewards API → `reward_markets` |
| `worker/news.rs` | 新闻采集入口 |
| `worker/news_helpers.rs` | 新闻采集辅助函数 |
| `worker/news_promotion.rs` | 新闻提升为 events/evidence |
| `worker/signal_recompute.rs` | 信号重算 |
| `worker/execution_dispatch.rs` | 执行请求分发与 confirmed live trade 对账 |
| `worker/execution_queue.rs` | 执行队列管理 |
| `worker/execution_reconcile.rs` | 订单/成交对账 |
| `worker/orderbook_stream.rs` | Orderbook stream — 仅保留 CLI 子命令兼容，核心逻辑已迁移到独立 `polyedge-orderbook` 服务 |
| `worker/rewards.rs` | 奖励机器人 tick；消费 API 入队的 run/cancel/reset 控制命令 |
| `worker/rewards/account_sync.rs` | rewards 外部余额、CLOB open-order 反查/BUY 收养重开、完整持仓、订单 scoring 与 UTC 当日账户级 rewards 聚合同步 |
| `worker/rewards/live_sync.rs` | rewards live 托管订单成交/状态同步、单订单失败隔离、Reset cancel-all 语义 |
| `worker/rewards/live_orders.rs` | rewards live 撤单、成交入账、post-fill exit/flatten intent |
| `worker/rewards/live_submission.rs` | rewards live 单笔提交、post-only 接受状态处理和 submission marker |
| `worker/rewards/live_pending.rs` | rewards live 持久化 intent 提交、开放订单匹配恢复和未知结果锁定 |
| `worker/rewards/live_helpers.rs` | rewards live 价格 tick、fill id、退出重试与订单状态转换辅助函数 |
| `worker/rewards/live_risk.rs` | rewards live placement/cancel 风控：盘口可用性、depth/rank/history/requote、库存 cap、低竞争 sleeve 独立订单/库存 cap |
| `worker/rewards/orderbook_events.rs` | rewards worker 本地盘口 cache、orderbook 内部 WS 消费、HTTP bootstrap、活跃 token 事件唤醒和 condition 盘口首次就绪检测（驱动 AI advisory 批量 worker） |
| `worker/rewards/polling.rs` | rewards poll loop、盘口读取、事件驱动 fast reconcile、外部同步节流和进程内盘口历史 |
| `worker/rewards/provider_advisory.rs` | rewards AI advisory cache gate、候选排序 helper、provider connector/permit helper |
| `worker/rewards/provider_refresh.rs` | rewards AI advisory / 信息风险统一 provider refresh：按 condition 先补 AI advisory 再补 info-risk，并受每轮 condition cap 与 provider 错误退避控制 |
| `worker/rewards/provider_batch.rs` | rewards AI advisory orderbook 事件驱动批量 worker：盘口首次就绪入队、攒批 `advise_batch`、缺失回退单请求、info-risk 同步推进（默认关闭，与 provider refresh 并存） |
| `worker/rewards/info_risk.rs` | rewards 信息风险异步扫描、provider 缓存命中、每轮扫描 cap、quote plan 风险应用 |
| `tests/rewards.rs` / `tests/rewards_reconciliation.rs` | rewards live 下单、风控、成交、对账、退出重试与增量持久化回归测试 |
| `worker/arbitrage.rs` | 套利扫描 |
| `worker/arbitrage_books.rs` | 套利盘口快照 |
| `worker/copytrade.rs` | copytrade 钱包跟踪与分析；消费 API 入队的 run/analyze/cancel/reset 兼容控制命令 |
| `worker/polymarket_config.rs` | Polymarket 配置刷新 |
| `worker/polymarket_events.rs` | Polymarket 用户事件 WebSocket |
| `worker/shared.rs` | 共享辅助函数 |

## CLI 子命令

| 命令 | 函数 | 描述 |
|---|---|---|
| `run`（默认） | `run_worker_service` | 兼容长期 worker 循环；正常部署由 API 内嵌 runtime 代替 |
| `sync-markets-once` | `sync_markets_once` | 一次性市场同步 |
| `ingest-news-once` | `ingest_news_once` | 一次性新闻采集 |
| `poll-news` | `poll_news` | 持续新闻轮询 |
| `promote-news-events` | `promote_news_events` | 新闻提升为 events |
| `scan-arbitrage-once` | `scan_arbitrage_once` | 一次性套利扫描 |
| `poll-arbitrage-radar` | `poll_arbitrage_radar` | 持续套利扫描 |
| `analyze-arbitrage-opportunities` | `analyze_arbitrage_opportunities` | 套利历史分析 |
| `scan-rewards-once` | `run_reward_bot_once` | 一次性消费 rewards 控制命令或执行 live 策略 tick；仅适合诊断，不维持长期订单 heartbeat |
| `poll-reward-bot` | `poll_reward_bot` | 持续消费 rewards 控制命令和 live 策略轮询 |
| `scan-reward-info-risks-once` | `scan_reward_info_risks_once` | 一次性异步扫描 rewards 候选、开放订单和持仓市场信息风险并写入缓存 |
| `poll-reward-info-risks` | `poll_reward_info_risks` | 持续异步扫描 rewards 候选、开放订单和持仓市场信息风险 |
| `scan-copytrade-once` | `run_copytrade_once` | 一次性消费 copytrade 控制命令，或扫描 active tracked wallets 并记录 source trades |
| `poll-copytrade` | `poll_copytrade` | 持续消费 copytrade 控制命令并轮询 tracked wallets |
| `analyze-wallets-once` | `analyze_wallets_once` | 一次性钱包分析 |
| `drain-execution-queue` | `drain_execution_queue` | 处理排队的执行请求 |
| `reconcile-paper-fills` | `reconcile_paper_fills` | Paper 交易对账 |
| `poll-paper-order-statuses` | `poll_paper_order_statuses` | Paper 订单状态轮询 |
| `poll-polymarket-order-statuses` | `poll_polymarket_order_statuses` | Live Polymarket 订单状态轮询 |
| `reconcile-polymarket-fills` | `reconcile_polymarket_fills` | Live Polymarket 成交对账 |
| `consume-polymarket-user-events` | `consume_polymarket_user_events` | 消费 Polymarket WS 事件 |

## 核心 Worker 数据流

### market_sync — 市场同步（已迁移到 orderbook 服务）

市场同步逻辑已迁移到 `polyedge-orderbook` 服务（`packages/orderbook/src/market_sync.rs`）。Orderbook 服务启动时先暴露 HTTP `/healthz`，再由后台任务执行 initial/periodic market sync，避免外部市场 API 延迟阻塞容器健康检查。Worker 中保留 `sync_markets_once` 函数供 CLI 子命令 `sync-markets-once` 使用，但 daemon 模式不再调度此任务。

### register-orderbook-tokens — 盘口 token 注册

```
register_orderbook_tokens()
    → 遍历活跃执行订单（Submitted/Open/PartiallyFilled）→ 解析市场 YES/NO asset_id
    → reward_bot_service.list_active_reward_book_token_ids() → rewards 活跃订单/持仓 token
    → reward_bot_service.list_eligible_reward_book_token_ids() → 当前最终 eligible + pre-AI deterministic eligible quote plan token
    → reward_bot_service.list_all_reward_candidate_token_ids() → rewards 候选 token 填充剩余额度
    → orderbook_registry.register_tokens("rewards_active", ...)
    → orderbook_registry.register_tokens("exec_orders", ...)
    → orderbook_registry.register_tokens("rewards_eligible", ...)
    → orderbook_registry.register_tokens("rewards_candidates", ...)
    // 通过 OrderbookHttpClient → HTTP POST /orderbook/register 注册到 orderbook 服务
```

此任务替代了原来的 `consume-orderbook-stream` 和 `sync-markets` 任务。Worker 不再直接运行盘口流或市场同步，而是通过 HTTP 告知 orderbook 服务需要订阅哪些 token。
注册任务最长每 60 秒执行一次，orderbook 服务重启后可自动恢复订阅。每个 source（rewards 活跃订单/持仓 token、活跃 execution token、当前最终 eligible 或 pre-AI deterministic eligible quote plan token、其余 rewards 候选 token）独立收集并各自去重、截断后注册；跨 source 去重和总量上限由 orderbook registry 聚合层负责（按固定优先级 `rewards_active > exec_orders > rewards_eligible > rewards_candidates` 合并去重后 `take(POLYEDGE_ORDERBOOK_STREAM__MAX_TOKENS)`）。`rewards_eligible` source 由该周期注册任务统一注册全部最终 eligible quote plan token，以及 AI/info-risk gate 清空实际下单腿后仍保存在 `orderbook_token_ids` 的 pre-AI deterministic eligible token（不再由 rewards full tick 单独注册），因此不会因 active 持仓覆盖 eligible token 而被清空，也不会因 AI advisory pending 和缺盘口互相等待，或两个注册者交替写入触发 WS 订阅重建振荡。候选来源只用于给尚未产生 quote plan 的市场提前预热盘口，受 `POLYEDGE_ORDERBOOK_STREAM__REWARD_CANDIDATE_TOKEN_CAP` 限制，默认只预热 100 个候选 token，设为 0 会清空 `rewards_candidates` source 但不影响 active/final-eligible/pre-AI-eligible token 注册。每个成功查询的 source 使用一次原子替换注册，空集合会清理远端旧 source；任一 source 的数据库查询失败时保留远端上一版集合，不会用空集合误删订阅。

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

### rewards — 奖励策略与控制命令

```
reward_bot_service.claim_next_control_command()
    → worker 执行 queued run_once / cancel_all / reset
    → reward_bot_service.complete_control_command() 或 fail_control_command()

无待处理控制命令时：
    fetch_reward_bot_inputs() // 获取奖励市场 + 盘口
        → prepare_live_cycle()
        → 低竞争 sleeve 计算竞争资金、预估 reward share、退出深度和盘口稳定性指标
        → 只应用已缓存 AI advisory，并在后台单实例按 condition 统一刷新缺失 AI advisory + 信息风险缓存（AI advisory 仅在该市场所有报价 token 盘口都已发布后才请求并写缓存，请求 payload 会读取最近 24 根 5m price-history candles；缺盘口的市场本轮跳过请求、不写缓存，等盘口到达后再评估，避免缓存空 watch/avoid 长期卡住市场）
        → 读取已缓存的信息风险并按配置标记/过滤 quote plan
        → 写入低竞争 observation，供 snapshot shadow report 汇总
        → sync managed rewards order trades/statuses
        → 批量同步 managed order scoring 状态与 CLOB open-order snapshot（收养/重开 active rewards BUY，关闭缺失 managed BUY）
        → 同步 UTC 当日账户级 maker rewards（`/rewards/user/total?sponsored=true` 聚合优先，对齐官网 native+sponsored 口径；明细 fallback 合并 native 与 sponsored-only）
        → 无近期 confirmed fill 时同步外部 balance + 链上 pUSD 余额回退 + 完整 positions 快照
        → LivePolymarketConnector.submit_token_order()
        → orderbook stream active-token event 或 reconcile_interval_sec 兜底：读取活跃盘口并对本系统托管订单做成交同步和安全撤单检查
```

Report: `RewardBotRunReport { markets_scanned, books_fetched, plans_built, eligible_plans, placed_orders, cancelled_orders, filled_orders, risk_cancelled_orders, reward_accrued }`

约束：后台 runtime 是 rewards 策略和控制命令的唯一执行者。rewards poll loop 在整个生命周期持有 Postgres advisory lease，多实例中只有 lease owner 会认证 CLOB、执行命令/full tick/fast reconcile 并维护 5 秒 heartbeat id 链；standby 实例不发心跳也不执行。API handler 只写控制命令；同一账户同一动作已有 `pending/running` 命令时会合并重复入队，避免重复点击制造多轮 full tick 或 cancel-all；共享 `RewardBotService` 会在真正入队时立即唤醒同进程 loop，配置 revision 变化还会立即触发 full cycle。`POLYEDGE_WORKER__POLL_REWARD_BOT=true` 控制 API 内嵌 runtime 是否启动 rewards loop。

自动 tick 只从 Postgres 的 `reward_markets` 读取奖励市场。长期 `poll-reward-bot` 启动后会通过 `OrderbookStreamClient` 连接 `polyedge-orderbook` 的内部 `/orderbook/stream`，维护 worker 进程内本地盘口 cache；本地 cache 使用 `POLYEDGE_ORDERBOOK_STREAM__MAX_LEVELS_PER_SIDE` 和 `POLYEDGE_ORDERBOOK_STREAM__BOOK_TTL_MS` 限制深度与过期读取。启动和重连时通过 `OrderbookHttpClient` / `POST /orderbook/batch` bootstrap 当前 rewards 活跃、eligible 和候选 token，后续缺失 token 也会按需 HTTP 补齐；周期注册任务会把全部最终 eligible quote plan token 和 pre-AI deterministic eligible token 注册到 orderbook `rewards_eligible` source，候选预热关闭时这些市场仍按需订阅准备 provider refresh 或挂单。内部 WS 连接建立最多等待 5 秒，已连接后若约 3 个 poll reconcile 周期没有收到任何事件，worker 会主动重连并重新 HTTP bootstrap。Postgres 候选 market pool 关联 Gamma `markets`，硬过滤非 open/tradable、高歧义、低流动性、低 24h 成交量、临近结算、Gamma spread 过宽、市场同步过期、奖励不足以及 FDV/launch/token/official-result 等高事件跳变风险市场；非 auto/enforce 单边回退模式仍预筛双边最小份额预算，auto/enforce 且启用 dominant single-side 时则保留候选到 planner 用真实盘口价格判断双边或单边可负担性；仅唯一且明确的 YES/NO token 会进入候选与订阅。通过门槛后按奖励、流动性、成交量、剩余时长和奖励 spread 综合排序。worker 使用本地 cache 读取候选和活跃 token 盘口，缺失时才回源 orderbook HTTP batch；若本 tick 没有新鲜缓存盘口，不会提交新 post-only 订单。

Worker 本地盘口 cache 的 TTL 按本地接收/写入时间计算，避免上游未来 `observed_at` 延长旧盘口寿命；上游 `observed_at` 仍保留给 planner 和 live 风控判断盘口新鲜度。

低竞争 rewards sleeve 已实现 v2，默认关闭。worker 会从 `RewardBotService` 读取标准候选和低竞争候选 profile，低竞争 profile 只放宽自身流动性/24h 成交量门槛，仍共享市场安全硬过滤；full tick 在 `prepare_live_cycle()` 后用 orderbook 服务提供的当前盘口和 worker 本地盘口历史计算 `qualified_competition_usd`、`estimated_reward_per_100_usd_day`、退出深度、退出滑点、样本数和 midpoint 波动。`low_competition_mode=observe` 会把指标写入 quote plan 但强制低竞争 bucket 不可挂；`enforce` 要求低竞争指标达标、`ai_advisory_enabled=true` 且 `info_risk_enabled=true/info_risk_mode=enforce`，之后仍走现有 AI advisory / info-risk cache gate，缺缓存或 provider 拒绝会 fail closed。AI/info-risk gate 完成后，worker 会把低竞争 observation 写入 `reward_low_competition_observations`，记录最终可挂状态、provider 拦截、样本不足、退出深度和滑点，API snapshot 再汇总最近 24 小时 shadow report；该 report 只给建议，不自动改配置。live placement 对低竞争 bucket 使用独立 `low_competition_max_markets`、`low_competition_max_open_orders`、`low_competition_per_market_usd` 和 `low_competition_max_position_usd`，同时继续受全局订单/市场上限、kill switch、盘口风控和账户外部 BUY notional 约束。该实现仍只从数据库和 orderbook HTTP/内部 WS cache 读取数据，不直接调用 Polymarket Gamma/CLOB 外部 API。

报价计划构建阶段只应用市场质量、概率区间、配置和非盘口依赖过滤，不再因为 `quote_bid_rank` 缺档、目标价格超出 rewards spread、auto 单边所需的退出深度/top1/top3 买盘集中度/HHI 或实际盘口价格预算而淘汰市场；quote plan 的腿可以只是 YES/NO token 占位元数据。live placement 准备创建订单时才用当前 orderbook materialize 真实腿：报价价格由 `quote_bid_rank=1|2|3` 选择 YES/NO 目标买盘价，粗 tick 盘口按买一/买二/买三（不同买价）选择，细 tick 盘口会从买一回退 `rank-1` 个 0.01 价格步长后选择不高于目标价的当前买盘档位，避免 0.001 tick 下买三只退两个细档；随后验证目标档位、rewards spread、touch ask、安全边际、auto/enforce 盘口指标和实际 size/notional。缺少、空或过期盘口时不下单、不写 12 小时 skip，而是保持 quote plan eligible 并写入等待 orderbook 订阅数据的 reason；后续 full tick 拿到新鲜盘口后会重新 materialize 并继续挂单流程。非 transient live 验证不通过时不下单，并把该 quote plan 标记 `live_skip_until` / `live_skip_reason`，跳过标记默认 12 小时后失效以便奖励范围或盘口变化后重新评估。报价大小把 `per_market_usd` 作为 YES + NO 两腿合计上限；live materializer 先把两腿 `rewards_min_size` 对齐到 CLOB 成本精度，再分配剩余额度，计划数量与 connector 实际提交数量一致。默认 `quote_mode=double` 且 `selection_mode=observe`，仍生成既有双边计划；当配置为 `quote_mode=auto`、`selection_mode=enforce` 且启用 dominant single-side 时，planner 只基于 YES/NO 概率生成初步单边或双边模式，盘口指标和双边预算不足后的可负担单腿回退都在 live materializer 中完成。`observe` 模式只把推荐模式和 `book_metrics` 写入 quote plan，不改变实际挂单。

AI advisory 启用后只在 full tick 的 `prepare_live_cycle()` 之后参与 live gate，不参与 fast reconcile。`prepare_live_cycle()` 构建新计划时会记录 AI 过滤前的 deterministic eligible condition 集合，并继承上一版 quote plan 中未过期且 provider/request_format/model 匹配的 advisory；继承阶段只应用已有决策，不因缺少缓存提前 fail closed。live tick 随后只查询 `reward_market_advisories` 缓存并应用 gate：缺少未过期 advisory、模型为空、低置信度、`watch/avoid` 或 `quote_mode=none` 都会把原本 eligible 的计划置为不可挂；AI 和 info-risk gate 都完成后才统一保存 quote plan 快照，避免预过滤 eligible 状态被周期性 orderbook 注册任务读取；live tick 不等待外部 AI provider，因此不会因为全量 advisory 请求拖住下单主循环。worker 会用进程内 `AtomicBool` 保证同一进程最多一个后台 market provider refresh 在跑；后台 refresh 仍把 gate 前的 quote plan、DB reward market、账户/仓位/开放订单、orderbook top levels、最近 24 根 5m price-history candles 和 candle summary 放入 provider payload，但 AI `input_hash` 只由市场身份/问题、奖励参数、计划 quote mode、相关策略配置和 candle summary 组成的稳定 cache-key payload 计算；info-risk `input_hash` 只由搜索 query、市场身份/问题/事件、计划 quote mode 和风险策略配置计算。账户余额、开放订单、持仓、即时盘口档位、盘口时间戳、quote plan reason/score 和 `market_synced_at` 等每轮动态字段不会进入缓存键，避免 provider refresh 已保存记录却在后续 full tick `cache_hits=0`。候选 condition 按开放订单、持仓、最终 eligible 或 pre-AI deterministic eligible quote plan、候选市场顺序去重，并受 `POLYEDGE_REWARDS__INFO_RISK_MAX_MARKETS_PER_CYCLE` 每轮 condition cap 约束（默认 50，AI 与 info-risk 共用）；每个 condition 内先查询/请求 AI advisory 并把命中或新写入的 advisory 挂到本轮内存 plan，再查询/请求同一 condition 的 info-risk，然后才进入下一个 condition，避免出现全量信息风险跑完后再全量跑 AI advisory。provider 成功后只写入 `reward_market_advisories` / `reward_market_info_risks` 缓存，供后续 full tick 使用，不用旧 cycle 覆盖当前 quote plan snapshot。AI advisory 和 info-risk 共用进程内 `Semaphore(1)`，同一 worker/API 进程内任意时刻只允许一个 AI provider HTTP 请求在飞。OpenAI/Anthropic API key、base URL、模型、超时和最低置信度来自 worker 环境变量；OpenAI-compatible base URL 可以是根地址或 `/v1` 地址，connector 会统一请求 `/v1/...` 并兼容 Bearer / `api-key` 认证头。MiMo provider 使用 `openai_chat_completions`，`openai_responses` 会返回 provider 未实现错误。API 内嵌 worker 启动时会记录 rewards poll loop 是否启用、AI key 是否配置、模型名和 interval；每轮 full tick 会记录 markets/books/plans/pre_ai_eligible_plans/eligible/open_orders/positions 以及 AI/info-risk 配置；缓存 gate 会记录 pre_ai_eligible_plans/ai_existing_advisories/ai_request_candidates/ai_pending_plans/cache_hits/skipped_missing_market/applied，后台 provider refresh 会分别记录 AI 与 info-risk 的 candidates/cache_hits/requested/saved/failures/skipped_missing_market 汇总和逐个 requesting/saved 进度。provider HTTP 传输失败，或明确返回限流、认证失败、服务端不可用（如 HTTP 429/401/403/5xx、`system_cpu_overloaded`、`overloaded`）时，后台 refresh 会停止本轮剩余 provider 请求，避免继续压垮 provider 或在错误配置下扫完整候选池。connector 会把 provider 返回的 confidence 钳制到 `0..=1`。只有 provider 高置信度返回 `allow` 时市场才可继续挂单；`single_yes/single_no` 仍只在 `selection_mode=enforce` 且 `quote_mode=auto` 下把已 eligible 的双边计划收窄为单腿。AI advisory 也可在成交后选择 `exit_policy`，但同样要求 allow、enforce 和置信度达标。

信息风险扫描仍有独立异步任务入口，但当 `ai_advisory_enabled=true` 时，独立 `scan-reward-info-risks-once` / `poll-reward-info-risks` 不再连续请求全量 info-risk provider，而是记录跳过，交给 full tick 启动的 market provider refresh 按 condition 同步推进 AI advisory 与 info-risk。AI advisory 未启用时，独立 info-risk 任务保持原行为：读取当前 rewards 配置、candidate markets、quote plans、开放订单和持仓，按开放订单、持仓、eligible quote plan、候选市场的顺序构建结构化风险请求；市场详情从 active rewards catalog 补齐，因此已持仓或已挂单市场即使不再适合新增报价也会被评估。请求按 input hash 查询 `reward_market_info_risks`；缓存未命中时调用 provider，每轮最多处理 `POLYEDGE_REWARDS__INFO_RISK_MAX_MARKETS_PER_CYCLE` 个 condition（默认 50，0 表示本轮不发 provider 请求）；每个 provider 请求都会先获取与 AI advisory 共用的单飞 permit。OpenAI/Anthropic API key、base URL、模型和超时复用 AI provider 环境变量；OpenAI-compatible 路径同样会规范化 root base URL 到 `/v1`，OpenAI Responses 可通过 `POLYEDGE_REWARDS__INFO_RISK_WEB_SEARCH_ENABLED=true` 启用 provider-native web search。info-risk task 启动会记录 interval、首轮延迟和 web search 开关；API 内嵌 runtime 中 info-risk 首轮会延迟一个 info-risk interval，避免启动时抢占 provider 通道；每轮扫描会记录开始、逐个 provider 请求进度、跳过原因、candidates/selected_conditions/cache_hits/requested/saved/failures/skipped_missing_market/applied_plans 汇总；provider 失败只记录 warning，若 provider HTTP 传输失败或明确返回限流、认证失败、服务端不可用（如 HTTP 429/401/403/5xx、`system_cpu_overloaded`），本轮会停止剩余外部请求并保留已有缓存。live full tick 只读取最新未过期缓存，并在 `info_risk_enabled=true` 时把结果附加到 quote plan；connector 会把 provider 返回的 confidence 钳制到 `0..=1`。当 `info_risk_mode=enforce` 时，缺少未过期风险缓存会 fail closed；已有风险置信度达到 `POLYEDGE_REWARDS__INFO_RISK_MIN_CONFIDENCE_BPS` 且达到过滤等级、临近结算或官方结果风险时也会把计划置为不可挂，既有 buy 会沿用“计划不可挂即撤单”的 live 风控路径。

live 模式会用 `LivePolymarketConnector::submit_token_order()` 提交 post-only GTC token 买单，用 `cancel_order()` 撤销本系统托管订单；未成交 post-only maker 买单不在本地按全局 notional 硬锁资金。不同 condition 的本系统未成交订单可复用资金，但同一 condition 会累计已有 managed BUY 剩余 notional 与待补 YES/NO 腿；账户开放 BUY 总额会同步到 `external_buy_notional`，其中无法归属到本系统 managed order 的外部 BUY notional 会先从 `available_usd` 中保守扣除，再做同 condition 准入，符合 CLOB 同市场余额有效性规则并降低人工/其它机器人挂单叠加风险。所有新报价和 post-fill exit/flatten 会先持久化本地 intent，再记录 submission attempt 后调用 CLOB；瞬时明确拒单会持久化回 Planned 并停止本轮后续买单，响应丢失则锁住本地订单并只通过严格开放订单匹配恢复 external order id。新建/恢复订单先保持 `scoring=false`，仅权威 scoring 查询可以置 true；`min_depth_usd` 会扣除自身剩余挂单，只统计外部 bid 深度。live placement materialize quote plan 时要求目标报价腿都有非空新鲜盘口，默认 `stale_book_ms=45000`，配置归一化会把低于 5000ms 的值抬到 5000ms；新建挂单路径遇到盘口缺失、空盘口或超过 `stale_book_ms` 时等待 orderbook 订阅/缓存恢复，不写长期 skip；新建 quote intent 与已落库待提交 BUY 在提交前都会复用 live 撤单风控（计划仍 eligible、报价漂移、min depth、bid rank、depth drop、fill velocity、mass cancel、kill switch 等），风险不通过的本地 intent 会在提交前取消；live full tick 和 fast reconcile 都会读取开放订单/持仓活跃 token 的盘口，缺盘口、空盘口、过期盘口、外部 bid 深度不足、bid rank 过高、盘口历史窗口风险、目标档位价格移动超过 `requote_drift_cents`、定期 requote 或全局 kill switch 会触发买单撤单，即使 `enabled=false` 已停止新增报价。每笔外部下单、撤单、已确认成交和状态变化会立即落库；撤单/成交同步会跳过本地 synthetic ID。外部单订单查询返回 404 时，worker 会按 token 和下单时间窗口分页查询认证账户 trades，并按 external order id 精确匹配 confirmed fill；仍无法确认时持久化 critical 对账锁并暂停新买单，后续成功查询会自动解除；若该 404 锁超过 5 分钟且仍无 CLOB/Data API 成交证据，worker 会把本地订单标记为 `cancelled` 以释放开放挂单计数。提交结果未知和取消结果未知不会仅因本地超时而 force-cancel；旧 `auto_cancel_stale_minutes` 配置已忽略。撤单接受后本地订单保留为待最终对账，下一轮先同步成交再确认取消，避免 cancel/fill 竞态丢成交；post-only violation 的 cancel rejected/unknown 会按最小 15 秒间隔重试，cancel accepted 但超过 30 秒仍未完成最终对账时会再次尝试撤单。worker 每轮还会读取 CLOB 账户开放订单 snapshot：未归属但 token 可唯一映射到 active reward market 的开放 BUY 会被收养为 managed order，已有同 external id 的非 open 本地 BUY 会在 CLOB 仍 open 时重开；已提交、open-like、普通 BUY managed order 若不在 snapshot 中且不处于提交未知、404、pending cancel、post-only violation 或其他对账锁状态，会本地标记为 `cancelled`，释放开放挂单计数；sell exit 仍走单订单/成交对账和 retry 逻辑。worker 仅在 trade 达到 `CONFIRMED` 后按 external trade id + external order id 幂等写入 fills、现金、库存和 PnL；买入 fill 与对应 exit intent 同事务落库，之后只撤同 condition 对侧仍开放的 buy sibling。`ExitAtMarkup` 卖价向上取整到 0.01 tick；明确退出拒单使用有界退避并在达到最大拒绝次数后停止自动重试，提交前低于 1 美元最小名义金额的退出单会进入短 reason 的 dust deferred 状态，每 300 秒重新评估但不重复拼接历史原因；FAK flatten 重试刷新当前 bid 价格时不会重置退避计数，同 token 有未完成 sell exit 时暂停新增 buy。`FlattenImmediately` 使用 FAK，缺 bid、退出单被拒绝，或终态部分成交且仍有持仓时，会持久化 deferred sell 并重试。每轮先同步 managed order 的 confirmed fills；CLOB open-order snapshot 和账户开放 buy notional 观测不受 confirmed fill 保护期影响。本轮新增 fill，或最新 confirmed fill 距今不足 120 秒时，只跳过外部 balance/positions 替换，防止最终一致性延迟回滚本地现金和库存。保护期结束后，成功 positions 快照原子替换该 rewards 账户全部持仓，失败时保留上一版。该同步在 `enabled=false` 且没有开放订单时也会尝试运行。SELL、非 rewards 市场和无法唯一映射 token 的外部开放订单明细，以及奖励结算对账仍是缺口，worker 仍需要独立维护组合风险。

poll loop 每轮读取持久化 rewards 配置；读取失败时不会使用默认配置冒险执行，也不会永久退出任务，而是等待 1 秒后重试。控制命令 wake、配置 revision 变化、活跃 rewards token 的 orderbook stream 更新和周期 timer 都会唤醒 loop；盘口事件触发的 fast reconcile 会按 1 秒最小间隔合并，避免高频盘口把 worker 打成紧循环。fast reconcile 每轮仍会用活跃盘口做风险撤单和退出 intent 处理，但外部重型同步独立节流：托管订单状态最小 5 秒间隔，CLOB open-order snapshot 最小 15 秒间隔，managed scoring 按 `min_scoring_check_sec` 且归一化下限 15 秒，账户级 rewards earnings 与 balance/positions snapshot 最小 60 秒间隔；full tick 和 `run_once` 完整同步后会刷新这些节流时间戳，避免紧随其后的盘口事件重复打外部 API。full tick 仍由 `POLYEDGE_REWARDS__POLL_INTERVAL_SECS` 作为全量候选发现和计划重建兜底，fast reconcile 仍由 `reconcile_interval_sec` 作为兜底 sweep；内部 WS 空闲重连只恢复事件源，不能替代这两个周期兜底。数据库 worker heartbeat 写入失败只记录告警；CLOB 订单 heartbeat 独立每 5 秒发送并串联 server 返回的 heartbeat id，单次请求 4 秒超时，失败或超时后清空 id 并按 5-60 秒退避重建链；首个失败和连续失败每 6 次记录 warn，其余连续失败降为 debug，恢复时记录 info。生产环境必须保持 poll loop 运行；一次性命令或有限循环退出后不再维护订单 heartbeat。

旧的未提交 quote intent 会先经过当前计划、盘口、kill switch 和撤单风险检查，再允许提交。任一提交结果未知、待最终对账或外部订单 404 会暂停全部新增买单，但不会阻断订单同步、风险撤单或卖出退出；外部订单 404 锁超过 5 分钟且仍无成交证据时会自动本地关闭；同一批次第一笔 POST 结果未知后也不会继续发送后续买单。CLOB 已明确返回的 HTTP 4xx 拒单会将当前 intent 标记为 error，不会误进入提交未知锁。managed order 会持久化实际提交价格/数量，post-only exit 被取消后的 replacement 仍保持 post-only。

关联 trade 按 ID 单独查询失败时，connector 会按该订单 token 和下单时间窗口扫描认证账户 trades，并按 external order id 精确匹配；只有所有预期关联 trade 都达到终态后才关闭订单。若认证 CLOB 已明确给出 matched size，但认证 trade 明细与历史页仍无法解码，worker 会再读取 Data API 钱包活动，并且只在 token、BUY、价格、时间窗口、累计数量与唯一 managed order 全部严格匹配时生成补账 fill。单订单已返回 404 时，无论认证账户 trade 扫描报错，还是扫描成功但没有返回该 external order id 的成交，都会继续执行 Data API 回退；此时还必须要求累计数量恰好等于本地订单剩余量，并且完整外部持仓快照已覆盖该数量。外部账户/持仓快照时间已覆盖该成交时，补账不会再次扣减现金或增加库存，但仍会关闭本地订单并创建退出 intent。任一订单的全部回退都失败时，worker 只跳过该订单并继续处理其余订单、账户快照和 stale 清理；如果同一外部订单 404 锁已持续超过 5 分钟，worker 会本地关闭该订单，不再中止整轮 reconcile。

### orderbook_stream — 盘口流（已迁移到 orderbook 服务）

盘口流逻辑已迁移到 `polyedge-orderbook` 服务（`packages/orderbook/src/stream.rs`）。Worker 中 `worker/orderbook_stream.rs` 仅保留 `consume-orderbook-stream` CLI 子命令兼容（daemon 模式不再调度），兼容路径同样消费完整 `book` 快照和 `price_change` 增量，并周期性全量 poll 当前 token。Worker 通过 `OrderbookHttpClient`（HTTP）读取 orderbook 服务的缓存数据，通过携带 `POLYEDGE_ORDERBOOK__WRITE_TOKEN` 的 `register-orderbook-tokens` 任务注册订阅 token；rewards poll loop 还会通过 `OrderbookStreamClient` 订阅 orderbook 服务内部 `/orderbook/stream`，将 WS/poll/ingest 更新写入 worker 本地 cache，并只用活跃 rewards token 的更新唤醒 fast reconcile。Standalone orderbook 服务遵守 `POLYEDGE_ORDERBOOK_STREAM__RESTART_INTERVAL_SECS`，HTTP `/orderbook/register` 原子替换对应 source 的 token 集合，缓存每侧盘口深度受 `POLYEDGE_ORDERBOOK_STREAM__MAX_LEVELS_PER_SIDE` 限制。

### arbitrage — 套利扫描

```
market_event_service.list_markets(status=Open)
    → (回退: fetch_gamma_markets if DB 为空)
    → 对每个市场获取盘口
    → detect_arbitrage_opportunities()
    → arbitrage_service.record_*()
```

Report: `ArbitrageScanRunReport { markets_scanned, snapshots/opportunities/validations recorded, expired, pruned }`

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
- **配置来源**：`infrastructure::Settings` 中的 worker、rewards、copytrade、arbitrage、news 配置段；盘口数据通过 `POLYEDGE_ORDERBOOK__SERVICE_URL` 连接 orderbook 服务

## 当前状态

- 常用维护/调试子命令已实现，`polyedge-worker` 仍作为 CLI 兼容入口保留
- `run` 主循环包含 register-orderbook-tokens、rewards、copytrade、arbitrage、news、execution、signal-recompute 等任务
- news worker 当前只抓取 RSS/Atom XML feed；未配置 `POLYEDGE_NEWS__SOURCES_JSON` 时会读取内置默认源列表，部署模板显式写入默认源并默认设置 `POLYEDGE_NEWS__ENABLED=true`、`POLYEDGE_WORKER__POLL_NEWS=true`
- rewards worker 会通过数据库命令队列接收前端 Run / Cancel / Reset 请求，API 进程不再执行 rewards 策略；仅支持 live 实盘模式，策略配置不依赖全局 system mode，但新买单和现有买单撤单遵守全局 kill switch
- copytrade worker 会通过数据库命令队列接收前端兼容控制命令；当前前端只暴露 Analyze，Run/Cancel/Reset 不再作为产品入口。API 进程不抓取 copytrade 输入，worker 负责 Data API 抓取、source trades 检测和钱包分析
- register-orderbook-tokens 每个 source 独立注册全量 token，由 orderbook registry 聚合层按固定优先级 `rewards_active`、`exec_orders`、`rewards_eligible`、`rewards_candidates` 跨 source 去重并 `take(POLYEDGE_ORDERBOOK_STREAM__MAX_TOKENS)` 截断总量；`rewards_eligible` 由周期任务统一注册全部最终 eligible quote plan token，并包含 AI/info-risk gate 前已 deterministic eligible 且保存在 `orderbook_token_ids` 的 token（不再由 rewards full tick 单独注册，也不因 active 持仓覆盖而被清空，还避免 AI advisory pending 与缺盘口互相等待），候选 token 优先来自 open/tradable 且 `volume_24h` 高的市场并受 `POLYEDGE_ORDERBOOK_STREAM__REWARD_CANDIDATE_TOKEN_CAP` 限制，空集合会清除对应旧 source
- rewards poll loop 在 Postgres 路径全程持有 advisory lease，统一覆盖 CLOB heartbeat、命令、orderbook 内部 WS 本地 cache、full tick 和 fast reconcile；本地盘口 cache 按本地接收 TTL 过期，避免上游未来时间戳延长缓存寿命；控制命令具备 5 分钟 running lease
- rewards orderbook 内部 WS client 建连最多等待 5 秒；已连接后若约 3 个 orderbook poll reconcile 周期无事件，会主动重连并重新 HTTP bootstrap 本地盘口 cache
- scheduled full tick 不再二次消费控制命令；拿不到 advisory lease 时保留到期状态并在后续轮询重试，不会把 command-only 周期记作已完成 full tick
- rewards poll loop 按账户写入 `reward_worker_heartbeats`；snapshot 的 `status.running` 仅在配置启用且最近 2 分钟存在 heartbeat 时为 true
- 低竞争市场 sleeve v2 已实现并默认关闭；启用后 worker 会构建独立低竞争候选 profile、计算低竞争指标、在 observe 模式只展示指标，在 enforce 模式通过独立小额度 cap 和 AI/info-risk fail-closed gate 后才允许低竞争 bucket 下单；full tick 会保存跨周期 observation，snapshot 提供 shadow report 和小额 enforce 建议，但不会自动启用。
- rewards full tick 已读取 Gamma `markets.category` 作为候选评分输入；命中 `preferred_categories` 时只增加候选排序分，不绕过市场质量、盘口和风控硬过滤。AI advisory 已接入 full tick：live tick 只读已有 cache 并 fail closed，后台 market provider refresh 按 condition 异步填充 `reward_market_advisories` 和 `reward_market_info_risks`，同一 condition 会先完成 AI advisory 再完成 info-risk 后才进入下一个 condition；AI 请求候选限定为 deterministic planner 原本 eligible 且仍缺少有效 advisory 的 condition，payload 包含 price-history 5m candles 和摘要，AI 开启后只有高置信度 `allow` 决策会放行新增挂单，provider 未配置、失败、缺缓存或低置信度都会让原本 eligible 的计划变为不可挂，且不会放宽任何硬过滤；provider confidence 会被钳制到 `0..=1`。信息风险已接入缓存过滤：AI 开启时由 market provider refresh 同步推进，AI 未开启时仍可由独立 info-risk worker 扫描；live tick 只读缓存并在 enforce 模式下过滤高风险/临近结算/官方结果盘口；新增买单会保守扣除未归属到本系统 managed order 的外部 BUY notional。
- rewards full tick 和 fast reconcile 在 managed order 同步后总会读取 CLOB open-order snapshot，收养/重开可映射到 active reward market 的开放 BUY，关闭缺失的普通 managed BUY，并刷新账户开放 buy notional；资金钱包地址优先使用 `POLYEDGE_POLYMARKET__FUNDER`，未配置时使用 `ACCOUNT_ID`；CLOB balance 为 0 或失败但链上 pUSD 余额大于 0 时，账户 snapshot 用 Polygon pUSD 余额回填；新确认成交所在周期及其后 120 秒只延后外部 balance/positions 替换，避免 CLOB/Data API 最终一致性回滚本地账本
- 默认大部分 worker 通过配置开关控制启用/禁用
- Polymarket live 任务需要真实凭证；Deposit Wallet 使用 `POLYEDGE_POLYMARKET__SIGNATURE_TYPE=poly_1271` + `POLYEDGE_POLYMARKET__FUNDER=<deposit_wallet>`，worker 会通过 connector 走 CLOB V2 `POLY_1271` 下单/撤单路径。
- Rewards 生产与测试入口均已移除 `RewardSimulationOutcome` / `simulated_orders` 旧命名，统一使用 `RewardTickOutcome` / `placed_orders`。

## 修改检查清单

- [ ] 新增 worker 任务时：(1) 在 `worker/` 中创建文件 (2) 在 `main.rs` 中添加 CLI 子命令 (3) 在 `run_worker_service()` 中添加到主循环
- [ ] 修改 worker 逻辑后检查对应的 application service 是否需要更新
- [ ] 新增 Report 类型时确保使用 `Default` derive 并包含有用的统计字段
- [ ] 运行 `cargo check --workspace --tests`
- [ ] 更新根目录 `AGENTS.md` 中的常用 worker 子命令列表
