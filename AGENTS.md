# Agent Guidelines

最后更新：2026-06-02

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

| Data | Worker | Source | Store | Interval |
|------|--------|--------|-------|----------|
| General markets | `sync-markets` | Gamma API `/markets/keyset` | `markets` table (Postgres) | 5 min |
| Reward markets | `sync-markets` | CLOB API `/rewards/markets/current` | `reward_markets` table (Postgres) | 5 min |
| Order books | `polyedge-orderbook` 服务 | CLOB WebSocket + `/book` poll | `InMemoryOrderbookCache`（orderbook 服务进程内，TTL 5 分钟） | WS real-time + 30s poll reconcile |

Orderbook 订阅由独立的 `polyedge-orderbook` 服务管理。该服务运行 WS + poll stream，维护进程内缓存和 `OrderbookSubscriptionRegistry`，暴露 HTTP API（`GET /orderbook/{token_id}`、`POST /orderbook/register` 等）。Worker 和 API 通过 `OrderbookHttpClient`（HTTP 调用 orderbook 服务）读取盘口数据和注册 token。

市场和奖励市场由 orderbook 服务同步写入 Postgres，盘口数据由 orderbook 服务流式写入进程内缓存。所有消费者从数据库或 orderbook 服务读取，不直接调用外部 API。

### Why This Architecture Exists

Previously the rewards bot fetched market data directly from Polymarket's CLOB API
every 60 seconds. The enrichment step (fetching `/markets/{condition_id}` for token
data) failed at scale due to rate limiting, causing only ~50 of 500+ markets to survive
the `tokens >= 2` filter. Centralizing API fetching in the sync worker with proper
retries solves this and ensures consistent data across all consumers.

### Anti-patterns to Avoid

- ❌ Calling Polymarket APIs directly from API handlers or strategy code
- ❌ Fetching market metadata (questions, tokens, slugs) from external APIs at request time
- ❌ Creating new connector calls outside the worker sync pipeline
- ❌ Reading market data from Polymarket when it exists in the database
- ❌ Fetching order books directly from CLOB when they exist in the in-memory cache
- ❌ Duplicating data fetching logic across workers, API handlers, and strategies

### Key Data Files

| File | Role |
|------|------|
| `apps/worker/src/worker/market_sync.rs` | Sync worker — fetches markets from Polymarket, writes to Postgres |
| `apps/worker/src/worker/orderbook_stream.rs` | Orderbook stream — 仅保留 CLI 子命令兼容，核心逻辑已迁移到 polyedge-orderbook 服务 |
| `apps/orderbook/src/main.rs` | 独立 orderbook 服务入口 — HTTP server + WS stream + token 注册 |
| `apps/worker/src/worker/rewards.rs` | Rewards bot — executes live strategy ticks and queued run/cancel/reset commands |
| `apps/api/src/handlers/rewards.rs` | Rewards API — reads snapshots/config and enqueues worker control commands |
| `crates/application/src/rewards/service.rs` | RewardBotService — reward markets, snapshots, live order lifecycle, control command queue |
| `crates/application/src/rewards/pagination.rs` | Rewards order pagination query and response metadata |
| `apps/worker/src/worker/rewards/live_sync.rs` | Rewards live managed-order trade/status sync |
| `apps/worker/src/worker/rewards/live_orders.rs` | Rewards live order submit/cancel/fill and post-fill exit/flatten |
| `apps/worker/src/worker/rewards/live_risk.rs` | Rewards live placement/cancel risk checks |
| `apps/worker/src/worker/rewards/polling.rs` | Rewards live poll loop, book fetch, in-process book history |
| `apps/worker/src/worker/copytrade.rs` | Copytrade worker — executes copy cycles and queued run/analyze/cancel/reset commands |
| `apps/api/src/handlers/copytrade.rs` | Copytrade API — reads snapshots/config and enqueues worker control commands |
| `crates/application/src/copytrade/service.rs` | CopyTradeService — copytrade config/snapshot/simulation and control command queue |
| `crates/application/src/orderbook_cache.rs` | OrderbookCache trait — `get_book`, `set_book`, `set_books` |
| `crates/application/src/orderbook_registry.rs` | OrderbookSubscriptionRegistry trait — 多来源 token 订阅注册 |
| `crates/infrastructure/src/stores/orderbook_cache.rs` | InMemoryOrderbookCache（TTL + 定期清理）；保留 Redis 实现 |
| `crates/infrastructure/src/stores/orderbook_registry.rs` | InMemoryOrderbookSubscriptionRegistry — 基于内存的订阅注册中心实现 |
| `migrations/0022_reward_bot_control_commands.sql` | Rewards API-to-worker command queue table |
| `migrations/0023_copytrade_control_commands.sql` | Copytrade API-to-worker command queue table |

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
- 前端不再提供 mock 数据模式；`POLYEDGE_API_BASE_URL` 必须指向 Rust 后端，读写和 SSE 都走真实 `/api/v1/...`。
- 当前控制台会话只保留 `off`，不是生产级真实会话。
- 后端 API 已覆盖 markets、events、news、evidences、signals、orders、trades、positions、pricing、arbitrage、rewards bot、risk、approvals、system、SSE、connector callback 和 orderbook（`GET /api/v1/orderbook/{token_id}`）等主路径。
- `polyedge-worker` 支持 news ingest、news promotion、arbitrage radar、rewards bot live 策略、copytrade 跟单、execution drain、paper reconciliation、Polymarket order/fill/user-event、orderbook token 注册任务。市场同步和 orderbook 订阅已迁移到独立 `polyedge-orderbook` 服务；orderbook 服务启动时先暴露 HTTP `/healthz`，再后台执行 initial/periodic market sync，避免外部 Polymarket API 延迟阻塞容器健康检查。
- 套利雷达是只读链路：发现、记录、校验、分析、展示和 SSE 推送已具备，但不会创建 execution request 或订单。
- Rewards bot 仅支持 `live` 实盘模式（`execution_mode` 字段保留用于向后兼容，始终视为 `live`）。它只使用独立的 `reward_markets` 表作为奖励市场来源，先按 rewards 配置预过滤候选市场，再通过 `OrderbookHttpClient`（HTTP 调用 polyedge-orderbook 服务）并发读取候选盘口、生成当前候选快照的 YES/NO post-only 双边买单计划。worker 通过 `LivePolymarketConnector::submit_token_order()` 提交 post-only GTC token 买单，并通过 `cancel_order()` 撤销本系统托管订单；未成交 maker 买单不在本地按全局 notional 硬锁同一笔 USDC，可跨不同市场同时报价。新挂单要求目标两腿都有非空盘口，reconcile 会在开放订单盘口缺失、空盘口、过期、深度/排名/盘口历史风险或定期 requote 触发时撤单；Polymarket 返回 post-only 非 live 接受状态时会立即尝试撤单。worker 会对本系统托管 rewards 订单轮询 Polymarket 订单关联成交，按 external trade id 幂等写入本地 fills、现金、库存和 PnL；买入成交后按配置撤 sibling legs 并执行 exit-at-markup 或 flatten sell。Reset 不清空本地账本，只按 cancel-all 先撤托管实盘订单，任一撤单被拒绝则命令失败。独立账户余额/库存全量对账、订单计分查询和奖励结算对账仍是缺口。API 服务不执行 rewards 策略或任务，前端 Run / Cancel / Reset 会写入数据库控制命令，由 worker 领取执行；`/api/v1/rewards-bot` 的 managed orders 使用后端分页并返回 `orders_page` 元数据。
- Polymarket connector 已迁移到 CLOB V2 Rust crate：`packages/backend/Cargo.toml` 保留 dependency key `polymarket-client-sdk`，实际指向 `polymarket_client_sdk_v2`；live CLOB 签名类型支持 `eoa`、`proxy`、`gnosis_safe`、`poly_1271`，其中 `poly_1271` 用于已有 Deposit Wallet（`FUNDER` 填 deposit wallet 地址），下单前会调用 CLOB balance allowance update。
- 聪明钱跟单（copy-trading）已具备完整子系统：跟踪多个 Polymarket 钱包地址（`TrackedWallet`）、通过 Polymarket Data API（`data-api.polymarket.com`，通过 `PolymarketDataApiConnector`）检测钱包新成交、四种跟单仓位模式（`FixedUsd`/`ProportionalToSource`/`CapitalRatio`/`MirrorPortfolioWeight`）、钱包分析统计（胜率/ROI/成交量）、per-wallet/per-market/total 敞口+单日亏损+冷却+滑点风控、确定性模拟引擎（模拟资金账本：capital/available/reserved/realized_pnl）、`Run/Analyze/Cancel/Reset` 与账户资金设置前端 UI；`mode=live` 已结构化支持但未接入真实下单（记录警告回退模拟）。API 服务不执行 copytrade 跟单循环、钱包分析、撤单或重置，前端操作会写入数据库控制命令，由 worker 领取执行；`POLYEDGE_COPYTRADE__ENABLED=true` 启用 worker 轮询。
- Polymarket 运行时不再提供 mock mode；市场列表走 Gamma 实时数据，私有订单/成交任务需要真实凭证、真实账户、小额演练和运维 runbook。
- 数据库迁移目前到 `0023_copytrade_control_commands.sql`。

## 主要缺口

- 生产级真实会话体系未完成；当前前端只保留 `off` 模式。
- 内部 JWT 签名 helper 已有代码路径，但当前不会从 `off` 签发可信令牌。
- `signals / risk / events` SSE 仍是 snapshot-backed stream；`arbitrage` 已是 outbox-backed 增量流，但尚未统一到全资源事件总线。
- 新闻源可以抓取、去重、提升为 events/evidences，但尚未自动生成 signals。
- Rewards live maker 已接入真实 post-only 买单提交、撤单、本系统托管订单成交同步、成交后现金/库存/PnL 更新、sibling leg 撤单和 exit/flatten sell 下单；仍未完成独立账户余额/库存全量对账、订单计分查询或奖励结算对账。实盘策略仍应沿用“未成交 maker 买单不硬锁全局 USDC、成交后才更新现金/库存并撤超额挂单”的资金模型。
- Polymarket live 链路已具备 CLOB V2 SDK、认证、token buy/sell 下单和撤单能力，并可配置已有 Deposit Wallet 的 `poly_1271` 签名；仍未实现 relayer 建钱包、pUSD 入金/approval 等 Deposit Wallet 生命周期管理，且仍需真实资金链路小额验证。

## 运行命令

前端：

```bash
cd packages/front
pnpm dev
pnpm lint
pnpm build
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
- 默认 runtime mode 是 `manual_confirm`。
- Polymarket connector 没有 mock mode；未配置真实账户/私钥时，不要开启 Polymarket 私有订单、成交或用户 websocket worker 任务。
- `POLYEDGE_POLYMARKET__SIGNATURE_TYPE` 可选 `eoa`、`proxy`、`gnosis_safe`、`poly_1271`；新 Deposit Wallet 使用 `poly_1271`，并将 `POLYEDGE_POLYMARKET__FUNDER` 设置为 deposit wallet 地址。
- 默认 arbitrage radar 和 news ingestion 是 disabled。
- 默认 rewards bot worker 是 disabled；前端 `/rewards` 的 Run / Cancel / Reset 只会入队命令，worker 需要同时设置 `POLYEDGE_REWARDS__ENABLED=true` 和 `POLYEDGE_WORKER__POLL_REWARD_BOT=true` 才会领取并执行。要产生新挂单和 live post-only 下单，还需要配置真实 Polymarket 凭证并确保 `polyedge-orderbook` 服务正在运行并同步了 reward 市场数据。
- `deploy/.env*.example` 环境变量模板已为每个变量提供用途说明；`deploy/.env.polymarket.example` 提供 Polymarket CLOB V2 live、Proxy/Gnosis Safe、Deposit Wallet（`poly_1271`）和 Rewards live worker 配置示例，真实凭证默认注释，建议私钥只放 `deploy/.env.worker`。
- Rewards bot 的 `max_markets=0`、`max_open_orders=0` 或 `quote_size_usd=0` 都表示不再新挂单；不是无限制。
- Rewards bot 未成交 post-only maker 买单不在本地按全局 notional 硬锁资金；`stale_book_ms=0` 只关闭盘口年龄检查，仍要求盘口存在且非空，开放 live 订单缺盘口会被撤单。
- 默认跟单 worker 是 disabled；前端 `/copy-trading` 的 Run / Analyze / Cancel / Reset 只会入队命令，worker 需要设置 `POLYEDGE_COPYTRADE__ENABLED=true` + `POLYEDGE_WORKER__POLL_COPYTRADE=true` 才会领取并执行；`POLYEDGE_WORKER__ANALYZE_WALLETS=true` 仍用于独立钱包分析循环。
- `POLYEDGE_POSTGRES__URL` / `POLYEDGE_REDIS__URL` 为空时，本地可能走内存路径，无法验证多进程共享状态和持久化 outbox。
- `POLYEDGE_ARBITRAGE__BOOK_SOURCE=polymarket` 会请求真实 Polymarket CLOB `/book`；live 冒烟必须使用真实 Polymarket refs。
- Docker Compose 部署中的 `polyedge-worker` 会把所有 `POLYEDGE_WORKER__...` 后台任务默认覆盖为 `false`；需要运行新闻、套利、rewards 或 copytrade 时必须在 `deploy/.env.worker` 显式设为 `true`。市场同步和 orderbook 订阅由独立 `polyedge-orderbook` 服务管理，不需要在 worker 中启用。

## Docker 部署

后端镜像从 `bin/polyedge-api`、`bin/polyedge-worker` 和 `bin/polyedge-orderbook` 复制预构建二进制；服务器部署不编译 Rust。构建机/CI 先执行：

```bash
./scripts/build-backend-bin.sh
git add bin/polyedge-api bin/polyedge-worker bin/polyedge-orderbook
```

服务器部署入口：

```bash
cp deploy/.env.example deploy/.env
# 编辑 deploy/.env，填入外部 PostgreSQL URL 和控制台 step-up code
# 各服务专属配置见 deploy/.env.{api,orderbook,worker,front}.example
# Polymarket live / Deposit Wallet 示例见 deploy/.env.polymarket.example
./scripts/deploy.sh all
```

`deploy/docker-compose.yml` 编排：

- `polyedge-orderbook`（独立 orderbook 服务，WS + poll + HTTP API）
- `polyedge-api`（依赖 orderbook 健康后启动，并通过 `POLYEDGE_ORDERBOOK__SERVICE_URL` 读取盘口）
- `polyedge-worker`（依赖 orderbook 服务健康）
- `polyedge-front`

`scripts/deploy.sh` 只接受简单目标参数：

- 不传参数或 `auto`：拉取最新代码，只在后端二进制或前端文件 hash 变化时 rebuild；后端容器未运行但 hash 未变时按 orderbook → API → Worker 顺序启动已有镜像。
- `all`：重建后端和前端镜像，并按 orderbook → API → Worker 顺序重启后端，同时重启 front。
- `api worker`：重建后端镜像，并重启 API 与 worker。
- `api`：只重建后端镜像并重启 API。
- `worker`：只重建后端镜像并重启 worker。
- `orderbook`（或 `ob`）：重建后端镜像并重启 orderbook 服务。
- `front`：只重建前端镜像并重启前端。
- 支持组合，例如 `api front` 或 `api,worker`。

部署脚本默认使用 `/tmp/polyedge-deploy.lock` 防止 cron/CI 重叠执行，默认 `COMPOSE_PARALLEL_LIMIT=1` 串行构建镜像；Auto 模式只有后端二进制（api / worker / orderbook）或前端文件 hash 改变时才 rebuild，后端容器未运行但 hash 未变时按 orderbook → API → Worker 顺序启动已有镜像。Compose 构建上下文已收窄：后端只上传 `bin/`，前端只上传 `packages/front/`，避免扫描本地 `packages/backend/target`、`node_modules`、`.next` 等大目录。

默认部署模板仍沿用本地 internal dev-auth 模式，只适合原型/内网共享环境；生产前需要真实会话体系、签名 internal JWT、key rotation 和撤销策略。

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
- `packages/backend/apps/worker/src/worker/copytrade.rs`
- `packages/backend/crates/application/src/rewards/service.rs`
- `packages/backend/crates/application/src/rewards/pagination.rs`
- `packages/backend/crates/application/src/copytrade.rs`
- `packages/backend/crates/application/src/copytrade/service.rs`
- `packages/backend/crates/connectors/src/polymarket/data_api.rs`
- `packages/backend/crates/connectors/src/orderbook.rs`
- `packages/backend/crates/infrastructure/src/stores/copytrade.rs`
- `packages/backend/crates/infrastructure/src/settings.rs`
- `packages/backend/crates/application/src/orderbook_registry.rs`
- `packages/backend/crates/infrastructure/src/stores/orderbook_registry.rs`
- `packages/backend/migrations/0023_copytrade_control_commands.sql`

部署：

- `packages/backend/Dockerfile`
- `deploy/backend.Dockerfile`
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
