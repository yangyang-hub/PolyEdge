# Agent Guidelines

最后更新：2026-06-04

## 维护规则

- **模块文档优先**：修改任何模块前，必须先查阅 `doc/modules/` 下对应的模块文档（索引见 [doc/modules/README.md](./doc/modules/README.md)）；修改后必须同步更新对应文档（顶部日期、关键文件、数据结构、当前状态）。
- 任何改变行为、路由、命令、环境变量、部署方式、依赖、集成状态或已知缺口的改动，都要同步更新本文件。
- 不要把设计文档里的目标能力写成已实现能力。
- 如果本文件、README、页面文案冲突，以本文件为仓库状态快照优先修正。

## 数据获取架构（编码时必须遵守）

### Single Source of Truth: Database + In-Memory Cache

ALL external API data MUST be fetched by background workers and stored in the database
or in-memory cache. Strategies, pages, and API handlers MUST read from these stores — NEVER
fetch directly from external APIs (Polymarket Gamma, CLOB, etc.).

### Market Data Pipeline

| Data | Producer | Source | Store | Interval |
|------|--------|--------|-------|----------|
| General markets | `polyedge-orderbook` market sync | Gamma API `/markets/keyset` | `markets` table (Postgres) | 5 min |
| Reward markets | `polyedge-orderbook` market sync | CLOB API `/rewards/markets/current` | `reward_markets` table (Postgres) | 5 min |
| Order books | `polyedge-orderbook` 服务 | CLOB WebSocket + `/books` batch poll（回退 `/book`） | `InMemoryOrderbookCache`（orderbook 服务进程内，TTL 5 分钟） | WS real-time + 30s full reconcile |

Orderbook 订阅由独立的 `polyedge-orderbook` 服务管理。该服务始终运行 WS + poll stream，维护进程内缓存和 `OrderbookSubscriptionRegistry`，暴露 HTTP API（`GET /orderbook/{token_id}`、`GET /orderbook/stats`、`POST /orderbook/register` 等）。Worker 和 API 通过 `OrderbookHttpClient`（HTTP 调用 orderbook 服务）读取盘口数据，Worker 通过携带 `POLYEDGE_ORDERBOOK__WRITE_TOKEN` 注册 token。`/orderbook/register` 会原子替换对应 source 当前有序 token 集合，避免 DELETE/POST 空窗和同一 source 单调增长；HTTP registry 最多保留 32 个 source，in-memory registry 在写锁内再次原子校验上限；`/orderbook/stats` 返回真实 cache 条目数、registry 来源数和 registry 去重 token 总数。聚合优先级固定为 `rewards_active`、`exec_orders`、`rewards_candidates`、`copytrade`；总量受 `POLYEDGE_ORDERBOOK_STREAM__MAX_TOKENS` 限制。register/ingest/delete 写接口要求共享写 token，未配置时写接口关闭；HTTP ingest 会先校验整批盘口，再批量写入并传播缓存错误。WS 同时消费完整 `book` 快照和 `price_change` 增量；所有缓存写入会先把 bids 按价格降序、asks 按价格升序排序，再保留每侧最多 `POLYEDGE_ORDERBOOK_STREAM__MAX_LEVELS_PER_SIDE` 档深度（默认 100），并拒绝旧 `observed_at` 覆盖更新盘口。poll reconciler 每个周期优先刷新 stale token，随后刷新其余注册 token，使用 CLOB `/books` 批量接口并在失败或遗漏时回退 `/book`，以修复未被发现的 WS 增量丢失；stale threshold 小于等于 0 时只关闭年龄 stale 优先级。

市场和奖励市场由 orderbook 服务同步写入 Postgres，盘口数据由 orderbook 服务流式写入进程内缓存。所有消费者从数据库或 orderbook 服务读取，不直接调用外部 API。

### Why This Architecture Exists

Previously the rewards bot fetched market data directly from Polymarket's CLOB API
every 60 seconds. The enrichment step (fetching `/markets/{condition_id}` for token
data) failed at scale due to rate limiting, causing only ~50 of 500+ markets to survive
the `tokens >= 2` filter. Centralizing API fetching in the designated sync producer
with proper retries solves this and ensures consistent data across all consumers.
The designated sync producer is now the standalone `polyedge-orderbook` service.

### Anti-patterns to Avoid

- ❌ Calling Polymarket APIs directly from API handlers or strategy code
- ❌ Fetching market metadata (questions, tokens, slugs) from external APIs at request time
- ❌ Creating new connector calls outside the designated worker/orderbook sync pipeline
- ❌ Reading market data from Polymarket when it exists in the database
- ❌ Fetching order books directly from CLOB when they exist in the in-memory cache
- ❌ Duplicating data fetching logic across workers, API handlers, and strategies

### Key Data Files

| File | Role |
|------|------|
| `apps/worker/src/worker/market_sync.rs` | 市场同步 CLI 兼容入口；daemon 同步已迁移到 orderbook 服务 |
| `apps/worker/src/worker/orderbook_stream.rs` | Orderbook stream — 仅保留 CLI 子命令兼容，核心逻辑已迁移到 polyedge-orderbook 服务 |
| `apps/orderbook/src/main.rs` | 独立 orderbook 服务入口 — HTTP server + WS stream + token 注册 |
| `apps/orderbook/src/http_api.rs` | Orderbook HTTP API — read/batch/stats/register/ingest、写 token 校验、最优档排序 |
| `crates/connectors/src/polymarket/gamma.rs` | Gamma markets connector — keyset 分页、重复 cursor 防护、market id 去重 |
| `crates/connectors/src/rewards.rs` + `rewards/orderbooks.rs` | Rewards catalog connector + CLOB `/books` batch poll and `/book` fallback |
| `apps/worker/src/worker/rewards.rs` | Rewards bot — executes live strategy ticks and queued run/cancel/reset commands |
| `apps/api/src/handlers/rewards.rs` | Rewards API — reads snapshots/config and enqueues worker control commands |
| `crates/application/src/rewards/service.rs` | RewardBotService — reward markets, snapshots, live order lifecycle, control command queue |
| `crates/application/src/rewards/pagination.rs` | Rewards order pagination query and response metadata |
| `apps/worker/src/worker/rewards/live_sync.rs` | Rewards live managed-order trade/status sync |
| `apps/worker/src/worker/rewards/account_sync.rs` | Rewards external balance and complete position snapshot sync |
| `apps/worker/src/worker/rewards/live_orders.rs` | Rewards live cancel/fill and post-fill exit/flatten intents |
| `apps/worker/src/worker/rewards/live_submission.rs` | Rewards live single-order submit and submission markers |
| `apps/worker/src/worker/rewards/live_pending.rs` | Rewards durable intent submit/recovery workflow |
| `apps/worker/src/worker/rewards/live_risk.rs` | Rewards live placement/cancel risk checks |
| `apps/worker/src/worker/rewards/polling.rs` | Rewards live poll loop, book fetch, in-process book history |
| `apps/worker/src/worker/copytrade.rs` | Copytrade worker — wallet tracking, source trade detection, and queued analyze commands |
| `apps/api/src/handlers/copytrade.rs` | Copytrade API — reads snapshots/config and enqueues worker control commands |
| `crates/application/src/copytrade/service.rs` | CopyTradeService — copytrade config, wallet tracking, source trade detection, and control command queue |
| `crates/application/src/orderbook_cache.rs` | OrderbookCache trait — `get_book`, `set_book`, `set_books`, `entry_count` |
| `crates/application/src/orderbook_registry.rs` | OrderbookSubscriptionRegistry trait — 多来源 token 订阅注册与来源统计 |
| `crates/infrastructure/src/stores/orderbook_cache.rs` | InMemoryOrderbookCache（TTL + 定期清理 + 每侧盘口深度裁剪）；保留 Redis 实现 |
| `crates/infrastructure/src/stores/orderbook_registry.rs` | InMemoryOrderbookSubscriptionRegistry — 来源有序 token 原子替换、确定性优先级聚合、来源与去重总数统计 |
| `migrations/0022_reward_bot_control_commands.sql` | Rewards API-to-worker command queue table |
| `migrations/0023_copytrade_control_commands.sql` | Copytrade API-to-worker command queue table |
| `migrations/0024_reward_markets_active_index.sql` | Reward market active/daily-rate query index |
| `migrations/0025_markets_active_volume_index.sql` | Open/tradable market 24h-volume query index |
| `migrations/0026_reward_control_running_lease_index.sql` | Rewards running control command lease query index |
| `migrations/0028_reward_positions_external_inventory.sql` | Allow complete external rewards account inventory outside the reward catalog |

## 仓库结构

- `doc/`：系统设计、API 契约、鉴权、存储、前后端计划等文档。
- `packages/front/`：`Next.js 16 + React 19 + Tailwind v4 + shadcn/ui` 控制台前端。前端代码规范（目录结构、数据层、文件行数上限、公共代码提取）见 [packages/front/AGENTS.md](./packages/front/AGENTS.md)，写或改前端代码前必须遵守。
- `packages/backend/`：Rust workspace，包含 `api / worker / orderbook / replay` apps，以及 `application / connectors / contracts / domain / infrastructure` crates。后端代码规范（分层架构、`include!` 模块化、文件行数上限、公共代码提取、测试组织）见 [packages/backend/AGENTS.md](./packages/backend/AGENTS.md)，写或改后端 Rust 代码前必须遵守。
- `deploy/`：Docker Compose 部署模板和环境变量示例。
- `scripts/`：构建、部署、冒烟脚本。
- `bin/`：部署镜像复制的预构建后端二进制。

## 当前状态

- 仓库已经不是纯文档仓库：前端控制台、Rust API、worker、迁移、配置和 Docker 部署入口都已具备。
- 前端控制台已有 `dashboard / markets / events / radar / rewards / copy-trading / signals / positions / risk / approvals / replay / settings` 页面。
- 前端数据层统一走 `src/lib/api/*`（读取按领域文件 `markets.ts` / `signals.ts` / `risk.ts`… 基于 `base.ts`，写操作走 `actions.ts`），页面装配在 `src/features/*/loaders` 和 `src/features/*/components`。`src/server/` 目前是空目录（历史遗留）。
- 前端仅支持中文，文案走 `@/lib/i18n/dictionaries` 字典导入。
- 前端不再提供 mock 数据模式；`NEXT_PUBLIC_POLYEDGE_API_BASE_URL` 必须指向 Rust 后端，读写和 SSE 都走真实 `/api/v1/...`。
- 当前控制台会话只保留 `off`，不是生产级真实会话。
- 后端 API 已覆盖 markets、events、news、evidences、signals、orders、trades、positions、pricing、arbitrage、rewards bot、risk、approvals、system、SSE、connector callback 和 orderbook（`GET /api/v1/orderbook/{token_id}`）等主路径。
- `polyedge-worker` 支持 news ingest、news promotion、arbitrage radar、rewards bot live 策略、copytrade 跟单、execution drain、paper reconciliation、Polymarket order/fill/user-event、orderbook token 注册任务。市场同步和 orderbook 订阅已迁移到独立 `polyedge-orderbook` 服务；orderbook 服务启动时先暴露 HTTP `/healthz`，再后台执行 initial/periodic market sync，避免外部 Polymarket API 延迟阻塞容器健康检查；Gamma 与 rewards 目录同步互不阻断，Gamma keyset 与 rewards 分页均具备重复 cursor、末页 sentinel、最大页数和去重保护，rewards 空目录或详情补全不完整时保留上一版目录；orderbook WS + poll stream 遵守 `POLYEDGE_ORDERBOOK_STREAM__RESTART_INTERVAL_SECS`，消费 `book` + `price_change`，每个 poll 周期对全部注册 token 做批量快照恢复，内部写接口要求 `POLYEDGE_ORDERBOOK__WRITE_TOKEN`，缓存统一排序后裁剪最优档位并拒绝旧快照覆盖。
- 套利雷达是只读链路：发现、记录、校验、分析、展示和 SSE 推送已具备，但不会创建 execution request 或订单。
- Rewards bot 仅支持 `live` 实盘模式（`execution_mode` 字段已移除，旧配置键读取时忽略）。它只使用 `reward_markets` 表作为奖励市场来源，并通过 `OrderbookHttpClient` 读取候选和活跃订单/持仓盘口，生成 YES/NO post-only 双边买单计划。worker 使用 `LivePolymarketConnector` 提交 post-only GTC token 买单、FAK flatten 卖单并撤销本系统托管订单；confirmed fill 按 external trade id + external order id 幂等入账，买入 fill 与退出 intent 同事务落库，提交结果未知、待最终对账或外部订单 404 会暂停新增买单但继续同步、撤单和卖出退出。full tick 和 fast reconcile 会先同步 managed orders，再仅在本轮没有新增 confirmed fill 时同步 CLOB balance 和 Data API 完整 positions；成功 positions 快照原子替换该账户全部持仓，失败时保留上一版，避免 Data API 最终一致性导致重复记账。即使 `enabled=false` 且没有开放订单，worker 仍会刷新外部账户状态。API 只从数据库读取 rewards snapshot，不直接请求 Polymarket；`orders` 与 `orders_page` 都描述本地 managed orders。账户范围外开放订单、订单计分查询和奖励结算对账仍是缺口。
- Rewards live 会在提交旧 intent 前先执行当前盘口/资格撤单检查；任一提交结果未知、待最终对账或外部订单 404 会暂停全部新增买单，但继续同步、撤单和卖出退出。CLOB `post_order` 只要返回订单 ID 就保留为 accepted 供后续成交/状态对账，包含 `unmatched` / `canceled` / 未知状态；managed order 的后续 upsert 会同步更新实际提交价格和数量，post-only exit 被取消后的重试仍保持 post-only。
- Polymarket connector 已迁移到 CLOB V2 Rust crate：`packages/backend/Cargo.toml` 保留 dependency key `polymarket-client-sdk`，实际指向 `polymarket_client_sdk_v2`；live CLOB 签名类型支持 `eoa`、`proxy`、`gnosis_safe`、`poly_1271`，其中 `poly_1271` 用于已有 Deposit Wallet（`FUNDER` 填 deposit wallet 地址），下单前会调用 CLOB balance allowance update；已支持 collateral balance 查询和开放订单全量分页，下单价格当前收敛到最多 2 位小数，同一 trade 内重复 maker entry 会聚合后入账。
- 聪明钱跟单（copy-trading）已精简为只读跟踪+分析子系统：跟踪多个 Polymarket 钱包地址（`TrackedWallet`）、通过 Polymarket Data API（`data-api.polymarket.com`，通过 `PolymarketDataApiConnector`）检测钱包新成交、钱包分析统计（胜率/ROI/成交量）、`Analyze` 与钱包管理前端 UI。模拟引擎（模拟资金账本、仓位、订单、PnL）已移除，跟单不会下单。未处理 source trades 按时间排序并记录。API 服务不执行 copytrade 跟单循环或钱包分析，前端操作会写入数据库控制命令，由 worker 领取执行；`POLYEDGE_COPYTRADE__ENABLED=true` 启用 worker 轮询。
- Polymarket 运行时不再提供 mock mode；市场列表走 Gamma 实时数据，私有订单/成交任务需要真实凭证、真实账户、小额演练和运维 runbook。
- 数据库迁移目前到 `0028_reward_positions_external_inventory.sql`。

## 主要缺口

- 生产级真实会话体系未完成；当前前端只保留 `off` 模式。
- 内部 JWT 签名 helper 已有代码路径，但当前不会从 `off` 签发可信令牌。
- `signals / risk / events` SSE 仍是 snapshot-backed stream；`arbitrage` 已是 outbox-backed 增量流，但尚未统一到全资源事件总线。
- 新闻源可以抓取、去重、提升为 events/evidences，但尚未自动生成 signals。
- Rewards live maker 已接入真实 post-only 买单提交、撤单、本系统托管订单成交同步、成交后现金/库存/PnL 更新、sibling leg 撤单和 exit/flatten sell 下单；worker 在 managed order 同步后刷新 CLOB 余额和 Data API 完整持仓快照，API 只从数据库读取且不再需要 Polymarket 凭证。仍未完成账户范围外开放订单同步、订单计分查询或奖励结算对账。实盘策略仍应沿用“未成交 maker 买单不硬锁全局 USDC、成交后才更新现金/库存并撤超额挂单”的资金模型。
- Polymarket live 链路已具备 CLOB V2 SDK、认证、token buy/sell 下单和撤单能力，并可配置已有 Deposit Wallet 的 `poly_1271` 签名；仍未实现 relayer 建钱包、pUSD 入金/approval 等 Deposit Wallet 生命周期管理，且仍需真实资金链路小额验证。

## 运行命令

前端：

```bash
cd packages/front
yarn dev
yarn lint
yarn build
```

后端：

```bash
cd packages/backend
cargo check --workspace
cargo test --workspace
cargo run -p polyedge-api
cargo run -p polyedge-worker
cargo run -p polyedge-orderbook
```

常用 worker 子命令：

```bash
cargo run -p polyedge-worker -- ingest-news-once
cargo run -p polyedge-worker -- poll-news
cargo run -p polyedge-worker -- promote-news-events
cargo run -p polyedge-worker -- scan-arbitrage-once
cargo run -p polyedge-worker -- poll-arbitrage-radar
cargo run -p polyedge-worker -- analyze-arbitrage-opportunities
cargo run -p polyedge-worker -- scan-rewards-once
cargo run -p polyedge-worker -- poll-reward-bot
cargo run -p polyedge-worker -- drain-execution-queue
cargo run -p polyedge-worker -- poll-polymarket-order-statuses
cargo run -p polyedge-worker -- reconcile-polymarket-fills
cargo run -p polyedge-worker -- consume-polymarket-user-events
cargo run -p polyedge-worker -- scan-copytrade-once
cargo run -p polyedge-worker -- poll-copytrade
cargo run -p polyedge-worker -- analyze-wallets-once
```

套利雷达冒烟：

```bash
./scripts/smoke-arbitrage-radar.sh
```

## 配置要点

- 后端默认监听 `0.0.0.0:38001`。
- 默认 runtime mode 是 `live_auto`。
- Polymarket connector 没有 mock mode；未配置真实账户/私钥时，不要开启 Polymarket 私有订单、成交或用户 websocket worker 任务。
- `POLYEDGE_POLYMARKET__SIGNATURE_TYPE` 可选 `eoa`、`proxy`、`gnosis_safe`、`poly_1271`；新 Deposit Wallet 使用 `poly_1271`，并将 `POLYEDGE_POLYMARKET__FUNDER` 设置为 deposit wallet 地址。
- 默认 arbitrage radar 和 news ingestion 是 disabled。
- 默认 rewards bot worker 是 disabled；前端 `/rewards` 的 Run / Cancel / Reset 只会入队命令，worker 需要同时设置 `POLYEDGE_REWARDS__ENABLED=true` 和 `POLYEDGE_WORKER__POLL_REWARD_BOT=true` 才会领取并执行。要产生新挂单和 live post-only 下单，还需要配置真实 Polymarket 凭证并确保 `polyedge-orderbook` 服务正在运行并同步了 reward 市场数据。
- `deploy/.env*.example` 环境变量模板已为每个变量提供用途说明；`deploy/.env.polymarket.example` 提供 Polymarket CLOB V2 live、Proxy/Gnosis Safe、Deposit Wallet（`poly_1271`）和 Rewards live worker 配置示例，真实凭证默认注释。建议私钥只放 `deploy/.env.worker`；API 不再需要 Polymarket 凭证，余额和持仓由 worker 同步到数据库后 API 从数据库读取。
- Rewards bot 的 `max_markets=0`、`max_open_orders=0` 或 `quote_size_usd=0` 都表示不再新挂单；不是无限制。
- Rewards bot 未成交 post-only maker 买单不在本地按全局 notional 硬锁资金；`stale_book_ms` 默认 45000，`stale_book_ms=0` 只关闭盘口年龄检查，仍要求盘口存在且非空，开放 live 订单缺盘口会被撤单。
- `POLYEDGE_ORDERBOOK_STREAM__MAX_TOKENS` 默认 3000；调高会增加 orderbook WS/poll 内存占用，调低会减少 rewards 候选盘口覆盖。活跃 rewards token 优先于 execution 和候选 token。`POLYEDGE_ORDERBOOK_STREAM__MAX_LEVELS_PER_SIDE` 默认 100，用于限制进程内缓存和 HTTP ingest 每个 token 的 bids/asks 保留深度；写入时先排序再裁剪，保留最优档位。poll 每周期会刷新全部注册 token；`POLYEDGE_ORDERBOOK_STREAM__STALE_THRESHOLD_MS=0` 只关闭年龄 stale 优先级。
- 默认跟单 worker 是 disabled；前端 `/copy-trading` 的 Run / Analyze / Cancel / Reset 只会入队命令，worker 需要设置 `POLYEDGE_COPYTRADE__ENABLED=true` + `POLYEDGE_WORKER__POLL_COPYTRADE=true` 才会领取并执行；`POLYEDGE_WORKER__ANALYZE_WALLETS=true` 仍用于独立钱包分析循环。
- `POLYEDGE_POSTGRES__URL` / `POLYEDGE_REDIS__URL` 为空时，本地可能走内存路径，无法验证多进程共享状态和持久化 outbox。
- Postgres rewards live worker 使用 advisory lease 串行化实盘周期，因此 `POLYEDGE_POSTGRES__MAX_CONNECTIONS` 必须至少为 2（默认 20）。
- `POLYEDGE_ORDERBOOK__SERVICE_URL` 默认 `http://localhost:38002`；orderbook 和 API/worker 部署在同一服务器时无需修改，跨服务器部署时设置为 orderbook 服务器的实际地址（如 `http://192.168.31.10:38002`）。`POLYEDGE_ORDERBOOK__WRITE_TOKEN` 是 orderbook/worker 部署必填共享密钥，分别放在 `deploy/.env.orderbook` 与 `deploy/.env.worker` 且值必须一致，不放入公共 `.env` 或 API/front 环境；`OrderbookHttpClient` 使用 5 秒连接超时和 30 秒请求超时。
- `POLYEDGE_ARBITRAGE__BOOK_SOURCE=polymarket` 会请求真实 Polymarket CLOB `/book`；live 冒烟必须使用真实 Polymarket refs。
- Docker 部署中的 `polyedge-worker` 后台任务按代码默认值均为 `false`；需要运行新闻、套利、rewards 或 copytrade 时必须在 `deploy/.env.worker` 显式设为 `true`。市场同步和 orderbook 订阅由独立 `polyedge-orderbook` 服务管理，不需要在 worker 中启用。

## Docker 部署

后端镜像从 `bin/` 目录复制预构建二进制；服务器部署不编译 Rust。构建机/CI 先执行：

```bash
./scripts/build-backend-bin.sh
git add bin/polyedge-api bin/polyedge-worker bin/polyedge-orderbook
```

跨服务器部署时只需构建目标服务器需要的二进制，例如 orderbook 服务器只需 `polyedge-orderbook`：

```bash
POLYEDGE_BACKEND_BINARY=polyedge-orderbook ./scripts/build-backend-bin.sh
```

服务器部署入口：

```bash
cp deploy/.env.example deploy/.env
# 编辑 deploy/.env，填入外部 PostgreSQL URL；在 deploy/.env.orderbook 和 deploy/.env.worker 设置相同的 POLYEDGE_ORDERBOOK__WRITE_TOKEN；纯内网默认 POLYEDGE_AUTH__DISABLED=true，不需要 step-up code
# 各服务专属配置见 deploy/.env.{api,orderbook,worker,front}.example
# Polymarket live / Deposit Wallet 示例见 deploy/.env.polymarket.example
# 跨服务器部署时设置 POLYEDGE_ORDERBOOK__SERVICE_URL 指向 orderbook 服务器地址
./scripts/deploy.sh all
```

`deploy/docker-compose.yml` 编排（各服务无启动依赖，可独立部署在不同服务器）：

- `polyedge-orderbook`（独立 orderbook 服务，WS + poll + HTTP API，使用 `deploy/orderbook.Dockerfile`）
- `polyedge-api`（通过 `POLYEDGE_ORDERBOOK__SERVICE_URL` 读取盘口，使用 `deploy/api.Dockerfile`）
- `polyedge-worker`（独立 worker 服务，使用 `deploy/worker.Dockerfile`）
- `polyedge-front`

`scripts/deploy.sh` 每个服务独立部署，互不依赖：

- 不传参数或 `auto`：拉取最新代码，per-service 检测二进制 hash 变化，只 rebuild 变化的镜像并 restart 变化或未运行的服务。
- `all`：重建所有可用镜像并重启所有可用服务。
- `api worker`：重建 api 和 worker 镜像并重启 API 与 worker。
- `api`：只重建 api 镜像并重启 API。
- `worker`：只重建 worker 镜像并重启 worker。
- `orderbook`（或 `ob`）：重建 orderbook 镜像并重启 orderbook 服务。
- `front`：只重建前端镜像并重启前端。
- 支持组合，例如 `api front` 或 `api,worker`。
- `POLYEDGE_SKIP_SERVICES=orderbook` 排除特定服务，适合同一服务器只部署部分服务的场景。

部署脚本默认使用 `/tmp/polyedge-deploy.lock` 防止 cron/CI 重叠执行，默认 `COMPOSE_PARALLEL_LIMIT=1` 串行构建镜像。Auto 模式 per-service 独立检测：api、worker、orderbook、front 各自独立镜像（每个二进制变化只触发对应镜像 rebuild），容器未运行但 hash 未变时直接启动已有镜像。前端 `yarn build` 前会读取 `deploy/.env.front` 并把 `NEXT_PUBLIC_*` 写入静态 bundle。Compose 构建上下文已收窄：后端只上传 `bin/`，前端只上传 `packages/front/`，避免扫描本地 `packages/backend/target`、`node_modules`、`.next` 等大目录。跨服务器部署时每台服务器只需本地存在的二进制，脚本只检查目标服务所需的文件。

## 关键入口

前端：

- `packages/front/src/lib/api/base.ts`
- `packages/front/src/lib/api/actions.ts`
- `packages/front/src/lib/api/copytrade.ts`
- `packages/front/src/app/api/stream/[channel]/route.ts`
- `packages/front/src/proxy.ts`
- `packages/front/src/lib/i18n/*`
- `packages/front/src/features/radar/*`
- `packages/front/src/features/copytrade/*`

后端：

- `packages/backend/apps/api/src/lib.rs`
- `packages/backend/apps/api/src/handlers/rewards.rs`
- `packages/backend/apps/api/src/handlers/copytrade.rs`
- `packages/backend/apps/worker/src/main.rs`
- `packages/backend/apps/orderbook/src/main.rs`
- `packages/backend/apps/worker/src/worker/rewards.rs`
- `packages/backend/apps/worker/src/worker/rewards/account_sync.rs`
- `packages/backend/apps/worker/src/worker/copytrade.rs`
- `packages/backend/crates/application/src/rewards/service.rs`
- `packages/backend/crates/application/src/rewards/pagination.rs`
- `packages/backend/crates/application/src/copytrade.rs`
- `packages/backend/crates/application/src/copytrade/service.rs`
- `packages/backend/crates/connectors/src/polymarket/data_api.rs`
- `packages/backend/crates/connectors/src/polymarket/live.rs` — `LivePolymarketConnector`：认证、下单、撤单、查询余额和挂单
- `packages/backend/crates/connectors/src/polymarket/models.rs` — Polymarket connector 类型定义（`PolymarketOpenOrder`、`PolymarketTokenOrderSide` 等）
- `packages/backend/crates/connectors/src/orderbook.rs`
- `packages/backend/crates/connectors/src/rewards.rs`
- `packages/backend/crates/infrastructure/src/stores/copytrade.rs`
- `packages/backend/crates/infrastructure/src/settings.rs`
- `packages/backend/crates/application/src/orderbook_registry.rs`
- `packages/backend/crates/infrastructure/src/stores/orderbook_registry.rs`
- `packages/backend/migrations/0028_reward_positions_external_inventory.sql`

部署：

- `deploy/orderbook.Dockerfile`
- `deploy/api.Dockerfile`
- `deploy/worker.Dockerfile`
- `packages/front/Dockerfile`
- `deploy/docker-compose.yml`
- `deploy/.env.example`
- `scripts/deploy.sh`
- `scripts/build-backend-bin.sh`

## 更新检查

改代码后至少检查：

- 是否新增、删除或重命名页面、API、worker 子命令、迁移或部署服务。
- 是否修改环境变量、默认端口、运行模式、鉴权方式或依赖。
- 是否改变前后端贯通状态、SSE 状态、Polymarket live 状态或部署命令。
- 顶部日期是否需要更新。
