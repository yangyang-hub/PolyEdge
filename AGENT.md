# AGENT.md

最后更新：2026-05-17

## 1. 文件用途

本文件是仓库级工作说明和状态快照，描述的是“当前代码已经实现了什么”和“当前还缺什么”，不是目标计划。

前端特定额外规则仍然以 [packages/front/AGENTS.md](./packages/front/AGENTS.md) 为准；其中关于 `Next.js 16` 的提醒必须继续遵守。

## 2. 维护规则

以下规则对后续代码修改生效：

1. 任何会改变行为、架构、路由、命令、环境变量、依赖、集成状态或已知缺口的代码改动，必须在同一轮修改里同步更新本文件。
2. 更新时优先维护以下部分：
   - `当前仓库状态`
   - `运行与调试`
   - `前后端贯通状态`
   - `关键入口文件`
   - 顶部“最后更新”日期
3. 不要把设计文档中的目标能力写成“已实现能力”。
4. 如果某个说明已经过时，优先改 `AGENT.md`，不要依赖 `README` 或旧页面文案作为真值来源。

## 3. 当前仓库状态

### 3.1 顶层目录

- `doc/`
  系统设计、API 契约、鉴权、存储、前后端实施计划等文档。
- `packages/front/`
  `Next.js 16 + React 19 + Tailwind v4 + shadcn/ui` 的控制台前端。
- `packages/backend/`
  Rust workspace，包含 `api / worker / replay` 三个 app 和 `application / connectors / contracts / domain / infrastructure` 五个 crate。

### 3.2 当前实现判断

- 这个仓库已经不是纯文档仓库。
- 前端控制台已经成型，包含实时状态、审批、风险控制、事件/信号/市场/持仓/回放等页面，并已接入 `zh-CN / en-US` 双语切换。
- 后端已经有可编译的 Rust workspace、Axum API、worker job、配置和迁移文件。

## 4. 前端现状

### 4.1 技术栈

- `Next.js 16.2.4`
- `React 19.2.4`
- `TypeScript`
- `Tailwind CSS v4`
- `shadcn/ui`
- `zod`

### 4.2 页面与结构

控制台路由位于 `packages/front/src/app/(console)`，当前页面包括：

- `dashboard`
- `markets`
- `events`
- `radar`
- `signals`
- `positions`
- `risk`
- `approvals`
- `replay`
- `settings`

实现模式已经收敛为：

1. `app/*/page.tsx` 只负责装配。
2. 页面数据在 `src/features/*/loaders`。
3. 交互 UI 在 `src/features/*/components`。
4. 共享布局和组件在 `src/components/shared` 与 `src/components/ui`。

控制台、认证页、错误页与 not-found 页的用户可见文案已经统一接入前端 i18n 字典；页面不使用 locale 路由前缀。

### 4.3 数据与交互层

- 契约类型：`packages/front/src/lib/contracts/*`
- mock 数据：`packages/front/src/lib/server/polyedge-mock-data.ts`
- 读接口适配：`packages/front/src/server/api/*`
- 写操作：`packages/front/src/server/actions/*`
- 页面装配辅助：`packages/front/src/server/loaders/*`
- 双语字典：`packages/front/src/lib/i18n/dictionaries.ts`
- 系统生成展示文案本地化：`packages/front/src/lib/i18n/generated-copy.ts`
- 语言读取与 provider：`packages/front/src/lib/i18n/server.ts`、`packages/front/src/lib/i18n/client.tsx`
- 语言切换写操作：`packages/front/src/server/actions/locale-actions.ts`

### 4.4 权限与实时层

- 控制台路由保护：`packages/front/src/proxy.ts`
- mock session：`packages/front/src/server/auth/console-session.ts`
- 角色模型：`viewer / operator / risk_admin / admin`
- SSE 入口：`packages/front/src/app/api/stream/[channel]/route.ts`
- SSE hooks：`packages/front/src/hooks/use-sse-stream.ts`
- 共享 realtime provider：`packages/front/src/components/shared/console-realtime-provider.tsx`

### 4.5 前端运行模式

- `POLYEDGE_API_BASE_URL` 未配置时：前端运行在 typed mock 模式。
- `POLYEDGE_API_BASE_URL` 已配置时：前端尝试切换到 live API 模式。
- live API 路径当前统一请求 Rust 后端的 `/api/v1/...`。
- live SSE 默认仍使用前端 mock-fallback；设置 `POLYEDGE_ENABLE_LIVE_SSE=1` 后会代理到 Rust 后端 `/api/v1/stream/{channel}`。
- 本地 live 联调可使用 `POLYEDGE_INTERNAL_AUTH_DEV_BYPASS=1` 的 dev-auth headers；签名模式使用 `POLYEDGE_INTERNAL_AUTH_KID` / `POLYEDGE_INTERNAL_AUTH_PRIVATE_KEY` 与后端 `POLYEDGE_AUTH__KEYS_JSON` 配对，但当前前端会拒绝在 `off | mock-session` 控制台会话下签发内部 JWT。
- `POLYEDGE_CONSOLE_AUTH` 当前只支持：
  - `off`
  - `mock-session`
- `off | mock-session` 只能视为本地/原型权限模式，不是可信生产会话；前端路由保护只从 mock session cookie 读取角色，不再接受请求头角色回退。
- 控制台语言由 `polyedge_locale` cookie 控制，支持 `zh-CN` 与 `en-US`；未设置或非法值会回退到默认 `zh-CN`。右上角语言切换组件通过 server action 写 cookie，然后刷新当前页面的服务端数据。

### 4.6 已知前端说明文件

- [packages/front/AGENTS.md](./packages/front/AGENTS.md) 有效，但只包含 Next.js 提醒。
- [packages/front/README.md](./packages/front/README.md) 仍是 `create-next-app` 默认模板，不应视为当前项目真值。

## 5. 后端现状

### 5.1 Workspace 结构

- apps
  - `packages/backend/apps/api`
  - `packages/backend/apps/worker`
  - `packages/backend/apps/replay`
- crates
  - `packages/backend/crates/application`
  - `packages/backend/crates/connectors`
  - `packages/backend/crates/contracts`
  - `packages/backend/crates/domain`
  - `packages/backend/crates/infrastructure`

### 5.2 API 层

`packages/backend/apps/api/src/lib.rs` 已实现 Axum 路由，当前可见的主要能力包括：

- `GET /healthz`
- `GET /readyz`
- `GET /api/v1/markets`
- `GET /api/v1/events`
- `GET /api/v1/news/source-health`
- `GET /api/v1/news/raw-events`
- `GET /api/v1/evidences`
- `GET /api/v1/signals`
- `GET /api/v1/signals/{signal_id}/transitions`
- `POST /api/v1/signals/{signal_id}/recompute`
- `POST /api/v1/signals/{signal_id}/approve`
- `POST /api/v1/signals/{signal_id}/reject`
- `POST /api/v1/signals/{signal_id}/execution-requests`
- `GET /api/v1/orders/drafts`
- `GET /api/v1/orders`
- `GET /api/v1/trades`
- `GET /api/v1/execution/requests`
- `GET /api/v1/positions`
- `GET /api/v1/pricing/estimates`
- `GET /api/v1/arbitrage/scans`
- `GET /api/v1/arbitrage/opportunities`
- `GET /api/v1/arbitrage/analysis`
- `GET /api/v1/risk/state`
- `GET /api/v1/risk/alerts`
- `GET /api/v1/risk/buckets`
- `GET /api/v1/approvals`
- `GET /api/v1/stream/{channel}`
- `POST /api/v1/system/mode`
- `POST /api/v1/system/kill-switch/trigger`
- `POST /api/v1/system/kill-switch/release`
- connector callback endpoints（含 Polymarket）

### 5.3 Worker 层

`packages/backend/apps/worker/src/main.rs` 当前已暴露这些入口：

- `ingest-fixtures`
- `ingest-news-once`
- `poll-news`
- `promote-news-events`
- `scan-arbitrage-once`
- `poll-arbitrage-radar`
- `analyze-arbitrage-opportunities`
- `drain-execution-queue`
- `reconcile-paper-fills`
- `poll-paper-order-statuses`
- `poll-polymarket-order-statuses`
- `reconcile-polymarket-fills`
- `consume-polymarket-user-events`

### 5.4 Replay 层

- `packages/backend/apps/replay` 已有可启动 skeleton。
- 当前更接近研究运行时骨架，而不是完整 replay 服务。

### 5.5 配置与迁移

- 默认配置由 `packages/backend/crates/infrastructure/src/settings.rs` 的 `Default` 实现提供，环境变量示例见 `packages/backend/.env.example`；当前没有 `packages/backend/config/default.toml` 文件。
- API 默认监听：`0.0.0.0:8080`
- 默认 runtime mode：`manual_confirm`
- 默认 Polymarket mode：`mock`
- 默认 arbitrage radar：`disabled`，默认盘口源为 `market_snapshot`，机会 TTL 为 60 秒，outbox 默认保留 24 小时，校验默认要求盘口年龄不超过 10 秒、gross edge 不低于 0.5%、容量不低于 1，并预留 fee/slippage buffer 各 0.5%
- 默认 news ingestion：`disabled`
- `postgres.url` 和 `redis.url` 默认仍为空
- 本地 live 套利雷达验证需要显式传 `POLYEDGE_POSTGRES__URL` / `POLYEDGE_REDIS__URL`，否则会走内存 store，无法验证持久化 outbox、scan 历史和多进程前后端共享状态
- `POLYEDGE_ARBITRAGE__BOOK_SOURCE=polymarket` 会请求真实 Polymarket CLOB `/book`；fixture 内演示 token 不是公网真实 token，live 冒烟需要替换为真实 `polymarket_condition_id` / `polymarket_yes_asset_id` / `polymarket_no_asset_id`，或回退 `market_snapshot`

数据库迁移目前已到：

- `0001_support_tables.sql`
- `0002_market_event_core.sql`
- `0003_evidence_signal_core.sql`
- `0004_pricing_and_signal_transitions.sql`
- `0005_risk_state.sql`
- `0006_signal_rejection_metadata.sql`
- `0007_execution_request_core.sql`
- `0008_execution_dispatch_metadata.sql`
- `0009_orders_trades_positions.sql`
- `0010_market_connector_refs.sql`
- `0011_news_ingestion.sql`
- `0012_news_source_health_list_index.sql`
- `0013_arbitrage_radar.sql`
- `0014_arbitrage_validation_events.sql`

## 6. 前后端贯通状态

### 6.1 当前结论

当前仓库处于“前端控制台和后端主链路已具备，本地 live API/SSE 联调路径已经基本打通，但生产级会话、真实部署和真实资金链路尚未闭环”的状态。

### 6.2 已经具备的部分

- 前端读取统一走 `src/server/api/*`
- 前端写操作统一走 `src/server/actions/*`
- 前端 live API 已统一请求 `/api/v1/...`
- 前端已有 SSE proxy / mock stream 机制，并可代理 Rust `/api/v1/stream/{channel}`
- 前端 `/radar` 已订阅 `arbitrage` SSE channel，可在初始快照之上增量合并 scan、机会、过期、校验和分析事件，并提供 active / validated / rejected / history 视图和只读 candidate preview
- 前端 live fetch 已能发送本地 dev-auth headers；签名内部 JWT helper 已具备，但在真实会话体系接入前不会从 `off | mock-session` 签发令牌
- 后端已有 `v1` REST API、worker 和交易/回写相关主链路
- 后端已有审批、风险告警、风险桶、新闻源健康和 raw news 的一等只读资源端点，前端不再依赖 `live-console-derived.ts` 派生这些资源
- 后端风险/审批派生资源会读取完整内部快照后再对响应应用展示 limit；成交回写后的风险指标按全局持仓聚合
- 后端 worker 已能把近期 raw news 按保守词面匹配提升为关联已有市场的 `events` 和 `evidences`
- 后端 worker 已有只读套利雷达入口，可记录扫描、发现盘口快照、机会、二次盘口快照与校验、`price_moved`、过期事件、outbox 清理和历史分析；该链路不会创建 execution request 或订单
- 套利雷达已通过 `/api/v1/arbitrage/scans`、`/api/v1/arbitrage/opportunities`、`/api/v1/arbitrage/analysis` 暴露只读 API，前端 `/radar` 页面已接入 typed mock/live API 适配
- 后端 `/api/v1/stream/arbitrage` 已使用套利 outbox 事件表/内存事件序列做增量 SSE，支持 `Last-Event-ID` 按 sequence 续传
- 本地套利链路可用 `./scripts/smoke-arbitrage-radar.sh` 冒烟检查 API、worker、只读端点和可选前端 SSE 代理

### 6.3 当前明确存在的缺口

1. `signals / risk / events` SSE 仍是 snapshot-backed stream：后端按间隔读取当前快照，会用 `Last-Event-ID` 避免重发最近事件，并在单个连接内按事件 ID 去重后发送；套利 `arbitrage` channel 已是 outbox-backed 增量流，但还不是跨所有资源统一的事件总线。
2. 前端权限当前仍是 `off | mock-session`，不是生产级真实会话体系。
3. 签名内部 JWT 链路已具备代码路径，但当前拒绝从 `off | mock-session` 签发；真实环境仍需要可信会话来源、Ed25519 key rotation 和撤销策略。
4. Polymarket live 模式已有 connector/worker 骨架，但仍需要真实凭证、真实账户、小额演练和运维 runbook 才能视为生产交易链路。
5. 套利雷达当前已闭合到发现、记录、机会校验、分析、只读展示、实时增量推送和 candidate preview；尚未创建 execution request 或订单。
6. 新闻源已支持 RSS/Atom 抓取、标准化、去重写入 `raw_events` 和 `news_source_health`，并可在 API/设置页查看 source health 与最近 raw news；worker 可将匹配到已有市场的 raw news 提升为 `events/evidences`，但尚未自动生成 `signals`。

### 6.4 因此的实际判断

默认运行态仍然应视为：

- 前端：mock-first 原型控制台
- 后端：真实业务骨架与本地 live API/SSE 联调路径
- 全链路生产化：未完成

## 7. 运行与调试

### 7.1 前端

在 `packages/front` 下：

```bash
pnpm dev
pnpm lint
pnpm build
```

默认环境变量见 `packages/front/.env.example`：

```bash
POLYEDGE_API_BASE_URL=
HOSTNAME=0.0.0.0
POLYEDGE_CONSOLE_AUTH=off
POLYEDGE_ENABLE_LIVE_SSE=0
POLYEDGE_INTERNAL_AUTH_DEV_BYPASS=1
```

### 7.2 后端

在 `packages/backend` 下：

```bash
cargo check --workspace
cargo test --workspace
cargo run -p polyedge-api
cargo run -p polyedge-worker -- ingest-fixtures
cargo run -p polyedge-worker -- ingest-news-once
cargo run -p polyedge-worker -- poll-news
cargo run -p polyedge-worker -- promote-news-events
cargo run -p polyedge-worker -- scan-arbitrage-once
cargo run -p polyedge-worker -- poll-arbitrage-radar
cargo run -p polyedge-worker -- analyze-arbitrage-opportunities
cargo run -p polyedge-worker -- drain-execution-queue
```

如果需要轮询或对账，可使用：

```bash
cargo run -p polyedge-worker -- reconcile-paper-fills
cargo run -p polyedge-worker -- poll-paper-order-statuses
cargo run -p polyedge-worker -- poll-polymarket-order-statuses
cargo run -p polyedge-worker -- reconcile-polymarket-fills
```

### 7.3 Docker 部署

仓库已包含前后端 Docker 部署入口：

- `packages/backend/Dockerfile`
  从仓库 `bin/polyedge-api` 复制预构建二进制，构建 `polyedge-api` 运行镜像；服务器部署不再编译 Rust。
- `packages/front/Dockerfile`
  构建 Next.js standalone 前端镜像。
- `deploy/docker-compose.yml`
  编排 `polyedge-api` 和 `polyedge-front`；PostgreSQL 与 Redis 不在 compose 内启动，通过环境变量连接外部实例。
- `deploy/.env.example`
  部署环境变量模板。
- `scripts/build-backend-bin.sh`
  在构建机/CI 上执行 `cargo build --release -p polyedge-api` 并复制产物到 `bin/polyedge-api`，该二进制需要提交到仓库。
- `scripts/deploy.sh`
  服务器部署脚本，支持从 GitHub 现有 checkout 执行 fast-forward 更新，也支持通过 `POLYEDGE_GIT_REPO` 初次 clone；更新后按 diff 增量重建镜像。

`scripts/deploy.sh` 的镜像重建判断：

- `bin/polyedge-api`、`packages/backend/Dockerfile`、`deploy/docker-compose.yml` 或 `.dockerignore` 变化时重建 `polyedge-api`。
- `packages/front`、`deploy/docker-compose.yml` 或 `.dockerignore` 变化时重建 `polyedge-front`。
- `POLYEDGE_FORCE_REBUILD=1` 会强制重建两个镜像。

默认部署模板沿用当前可工作的本地 internal dev-auth 模式：

- `POLYEDGE_RUNTIME__ENVIRONMENT=local`
- `POLYEDGE_AUTH__KEYS_JSON=[]`
- `POLYEDGE_INTERNAL_AUTH_DEV_BYPASS=1`

这只适合原型/内网共享环境；生产前仍需要真实会话体系、签名 internal JWT、key rotation 和撤销策略。

## 8. 关键入口文件

### 8.1 前端

- `packages/front/src/server/api/base.ts`
- `packages/front/src/server/api/arbitrage.ts`
- `packages/front/src/server/api/news.ts`
- `packages/front/src/lib/i18n/locales.ts`
- `packages/front/src/lib/i18n/dictionaries.ts`
- `packages/front/src/lib/i18n/generated-copy.ts`
- `packages/front/src/lib/i18n/server.ts`
- `packages/front/src/lib/i18n/client.tsx`
- `packages/front/src/app/(console)/radar/page.tsx`
- `packages/front/src/features/radar/loaders/radar-page-data.ts`
- `packages/front/src/features/radar/components/arbitrage-radar-workbench.tsx`
- `packages/front/src/components/shared/language-switcher.tsx`
- `packages/front/src/server/actions/approval-actions.ts`
- `packages/front/src/server/actions/risk-actions.ts`
- `packages/front/src/server/actions/locale-actions.ts`
- `packages/front/src/app/api/stream/[channel]/route.ts`
- `packages/front/src/proxy.ts`
- `packages/front/src/components/shared/console-realtime-provider.tsx`

### 8.2 后端

- `packages/backend/apps/api/src/lib.rs`
- `packages/backend/apps/api/src/main.rs`
- `packages/backend/apps/worker/src/main.rs`
- `packages/backend/crates/application/src/arbitrage.rs`
- `packages/backend/crates/application/src/lib.rs`
- `packages/backend/crates/connectors/src/polymarket.rs`
- `packages/backend/crates/infrastructure/src/runtime.rs`
- `packages/backend/crates/infrastructure/src/settings.rs`
- `packages/backend/migrations/0013_arbitrage_radar.sql`
- `packages/backend/migrations/0014_arbitrage_validation_events.sql`
- `packages/backend/.env.example`
- `packages/backend/Dockerfile`
- `packages/front/Dockerfile`
- `deploy/docker-compose.yml`
- `deploy/.env.example`
- `bin/README.md`
- `scripts/deploy.sh`
- `scripts/build-backend-bin.sh`

## 9. 更新本文件时的最小检查清单

每次修改代码后，至少检查以下问题是否需要同步更新本文件：

1. 是否新增、删除或重命名了页面、路由、API、worker 子命令或迁移。
2. 是否新增、删除或修改了环境变量、默认端口、运行模式或鉴权方式。
3. 是否改变了前后端贯通状态，例如：
   - `/api` 与 `/api/v1` 已统一
   - SSE 后端端点已补齐
   - 内部鉴权 token 已真正接通
4. 是否新增了关键入口文件，导致“关键入口文件”部分过时。
5. 顶部“最后更新”日期是否需要刷新。
