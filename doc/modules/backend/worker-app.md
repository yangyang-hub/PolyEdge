# Worker App（后台任务服务）

最后更新：2026-06-24

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
| `worker/database_maintenance.rs` | 数据库维护任务：定期调用 `DatabaseMaintenanceService` 并记录逐表清理统计 |
| `worker/orderbook_registration.rs` | Worker orderbook token 注册：周期注册 rewards/执行订单 token，并在 rewards 新买单持久化后即时刷新 `rewards_active` source |
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
| `worker/rewards/account_sync.rs` | rewards 外部余额、CLOB open-order 反查/BUY 收养重开、完整持仓、检测库存原价卖出 intent、订单 scoring 与 UTC 当日账户级 rewards 聚合同步 |
| `worker/rewards/live_sync.rs` | rewards live 托管订单成交/状态同步、单订单失败隔离、Reset cancel-all 语义 |
| `worker/rewards/live_orders.rs` | rewards live 撤单、同一 external order in-flight 去重、成交入账、post-fill exit/flatten intent |
| `worker/rewards/live_submission.rs` | rewards live 单笔提交、post-only 接受状态处理和 submission marker |
| `worker/rewards/live_pending.rs` | rewards live 持久化 intent 提交、SELL 退出持仓裁剪/无仓位关闭、开放订单匹配恢复和未知结果锁定 |
| `worker/rewards/live_helpers.rs` | rewards live 价格 tick、fill id、退出重试与订单状态转换辅助函数 |
| `worker/rewards/live_orderbook_risk.rs` | rewards live orderbook 可用性/新鲜度 helper：新增挂单 stale 余量、近期 BUY stale-only 撤单 grace、等待原因 |
| `worker/rewards/live_requote.rs` | rewards live 换价 guard：报价漂移识别、历史盘口稳定确认、冷却和单轮限速 |
| `worker/rewards/live_placement_limits.rs` | rewards live placement 资金预算、provider 前 funding precheck、同 condition BUY 缺口名义金额和低竞争开放订单占比 helper |
| `worker/rewards/live_risk.rs` | rewards live placement/cancel 风控：depth/rank/history/requote、库存 cap、订单创建准入与撤单候选 |
| `worker/rewards/orderbook_events.rs` | rewards worker 本地盘口 cache、orderbook 内部 WS 消费、HTTP bootstrap、活跃 token 更新 channel 和 condition 盘口首次就绪检测（驱动撤单快路径与 AI advisory 批量 worker） |
| `worker/rewards/event_cancel.rs` | rewards orderbook 事件驱动 hard-risk 撤单快路径：独立 task 消费活跃 token 更新，只做 cancel-only 风控，不跑重型同步/重挂/换价 |
| `worker/rewards/low_competition_probe.rs` | rewards 低竞争 gate 前盘口探测：用 `rewards_low_competition_probe` source 按最多 10 个市场一批预热盘口和历史样本 |
| `worker/rewards/polling.rs` | rewards poll loop、盘口读取、独立事件撤单 worker 接线、fast reconcile、外部同步节流、5 天历史清理、进程内盘口历史和独立后台盘口预热 task（`run_reward_managed_orderbook_cache_prewarm`，每 5 秒刷新活跃/eligible/候选 token 本地 cache，不阻塞 fast reconcile） |
| `worker/rewards/provider_advisory.rs` | rewards AI advisory cache gate、候选排序 helper、provider connector/permit helper |
| `worker/rewards/provider_refresh_batch.rs` | rewards 主 provider refresh 的批量请求 helper：按配置批量请求 AI advisory / info-risk，逐项保存，漏项或错配回退单市场 |
| `worker/rewards/provider_refresh_orderbook.rs` | rewards 主 provider refresh 的临时 orderbook source helper：按临时批次订阅 AI 所需盘口，非 avoid advisory 后合并提升 `rewards_eligible` |
| `worker/rewards/provider_refresh.rs` | rewards AI advisory / 信息风险 provider refresh：full tick 后分别启动 AI advisory 与 info-risk 后台 task，各自单实例、各自候选 cap；AI 盘口使用 `rewards_ai_provider` 临时 source 每批最多 10 个市场，info-risk 不再占用 AI 临时盘口批次 |
| `worker/rewards/provider_batch.rs` | rewards AI advisory orderbook 事件驱动批量 worker：盘口首次就绪入队、攒批 `advise_batch`、缺失回退单请求、info-risk 同步推进（默认关闭，与 provider refresh 并存） |
| `worker/rewards/info_risk.rs` | rewards 信息风险异步扫描、provider 缓存命中、每轮扫描 cap、quote plan 风险应用 |
| `tests/rewards.rs` / `tests/rewards_orderbook_risk.rs` / `tests/rewards_reconciliation.rs` | rewards live 下单、orderbook stale 防抖风控、成交、对账、退出重试与增量持久化回归测试 |
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
| `run-database-maintenance-once` | `run_database_maintenance_once` | 一次性执行数据库历史/缓存/队列表 retention 清理 |
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

### database-maintenance — 数据库维护

```
run_database_maintenance_once()
    → DatabaseMaintenanceService.prune_history(now)
    → PostgresDatabaseMaintenanceStore 按 retention cutoff 分批 DELETE
    → 日志输出逐表 deleted 计数和 total_deleted
```

内嵌 worker runtime 默认通过 `POLYEDGE_WORKER__DATABASE_MAINTENANCE=true` 每 3600 秒执行一次；本地 `.env.example` 默认关闭，避免开发进程意外清历史。当前覆盖 raw events、AI/info-risk 过期缓存、reward price-history candles、rewards/copytrade 控制命令、copytrade events/source trades、outbox/external dedup、LLM calls、audit logs 和 mode transitions。每个表每轮最多 20 批、每批 10,000 行；清理失败只记录 warn，不影响其他 worker 循环。

### market_sync — 市场同步（已迁移到 orderbook 服务）

市场同步逻辑已迁移到 `polyedge-orderbook` 服务（`packages/orderbook/src/market_sync.rs`）。Orderbook 服务启动时先暴露 HTTP `/healthz`，再由后台任务执行 initial/periodic market sync，避免外部市场 API 延迟阻塞容器健康检查。Worker 中保留 `sync_markets_once` 函数供 CLI 子命令 `sync-markets-once` 使用，但 daemon 模式不再调度此任务。

### register-orderbook-tokens — 盘口 token 注册

```
register_orderbook_tokens()
    → 遍历活跃执行订单（Submitted/Open/PartiallyFilled）→ 解析市场 YES/NO asset_id
    → reward_bot_service.list_active_reward_book_token_ids() → rewards 活跃订单/持仓 token
    → reward_bot_service.list_eligible_reward_book_token_ids() → 当前最终可挂单 eligible quote plan token
    → rewards full tick 通过 rewards_low_competition_probe 临时注册低竞争 gate 前候选 token（最多 10 个市场/批）
    → reward_bot_service.list_all_reward_candidate_token_ids() → rewards 候选 token 填充剩余额度
    → orderbook_registry.register_tokens("rewards_active", ...)
    → orderbook_registry.register_tokens("exec_orders", ...)
    → orderbook_registry.register_tokens("rewards_eligible", ...)
    → orderbook_registry.register_tokens("rewards_candidates", ...)
    // 通过 OrderbookHttpClient → HTTP POST /orderbook/register 注册到 orderbook 服务
```

此任务替代了原来的 `consume-orderbook-stream` 和 `sync-markets` 任务。Worker 不再直接运行盘口流或市场同步，而是通过 HTTP 告知 orderbook 服务需要订阅哪些 token。
注册任务最长每 60 秒执行一次，orderbook 服务重启后可自动恢复订阅；rewards live tick 在新买单 intent 持久化并并入 open_orders 后，还会立即重新注册 `rewards_active` source，避免刚落库的实盘订单等待下一个周期注册才被 orderbook 订阅覆盖。每个 source（rewards 活跃订单/持仓 token、活跃 execution token、当前最终 eligible quote plan token、低竞争 gate 前探测 token、其余 rewards 候选 token）独立收集并各自去重、截断后注册；跨 source 去重和总量上限由 orderbook registry 聚合层负责（按固定优先级 `rewards_active > exec_orders > rewards_eligible > rewards_ai_provider > rewards_low_competition_probe > rewards_candidates` 合并去重后 `take(POLYEDGE_ORDERBOOK_STREAM__MAX_TOKENS)`）。`rewards_eligible` source 只覆盖最终可挂单市场；AI advisory/info-risk gate 前的 deterministic eligible token 不再长期注册到该 source，而是由后台 provider refresh 临时注册 `rewards_ai_provider` source，每批最多 10 个市场，下一批原子替换上一批，完成后清空；低竞争 gate 前候选由 rewards full tick 维护 `rewards_low_competition_probe` source，每批最多 10 个市场，批次在盘口历史达到低竞争样本要求、候选失效或 5 分钟超时后轮转。provider refresh 的候选顺序保留开放订单/持仓最高优先级，剩余普通候选与低竞争通过 gate 的 pre-AI 市场按约 2:1 混排。provider 返回非 `avoid` advisory 后会把该市场 token 即时合并到 `rewards_eligible` source，后续 full tick 再用持久 quote plan 校正；真正下单仍要经过 live 盘口、资金和订单风控。候选来源只用于给尚未产生 quote plan 的市场提前预热盘口，受 `POLYEDGE_ORDERBOOK_STREAM__REWARD_CANDIDATE_TOKEN_CAP` 限制，默认只预热 50 个候选 token，设为 0 会清空 `rewards_candidates` source 但不影响 active/final-eligible token、AI provider 临时订阅或低竞争 probe 批次。每个成功查询的 source 使用一次原子替换注册；周期注册任务对空集合做防抖，`rewards_active`/`exec_orders` 连续 2 轮成功为空才清远端 source，`rewards_eligible`/`rewards_candidates` 连续 3 轮成功为空才清远端 source，即时 `rewards_active` 刷新若读到空集合会保留上一版等待周期任务确认；任一 source 的数据库查询失败时保留远端上一版集合，不会用空集合误删订阅。

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
        → 低竞争 probe 先用 `rewards_low_competition_probe` source 按最多 10 个市场/批预热 gate 前候选盘口
        → 低竞争 sleeve 计算竞争资金、预估 reward share、退出深度和盘口稳定性指标
        → provider refresh 前执行 live funding precheck：无开放订单/持仓的新 condition 若当前可用资金放不下最低 rewards size 待补腿，先标记 funding reason，不进入 AI/info-risk 普通候选队列
        → 只应用已缓存 AI advisory，并在后台分别单实例刷新缺失 AI advisory 与信息风险缓存（两条队列各自开放订单/持仓优先，普通候选与低竞争通过 gate 的 pre-AI 市场按约 2:1 混排；AI advisory 使用 `rewards_ai_provider` 临时 orderbook source，每批最多 10 个市场，切换下一批前取消上一批；AI advisory / info-risk provider 请求分别受 rewards 配置 `ai_advisory_batch_size` / `info_risk_batch_size` 控制，1 为逐市场，>1 批量请求并对漏项/错配回退单市场；仅在该市场所有报价 token 盘口都已发布后才请求 AI 并写缓存，请求 payload 会读取最近 24 根 5m price-history candles；缺盘口的市场本轮跳过请求、不写缓存，等盘口到达后再评估，避免缓存空 watch/avoid 长期卡住市场）
        → 读取已缓存的信息风险并按配置标记/过滤 quote plan
        → info-risk enforce 开启时应用首单 gate：新 condition 首次 BUY 需先有信息风险缓存并满足观察窗口，已有订单/持仓 condition 跳过
        → 写入低竞争 observation，供 snapshot shadow report 汇总
        → sync managed rewards order trades/statuses
        → 批量同步 managed order scoring 状态与 CLOB open-order snapshot（收养/重开 active rewards BUY，关闭缺失 managed BUY）
        → 同步 UTC 当日账户级 maker rewards（`/rewards/user/total?sponsored=true` 聚合优先，对齐官网 native+sponsored 口径；明细 fallback 合并 native 与 sponsored-only）
        → 无近期 confirmed fill 时同步外部 balance + 链上 pUSD 余额回退 + 完整 positions 快照
        → LivePolymarketConnector.submit_token_order()
        → orderbook stream active-token event：独立 cancel worker 只读更新 token 的活跃盘口，直接执行 hard-risk cancel-only 检查
        → orderbook stream wake 或 reconcile_interval_sec 兜底：普通 fast reconcile 读取活跃盘口并做成交同步、退出提交、完整撤单检查和节流外部同步
```

Report: `RewardBotRunReport { markets_scanned, books_fetched, plans_built, eligible_plans, placed_orders, cancelled_orders, filled_orders, risk_cancelled_orders, reward_accrued }`

约束：后台 runtime 是 rewards 策略和控制命令的唯一执行者。rewards poll loop 在整个生命周期持有 Postgres advisory lease，多实例中只有 lease owner 会认证 CLOB、执行命令/full tick/fast reconcile 并维护 5 秒 heartbeat id 链；standby 实例不发心跳也不执行。API handler 只写控制命令；同一账户同一动作已有 `pending/running` 命令时会合并重复入队，避免重复点击制造多轮 full tick 或 cancel-all；共享 `RewardBotService` 会在真正入队时立即唤醒同进程 loop，配置 revision 变化还会立即触发 full cycle。`POLYEDGE_WORKER__POLL_REWARD_BOT=true` 控制 API 内嵌 runtime 是否启动 rewards loop。

自动 tick 只从 Postgres 的 `reward_markets` 读取奖励市场。长期 `poll-reward-bot` 启动后会通过 `OrderbookStreamClient` 连接 `polyedge-orderbook` 的内部 `/orderbook/stream`，维护 worker 进程内本地盘口 cache；本地 cache 使用 `POLYEDGE_ORDERBOOK_STREAM__MAX_LEVELS_PER_SIDE` 和 `POLYEDGE_ORDERBOOK_STREAM__BOOK_TTL_MS` 限制深度与过期读取。启动和重连时通过 `OrderbookHttpClient` / `POST /orderbook/batch` bootstrap 当前 rewards 活跃、eligible、低竞争 probe 和候选 token，后续缺失 token 也会按需 HTTP 补齐；周期注册任务会把全部最终 eligible quote plan token 注册到 orderbook `rewards_eligible` source，pre-AI deterministic eligible 市场由 provider refresh 的 `rewards_ai_provider` 临时 source 按批订阅准备 AI 评估，低竞争 gate 前候选由 full tick 的 `rewards_low_competition_probe` source 按最多 10 个市场一批预热。内部 WS 连接建立最多等待 5 秒，已连接后若约 3 个 poll reconcile 周期没有收到任何事件，worker 会主动重连并重新 HTTP bootstrap。Postgres 候选 market pool 关联 Gamma `markets`，硬过滤非 open/tradable、高歧义、低流动性、低 24h 成交量、临近结算、Gamma spread 过宽、市场同步过期、奖励不足以及 FDV/launch/token/official-result 等高事件跳变风险市场；不再按 `per_market_usd` 预筛双边最小份额预算，高最小份额市场会保留到 live materializer 和实际钱包余额准入层处理。full tick 会在 AI/info-risk provider refresh 之前先对无开放订单/持仓的新 condition 做 live funding precheck，当前余额无法补齐最低 rewards size 待补腿的计划会先标为不可挂并跳出普通 provider 候选；已有 open-like 订单或持仓的 condition 跳过该前置资金门槛，仍进入 provider 队列以继续风险管理。仅唯一且明确的 YES/NO token 会进入候选与订阅。通过门槛后按奖励、流动性、成交量、剩余时长和奖励 spread 综合排序。worker 使用本地 cache 读取候选和活跃 token 盘口，遇到本地缺失、过期或接近新挂单 freshness headroom 的 token 会提前回源 orderbook HTTP batch 并回填本地 cache；这些 batch 请求会携带 `refresh_if_stale_ms`，默认 `stale_book_ms=45000` 时 placement 最大盘口年龄约 35 秒、HTTP 预刷新阈值约 25 秒，orderbook 服务若自身缓存也超过约 25 秒会同步 `/books` 刷新后再返回；若本 tick 没有新鲜缓存盘口，不会提交新 post-only 订单。

full tick 会在开头读取候选盘口；低竞争指标完成后先执行 live funding precheck，再标记 `pre_ai_eligible` 并启动 AI/info-risk provider refresh，因此明显资金不足的新 condition 不会消耗模型 token。完成 AI/info-risk cache gate 后会立即用本轮内存 quote plan 原子替换 `rewards_eligible` orderbook source，让新放行的 token 不必等待周期注册任务；订单同步和账户同步完成后，进入撤单、待提交 intent 和新挂单前，会对当前 open-like 订单与 eligible quote plan token 再执行一次同样的本地 cache / orderbook HTTP batch 回填并合并到本轮 books；该 batch 会要求 orderbook 服务按 placement 预刷新阈值补齐自身 stale 缓存，而不是只返回可能已经超过 live 窗口的旧缓存。随后 worker 用当前 books materialize quote readiness 并保存 quote plan 快照，避免 tick 前半段 I/O 耗时让盘口落入 placement stale 窗口，也避免控制台读到尚未 live 验证的 eligible 中间态。

Worker 本地盘口 cache 的 TTL 按本地接收/写入时间计算，避免上游未来 `observed_at` 延长旧盘口寿命；上游 `observed_at` 仍保留给盘口内容版本和历史记录，planner/live materializer 与 live 风控判断盘口新鲜度都使用 `confirmed_at`。

低竞争 rewards sleeve 已实现 v2，默认关闭。worker 会从 `RewardBotService` 读取标准候选和低竞争候选 profile，低竞争 profile 只放宽自身流动性/24h 成交量门槛，仍共享市场安全硬过滤；full tick 在读取候选后先用进程内 `LowCompetitionProbeState` 选择最多 10 个低竞争候选市场注册到 `rewards_low_competition_probe` source，批次在两腿盘口历史达到 `low_competition_min_book_samples`、候选失效或 5 分钟超时后轮转；随后 `prepare_live_cycle()` 用 orderbook 服务提供的当前盘口和 worker 本地盘口历史计算 `planned_notional_usd`、`qualified_competition_usd`、`estimated_reward_per_100_usd_day`、退出深度、退出滑点、样本数和 midpoint 波动。`low_competition_mode=observe` 会把指标写入 quote plan 但强制低竞争 bucket 不可挂；`enforce` 要求低竞争指标达标、`ai_advisory_enabled=true` 且 `info_risk_enabled=true/info_risk_mode=enforce`，之后仍走现有 AI advisory / info-risk cache gate，缺缓存或 provider 拒绝会 fail closed；低竞争 gate 因缺 fresh midpoint、盘口指标或历史样本不足而失败时会保留 quote legs 和 `orderbook_token_ids`，reason 使用 `low-competition data unavailable`，避免后续误报缺 YES/NO token；低竞争通过 gate 的 pre-AI 市场在后台 AI advisory refresh 中与普通候选按约 2:1 混排进入每批 10 个市场的临时盘口订阅和 AI 分析，info-risk refresh 使用独立队列补缓存。AI/info-risk gate 完成后，worker 会把低竞争 observation 写入 `reward_low_competition_observations`，使用 gate 前固化的计划 notional，记录最终可挂状态、provider 拦截、样本不足、退出深度和滑点，API snapshot 再汇总最近 24 小时 shadow report；该 report 只给建议，不自动改配置。live placement 对低竞争 bucket 使用独立 `low_competition_max_markets`、`low_competition_max_open_orders` 和 `low_competition_max_position_usd`，并额外限制低竞争 open-like 订单最多约占全局 `max_open_orders` 的 30%（全局允许时至少 1 单），同时继续受全局订单/市场上限、kill switch、盘口风控、实际钱包余额和账户外部 BUY notional 约束。该实现仍只从数据库和 orderbook HTTP/内部 WS cache 读取数据，不直接调用 Polymarket Gamma/CLOB 外部 API。

报价计划构建阶段只应用市场质量、概率区间、配置和非盘口依赖过滤，不再因为 `quote_bid_rank` 缺档、目标价格超出 rewards spread、auto 单边所需的退出深度/top1/top3 买盘集中度/HHI、实际盘口价格预算、`per_market_usd` 或 `quote_size_usd` 而淘汰市场；quote plan 的腿可以只是 YES/NO token 占位元数据。live placement 准备创建订单时才用当前 orderbook materialize 真实腿：报价价格由 `quote_bid_rank=1|2|3` 选择 YES/NO 目标买盘价，粗 tick 盘口按买一/买二/买三（不同买价）选择，细 tick 盘口会从买一回退 `rank-1` 个 0.01 价格步长后选择不高于目标价的当前买盘档位，避免 0.001 tick 下买三只退两个细档；随后验证目标档位、rewards spread、touch ask、安全边际、auto/enforce 盘口指标和实际 size/notional。缺少、空、过期或已进入 placement freshness headroom 的盘口不创建新订单、不写 12 小时 skip，而是保持 quote plan eligible 并写入等待 orderbook 订阅数据的 reason；持久化 quote plan 时会刷新 `quote_readiness`，其中 `eligible=true` 但没有真实 price/size/notional 报价腿的计划标记为 `waiting_orderbook`，只有已有真实报价腿的计划标记为 `ready_to_quote` 并计入 API snapshot 的 `status.ready_quote_markets`。worker 会先对超过新挂单 freshness headroom 的本地盘口通过 orderbook HTTP batch 刷新，默认保留 10 秒 stale 余量（短 stale 窗口保留半窗），后续 full tick 拿到带足够新鲜度余量的盘口后会重新 materialize 并继续挂单流程。非 transient live 验证不通过时不下单，并把该 quote plan 标记 `live_skip_until` / `live_skip_reason`，跳过标记默认 12 小时后失效以便奖励范围或盘口变化后重新评估。开放订单目标价漂移超过 `requote_drift_cents` 不再单轮全量撤单；worker 会要求 `requote_drift_confirm_sec` 秒前的历史盘口同方向仍超阈值、订单年龄超过 `requote_drift_cooldown_sec`，并且每个 reconcile 周期最多撤 `requote_drift_max_cancels_per_cycle` 个 drift 候选。报价大小不再使用 `per_market_usd` 或 `quote_size_usd` 作为构造上限；live materializer 只把报价腿按 CLOB 成本精度向上对齐到 `rewards_min_size`，并满足 Polymarket 1 美元最小名义金额。实际能否新增同一 condition 的 YES/NO 腿由 `available_usd` 扣除未归属外部 BUY notional 后的余额决定；full tick 会在 provider refresh 之前先用同一预算逻辑拦截无 active exposure 且待补最低 rewards size 腿放不下的新 condition，live placement 阶段仍会在下单前复核资金并写入 funding reason，等待下一轮余额或开放订单同步后重新评估。默认 `quote_mode=double` 且 `selection_mode=observe`，仍生成既有双边计划；当配置为 `quote_mode=auto`、`selection_mode=enforce` 且启用 dominant single-side 时，planner 只基于 YES/NO 概率生成初步单边或双边模式，盘口指标和双边不可行后的单腿回退都在 live materializer 中完成。`observe` 模式只把推荐模式和 `book_metrics` 写入 quote plan，不改变实际挂单。

AI advisory 启用后只在 full tick 的 `prepare_live_cycle()` 之后参与 live gate，不参与 fast reconcile。`prepare_live_cycle()` 构建新计划时会记录 AI 过滤前的 deterministic eligible condition 集合，并继承上一版 quote plan 中未过期且 provider/request_format/model 匹配的 advisory；继承阶段只应用已有决策，不因缺少缓存提前 fail closed。live tick 随后只查询 `reward_market_advisories` 缓存并应用 gate：缺少未过期 advisory 或模型为空仍会把原本 eligible 的计划置为不可挂等待 provider；AI `avoid` 仍硬拦；AI `watch`、低置信度 `allow/watch` 或非 avoid 的 `quote_mode=none` 不再硬拦，而是保留 deterministic 报价腿继续进入 live 盘口、资金和订单风控；高置信度 `allow` 的 `single_yes/single_no` 仍可在 `selection_mode=enforce` 且 `quote_mode=auto` 下把双边计划收窄为单腿。AI 和 info-risk gate 都完成后还会等待 live action 盘口刷新与 readiness materialize，再统一保存 quote plan 快照，避免预过滤 eligible 状态或尚未 live 验证的中间态被周期性 orderbook 注册任务和控制台读取；live tick 不等待外部 AI provider，因此不会因为全量 advisory 请求拖住下单主循环。full tick 后台 provider refresh 拆成 AI advisory 与 info-risk 两个独立 Tokio task，分别用进程内 `AtomicBool` 保证各自单实例；后台 refresh 仍把 gate 前的 quote plan、DB reward market、账户/仓位/开放订单、orderbook top levels、最近 24 根 5m price-history candles 和 candle summary 放入 provider payload，但 funding precheck 已拦截的无 active exposure 新 condition 不进入普通 provider 候选队列，已有开放订单或持仓的 condition 仍保留最高优先级以继续覆盖风险管理。AI `input_hash` 只由市场身份/问题、奖励参数、计划 quote mode、相关策略配置和 candle summary 组成的稳定 cache-key payload 计算；info-risk `input_hash` 只由搜索 query、市场身份/问题/事件、计划 quote mode 和风险策略配置计算。账户余额、开放订单、持仓、即时盘口档位、盘口时间戳、quote plan reason/score 和 `market_synced_at` 等每轮动态字段不会进入缓存键，避免 provider refresh 已保存记录却在后续 full tick `cache_hits=0`。新写入的 AI advisory / info-risk 缓存会按稳定键增加最多 20% 且最多 15 分钟的正向 TTL jitter；provider refresh 和事件驱动批量通道看到缓存仍未过期但已进入同一刷新窗口时，会继续把旧缓存用于当前 cycle，同时发起后台续期，避免等到过期后集中出现 provider pending。AI 与 info-risk 各自保留开放订单、持仓最高优先级，剩余普通 eligible/candidate 市场与低竞争通过 gate 的 pre-AI quote plan 按约 2:1 混排，分别受 `POLYEDGE_REWARDS__INFO_RISK_MAX_MARKETS_PER_CYCLE` 每轮 condition cap 约束（默认 50）；AI refresh 对 AI 盘口按最多 10 个市场一批注册 `rewards_ai_provider` 临时 source，批次切换会取消上一批，结束后清空，并按 `ai_advisory_batch_size` 批量补 AI advisory、把命中或新写入的 advisory 挂到本轮内存 plan；info-risk refresh 不再等待 AI 临时盘口批次，独立按 `info_risk_batch_size` 批量补 info-risk，默认值 1 保持逐市场请求，漏项、错配或整体解析失败会回退单市场。provider 成功后只写入 `reward_market_advisories` / `reward_market_info_risks` 缓存，供后续 full tick 使用，不用旧 cycle 覆盖当前 quote plan snapshot；非 `avoid` advisory 会即时把该市场 token 合并注册到 `rewards_eligible` source，后续 full tick 再用持久 quote plan 校正。AI advisory 和 info-risk 分别使用独立进程内 `Semaphore(1)`，同一 worker/API 进程内各自单飞但可彼此并行。OpenAI/Anthropic API key、base URL、模型、超时和最低置信度来自 worker 环境变量；OpenAI-compatible base URL 可以是根地址或 `/v1` 地址，connector 会统一请求 `/v1/...` 并兼容 Bearer / `api-key` 认证头。MiMo provider 使用 `openai_chat_completions`，`openai_responses` 会返回 provider 未实现错误。API 内嵌 worker 启动时会记录 rewards poll loop 是否启用、AI key 是否配置、模型名和 interval；每轮 full tick 会记录 markets/books/plans/pre_ai_eligible_plans/eligible/open_orders/positions 以及 AI/info-risk 配置；缓存 gate 会记录 pre_ai_eligible_plans/ai_existing_advisories/ai_request_candidates/ai_pending_plans/cache_hits/skipped_missing_market/applied，后台 provider refresh 会分别记录 AI 与 info-risk 的 candidates/cache_hits/requested/saved/failures/skipped_missing_market 汇总和逐个 requesting/saved 进度。provider HTTP 传输失败，或明确返回限流、认证失败、服务端不可用（如 HTTP 429/401/403/5xx、`system_cpu_overloaded`、`overloaded`）时，对应 refresh 会停止本轮剩余 provider 请求，避免继续压垮 provider 或在错误配置下扫完整候选池。connector 会把 provider 返回的 confidence 钳制到 `0..=1`。AI advisory 不能绕过市场质量、盘口和风控硬过滤；成交后退出策略由 `post_fill_strategy` 配置决定，不再由 AI advisory 的 `exit_policy` 覆盖。

信息风险扫描仍有独立异步任务入口，但当 `ai_advisory_enabled=true` 时，独立 `scan-reward-info-risks-once` / `poll-reward-info-risks` 不再连续请求全量 info-risk provider，而是记录跳过，交给 full tick 启动的专用 info-risk provider refresh task。AI advisory 未启用时，独立 info-risk 任务保持原行为：读取当前 rewards 配置、candidate markets、quote plans、开放订单和持仓，按开放订单、持仓、eligible quote plan、候选市场的顺序构建结构化风险请求；市场详情从 active rewards catalog 补齐，因此已持仓或已挂单市场即使不再适合新增报价也会被评估。请求按 input hash 查询 `reward_market_info_risks`；缓存未命中或已进入提前刷新窗口时调用 provider，每轮最多处理 `POLYEDGE_REWARDS__INFO_RISK_MAX_MARKETS_PER_CYCLE` 个 condition（默认 50，0 表示本轮不发 provider 请求）；每个 provider 请求都会先获取 info-risk 专用单飞 permit。OpenAI/Anthropic API key、base URL、模型和超时复用 AI provider 环境变量；OpenAI-compatible 路径同样会规范化 root base URL 到 `/v1`，OpenAI Responses 可通过 `POLYEDGE_REWARDS__INFO_RISK_WEB_SEARCH_ENABLED=true` 启用 provider-native web search。info-risk task 启动会记录 interval、首轮延迟和 web search 开关；API 内嵌 runtime 中 info-risk 首轮会延迟一个 info-risk interval，避免启动时抢占 provider 通道；每轮扫描会记录开始、逐个 provider 请求进度、跳过原因、candidates/selected_conditions/cache_hits/requested/saved/failures/skipped_missing_market/applied_plans 汇总；provider 失败只记录 warning，若 provider HTTP 传输失败或明确返回限流、认证失败、服务端不可用（如 HTTP 429/401/403/5xx、`system_cpu_overloaded`），本轮会停止剩余外部请求并保留已有缓存。live full tick 只读取最新未过期缓存，并在 `info_risk_enabled=true` 时把结果附加到 quote plan；connector 会把 provider 返回的 confidence 钳制到 `0..=1`。当 `info_risk_mode=enforce` 时，缺少未过期风险缓存会 fail closed；已有风险置信度达到 `POLYEDGE_REWARDS__INFO_RISK_MIN_CONFIDENCE_BPS` 后，`critical`、官方结果、`resolution_imminent=true` 或配置为 `low/medium` 避免等级时命中的风险等级会把计划置为不可挂；普通 `high` 风险以及仅 `risk_type=imminent_resolution` 但 `resolution_imminent=false` 的结果只作为信息提示保留并继续走 live 盘口、资金和订单风控。既有 buy 会沿用“计划不可挂即撤单”的 live 风控路径。

live 模式会用 `LivePolymarketConnector::submit_token_order()` 提交 post-only GTC token 买单，用 `cancel_order()` 撤销本系统托管订单；未成交 post-only maker 买单不在本地按全局 notional 硬锁资金。不同 condition 的本系统未成交订单可复用资金，但同一 condition 会累计已有 managed BUY 剩余 notional 与待补 YES/NO 腿；账户开放 BUY 总额会同步到 `external_buy_notional`，其中无法归属到本系统 managed order 的外部 BUY notional 会先从 `available_usd` 中保守扣除，再做同 condition 准入，符合 CLOB 同市场余额有效性规则并降低人工/其它机器人挂单叠加风险。

所有新报价和 post-fill exit/flatten 会先持久化本地 intent，再记录 submission attempt 后调用 CLOB；瞬时明确拒单会持久化回 Planned 并停止本轮后续买单，响应丢失则锁住本地订单并只通过严格开放订单匹配恢复 external order id。新建/恢复订单先保持 `scoring=false`，仅权威 scoring 查询可以置 true；`min_depth_usd` 会扣除自身剩余挂单，只统计外部 bid 深度。live placement materialize quote plan 时要求目标报价腿都有非空新鲜盘口，默认 `stale_book_ms=45000`、placement 最大盘口年龄约 35 秒，配置归一化会把低于 5000ms 的值抬到 5000ms；盘口 freshness 使用 orderbook `confirmed_at` 而不是内容版本 `observed_at`，安静市场只要最近被 poll/WS 确认过就不会因内容未变化被误判 stale。

新建挂单路径遇到盘口缺失、空盘口、超过 `stale_book_ms` 或已进入 placement freshness headroom 时等待 orderbook 订阅/缓存恢复，不写长期 skip；新建 quote intent 与已落库待提交 BUY 在提交前都会复用 live 撤单风控（计划仍 eligible、报价漂移、min depth、bid rank、depth drop、fill velocity、mass cancel、best ask touch、kill switch 等），并在真正 POST 前用 orderbook 服务做 1 秒 max-age last-look，风险不通过的本地 intent 会在提交前取消，last-look 缺盘口或刷新失败则 fail closed 等下轮重试。live full tick、fast reconcile 和独立事件撤单 worker 都会读取开放订单/持仓活跃 token 的盘口。事件撤单 worker 只处理缺盘口、空盘口、SELL 盘口过期、BUY 非 stale 硬风险（计划不可挂、best ask touch、外部 bid 深度不足、bid rank 过高、盘口历史窗口风险、全局 kill switch 等），不做订单/账户同步、重挂、退出提交、报价漂移换价或定期 requote；full/fast 路径仍覆盖完整撤单检查。

即使 `enabled=false` 已停止新增报价，硬风险仍会触发撤单；价格漂移属于策略性换价，只在历史同向确认、订单冷却后按单轮上限进入撤单；近期已有 external order id 的 BUY 若仅因盘口刚超过 `stale_book_ms` 会短暂延迟 stale-only 撤单，给即时 `rewards_active` 注册、WS/poll reconcile 和 HTTP bootstrap 恢复盘口的时间，缺盘口/空盘口和其他硬风险仍立即撤单。每笔外部下单、撤单、已确认成交和状态变化会立即落库；所有 live 撤单请求共享进程内 in-flight 去重，同一 external order id 在一次 `cancel_order()` 未返回前不会被事件撤单 worker 和普通 reconcile 重复发送；撤单/成交同步会跳过本地 synthetic ID。

外部单订单查询返回 404 时，worker 会按 token 和下单时间窗口分页查询认证账户 trades，并按 external order id 精确匹配 confirmed fill；仍无法确认时持久化 critical 对账锁并暂停新买单，后续成功查询会自动解除；若该 404 锁超过 5 分钟且仍无 CLOB/Data API 成交证据，worker 会把本地订单标记为 `cancelled` 以释放开放挂单计数。提交结果未知订单现在会在恢复查询确认 CLOB 无对应 open 订单后经过 `LIVE_SUBMISSION_UNKNOWN_CLOSE_AFTER_SECS`（默认 600 秒）宽限自动本地关闭（与 404 锁一致），但如果后续 positions 快照显示该 BUY token 在提交后出现库存，则继续保留对账锁等待确认；取消结果未知订单仍不会仅因本地超时 force-cancel；旧 `auto_cancel_stale_minutes` 配置已忽略。撤单接受后本地订单保留为待最终对账，下一轮先同步成交再确认取消，避免 cancel/fill 竞态丢成交；post-only violation 的 cancel rejected/unknown 会按最小 15 秒间隔重试，cancel accepted 但超过 30 秒仍未完成最终对账时会再次尝试撤单。

worker 每轮还会读取 CLOB 账户开放订单 snapshot：未归属但 token 可唯一映射到 active reward market 的开放 BUY 会被收养为 managed order，已有同 external id 的非 open 本地 BUY 会在 CLOB 仍 open 时重开；已提交、open-like、普通 BUY managed order 若不在 snapshot 中且不处于提交未知、404、pending cancel、post-only violation 或其他对账锁状态，会本地标记为 `cancelled`，释放开放挂单计数；sell exit 仍走单订单/成交对账和 retry 逻辑。worker 仅在 trade 达到 `CONFIRMED` 后按 external trade id + external order id 幂等写入 fills、现金、库存和 PnL；买入 fill 与对应 exit intent 同事务落库，之后只撤同 condition 对侧仍开放的 buy sibling。

`ExitAtMarkup` 价格以被吃买单原价加 `exit_markup_cents` 为基准并向上取整到 0.01 tick，默认加价为 0；`HoldAndRequote` 按被吃买单原价持久化 post-only SELL intent，之后继续正常报价；外部 positions 快照检测到尚无 open-like SELL 的非零库存时，也会按该持仓 `avg_price` 向上对齐 tick 后创建原价 post-only SELL intent，避免已有库存无人接管退出；退出 floor 始终用 intent price 与当前持仓 `avg_price` 的较高值，不会用 midpoint 或低于 floor 的买一价覆盖；提交前会把 SELL size 裁剪到当前同 token 持仓，若没有对应持仓或剩余 size 为 0，会关闭本地 stale exit 并记录 warning，避免余额不足拒单反复刷屏；提交前低于 1 美元最小名义金额的退出单会进入短 reason 的 dust deferred 状态，每 300 秒重新评估但不重复拼接历史原因。同 token 有未完成 sell exit 时暂停新增 buy。

`ExitAtMarkup`、`HoldAndRequote` 和外部库存补退出按 post-only maker SELL 提交 floor；如果当前买一已大于等于 maker 价格，原价 post-only 会穿盘口无法 resting，worker 会保留 `exit_pending` 并按 30 秒退避等待可作为 maker 挂出。`FlattenImmediately` 会持久化非 post-only flatten intent，提交前读取当前 best bid；best bid 不低于 floor 时按 best bid 用 FAK/taker SELL 尝试非亏损平仓，best bid 缺失或低于 floor 时按 30 秒退避保留 deferred exit。post-only 明确退出拒单使用有界退避，达到最大拒绝次数后停止盲目重试。

每轮先同步 managed order 的 confirmed fills；CLOB open-order snapshot 和账户开放 buy notional 观测不受 confirmed fill 保护期影响。本轮新增 fill，或最新 confirmed fill 距今不足 120 秒时，只跳过外部 balance/positions 替换，防止最终一致性延迟回滚本地现金和库存。保护期结束后，成功 positions 快照原子替换该 rewards 账户全部持仓，并为没有退出单的库存补原价 SELL intent；失败时保留上一版。该同步在 `enabled=false` 且没有开放订单时也会尝试运行。SELL、非 rewards 市场和无法唯一映射 token 的外部开放订单明细，以及奖励结算对账仍是缺口，worker 仍需要独立维护组合风险。
每次成功读取 CLOB 账户开放订单 snapshot 后，worker 还会把仍出现在该 snapshot 中且外部剩余量为正、状态非 filled/matched/cancelled/expired 的本系统 managed 外部订单数量写入 `RewardBotService` 热缓存，供控制台 `status.open_orders` 优先展示，避免本地仍 open-like 但外部已不开放或已完全成交的订单短时间抬高开放挂单数。

SELL 退出 intent 持久化的是非亏损退出 floor（被吃买单原价、markup 后价格或外部持仓 `avg_price`，并在提交前再与当前持仓 `avg_price` 取较高值）；提交前不会使用 midpoint 或页面“当前价”降价。`ExitAtMarkup`、`HoldAndRequote` 和外部库存补退出按该 floor 提交 post-only maker SELL 来保留反向流动性奖励机会；若当前 orderbook 买一已大于等于 floor，原价卖单会穿盘口，因此只记录 `reward_live_exit_post_only_crossing_deferred` 并按 30 秒退避等待可 resting 的盘口。`FlattenImmediately` 只有在 best bid 不低于 floor 时才用非 post-only FAK/taker SELL 按 best bid 尝试非亏损平仓；best bid 缺失或低于 floor 时保留 deferred exit 并按 30 秒退避。SELL 提交前会用当前 positions 校验并裁剪 size；无对应 token 持仓时关闭 stale exit。

poll loop 每轮读取持久化 rewards 配置；读取失败时不会使用默认配置冒险执行，也不会永久退出任务，而是等待 1 秒后重试。控制命令 wake、配置 revision 变化、活跃 rewards token 的 orderbook stream 更新和周期 timer 都会唤醒 loop；活跃盘口事件还会进入独立事件撤单 channel，由 `RewardEventCancelGuard` 常驻 task 立即执行 hard-risk cancel-only 检查，不等待 poll loop 当前 full tick、控制命令或外部同步结束。普通 fast reconcile 仍会被活跃盘口 wake 或周期 timer 唤醒，用于成交同步、退出 intent、完整撤单检查和节流外部同步。持有 live advisory lease 的 poll loop 启动后会立即尝试一次 rewards 历史清理，之后每 5 天清理一次 5 天前的终态 managed orders（`cancelled`/`filled`/`error`）、risk events 和低竞争 observations；清理失败只记录 warn，不退出交易循环，并且不会删除 `planned`/`open`/`exit_pending`、fills、positions 或 account state。fast reconcile 每轮仍会用活跃盘口做风险撤单和退出 intent 处理，但外部重型同步独立节流：托管订单状态最小 5 秒间隔，CLOB open-order snapshot 最小 15 秒间隔，managed scoring 按 `min_scoring_check_sec` 且归一化下限 15 秒，账户级 rewards earnings 与 balance/positions snapshot 最小 60 秒间隔；full tick 和 `run_once` 完整同步后会刷新这些节流时间戳，避免紧随其后的盘口事件重复打外部 API。full tick 仍由 `POLYEDGE_REWARDS__POLL_INTERVAL_SECS` 作为全量候选发现和计划重建兜底，fast reconcile 仍由 `reconcile_interval_sec` 作为兜底 sweep；内部 WS 空闲重连只恢复事件源，不能替代这两个周期兜底。数据库 worker heartbeat 写入失败只记录告警；CLOB 订单 heartbeat 独立每 5 秒发送并串联 server 返回的 heartbeat id，单次请求 4 秒超时，失败或超时后清空 id 并按 5-60 秒退避重建链；首个失败和连续失败每 6 次记录 warn，其余连续失败降为 debug，恢复时记录 info。生产环境必须保持 poll loop 运行；一次性命令或有限循环退出后不再维护订单 heartbeat。

旧的未提交 quote intent 会先经过当前计划、盘口、kill switch 和撤单风险检查，再允许提交；BUY intent 在真正 POST 前还会用 orderbook 服务做 1 秒 max-age last-look，若盘口缺失、刷新失败或出现 best ask touch 等硬风险则不提交。任一提交结果未知、待最终对账或外部订单 404 会暂停全部新增买单，但不会阻断订单同步、风险撤单或卖出退出；外部订单 404 锁超过 5 分钟且仍无成交证据时会自动本地关闭；提交结果未知订单在恢复查询确认 CLOB 无对应 open 订单后经过 `LIVE_SUBMISSION_UNKNOWN_CLOSE_AFTER_SECS`（默认 600 秒）宽限也会自动本地关闭，不再需要手动改库，但若 positions 快照显示该 BUY token 在提交后出现库存则继续保留对账锁。同一批次第一笔 POST 结果未知后也不会继续发送后续买单。CLOB 已明确返回的 HTTP 4xx 拒单会将当前 intent 标记为 error，不会误进入提交未知锁。managed order 会持久化实际提交数量；SELL intent 的 price 保留非亏损退出 floor，post-only exit 被取消后的 replacement 会保留退出 floor 并在后续按 maker 规则重试，flatten replacement 保留退出 floor 并在后续按 best bid 非亏损 FAK 或继续等待。

关联 trade 按 ID 单独查询失败时，connector 会按该订单 token 和下单时间窗口扫描认证账户 trades，并按 external order id 精确匹配；只有所有预期关联 trade 都达到终态后才关闭订单。若认证 CLOB 已明确给出 matched size，但认证 trade 明细与历史页仍无法解码，worker 会再读取 Data API 钱包活动，并且只在 token、BUY、价格、时间窗口、累计数量与唯一 managed order 全部严格匹配时生成补账 fill。单订单已返回 404 时，无论认证账户 trade 扫描报错，还是扫描成功但没有返回该 external order id 的成交，都会继续执行 Data API 回退；此时还必须要求累计数量恰好等于本地订单剩余量，并且完整外部持仓快照已覆盖该数量。外部账户/持仓快照时间已覆盖该成交时，补账不会再次扣减现金或增加库存，但仍会关闭本地订单并创建退出 intent。任一订单的全部回退都失败时，worker 只跳过该订单并继续处理其余订单、账户快照和 stale 清理；如果同一外部订单 404 锁已持续超过 5 分钟，worker 会本地关闭该订单，不再中止整轮 reconcile。

### orderbook_stream — 盘口流（已迁移到 orderbook 服务）

盘口流逻辑已迁移到 `polyedge-orderbook` 服务（`packages/orderbook/src/stream.rs`）。Worker 中 `worker/orderbook_stream.rs` 仅保留 `consume-orderbook-stream` CLI 子命令兼容（daemon 模式不再调度），兼容路径同样消费完整 `book` 快照和 `price_change` 增量，并周期性全量 poll 当前 token。Worker 通过 `OrderbookHttpClient`（HTTP）读取 orderbook 服务的缓存数据，通过携带 `POLYEDGE_ORDERBOOK__WRITE_TOKEN` 的 `register-orderbook-tokens` 任务注册订阅 token；rewards poll loop 还会通过 `OrderbookStreamClient` 订阅 orderbook 服务内部 `/orderbook/stream`，将 WS/poll/ingest 更新写入 worker 本地 cache，并把活跃 rewards token 更新同时用于唤醒普通 fast reconcile 和投递到独立 hard-risk 撤单 worker。Standalone orderbook 服务遵守 `POLYEDGE_ORDERBOOK_STREAM__RESTART_INTERVAL_SECS`，HTTP `/orderbook/register` 原子替换对应 source 的 token 集合，缓存每侧盘口深度受 `POLYEDGE_ORDERBOOK_STREAM__MAX_LEVELS_PER_SIDE` 限制。

### arbitrage — 套利扫描

```
market_event_service.list_markets(status=Open)
    → (回退: fetch_gamma_markets if DB 为空)
    → 对每个市场获取盘口
    → detect_arbitrage_opportunities()
    → arbitrage_service.record_*()
    → prune old arbitrage_events and arbitrage_scans by retention cutoff
```

Report: `ArbitrageScanRunReport { markets_scanned, snapshots/opportunities/validations recorded, expired, events_pruned, scans_pruned, snapshots_pruned, scan_opportunities_pruned }`

每轮扫描完成后，worker 使用 `arbitrage.event_retention_hours` 计算 cutoff：先清理旧 `arbitrage_events`，再删除 cutoff 前的旧 `arbitrage_scans`。`market_book_snapshots`、`arbitrage_opportunities` 和 validations 通过数据库外键级联删除；Postgres 实现分批删除旧 scan，避免 `market_book_snapshots` 因持续扫描无限膨胀。

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
- `run` 主循环包含 database-maintenance、register-orderbook-tokens、rewards、copytrade、arbitrage、news、execution、signal-recompute 等任务
- database-maintenance 默认生产模板开启、本地模板关闭；它集中清理可增长历史/缓存/队列表，避免 `reward_market_candles`、AI/info-risk cache、raw events、copytrade/source trade、控制命令、outbox/dedup、LLM/audit 等表无限膨胀。
- arbitrage 每轮扫描结束后按 `arbitrage.event_retention_hours` 自动清理旧 scan 历史；旧 `market_book_snapshots` 通过 `arbitrage_scans` 外键级联删除，日志会输出 `scans_pruned`、`snapshots_pruned` 和 `scan_opportunities_pruned`
- news worker 当前只抓取 RSS/Atom XML feed；未配置 `POLYEDGE_NEWS__SOURCES_JSON` 时会读取内置默认源列表，部署模板显式写入默认源并默认设置 `POLYEDGE_NEWS__ENABLED=true`、`POLYEDGE_WORKER__POLL_NEWS=true`
- rewards worker 会通过数据库命令队列接收前端 Run / Cancel / Reset 请求，API 进程不再执行 rewards 策略；仅支持 live 实盘模式，策略配置不依赖全局 system mode，但新买单和现有买单撤单遵守全局 kill switch
- copytrade worker 会通过数据库命令队列接收前端兼容控制命令；当前前端只暴露 Analyze，Run/Cancel/Reset 不再作为产品入口。API 进程不抓取 copytrade 输入，worker 负责 Data API 抓取、source trades 检测和钱包分析
- register-orderbook-tokens 每个 source 独立注册全量 token，由 orderbook registry 聚合层按固定优先级 `rewards_active`、`exec_orders`、`rewards_eligible`、`rewards_ai_provider`、`rewards_low_competition_probe`、`rewards_candidates` 跨 source 去重并 `take(POLYEDGE_ORDERBOOK_STREAM__MAX_TOKENS)` 截断总量；`rewards_eligible` 由周期任务统一注册全部最终 eligible quote plan token，AI gate 前的 deterministic eligible token 由 AI advisory provider refresh 以最多 10 个市场一批注册到临时 `rewards_ai_provider` source，低竞争 gate 前候选由 full tick 以最多 10 个市场一批注册到 `rewards_low_competition_probe` source，且 AI/info-risk 两条 refresh 队列各自保留开放订单/持仓优先，剩余普通候选与低竞争通过 gate 的 pre-AI 市场按约 2:1 混排；rewards live 新买单落库后会即时刷新 `rewards_active` source，候选 token 优先来自 open/tradable 且 `volume_24h` 高的市场并受 `POLYEDGE_ORDERBOOK_STREAM__REWARD_CANDIDATE_TOKEN_CAP` 限制；空集合成功结果会防抖后再清 source，active/exec 连续 2 轮、eligible/candidates 连续 3 轮才清理
- rewards poll loop 在 Postgres 路径全程持有 advisory lease，统一覆盖 CLOB heartbeat、命令、orderbook 内部 WS 本地 cache、独立事件撤单 worker、full tick、fast reconcile 和 5 天历史清理；本地盘口 cache 按本地接收 TTL 过期，避免上游未来时间戳延长缓存寿命；控制命令具备 5 分钟 running lease
- 活跃 rewards token 的 orderbook 更新会进入独立事件撤单 worker，按更新 token 过滤开放订单并立即执行 hard-risk cancel-only 检查；该路径不跑订单/账户同步、重挂、退出提交、报价漂移换价或定期 requote，普通 fast reconcile 仍作为完整检查和周期兜底
- rewards orderbook 内部 WS client 建连最多等待 5 秒；已连接后若约 3 个 orderbook poll reconcile 周期无事件，会主动重连并重新 HTTP bootstrap 本地盘口 cache
- scheduled full tick 不再二次消费控制命令；拿不到 advisory lease 时保留到期状态并在后续轮询重试，不会把 command-only 周期记作已完成 full tick
- rewards poll loop 按账户写入 `reward_worker_heartbeats`；snapshot 的 `status.running` 仅在配置启用且最近 2 分钟存在 heartbeat 时为 true
- rewards SELL 退出 intent 按非亏损 floor 执行：`ExitAtMarkup`、`HoldAndRequote` 和外部库存补退出走 post-only maker SELL；`FlattenImmediately` 在 best bid 不低于 floor 时走非 post-only FAK/taker SELL，否则按 30 秒退避等待非亏损 bid。提交前会按当前 token 持仓裁剪 size，无持仓 stale exit 会关闭，不使用 midpoint 或页面“当前价”降价卖出。
- 低竞争市场 sleeve v2 已实现并默认关闭；启用后 worker 会构建独立低竞争候选 profile，并在 gate 前用 `rewards_low_competition_probe` 独立 source 按最多 10 个市场一批预热盘口和历史样本；observe 模式只展示指标，enforce 模式通过独立小额度 cap 和 AI/info-risk fail-closed gate 后才允许低竞争 bucket 下单；通过低竞争 gate 的 pre-AI 市场会在后台 AI advisory refresh 中与普通候选按约 2:1 混排进入每批 10 个市场的临时盘口订阅和 AI 分析，info-risk refresh 使用独立队列补缓存；full tick 会用 gate 前固化的计划 notional 保存跨周期 observation，缺 fresh midpoint/盘口指标/历史样本时保留 quote legs 和 `orderbook_token_ids` 并标记 data unavailable，snapshot 提供 shadow report 和小额 enforce 建议，但不会自动启用；live placement 会把低竞争 open-like 订单限制在全局 `max_open_orders` 的约 30% 内（全局允许时至少 1 单）。
- rewards full tick 已读取 Gamma `markets.category` 作为候选评分输入；命中 `preferred_categories` 时只增加候选排序分，不绕过市场质量、盘口和风控硬过滤。AI advisory 已接入 full tick：live tick 只读已有 cache，后台 AI advisory provider refresh 与 info-risk provider refresh 两个 task 分别异步填充 `reward_market_advisories` 和 `reward_market_info_risks`，两条队列各自保留开放订单/持仓最高优先级，剩余普通 eligible/candidate 市场与低竞争通过 gate 的 pre-AI quote plan 按约 2:1 混排；新写入的 AI/info-risk cache 使用确定性 TTL jitter 打散过期时间，provider refresh 会在未过期缓存进入刷新窗口时提前续期；AI advisory 使用临时盘口批次补缓存，info-risk 独立批量/逐市场补缓存，`ai_advisory_batch_size` 和 `info_risk_batch_size` 分别控制主 refresh 单次 provider 请求包含的市场数，默认 1 保持逐市场，批量解析按 condition 拆分保存，漏项/错配或整体解析失败会回退单市场，provider 过载则停止对应 task 本轮剩余请求；AI 请求候选限定为 deterministic planner 原本 eligible 且仍缺少有效 advisory 的 condition，payload 包含 price-history 5m candles 和摘要；AI 开启后 provider 未配置、失败或缺缓存仍 fail closed，`avoid` 硬拦截，但 `watch`、低置信度 `allow/watch` 和非 avoid 的 `quote_mode=none` 会回退 deterministic 计划继续进入 live 盘口、资金和订单风控；高置信度 `allow` 只在 `selection_mode=enforce` 且 `quote_mode=auto` 时可把报价收窄为单腿，且不会放宽任何硬过滤；provider confidence 会被钳制到 `0..=1`。信息风险已接入缓存过滤：AI 开启时由 full tick 专用 info-risk provider refresh 推进，AI 未开启时仍可由独立 info-risk worker 扫描；live tick 只读缓存，enforce 模式下缺缓存仍 fail closed；新 condition 首次 BUY 报价还会按配置等待信息风险缓存和首单观察窗口，已有订单/持仓 condition 不受该首单 gate 限制；已有风险中只有 `critical`、官方结果、`resolution_imminent=true` 或配置为 `low/medium` 避免等级时命中的风险等级会硬拦截，普通 `high` 风险和仅 `risk_type=imminent_resolution` 但 `resolution_imminent=false` 的结果保留为信息提示并继续进入 live 盘口、资金和订单风控；新增买单会保守扣除未归属到本系统 managed order 的外部 BUY notional。
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
