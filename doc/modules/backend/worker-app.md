# Worker App（后台任务服务）

最后更新：2026-05-31

## 概述

`polyedge-worker` 是基于 Tokio 的异步后台任务服务。它运行所有周期性和流式任务：市场同步、新闻采集、信号重算、执行分发、订单对账、盘口流、奖励机器人、套利扫描和跟单执行。

## 设计目标

- 每个 worker 任务是独立的函数，可通过 CLI 子命令单独运行或组合运行
- 支持优雅关闭（`tokio::signal::ctrl_c()`）
- 每次运行生成结构化 Report 用于监控和日志
- 通过 `AppState` 共享所有服务实例

## 架构与关键文件

| 文件 | 职责 |
|---|---|
| `main.rs` | 入口：CLI 参数解析（18 个子命令）、`run_worker_service()` 主循环（~455 行） |
| `worker/service.rs` | Worker 编排服务 |
| `worker/market_sync.rs` | 市场同步：Gamma API → `markets` 表 + Rewards API → `reward_markets` 表 |
| `worker/news.rs` | 新闻采集入口 |
| `worker/news_helpers.rs` | 新闻采集辅助函数 |
| `worker/news_promotion.rs` | 新闻提升为 events/evidence |
| `worker/signal_recompute.rs` | 信号重算 |
| `worker/execution_dispatch.rs` | 执行请求分发到连接器 |
| `worker/execution_queue.rs` | 执行队列管理 |
| `worker/execution_reconcile.rs` | 订单/成交对账 |
| `worker/orderbook_stream.rs` | WebSocket 盘口流实时订阅 |
| `worker/rewards.rs` | 奖励机器人 tick；消费 API 入队的 run/cancel/reset 控制命令 |
| `worker/arbitrage.rs` | 套利扫描 |
| `worker/arbitrage_books.rs` | 套利盘口快照 |
| `worker/copytrade.rs` | 跟单执行；消费 API 入队的 run/analyze/cancel/reset 控制命令 |
| `worker/polymarket_config.rs` | Polymarket 配置刷新 |
| `worker/polymarket_events.rs` | Polymarket 用户事件 WebSocket |
| `worker/shared.rs` | 共享辅助函数 |

## CLI 子命令

| 命令 | 函数 | 描述 |
|---|---|---|
| `run`（默认） | `run_worker_service` | 长期运行的 worker 服务循环 |
| `sync-markets-once` | `sync_markets_once` | 一次性市场同步 |
| `ingest-news-once` | `ingest_news_once` | 一次性新闻采集 |
| `poll-news` | `poll_news` | 持续新闻轮询 |
| `promote-news-events` | `promote_news_events` | 新闻提升为 events |
| `scan-arbitrage-once` | `scan_arbitrage_once` | 一次性套利扫描 |
| `poll-arbitrage-radar` | `poll_arbitrage_radar` | 持续套利扫描 |
| `analyze-arbitrage-opportunities` | `analyze_arbitrage_opportunities` | 套利历史分析 |
| `scan-rewards-once` | `run_reward_bot_once` | 一次性消费 rewards 控制命令或执行奖励模拟 |
| `poll-reward-bot` | `poll_reward_bot` | 持续消费 rewards 控制命令和奖励模拟轮询 |
| `scan-copytrade-once` | `run_copytrade_once` | 一次性消费 copytrade 控制命令或执行跟单循环 |
| `poll-copytrade` | `poll_copytrade` | 持续消费 copytrade 控制命令和跟单轮询 |
| `analyze-wallets-once` | `analyze_wallets_once` | 一次性钱包分析 |
| `drain-execution-queue` | `drain_execution_queue` | 处理排队的执行请求 |
| `reconcile-paper-fills` | `reconcile_paper_fills` | Paper 交易对账 |
| `poll-paper-order-statuses` | `poll_paper_order_statuses` | Paper 订单状态轮询 |
| `poll-polymarket-order-statuses` | `poll_polymarket_order_statuses` | Live Polymarket 订单状态轮询 |
| `reconcile-polymarket-fills` | `reconcile_polymarket_fills` | Live Polymarket 成交对账 |
| `consume-polymarket-user-events` | `consume_polymarket_user_events` | 消费 Polymarket WS 事件 |
| `consume-orderbook-stream` | `consume_orderbook_stream` | 消费盘口 WS 流 |

## 核心 Worker 数据流

### market_sync — 市场同步

```
PolymarketGammaConnector.fetch_markets()
    → gamma_market_to_view() 转换
    → market_event_service.upsert_markets() 写入 markets 表

PolymarketRewardsConnector.fetch_current_markets()
    → reward_market_from_connector() 转换
    → reward_bot_service.upsert_reward_markets() 写入 reward_markets 表
```

Report: `MarketSyncReport { fetched, upserted }`

### copytrade — 跟单

```
copytrade_service.claim_next_control_command()
    → worker 执行 queued run_once / analyze_wallets / cancel_all / reset
    → copytrade_service.complete_control_command() 或 fail_control_command()

无待处理控制命令时：
    fetch_copytrade_inputs() // 获取钱包活动 + 盘口
        → copytrade_service.run_copy_cycle() // 业务逻辑
```

Report: `CopyTradeRunReport { wallets_scanned, trades_detected, orders_placed, orders_filled, orders_skipped }`

约束：worker 是 copytrade 手动控制命令和跟单循环的唯一执行者。API 只把 `run_once` / `analyze_wallets` / `cancel_all` / `reset` 写入 `copytrade_control_commands`；worker 每轮先领取并处理待执行命令，处理到命令时跳过本轮自动 tick。

### rewards — 奖励模拟与控制命令

```
reward_bot_service.claim_next_control_command()
    → worker 执行 queued run_once / cancel_all / reset
    → reward_bot_service.complete_control_command() 或 fail_control_command()

无待处理控制命令时：
    fetch_reward_bot_inputs() // 获取奖励市场 + 盘口
        → reward_bot_service.run_simulation() // 自动模拟 tick
```

Report: `RewardBotRunReport { markets_scanned, books_fetched, plans_built, eligible_plans, simulated_orders, cancelled_orders, filled_orders, reward_accrued }`

约束：worker 是 rewards 策略和控制命令的唯一执行者。API 只把 `run_once` / `cancel_all` / `reset` 写入 `reward_control_commands`；worker 每轮先领取并处理待执行命令，处理到命令时跳过本轮自动 tick。`run_once` 命令会强制执行一次模拟（即使自动挂单开关关闭），`cancel_all` 撤销开放模拟订单并释放 reserved，`reset` 重置模拟资金池。

自动 tick 只从 Postgres 的 `reward_markets` 读取奖励市场、从进程内 `InMemoryOrderbookCache` 读取盘口（TTL 默认 5 分钟，后台清理任务每 60 秒淘汰过期条目）。每个 tick 只读取 bounded candidate market pool（默认至少 100、最多 500 个高日奖励市场），先按配置预过滤奖励市场，再并发读取候选盘口缓存；若本 tick 没有新鲜缓存盘口，模拟器不会产生盘口成交或 rewards 计提，只刷新当前候选计划/保留订单状态。

### orderbook_stream — 盘口流

```
collect_orderbook_subscription_tokens() // 从开放订单 + 全量奖励候选市场（不限于已有仓位）收集 token ID
    → ClobWsClient.subscribe_orderbook() // WebSocket 订阅（连接时固定 token 列表）
    → tokio::select! { ws消息, token刷新定时器 } // 每 token_refresh_interval_secs 重新评估订阅列表
    → token_list 变化 → 更新 poll reconciler 的共享 token 列表 + 断开 WS 触发重连
    → poll reconciler 每 poll_reconcile_interval_secs 拉取 stale token 的盘口（动态读取最新 token 列表）
    → orderbook_cache.set_book() // 写入 InMemoryOrderbookCache（带 TTL）
```

Report: `OrderbookStreamReport { subscribed_tokens, ws_snapshots_received, poll_reconciliations, poll_failures }`

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
- **配置来源**：`infrastructure::Settings` 中的 worker、rewards、copytrade、arbitrage、news、orderbook_stream 配置段

## 当前状态

- 所有 18 个子命令已实现
- `run` 主循环包含 market_sync、orderbook_stream、rewards、copytrade、arbitrage、news、execution 等任务
- rewards worker 会通过数据库命令队列接收前端 Run / Cancel / Reset 请求，API 进程不再执行 rewards 策略
- copytrade worker 会通过数据库命令队列接收前端 Run / Analyze / Cancel / Reset 请求，API 进程不再执行跟单任务或抓取跟单输入
- 默认大部分 worker 通过配置开关控制启用/禁用
- Polymarket live 任务需要真实凭证

## 修改检查清单

- [ ] 新增 worker 任务时：(1) 在 `worker/` 中创建文件 (2) 在 `main.rs` 中添加 CLI 子命令 (3) 在 `run_worker_service()` 中添加到主循环
- [ ] 修改 worker 逻辑后检查对应的 application service 是否需要更新
- [ ] 新增 Report 类型时确保使用 `Default` derive 并包含有用的统计字段
- [ ] 运行 `cargo check --workspace --tests`
- [ ] 更新根目录 `AGENTS.md` 中的常用 worker 子命令列表
