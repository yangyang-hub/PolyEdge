# Worker App（后台任务服务）

最后更新：2026-06-13

## 概述

Worker 代码现在同时提供共享库和兼容 CLI。`polyedge-api` 在同一进程内启动 `WorkerRuntime`，运行新闻采集、信号重算、执行分发、订单对账、奖励机器人、套利扫描、跟单执行和 orderbook token 注册；`polyedge-worker` 二进制继续提供维护/调试子命令，但 Docker 不再部署独立常驻 worker 容器。

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
| `worker/rewards/account_sync.rs` | rewards 外部余额、完整持仓、订单 scoring 与 UTC 当日账户级 rewards 聚合同步 |
| `worker/rewards/live_sync.rs` | rewards live 托管订单成交/状态同步、单订单失败隔离、Reset cancel-all 语义 |
| `worker/rewards/live_orders.rs` | rewards live 撤单、成交入账、post-fill exit/flatten intent |
| `worker/rewards/live_submission.rs` | rewards live 单笔提交、post-only 接受状态处理和 submission marker |
| `worker/rewards/live_pending.rs` | rewards live 持久化 intent 提交、开放订单匹配恢复和未知结果锁定 |
| `worker/rewards/live_helpers.rs` | rewards live 价格 tick、fill id、退出重试与订单状态转换辅助函数 |
| `worker/rewards/live_risk.rs` | rewards live placement/cancel 风控：盘口可用性、depth/rank/history/requote、库存 cap |
| `worker/rewards/orderbook_events.rs` | rewards worker 本地盘口 cache、orderbook 内部 WS 消费、HTTP bootstrap 和活跃 token 事件唤醒 |
| `worker/rewards/polling.rs` | rewards poll loop、盘口读取、事件驱动 fast reconcile 和进程内盘口历史 |
| `tests/rewards.rs` / `tests/rewards_reconciliation.rs` | rewards live 下单、风控、成交、对账、退出重试与增量持久化回归测试 |
| `worker/arbitrage.rs` | 套利扫描 |
| `worker/arbitrage_books.rs` | 套利盘口快照 |
| `worker/copytrade.rs` | 跟单执行；消费 API 入队的 run/analyze/cancel/reset 控制命令 |
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
| `scan-copytrade-once` | `run_copytrade_once` | 一次性消费 copytrade 控制命令或执行跟单循环 |
| `poll-copytrade` | `poll_copytrade` | 持续消费 copytrade 控制命令和跟单轮询 |
| `analyze-wallets-once` | `analyze_wallets_once` | 一次性钱包分析 |
| `drain-execution-queue` | `drain_execution_queue` | 处理排队的执行请求 |
| `reconcile-paper-fills` | `reconcile_paper_fills` | Paper 交易对账 |
| `poll-paper-order-statuses` | `poll_paper_order_statuses` | Paper 订单状态轮询 |
| `poll-polymarket-order-statuses` | `poll_polymarket_order_statuses` | Live Polymarket 订单状态轮询 |
| `reconcile-polymarket-fills` | `reconcile_polymarket_fills` | Live Polymarket 成交对账 |
| `consume-polymarket-user-events` | `consume_polymarket_user_events` | 消费 Polymarket WS 事件 |

## 核心 Worker 数据流

### market_sync — 市场同步（已迁移到 orderbook 服务）

市场同步逻辑已迁移到 `polyedge-orderbook` 服务（`apps/orderbook/src/market_sync.rs`）。Orderbook 服务启动时先暴露 HTTP `/healthz`，再由后台任务执行 initial/periodic market sync，避免外部市场 API 延迟阻塞容器健康检查。Worker 中保留 `sync_markets_once` 函数供 CLI 子命令 `sync-markets-once` 使用，但 daemon 模式不再调度此任务。

### register-orderbook-tokens — 盘口 token 注册

```
register_orderbook_tokens()
    → 遍历活跃执行订单（Submitted/Open/PartiallyFilled）→ 解析市场 YES/NO asset_id
    → reward_bot_service.list_active_reward_book_token_ids() → rewards 活跃订单/持仓 token
    → reward_bot_service.list_eligible_reward_book_token_ids() → 当前 eligible quote plan token
    → reward_bot_service.list_all_reward_candidate_token_ids() → rewards 候选 token 填充剩余额度
    → orderbook_registry.register_tokens("rewards_active", ...)
    → orderbook_registry.register_tokens("exec_orders", ...)
    → orderbook_registry.register_tokens("rewards_eligible", ...)
    → orderbook_registry.register_tokens("rewards_candidates", ...)
    // 通过 OrderbookHttpClient → HTTP POST /orderbook/register 注册到 orderbook 服务
```

此任务替代了原来的 `consume-orderbook-stream` 和 `sync-markets` 任务。Worker 不再直接运行盘口流或市场同步，而是通过 HTTP 告知 orderbook 服务需要订阅哪些 token。
注册任务最长每 60 秒执行一次，orderbook 服务重启后可自动恢复订阅。注册总量受 `POLYEDGE_ORDERBOOK_STREAM__MAX_TOKENS` 限制；分配顺序固定为 rewards 活跃订单/持仓 token、活跃 execution token、当前 eligible quote plan token、其余 rewards 候选 token。候选来源始终保留，用于给尚未产生 quote plan 的市场预热盘口，避免 eligible-only 冷启动。每个成功查询的 source 使用一次原子替换注册，空集合会清理远端旧 source；任一 source 的数据库查询失败时保留远端上一版集合，不会用空集合误删订阅。

### copytrade — 跟单

```
copytrade_service.claim_next_control_command()
    → worker 执行 queued run_once / analyze_wallets / cancel_all / reset
    → copytrade_service.complete_control_command() 或 fail_control_command()

无待处理控制命令时：
    fetch_copytrade_inputs() // 获取钱包活动 + 盘口
        → orderbook_registry.register_tokens("copytrade", current_activity_tokens)
        → copytrade_service.run_copy_cycle() // 业务逻辑
```

Report: `CopyTradeRunReport { wallets_scanned, trades_detected, orders_placed, orders_filled, orders_skipped }`

约束：worker 是 copytrade 手动控制命令和跟单循环的唯一执行者。API 只把 `run_once` / `analyze_wallets` / `cancel_all` / `reset` 写入 `copytrade_control_commands`；worker 每轮先领取并处理待执行命令，处理到命令时跳过本轮自动 tick。
Copytrade 每轮会用当前钱包活动 token 替换 `copytrade` 来源的 orderbook 订阅集合，不保留历史活动 token，避免 orderbook 服务 registry 长期单调增长。
应用服务会把未处理 source trades 按时间排序，并在同一 tick 内依次执行暂停钱包、wallet+token cooldown、UTC 日亏损和运行中 per-wallet/per-market/total exposure cap；accepted buy 会立即占用后续决策的 headroom。crossed 模拟订单完整成交，resting 概率成交才应用 `max_fill_ratio`，无本地持仓的 sell 会被跳过。

### rewards — 奖励策略与控制命令

```
reward_bot_service.claim_next_control_command()
    → worker 执行 queued run_once / cancel_all / reset
    → reward_bot_service.complete_control_command() 或 fail_control_command()

无待处理控制命令时：
    fetch_reward_bot_inputs() // 获取奖励市场 + 盘口
        → prepare_live_cycle()
        → sync managed rewards order trades/statuses
        → 批量同步 managed order scoring 状态与 UTC 当日账户级 maker rewards（`/rewards/user/total` 聚合优先，`/rewards/user` 明细 fallback）
        → 无近期 confirmed fill 时同步外部 balance + 链上 pUSD 余额回退 + 完整 positions 快照
        → LivePolymarketConnector.submit_token_order()
        → orderbook stream active-token event 或 reconcile_interval_sec 兜底：读取活跃盘口并对本系统托管订单做成交同步和安全撤单检查
```

Report: `RewardBotRunReport { markets_scanned, books_fetched, plans_built, eligible_plans, placed_orders, cancelled_orders, filled_orders, risk_cancelled_orders, reward_accrued }`

约束：后台 runtime 是 rewards 策略和控制命令的唯一执行者。rewards poll loop 在整个生命周期持有 Postgres advisory lease，多实例中只有 lease owner 会认证 CLOB、执行命令/full tick/fast reconcile 并维护 5 秒 heartbeat id 链；standby 实例不发心跳也不执行。API handler 只写控制命令；共享 `RewardBotService` 会立即唤醒同进程 loop，配置 revision 变化还会立即触发 full cycle。`POLYEDGE_WORKER__POLL_REWARD_BOT=true` 控制 API 内嵌 runtime 是否启动 rewards loop。

自动 tick 只从 Postgres 的 `reward_markets` 读取奖励市场。长期 `poll-reward-bot` 启动后会通过 `OrderbookStreamClient` 连接 `polyedge-orderbook` 的内部 `/orderbook/stream`，维护 worker 进程内本地盘口 cache；本地 cache 使用 `POLYEDGE_ORDERBOOK_STREAM__MAX_LEVELS_PER_SIDE` 和 `POLYEDGE_ORDERBOOK_STREAM__BOOK_TTL_MS` 限制深度与过期读取。启动和重连时通过 `OrderbookHttpClient` / `POST /orderbook/batch` bootstrap 当前 rewards 活跃、eligible 和候选 token，后续缺失 token 也会按需 HTTP 补齐；内部 WS 连接建立最多等待 5 秒，已连接后若约 3 个 poll reconcile 周期没有收到任何事件，worker 会主动重连并重新 bootstrap。Postgres 候选 market pool 关联 Gamma `markets`，硬过滤非 open/tradable、高歧义、低流动性、低 24h 成交量、临近结算、Gamma spread 过宽、市场同步过期、奖励不足和最小份额预算不可行的市场；仅唯一且明确的 YES/NO token 会进入候选与订阅。通过门槛后按奖励、流动性、成交量、剩余时长和奖励 spread 综合排序。worker 使用本地 cache 读取候选和活跃 token 盘口，缺失时才回源 orderbook HTTP batch；若本 tick 没有新鲜缓存盘口，不会提交新 post-only 订单。

报价价格由 `quote_bid_rank=1|2|3` 选择 YES/NO 盘口的买一/买二/买三（按不同买价计档）；任一腿缺少目标档位或目标价格超出有效 rewards spread 时不挂单。报价大小把 `per_market_usd` 作为 YES + NO 两腿合计上限；planner 先把两腿 `rewards_min_size` 对齐到 CLOB 成本精度，再分配剩余额度，计划数量与 connector 实际提交数量一致。

live 模式会用 `LivePolymarketConnector::submit_token_order()` 提交 post-only GTC token 买单，用 `cancel_order()` 撤销本系统托管订单；未成交 post-only maker 买单不在本地按全局 notional 硬锁资金。不同 condition 可复用资金，但同一 condition 会累计已有 managed BUY 剩余 notional 与待补 YES/NO 腿，整组超过 `available_usd` 时不生成新 intent，符合 CLOB 同市场余额有效性规则。所有新报价和 post-fill exit/flatten 会先持久化本地 intent，再记录 submission attempt 后调用 CLOB；瞬时明确拒单会持久化回 Planned 并停止本轮后续买单，响应丢失则锁住本地订单并只通过严格开放订单匹配恢复 external order id。新建/恢复订单先保持 `scoring=false`，仅权威 scoring 查询可以置 true；`min_depth_usd` 会扣除自身剩余挂单，只统计外部 bid 深度。live placement 要求目标两腿都有非空盘口，默认 `stale_book_ms=45000`，`stale_book_ms=0` 只关闭盘口年龄检查；live full tick 和 fast reconcile 都会读取开放订单/持仓活跃 token 的盘口，缺盘口、空盘口、过期盘口、外部 bid 深度不足、bid rank 过高、盘口历史窗口风险、目标档位价格移动超过 `requote_drift_cents`、定期 requote 或全局 kill switch 会触发买单撤单，即使 `enabled=false` 已停止新增报价。每笔外部下单、撤单、已确认成交和状态变化会立即落库；撤单/成交同步会跳过本地 synthetic ID。外部单订单查询返回 404 时，worker 会按 token 和下单时间窗口分页查询认证账户 trades，并按 external order id 精确匹配 confirmed fill；仍无法确认时持久化 critical 对账锁并暂停新买单，后续成功查询会自动解除；若该 404 锁超过 5 分钟且仍无 CLOB/Data API 成交证据，worker 会把本地订单标记为 `cancelled` 以释放开放挂单计数。提交结果未知和取消结果未知不会仅因本地超时而 force-cancel；旧 `auto_cancel_stale_minutes` 配置已忽略。撤单接受后本地订单保留为待最终对账，下一轮先同步成交再确认取消，避免 cancel/fill 竞态丢成交。worker 仅在 trade 达到 `CONFIRMED` 后按 external trade id + external order id 幂等写入 fills、现金、库存和 PnL；买入 fill 与对应 exit intent 同事务落库，之后只撤同 condition 对侧仍开放的 buy sibling。`ExitAtMarkup` 卖价向上取整到 0.01 tick；退出拒单和提交前低于 1 美元最小名义金额都会保持 `ExitPending` 并按最多 300 秒的有界指数退避持续重试，同 token 有未完成 sell exit 时暂停新增 buy。`FlattenImmediately` 使用 FAK，缺 bid、退出单被拒绝，或终态部分成交且仍有持仓时，会持久化 deferred sell 并重试。每轮先同步 managed order 的 confirmed fills；本轮新增 fill，或最新 confirmed fill 距今不足 120 秒时，都会跳过整次外部账户替换，防止最终一致性延迟回滚本地现金和库存。保护期结束后，成功 positions 快照原子替换该 rewards 账户全部持仓，失败时保留上一版。该同步在 `enabled=false` 且没有开放订单时也会尝试运行。账户范围外开放订单明细和奖励结算对账仍是缺口，worker 仍需要独立维护组合风险。

poll loop 每轮读取持久化 rewards 配置；读取失败时不会使用默认配置冒险执行，也不会永久退出任务，而是等待 1 秒后重试。控制命令 wake、配置 revision 变化、活跃 rewards token 的 orderbook stream 更新和周期 timer 都会唤醒 loop；盘口事件触发的 fast reconcile 会按 1 秒最小间隔合并，避免高频盘口把 worker 打成紧循环。full tick 仍由 `POLYEDGE_REWARDS__POLL_INTERVAL_SECS` 作为全量候选发现和计划重建兜底，fast reconcile 仍由 `reconcile_interval_sec` 作为兜底 sweep；内部 WS 空闲重连只恢复事件源，不能替代这两个周期兜底。数据库 worker heartbeat 写入失败只记录告警；CLOB 订单 heartbeat 独立每 5 秒发送并串联 server 返回的 heartbeat id，单次请求 4 秒超时，失败或超时后清空 id 以便下一轮重建链。生产环境必须保持 poll loop 运行；一次性命令或有限循环退出后不再维护订单 heartbeat。

旧的未提交 quote intent 会先经过当前计划、盘口、kill switch 和撤单风险检查，再允许提交。任一提交结果未知、待最终对账或外部订单 404 会暂停全部新增买单，但不会阻断订单同步、风险撤单或卖出退出；外部订单 404 锁超过 5 分钟且仍无成交证据时会自动本地关闭；同一批次第一笔 POST 结果未知后也不会继续发送后续买单。CLOB 已明确返回的 HTTP 4xx 拒单会将当前 intent 标记为 error，不会误进入提交未知锁。managed order 会持久化实际提交价格/数量，post-only exit 被取消后的 replacement 仍保持 post-only。

关联 trade 按 ID 单独查询失败时，connector 会按该订单 token 和下单时间窗口扫描认证账户 trades，并按 external order id 精确匹配；只有所有预期关联 trade 都达到终态后才关闭订单。若认证 CLOB 已明确给出 matched size，但认证 trade 明细与历史页仍无法解码，worker 会再读取 Data API 钱包活动，并且只在 token、BUY、价格、时间窗口、累计数量与唯一 managed order 全部严格匹配时生成补账 fill。单订单已返回 404 时，无论认证账户 trade 扫描报错，还是扫描成功但没有返回该 external order id 的成交，都会继续执行 Data API 回退；此时还必须要求累计数量恰好等于本地订单剩余量，并且完整外部持仓快照已覆盖该数量。外部账户/持仓快照时间已覆盖该成交时，补账不会再次扣减现金或增加库存，但仍会关闭本地订单并创建退出 intent。任一订单的全部回退都失败时，worker 只跳过该订单并继续处理其余订单、账户快照和 stale 清理；如果同一外部订单 404 锁已持续超过 5 分钟，worker 会本地关闭该订单，不再中止整轮 reconcile。

### orderbook_stream — 盘口流（已迁移到 orderbook 服务）

盘口流逻辑已迁移到 `polyedge-orderbook` 服务（`apps/orderbook/src/stream.rs`）。Worker 中 `worker/orderbook_stream.rs` 仅保留 `consume-orderbook-stream` CLI 子命令兼容（daemon 模式不再调度），兼容路径同样消费完整 `book` 快照和 `price_change` 增量，并周期性全量 poll 当前 token。Worker 通过 `OrderbookHttpClient`（HTTP）读取 orderbook 服务的缓存数据，通过携带 `POLYEDGE_ORDERBOOK__WRITE_TOKEN` 的 `register-orderbook-tokens` 任务注册订阅 token；rewards poll loop 还会通过 `OrderbookStreamClient` 订阅 orderbook 服务内部 `/orderbook/stream`，将 WS/poll/ingest 更新写入 worker 本地 cache，并只用活跃 rewards token 的更新唤醒 fast reconcile。Standalone orderbook 服务遵守 `POLYEDGE_ORDERBOOK_STREAM__RESTART_INTERVAL_SECS`，HTTP `/orderbook/register` 原子替换对应 source 的 token 集合，缓存每侧盘口深度受 `POLYEDGE_ORDERBOOK_STREAM__MAX_LEVELS_PER_SIDE` 限制。

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
settings.news.sources (enabled 过滤)
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

- 所有 18 个子命令已实现
- `run` 主循环包含 register-orderbook-tokens、rewards、copytrade、arbitrage、news、execution、signal-recompute 等任务
- rewards worker 会通过数据库命令队列接收前端 Run / Cancel / Reset 请求，API 进程不再执行 rewards 策略；仅支持 live 实盘模式，策略配置不依赖全局 system mode，但新买单和现有买单撤单遵守全局 kill switch
- copytrade worker 会通过数据库命令队列接收前端 Run / Analyze / Cancel / Reset 请求，API 进程不再执行跟单任务或抓取跟单输入
- copytrade worker 注册 orderbook token 时会原子替换 `copytrade` source 当前活动 token 集合，防止历史钱包活动 token 无限留在 orderbook 订阅 registry 中
- register-orderbook-tokens 会按 `POLYEDGE_ORDERBOOK_STREAM__MAX_TOKENS` 限制总量并固定优先级：`rewards_active`、`exec_orders`、`rewards_eligible`、`rewards_candidates`；候选 token 优先来自 open/tradable 且 `volume_24h` 高的市场，空集合会清除对应旧 source
- rewards poll loop 在 Postgres 路径全程持有 advisory lease，统一覆盖 CLOB heartbeat、命令、orderbook 内部 WS 本地 cache、full tick 和 fast reconcile；控制命令具备 5 分钟 running lease
- rewards orderbook 内部 WS client 建连最多等待 5 秒；已连接后若约 3 个 orderbook poll reconcile 周期无事件，会主动重连并重新 HTTP bootstrap 本地盘口 cache
- scheduled full tick 不再二次消费控制命令；拿不到 advisory lease 时保留到期状态并在后续轮询重试，不会把 command-only 周期记作已完成 full tick
- rewards poll loop 按账户写入 `reward_worker_heartbeats`；snapshot 的 `status.running` 仅在配置启用且最近 2 分钟存在 heartbeat 时为 true
- rewards full tick 和 fast reconcile 在 managed order 同步后刷新外部余额/完整持仓快照；资金钱包地址优先使用 `POLYEDGE_POLYMARKET__FUNDER`，未配置时使用 `ACCOUNT_ID`；CLOB balance 为 0 或失败但链上 pUSD 余额大于 0 时，账户 snapshot 用 Polygon pUSD 余额回填；新确认成交所在周期及其后 120 秒会延后整次账户快照替换，避免 CLOB/Data API 最终一致性回滚本地账本
- 默认大部分 worker 通过配置开关控制启用/禁用
- Polymarket live 任务需要真实凭证；Deposit Wallet 使用 `POLYEDGE_POLYMARKET__SIGNATURE_TYPE=poly_1271` + `POLYEDGE_POLYMARKET__FUNDER=<deposit_wallet>`，worker 会通过 connector 走 CLOB V2 `POLY_1271` 下单/撤单路径。
- Rewards 生产与测试入口均已移除 `RewardSimulationOutcome` / `simulated_orders` 旧命名，统一使用 `RewardTickOutcome` / `placed_orders`。

## 修改检查清单

- [ ] 新增 worker 任务时：(1) 在 `worker/` 中创建文件 (2) 在 `main.rs` 中添加 CLI 子命令 (3) 在 `run_worker_service()` 中添加到主循环
- [ ] 修改 worker 逻辑后检查对应的 application service 是否需要更新
- [ ] 新增 Report 类型时确保使用 `Default` derive 并包含有用的统计字段
- [ ] 运行 `cargo check --workspace --tests`
- [ ] 更新根目录 `AGENTS.md` 中的常用 worker 子命令列表
