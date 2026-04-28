# PolyEdge

PolyEdge 是一个面向 Polymarket 的事件驱动交易系统原型仓库，目标是把外部事件与证据转成概率更新、交易信号和风险受控的执行动作。

当前仓库已经包含两部分：

- `packages/front`：`Next.js 16 + React 19` 控制台前端
- `packages/backend`：Rust workspace，包含 `api / worker / replay`

如果你想先了解“当前代码已经实现到哪一步”，优先看 [AGENT.md](./AGENT.md)。设计目标和长期方案在 `doc/` 目录。

## 当前状态

仓库已经不是纯文档状态：

- 前端控制台已经有 `dashboard / markets / events / signals / positions / risk / approvals / replay / settings`
- 后端已经有 Axum API、worker 子命令、配置和数据库迁移
- 默认体验仍然是 mock-first
- 本地 live API/SSE 联调路径已经基本打通，生产化链路仍未闭环

当前最重要的现实判断：

1. 前端在未配置 `POLYEDGE_API_BASE_URL` 时，走 typed mock 数据。
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
POLYEDGE_CONSOLE_AUTH=off
POLYEDGE_ENABLE_LIVE_SSE=0
POLYEDGE_INTERNAL_AUTH_DEV_BYPASS=1
```

说明：

1. `POLYEDGE_API_BASE_URL` 留空时，前端走 mock 数据。
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
```

worker 常用子命令：

```bash
cargo run -p polyedge-worker -- ingest-fixtures
cargo run -p polyedge-worker -- ingest-news-once
cargo run -p polyedge-worker -- poll-news
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

- API 默认监听 `127.0.0.1:8080`
- runtime 默认模式是 `manual_confirm`
- polymarket 默认模式是 `mock`
- `postgres.url` 和 `redis.url` 默认仍为空

说明：

1. 环境变量命名采用 `POLYEDGE_<section>__<field>`，例如 `POLYEDGE_SERVER__PORT=8080`。
2. 留空的可选项会被视为未配置，例如 `POLYEDGE_POSTGRES__URL=`。
3. `POLYEDGE_AUTH__KEYS_JSON` 使用 JSON 数组格式配置验签公钥。

## 当前已实现能力

### 前端

- 控制台布局、顶部状态条、侧边导航和实时状态栏
- `dashboard / markets / events / signals / positions / risk / approvals / replay`
- feature loader + feature component 分层
- Server Actions 驱动的审批、风险控制和 kill switch UI 链路
- SSE mock/live 代理与共享 realtime provider

### 后端

- Axum API 骨架与多个 `v1` 资源路由
- 风险模式切换、signal approve/reject、execution request 等写路径
- 审批、风险告警、风险桶与 SSE stream 只读资源
- worker 侧的 fixture ingest、执行队列、fill/status reconcile 流程
- RSS/Atom 新闻源抓取、标准化、去重入 `raw_events` 和 source health 记录
- Polymarket connector 与 paper/mock 执行相关代码
- PostgreSQL schema 迁移

## 当前未闭合的部分

以下是目前最重要的集成缺口：

1. SSE 仍是 snapshot-backed stream，不是持久化事件总线或 outbox 驱动的精确增量流。
2. 前端权限仍以 `off | mock-session` 为主，不是生产级会话系统。
3. 签名内部 JWT 代码路径已具备，但真实环境还需要 key rotation、会话来源和撤销策略。
4. Polymarket live 交易链路仍需要真实凭证、小额演练、部署配置和运维 runbook。
5. 新闻源当前只闭合到 `raw_events` 入库和健康状态，尚未自动生成 `events/evidences/signals`。

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
