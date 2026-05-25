# AGENT.md

最后更新：2026-05-24

## 文件用途

这是仓库级工作说明和状态快照，用来说明当前代码已经具备什么、缺什么，以及后续改动时需要同步维护哪些信息。前端专属规则仍以 [packages/front/AGENTS.md](./packages/front/AGENTS.md) 为准，其中 `Next.js 16` 提醒必须继续遵守。

## 维护规则

- 任何改变行为、路由、命令、环境变量、部署方式、依赖、集成状态或已知缺口的改动，都要同步更新本文件。
- 不要把设计文档里的目标能力写成已实现能力。
- 如果本文件、README、页面文案冲突，以本文件为仓库状态快照优先修正。

## 仓库结构

- `doc/`：系统设计、API 契约、鉴权、存储、前后端计划等文档。
- `packages/front/`：`Next.js 16 + React 19 + Tailwind v4 + shadcn/ui` 控制台前端。
- `packages/backend/`：Rust workspace，包含 `api / worker / replay` apps，以及 `application / connectors / contracts / domain / infrastructure` crates。
- `deploy/`：Docker Compose 部署模板和环境变量示例。
- `scripts/`：构建、部署、冒烟脚本。
- `bin/`：部署镜像复制的预构建后端二进制。

## 当前状态

- 仓库已经不是纯文档仓库：前端控制台、Rust API、worker、迁移、配置和 Docker 部署入口都已具备。
- 前端控制台已有 `dashboard / markets / events / radar / rewards / signals / positions / risk / approvals / replay / settings` 页面。
- 前端读取统一走 `src/server/api/*`，写操作统一走 `src/server/actions/*`，页面装配在 `src/features/*/loaders` 和 `src/features/*/components`。
- 前端支持 `zh-CN / en-US`，语言由 `polyedge_locale` cookie 控制。
- 前端不再提供 mock 数据模式；`POLYEDGE_API_BASE_URL` 必须指向 Rust 后端，读写和 SSE 都走真实 `/api/v1/...`。
- 当前控制台会话只保留 `off`，不是生产级真实会话。
- 后端 API 已覆盖 markets、events、news、evidences、signals、orders、trades、positions、pricing、arbitrage、rewards bot、risk、approvals、system、SSE 和 connector callback 等主路径。
- `polyedge-worker` 支持 fixture/news ingest、news promotion、arbitrage radar、rewards bot 模拟、execution drain、paper reconciliation、Polymarket order/fill/user-event 任务。
- 套利雷达是只读链路：发现、记录、校验、分析、展示和 SSE 推送已具备，但不会创建 execution request 或订单。
- Rewards bot 已接入模拟链路：扫描 Polymarket CLOB rewards 当前市场、拉取候选 token 盘口、生成 YES/NO post-only 双边买单计划，并可写入模拟托管挂单；当前不会实盘下单。
- Polymarket connector 已迁移到 CLOB V2 Rust crate：`packages/backend/Cargo.toml` 保留 dependency key `polymarket-client-sdk`，实际指向 `polymarket_client_sdk_v2`。
- 默认 Polymarket mode 仍是 `mock`；live 交易仍需要真实凭证、真实账户、小额演练和运维 runbook。
- 数据库迁移目前到 `0015_reward_bot.sql`。

## 主要缺口

- 生产级真实会话体系未完成；当前前端只保留 `off` 模式。
- 内部 JWT 签名 helper 已有代码路径，但当前不会从 `off` 签发可信令牌。
- `signals / risk / events` SSE 仍是 snapshot-backed stream；`arbitrage` 已是 outbox-backed 增量流，但尚未统一到全资源事件总线。
- 新闻源可以抓取、去重、提升为 events/evidences，但尚未自动生成 signals。
- Rewards bot 当前只做模拟；尚未接入真实 post-only 下单、订单计分查询、成交处理、退出卖单或真实库存同步。
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
cargo run -p polyedge-worker -- ingest-fixtures
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
```

套利雷达冒烟：

```bash
./scripts/smoke-arbitrage-radar.sh
```

## 配置要点

- 后端默认监听 `0.0.0.0:8080`。
- 默认 runtime mode 是 `manual_confirm`。
- 默认 Polymarket mode 是 `mock`。
- 默认 arbitrage radar 和 news ingestion 是 disabled。
- 默认 rewards bot worker 模拟是 disabled；前端 `/rewards` 可以手动运行模拟，worker 需要同时设置 `POLYEDGE_REWARDS__ENABLED=true` 和 `POLYEDGE_WORKER__POLL_REWARD_BOT=true`。
- `POLYEDGE_POSTGRES__URL` / `POLYEDGE_REDIS__URL` 为空时，本地可能走内存路径，无法验证多进程共享状态和持久化 outbox。
- `POLYEDGE_ARBITRAGE__BOOK_SOURCE=polymarket` 会请求真实 Polymarket CLOB `/book`；fixture 中演示 token 不是公网真实 token，live 冒烟必须替换成真实 Polymarket refs。

## Docker 部署

后端镜像从 `bin/polyedge-api` 和 `bin/polyedge-worker` 复制预构建二进制；服务器部署不编译 Rust。构建机/CI 先执行：

```bash
./scripts/build-backend-bin.sh
git add bin/polyedge-api bin/polyedge-worker
```

服务器部署入口：

```bash
cp deploy/.env.example deploy/.env
# 编辑 deploy/.env，填入外部 PostgreSQL / Redis URL 和控制台 step-up code
./scripts/deploy.sh all
```

`deploy/docker-compose.yml` 编排：

- `polyedge-api`
- `polyedge-worker`
- `polyedge-front`

`scripts/deploy.sh` 只接受简单目标参数：

- `all` 或不传参数：重建后端和前端镜像，并重启 API、worker、front。
- `api worker`：重建后端镜像，并重启 API 与 worker。
- `api`：只重建后端镜像并重启 API。
- `worker`：只重建后端镜像并重启 worker。
- `front`：只重建前端镜像并重启前端。
- 支持组合，例如 `api front` 或 `api,worker`。

默认部署模板仍沿用本地 internal dev-auth 模式，只适合原型/内网共享环境；生产前需要真实会话体系、签名 internal JWT、key rotation 和撤销策略。

## 关键入口

前端：

- `packages/front/src/server/api/base.ts`
- `packages/front/src/server/actions/*`
- `packages/front/src/app/api/stream/[channel]/route.ts`
- `packages/front/src/proxy.ts`
- `packages/front/src/lib/i18n/*`
- `packages/front/src/features/radar/*`

后端：

- `packages/backend/apps/api/src/lib.rs`
- `packages/backend/apps/worker/src/main.rs`
- `packages/backend/crates/application/src/*`
- `packages/backend/crates/connectors/src/polymarket.rs`
- `packages/backend/crates/infrastructure/src/settings.rs`
- `packages/backend/migrations/*`

部署：

- `packages/backend/Dockerfile`
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
