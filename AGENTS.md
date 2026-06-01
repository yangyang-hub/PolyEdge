# Agent Guidelines

最后更新：2026-06-01

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
| Order books | `orderbook-stream` | CLOB WebSocket + `/book` poll | `InMemoryOrderbookCache`（进程内，TTL 5 分钟） | WS real-time + 30s poll reconcile |

All three are written by workers. All consumers read from the store, never from the API.

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
| `apps/worker/src/worker/orderbook_stream.rs` | Orderbook stream — WS + poll, writes to InMemoryOrderbookCache, 动态 token 刷新 |
| `apps/worker/src/worker/rewards.rs` | Rewards bot — executes simulation ticks and queued run/cancel/reset commands |
| `apps/api/src/handlers/rewards.rs` | Rewards API — reads snapshots/config and enqueues worker control commands |
| `crates/application/src/rewards/service.rs` | RewardBotService — reward markets, snapshots, simulation persistence, control command queue |
| `apps/worker/src/worker/copytrade.rs` | Copytrade worker — executes copy cycles and queued run/analyze/cancel/reset commands |
| `apps/api/src/handlers/copytrade.rs` | Copytrade API — reads snapshots/config and enqueues worker control commands |
| `crates/application/src/copytrade/service.rs` | CopyTradeService — copytrade config/snapshot/simulation and control command queue |
| `crates/application/src/orderbook_cache.rs` | OrderbookCache trait — `get_book`, `set_book`, `set_books` |
| `crates/infrastructure/src/stores/orderbook_cache.rs` | InMemoryOrderbookCache（TTL + 定期清理）；保留 Redis 实现 |
| `migrations/0022_reward_bot_control_commands.sql` | Rewards API-to-worker command queue table |
| `migrations/0023_copytrade_control_commands.sql` | Copytrade API-to-worker command queue table |

## 仓库结构

- `doc/`：系统设计、API 契约、鉴权、存储、前后端计划等文档。
- `packages/front/`：`Next.js 16 + React 19 + Tailwind v4 + shadcn/ui` 控制台前端。前端代码规范（目录结构、数据层、文件行数上限、公共代码提取）见 [packages/front/AGENTS.md](./packages/front/AGENTS.md)，写或改前端代码前必须遵守。
- `packages/backend/`：Rust workspace，包含 `api / worker / replay` apps，以及 `application / connectors / contracts / domain / infrastructure` crates。后端代码规范（分层架构、`include!` 模块化、文件行数上限、公共代码提取、测试组织）见 [packages/backend/AGENTS.md](./packages/backend/AGENTS.md)，写或改后端 Rust 代码前必须遵守。
- `deploy/`：Docker Compose 部署模板和环境变量示例。
- `scripts/`：构建、部署、冒烟脚本。
- `bin/`：部署镜像复制的预构建后端二进制。

## 当前状态

- 仓库已经不是纯文档仓库：前端控制台、Rust API、worker、迁移、配置和 Docker 部署入口都已具备。
- 前端控制台已有 `dashboard / markets / events / radar / rewards / copy-trading / signals / positions / risk / approvals / replay / settings` 页面。
- 前端数据层统一走 `src/lib/api/*`（读取按领域文件 `markets.ts` / `signals.ts` / `risk.ts`… 基于 `base.ts`，写操作走 `actions.ts`），页面装配在 `src/features/*/loaders` 和 `src/features/*/components`。`src/server/` 目前是空目录（历史遗留）。
- 前端支持 `zh-CN / en-US`，语言由 `polyedge_locale` cookie 控制。
- 前端不再提供 mock 数据模式；`POLYEDGE_API_BASE_URL` 必须指向 Rust 后端，读写和 SSE 都走真实 `/api/v1/...`。
- 当前控制台会话只保留 `off`，不是生产级真实会话。
- 后端 API 已覆盖 markets、events、news、evidences、signals、orders、trades、positions、pricing、arbitrage、rewards bot、risk、approvals、system、SSE 和 connector callback 等主路径。
- `polyedge-worker` 支持 news ingest、news promotion、arbitrage radar、rewards bot 模拟、execution drain、paper reconciliation、Polymarket order/fill/user-event 任务。
- 套利雷达是只读链路：发现、记录、校验、分析、展示和 SSE 推送已具备，但不会创建 execution request 或订单。
- Rewards bot 已是有状态的逐 tick 做市模拟引擎：只使用独立的 `reward_markets` 表作为奖励市场来源，先按 rewards 配置预过滤候选市场，再从 worker 进程内 InMemoryOrderbookCache（TTL 5 分钟）并发读取候选盘口、生成当前候选快照的 YES/NO post-only 双边买单计划，并维护共享资金池账本（capital/available/reserved/realized_pnl/reward_earned）。开放模拟买单采用软资金复用：同一 `account_capital_usd` 可在多个市场同时报价，单腿计划 notional 以 `min(quote_size_usd, account_capital_usd)` 为目标，只有模拟成交时才消耗 `available_usd`；历史 `reserved_usd` 会在下一次 rewards tick 自动释放。缺少新鲜缓存盘口时不会模拟成交或计提奖励；成交模拟只在新鲜盘口穿透/触顶时触发（确定性伪随机可复现）；成交后策略（加价出场 / 持有续挂 / 市价平仓 / 成交即撤对侧）、撤单策略（中点漂移、掉出 max_spread）、以及基于 Polymarket Qmin 公式的做市奖励金额累加已具备；当前仍不会实盘下单。API 服务不执行 rewards 策略或任务，前端 Run / Cancel / Reset 会写入数据库控制命令，由 worker 领取执行。
- Polymarket connector 已迁移到 CLOB V2 Rust crate：`packages/backend/Cargo.toml` 保留 dependency key `polymarket-client-sdk`，实际指向 `polymarket_client_sdk_v2`。
- 聪明钱跟单（copy-trading）已具备完整子系统：跟踪多个 Polymarket 钱包地址（`TrackedWallet`）、通过 Polymarket Data API（`data-api.polymarket.com`，通过 `PolymarketDataApiConnector`）检测钱包新成交、四种跟单仓位模式（`FixedUsd`/`ProportionalToSource`/`CapitalRatio`/`MirrorPortfolioWeight`）、钱包分析统计（胜率/ROI/成交量）、per-wallet/per-market/total 敞口+单日亏损+冷却+滑点风控、确定性模拟引擎（模拟资金账本：capital/available/reserved/realized_pnl）、`Run/Analyze/Cancel/Reset` 与账户资金设置前端 UI；`mode=live` 已结构化支持但未接入真实下单（记录警告回退模拟）。API 服务不执行 copytrade 跟单循环、钱包分析、撤单或重置，前端操作会写入数据库控制命令，由 worker 领取执行；`POLYEDGE_COPYTRADE__ENABLED=true` 启用 worker 轮询。
- Polymarket 运行时不再提供 mock mode；市场列表走 Gamma 实时数据，私有订单/成交任务需要真实凭证、真实账户、小额演练和运维 runbook。
- 数据库迁移目前到 `0023_copytrade_control_commands.sql`。

## 主要缺口

- 生产级真实会话体系未完成；当前前端只保留 `off` 模式。
- 内部 JWT 签名 helper 已有代码路径，但当前不会从 `off` 签发可信令牌。
- `signals / risk / events` SSE 仍是 snapshot-backed stream；`arbitrage` 已是 outbox-backed 增量流，但尚未统一到全资源事件总线。
- 新闻源可以抓取、去重、提升为 events/evidences，但尚未自动生成 signals。
- Rewards bot 当前只做模拟：已具备资金池账本（开放买单软复用、成交扣款）、成交模拟、Qmin 奖励金额计算、成交后处理与撤单策略，前端 `/rewards` 提供 Run / Cancel / Reset 入队和事件分类（挂单/撤单/吃单/奖励）视图；尚未接入真实 post-only 下单、订单计分查询、真实成交处理或真实库存同步。后续实现实盘 rewards maker 时应沿用“未成交 maker 买单不硬锁全局 USDC、成交后才更新现金/库存并撤超额挂单”的策略模型。
- Polymarket live 链路已具备骨架和 CLOB V2 SDK，仍需真实资金链路验证。

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
- 默认 arbitrage radar 和 news ingestion 是 disabled。
- 默认 rewards bot worker 模拟是 disabled；前端 `/rewards` 的 Run / Cancel / Reset 只会入队命令，worker 需要同时设置 `POLYEDGE_REWARDS__ENABLED=true` 和 `POLYEDGE_WORKER__POLL_REWARD_BOT=true` 才会领取并执行。
- Rewards bot 的 `max_markets=0`、`max_open_orders=0` 或 `quote_size_usd=0` 都表示不再新挂单；不是无限制。
- Rewards bot 模拟开放买单不会逐单锁定资金；`account_capital_usd=200` 时可以在多个市场同时挂 200U 级别买单，但模拟成交会消耗 `available_usd`，后续成交仍受资金池现金限制。
- 默认跟单 worker 是 disabled；前端 `/copy-trading` 的 Run / Analyze / Cancel / Reset 只会入队命令，worker 需要设置 `POLYEDGE_COPYTRADE__ENABLED=true` + `POLYEDGE_WORKER__POLL_COPYTRADE=true` 才会领取并执行；`POLYEDGE_WORKER__ANALYZE_WALLETS=true` 仍用于独立钱包分析循环。
- `POLYEDGE_POSTGRES__URL` / `POLYEDGE_REDIS__URL` 为空时，本地可能走内存路径，无法验证多进程共享状态和持久化 outbox。
- `POLYEDGE_ARBITRAGE__BOOK_SOURCE=polymarket` 会请求真实 Polymarket CLOB `/book`；live 冒烟必须使用真实 Polymarket refs。
- Docker Compose 部署中的 `polyedge-worker` 会把所有 `POLYEDGE_WORKER__...` 后台任务默认覆盖为 `false`；需要运行市场同步、orderbook stream、新闻、套利、rewards 或 copytrade 时必须在 `deploy/.env` 显式设为 `true`。

## Docker 部署

后端镜像从 `bin/polyedge-api` 和 `bin/polyedge-worker` 复制预构建二进制；服务器部署不编译 Rust。构建机/CI 先执行：

```bash
./scripts/build-backend-bin.sh
git add bin/polyedge-api bin/polyedge-worker
```

服务器部署入口：

```bash
cp deploy/.env.example deploy/.env
# 编辑 deploy/.env，填入外部 PostgreSQL URL 和控制台 step-up code
./scripts/deploy.sh all
```

`deploy/docker-compose.yml` 编排：

- `polyedge-api`
- `polyedge-worker`
- `polyedge-front`

`scripts/deploy.sh` 只接受简单目标参数：

- 不传参数或 `auto`：拉取最新代码，只在后端二进制或前端文件 hash 变化时 rebuild；容器未运行但 hash 未变时只启动已有镜像。
- `all`：重建后端和前端镜像，并重启 API、worker、front。
- `api worker`：重建后端镜像，并重启 API 与 worker。
- `api`：只重建后端镜像并重启 API。
- `worker`：只重建后端镜像并重启 worker。
- `front`：只重建前端镜像并重启前端。
- 支持组合，例如 `api front` 或 `api,worker`。

部署脚本默认使用 `/tmp/polyedge-deploy.lock` 防止 cron/CI 重叠执行，默认 `COMPOSE_PARALLEL_LIMIT=1` 串行构建镜像；Auto 模式只有后端二进制或前端文件 hash 改变时才 rebuild，容器未运行但 hash 未变时只启动已有镜像。Compose 构建上下文已收窄：后端只上传 `bin/`，前端只上传 `packages/front/`，避免扫描本地 `packages/backend/target`、`node_modules`、`.next` 等大目录。

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
- `packages/backend/apps/worker/src/worker/rewards.rs`
- `packages/backend/apps/worker/src/worker/copytrade.rs`
- `packages/backend/crates/application/src/rewards/service.rs`
- `packages/backend/crates/application/src/copytrade.rs`
- `packages/backend/crates/application/src/copytrade/service.rs`
- `packages/backend/crates/connectors/src/polymarket/data_api.rs`
- `packages/backend/crates/infrastructure/src/stores/copytrade.rs`
- `packages/backend/crates/infrastructure/src/settings.rs`
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
