# Worker App（后台任务服务）

最后更新：2026-06-02

## 概述

`polyedge-worker` 是基于 Tokio 的异步后台任务服务。它运行所有周期性任务：新闻采集、信号重算、执行分发、订单对账、奖励机器人、套利扫描、跟单执行和 orderbook token 注册。市场同步和盘口流已迁移到独立的 `polyedge-orderbook` 服务。

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
| `worker/orderbook_stream.rs` | Orderbook stream — 仅保留 CLI 子命令兼容，核心逻辑已迁移到独立 `polyedge-orderbook` 服务 |
| `worker/rewards.rs` | 奖励机器人 tick；消费 API 入队的 run/cancel/reset 控制命令 |
| `worker/rewards/live_sync.rs` | rewards live 托管订单成交/状态同步、Reset cancel-all 语义 |
| `worker/rewards/live_orders.rs` | rewards live 下单、撤单、成交入账、post-fill exit/flatten |
| `worker/rewards/live_risk.rs` | rewards live placement/cancel 风控：盘口可用性、depth/rank/history/requote、库存 cap |
| `worker/rewards/polling.rs` | rewards poll loop、盘口读取和进程内盘口历史 |
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
| `scan-rewards-once` | `run_reward_bot_once` | 一次性消费 rewards 控制命令或执行 live 策略 tick |
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
    → reward_bot_service.list_all_reward_candidate_token_ids() → 奖励候选 token
    → orderbook_registry.register_tokens("exec_orders", ...)
    → orderbook_registry.register_tokens("rewards", ...)
    // 通过 OrderbookHttpClient → HTTP POST /orderbook/register 注册到 orderbook 服务
```

此任务替代了原来的 `consume-orderbook-stream` 和 `sync-markets` 任务。Worker 不再直接运行盘口流或市场同步，而是通过 HTTP 告知 orderbook 服务需要订阅哪些 token。

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

### rewards — 奖励策略与控制命令

```
reward_bot_service.claim_next_control_command()
    → worker 执行 queued run_once / cancel_all / reset
    → reward_bot_service.complete_control_command() 或 fail_control_command()

无待处理控制命令时：
    fetch_reward_bot_inputs() // 获取奖励市场 + 盘口
        → prepare_live_cycle()
        → sync managed rewards order trades/statuses
        → LivePolymarketConnector.submit_token_order()
        → reconcile_interval_sec: 读取活跃盘口并对本系统托管订单做成交同步和安全撤单检查
```

Report: `RewardBotRunReport { markets_scanned, books_fetched, plans_built, eligible_plans, simulated_orders, cancelled_orders, filled_orders, reward_accrued }`

约束：worker 是 rewards 策略和控制命令的唯一执行者。API 只把 `run_once` / `cancel_all` / `reset` 写入 `reward_control_commands`；worker 每轮先领取并处理待执行命令，处理到命令时跳过本轮自动 tick。`run_once` 会强制执行一次 live 策略 tick（即使自动开关关闭）；`cancel_all` 调用 Polymarket cancel 并同步本地状态，任一撤单拒绝会让命令失败；`reset` 按 cancel-all 执行且不会清空本地账本，避免产生孤儿实盘订单。服务模式下 `POLYEDGE_WORKER__POLL_REWARD_BOT=true` 会运行与 `poll-reward-bot` CLI 相同的 full tick + fast reconcile loop，`RewardBotConfig.reconcile_interval_sec` 会生效。

自动 tick 只从 Postgres 的 `reward_markets` 读取奖励市场、通过 `OrderbookHttpClient`（HTTP 调用 polyedge-orderbook 服务）读取盘口。每个 tick 只读取候选 market pool（默认至少 100；随 `max_markets` / `max_open_orders` 扩展到 `u16::MAX`），先按配置预过滤奖励市场，再并发读取候选盘口缓存；若本 tick 没有新鲜缓存盘口，不会提交新 post-only 订单。

live 模式会用 `LivePolymarketConnector::submit_token_order()` 提交 post-only GTC token 买单，用 `cancel_order()` 撤销本系统托管订单；未成交 post-only maker 买单不在本地按全局 notional 硬锁资金。live placement 要求目标两腿都有非空盘口，`stale_book_ms=0` 只关闭盘口年龄检查；live reconcile 会读取开放订单活跃 token 的盘口，缺盘口、空盘口、过期盘口、深度不足、bid rank 过高、盘口历史窗口风险或定期 requote 会触发撤单，即使 `enabled=false` 已停止新增报价。Polymarket 返回 post-only 非 live 接受状态（如 `matched` / `delayed`）时会被视为安全违规并立即尝试撤单。worker 会对本系统托管 rewards 订单轮询 Polymarket 关联 trades，按 external trade id 幂等写入 fills、现金、库存和 PnL；买入成交后按配置撤 sibling legs，并提交 `ExitAtMarkup` 或 `FlattenImmediately` sell。独立账户余额/库存全量对账、订单计分查询和奖励结算对账尚未闭环，worker 仍需要独立维护组合风险，因为 CLOB 的 balance/allowance 检查不是跨市场组合风控系统。

### orderbook_stream — 盘口流（已迁移到 orderbook 服务）

盘口流逻辑已迁移到 `polyedge-orderbook` 服务（`apps/orderbook/src/stream.rs`）。Worker 中 `worker/orderbook_stream.rs` 仅保留 `consume-orderbook-stream` CLI 子命令兼容（daemon 模式不再调度）。Worker 通过 `OrderbookHttpClient`（HTTP）读取 orderbook 服务的缓存数据，通过 `register-orderbook-tokens` 任务注册订阅 token。

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
- rewards worker 会通过数据库命令队列接收前端 Run / Cancel / Reset 请求，API 进程不再执行 rewards 策略；仅支持 live 实盘模式，不依赖全局 system mode
- copytrade worker 会通过数据库命令队列接收前端 Run / Analyze / Cancel / Reset 请求，API 进程不再执行跟单任务或抓取跟单输入
- 默认大部分 worker 通过配置开关控制启用/禁用
- Polymarket live 任务需要真实凭证；Deposit Wallet 使用 `POLYEDGE_POLYMARKET__SIGNATURE_TYPE=poly_1271` + `POLYEDGE_POLYMARKET__FUNDER=<deposit_wallet>`，worker 会通过 connector 走 CLOB V2 `POLY_1271` 下单/撤单路径。
- 当前 `cargo check --workspace --tests` 编译通过；worker 生产入口不保留仅测试目标使用的 `RewardExecutionMode` 间接导入，测试目标显式导入该类型。

## 修改检查清单

- [ ] 新增 worker 任务时：(1) 在 `worker/` 中创建文件 (2) 在 `main.rs` 中添加 CLI 子命令 (3) 在 `run_worker_service()` 中添加到主循环
- [ ] 修改 worker 逻辑后检查对应的 application service 是否需要更新
- [ ] 新增 Report 类型时确保使用 `Default` derive 并包含有用的统计字段
- [ ] 运行 `cargo check --workspace --tests`
- [ ] 更新根目录 `AGENTS.md` 中的常用 worker 子命令列表
