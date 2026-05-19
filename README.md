# PolyEdge

PolyEdge 是一个面向 Polymarket 的事件驱动交易系统原型仓库，目标是把外部事件与证据转成概率更新、交易信号和风险受控的执行动作。

当前仓库已经包含两部分：

- `packages/front`：`Next.js 16 + React 19` 控制台前端
- `packages/backend`：Rust workspace，包含 `api / worker / replay`

如果你想先了解“当前代码已经实现到哪一步”，优先看 [AGENT.md](./AGENT.md)。设计目标和长期方案在 `doc/` 目录。

## 当前状态

仓库已经不是纯文档状态：

- 前端控制台已经有 `dashboard / markets / events / radar / signals / positions / risk / approvals / replay / settings`
- 后端已经有 Axum API、worker 子命令、配置和数据库迁移
- 默认体验仍然是 mock-first
- 本地 live API/SSE 联调路径已经基本打通，生产化链路仍未闭环

当前最重要的现实判断：

1. 前端在未配置 `POLYEDGE_API_BASE_URL` 时，读数据走 typed mock；审批、风控切换、执行提交等写操作会失败。
2. 配置 `POLYEDGE_API_BASE_URL` 后，前端会请求后端 `/api/v1/...`，并可用 `POLYEDGE_ENABLE_LIVE_SSE=1` 代理 Rust SSE。
3. 因此，这个仓库适合用于界面开发、契约收敛、后端能力建设和本地 live 联调，而不是直接当成已上线系统。

## 仓库结构

```text
PolyEdge/
├── AGENT.md
├── README.md
├── doc/
├── packages/
│   ├── front/
│   └── backend/
```

### `doc/`

设计与契约文档，包括：

- 系统总设计
- 前端设计与实施计划
- 后端设计与实施计划
- API 契约
- 鉴权设计
- 存储 schema
- LLM 治理
- Polymarket connector 设计

### `packages/front/`

前端控制台，主要结构：

- `src/app/(console)`：页面路由
- `src/features/*`：按业务域拆分的 loader 和组件
- `src/server/api/*`：前端读取层
- `src/server/actions/*`：前端写操作
- `src/app/api/stream/[channel]/route.ts`：SSE proxy / mock stream 入口

### `packages/backend/`

Rust workspace，主要结构：

- `apps/api`：Axum HTTP API
- `apps/worker`：后台任务与执行/回写流程
- `apps/replay`：研究/回放运行时骨架
- `crates/application`：用例编排
- `crates/domain`：领域模型与规则
- `crates/contracts`：HTTP/DTO 契约
- `crates/infrastructure`：配置、存储、鉴权、运行时
- `crates/connectors`：外部连接器

## 快速开始

这个仓库不是单一工具链的 monorepo。前端和后端要分别进入各自目录运行。

### 环境要求

建议本地至少具备：

- `Node.js 20+`
- `pnpm`
- `Rust` 与 `cargo`
- `PostgreSQL` 和 `Redis`（后端真实运行时需要）

## 前端运行

进入前端目录：

```bash
cd packages/front
```

安装依赖并启动：

```bash
pnpm install
pnpm dev
```

常用命令：

```bash
pnpm lint
pnpm build
```

默认环境变量见 [packages/front/.env.example](./packages/front/.env.example)：

```bash
POLYEDGE_API_BASE_URL=
HOSTNAME=0.0.0.0
POLYEDGE_CONSOLE_AUTH=off
POLYEDGE_ENABLE_LIVE_SSE=0
POLYEDGE_INTERNAL_AUTH_DEV_BYPASS=1
```

说明：

1. `POLYEDGE_API_BASE_URL` 留空时，前端读数据走 mock；受保护写操作需要配置真实后端。
2. `POLYEDGE_CONSOLE_AUTH` 当前只支持 `off` 和 `mock-session`。
3. 本地 live 联调可使用 dev-auth bypass；签名内部 JWT 需要配置 `POLYEDGE_INTERNAL_AUTH_KID` / `POLYEDGE_INTERNAL_AUTH_PRIVATE_KEY` 和后端验签公钥。

## 后端运行

进入后端目录：

```bash
cd packages/backend
```

常用命令：

```bash
cargo check --workspace
cargo test --workspace
cargo run -p polyedge-api
cargo run -p polyedge-worker
```

`polyedge-api` 只负责 HTTP API / SSE / 前端交互，可以在外部负载均衡后面运行多个实例。后台任务统一由 `polyedge-worker` 常驻服务调度；是否启用各类任务由 `POLYEDGE_WORKER__...` 配置控制。

worker 仍保留以下维护/调试子命令，正常运行时不需要逐个手动执行：

```bash
cargo run -p polyedge-worker -- ingest-fixtures
cargo run -p polyedge-worker -- ingest-news-once
cargo run -p polyedge-worker -- poll-news
cargo run -p polyedge-worker -- promote-news-events
cargo run -p polyedge-worker -- scan-arbitrage-once
cargo run -p polyedge-worker -- poll-arbitrage-radar
cargo run -p polyedge-worker -- analyze-arbitrage-opportunities
cargo run -p polyedge-worker -- drain-execution-queue
cargo run -p polyedge-worker -- reconcile-paper-fills
cargo run -p polyedge-worker -- poll-paper-order-statuses
cargo run -p polyedge-worker -- poll-polymarket-order-statuses
cargo run -p polyedge-worker -- reconcile-polymarket-fills
cargo run -p polyedge-worker -- consume-polymarket-user-events
```

后端通过环境变量配置，示例见 [packages/backend/.env.example](./packages/backend/.env.example)。

建议先在 `packages/backend` 下创建本地配置：

```bash
cp .env.example .env
```

默认值对应当前开发环境：

- API 默认监听 `0.0.0.0:8080`
- runtime 默认模式是 `manual_confirm`
- polymarket 默认模式是 `mock`
- `postgres.url` 和 `redis.url` 默认仍为空

说明：

1. 环境变量命名采用 `POLYEDGE_<section>__<field>`，例如 `POLYEDGE_SERVER__PORT=8080`。
2. 留空的可选项会被视为未配置，例如 `POLYEDGE_POSTGRES__URL=`。
3. `POLYEDGE_AUTH__KEYS_JSON` 使用 JSON 数组格式配置验签公钥。

### Worker 常驻服务

```bash
POLYEDGE_ARBITRAGE__ENABLED=true
POLYEDGE_WORKER__POLL_ARBITRAGE_RADAR=true
POLYEDGE_WORKER__ANALYZE_ARBITRAGE_OPPORTUNITIES=true
cargo run -p polyedge-worker
```

可选开启新闻抓取：

```bash
POLYEDGE_WORKER__POLL_NEWS=true
POLYEDGE_NEWS__ENABLED=true
POLYEDGE_NEWS__SOURCES_JSON='[{"id":"sec_feed","source_type":"official","url":"https://example.com/sec.rss","reliability":"0.95","enabled":true}]'
```

可选开启执行/回写类任务：

```bash
POLYEDGE_WORKER__DRAIN_EXECUTION_QUEUE=true
POLYEDGE_WORKER__POLL_PAPER_ORDER_STATUSES=true
POLYEDGE_WORKER__RECONCILE_PAPER_FILLS=true
POLYEDGE_WORKER__POLL_POLYMARKET_ORDER_STATUSES=true
POLYEDGE_WORKER__RECONCILE_POLYMARKET_FILLS=true
POLYEDGE_WORKER__CONSUME_POLYMARKET_USER_EVENTS=true
```

不要同时运行多个 `polyedge-worker` 常驻实例，除非先为具体任务加分布式锁/租约。API 可以多实例，worker 默认按单实例后台调度设计。

### 套利雷达 live 冒烟

本地验证真实盘口链路时，先确保已应用迁移到 `0014_arbitrage_validation_events.sql`，再使用：

```bash
POLYEDGE_POSTGRES__URL=postgres://...
POLYEDGE_REDIS__URL=redis://...
POLYEDGE_ARBITRAGE__BOOK_SOURCE=polymarket
POLYEDGE_ARBITRAGE__SCAN_LIMIT=1
POLYEDGE_ENABLE_LIVE_SSE=1
cargo run -p polyedge-api
cargo run -p polyedge-worker
```

前端 `/radar` 应能看到 scan、机会、validation、过期和 analysis 事件。`poll-arbitrage-radar` 会持续扫描，并按 `POLYEDGE_ARBITRAGE__EVENT_RETENTION_HOURS` 清理旧 outbox 事件。

也可以用本地冒烟脚本检查 API、worker、套利只读端点和可选前端 SSE 代理：

```bash
POLYEDGE_FRONT_BASE_URL=http://127.0.0.1:3001 \
./scripts/smoke-arbitrage-radar.sh
```

说明：

1. `packages/backend/.env` 中 `POLYEDGE_POSTGRES__URL` / `POLYEDGE_REDIS__URL` 留空时，后端会回退到内存 store；要验证持久化和 outbox，必须显式传真实连接串。
2. `POLYEDGE_ARBITRAGE__BOOK_SOURCE=polymarket` 会实时请求 Polymarket CLOB `/book`。fixture 里的演示 token 不是公网真实 token，live 冒烟时需要先把待测市场替换为真实 `polymarket_*` 引用，或把 `POLYEDGE_ARBITRAGE__BOOK_SOURCE` 改回 `market_snapshot`。
3. `scripts/smoke-arbitrage-radar.sh` 未设置 `POLYEDGE_SMOKE_BEARER_TOKEN` 时会发送本地 dev-auth header；如果后端没有关闭验签或没有启用 dev-auth，本脚本会跳过受保护端点或返回认证失败。

## Docker 部署

仓库包含一个服务器部署入口：

先在构建机或 CI 上生成 Linux 服务器可运行的后端二进制，并提交到仓库：

```bash
./scripts/build-backend-bin.sh
git add bin/polyedge-api bin/polyedge-worker
git commit -m "Build backend binaries"
```

服务器上执行：

```bash
cp deploy/.env.example deploy/.env
# 编辑 deploy/.env，填入外部 PostgreSQL / Redis URL 和控制台 step-up code
./scripts/deploy.sh
```

`deploy/docker-compose.yml` 启动：

- `polyedge-api`
- `polyedge-worker`
- `polyedge-front`

PostgreSQL 和 Redis 不会由 compose 创建，需要在 `deploy/.env` 里配置已有服务地址。

部署脚本会在启动前从 GitHub 更新当前 checkout，并按变更范围增量重建镜像：

- `bin/polyedge-api`、`bin/polyedge-worker` 或后端部署文件变化：重建后端镜像
- `packages/front` 或前端部署文件变化：重建 `polyedge-front`
- 只有文档等无关文件变化：不重建镜像，只执行 `docker compose up -d`

更新指定分支：

```bash
POLYEDGE_GIT_BRANCH=main ./scripts/deploy.sh
```

如果是首次在空目录部署，也可以把脚本放到服务器后指定仓库地址：

```bash
POLYEDGE_DEPLOY_DIR=/opt/polyedge \
POLYEDGE_GIT_REPO=https://github.com/<owner>/<repo>.git \
POLYEDGE_GIT_BRANCH=main \
./scripts/deploy.sh
```

## 当前已实现能力

### 前端

- 控制台布局、顶部状态条、侧边导航和实时状态栏
- `dashboard / markets / events / radar / signals / positions / risk / approvals / replay`
- feature loader + feature component 分层
- Server Actions 驱动的审批、风险控制和 kill switch UI 链路
- SSE mock/live 代理与共享 realtime provider，`radar` 页面支持 arbitrage outbox 增量事件

### 后端

- Axum API 骨架与多个 `v1` 资源路由
- 风险模式切换、signal approve/reject、execution request 等写路径
- 审批、风险告警、风险桶与 SSE stream 只读资源
- worker 侧的 fixture ingest、执行队列、fill/status reconcile、raw news event/evidence promotion 流程
- worker 侧只读套利雷达，可扫描盘口、记录机会、重新拉并记录最新盘口来校验机会、识别 `price_moved`、过期旧机会并生成历史分析，不会创建 execution request 或订单
- 套利雷达只读 API 与前端 `/radar` 页面，支持查看 scan、机会列表、校验结果、active/validated/rejected/history 视图、只读 candidate preview 和分析摘要
- `/api/v1/stream/arbitrage` 使用套利 outbox sequence 做实时增量 SSE，并支持 `Last-Event-ID` 续传
- RSS/Atom 新闻源抓取、标准化、去重入 `raw_events`、source health 记录和 raw news 只读 API
- Polymarket connector 与 paper/mock 执行相关代码
- PostgreSQL schema 迁移

## 当前未闭合的部分

以下是目前最重要的集成缺口：

1. `signals / risk / events` SSE 仍是 snapshot-backed stream；`arbitrage` 已是 outbox-backed 增量流，但还不是跨所有资源统一的事件总线。
2. 前端权限仍以 `off | mock-session` 为主，不是生产级会话系统。
3. 签名内部 JWT 代码路径已具备，但真实环境还需要 key rotation、会话来源和撤销策略。
4. Polymarket live 交易链路仍需要真实凭证、小额演练、部署配置和运维 runbook。
5. 套利雷达当前闭合到机会发现、记录、校验、分析、实时增量推送、只读展示和 candidate preview；尚未创建 execution request 或订单。
6. 新闻源当前已闭合到 `raw_events` 入库、健康状态、只读查看和保守提升为 `events/evidences`，尚未自动生成 `signals`。

这意味着：

- 前端界面和 BFF 层已经较完整
- 后端主链路已经开始成型
- 本地 live 联调可以推进，但生产化仍需要继续收口

## 推荐阅读顺序

如果你刚接手这个项目，建议按这个顺序阅读：

1. [AGENT.md](./AGENT.md)
2. [doc/polyedge-design.md](./doc/polyedge-design.md)
3. [doc/polyedge-frontend-design.md](./doc/polyedge-frontend-design.md)
4. [doc/polyedge-backend-design.md](./doc/polyedge-backend-design.md)
5. [doc/polyedge-api-contract.md](./doc/polyedge-api-contract.md)
6. [doc/polyedge-auth-design.md](./doc/polyedge-auth-design.md)

## 说明

- 根目录 `README.md` 用于项目入口说明。
- 根目录 [AGENT.md](./AGENT.md) 用于记录“当前仓库真实状态”和开发维护约定。
- 如果两者描述出现冲突，以 `AGENT.md` 为准。
