# AGENT.md

最后更新：2026-04-28

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
- 前端控制台已经成型，包含实时状态、审批、风险控制、事件/信号/市场/持仓/回放等页面。
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

### 4.3 数据与交互层

- 契约类型：`packages/front/src/lib/contracts/*`
- mock 数据：`packages/front/src/lib/server/polyedge-mock-data.ts`
- 读接口适配：`packages/front/src/server/api/*`
- 写操作：`packages/front/src/server/actions/*`
- 页面装配辅助：`packages/front/src/server/loaders/*`

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
- 本地 live 联调可使用 `POLYEDGE_INTERNAL_AUTH_DEV_BYPASS=1` 的 dev-auth headers；签名模式使用 `POLYEDGE_INTERNAL_AUTH_KID` / `POLYEDGE_INTERNAL_AUTH_PRIVATE_KEY` 与后端 `POLYEDGE_AUTH__KEYS_JSON` 配对。
- `POLYEDGE_CONSOLE_AUTH` 当前只支持：
  - `off`
  - `mock-session`

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

- 默认配置：`packages/backend/config/default.toml`
- API 默认监听：`127.0.0.1:8080`
- 默认 runtime mode：`manual_confirm`
- 默认 Polymarket mode：`mock`
- `postgres.url` 和 `redis.url` 默认仍为空

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

## 6. 前后端贯通状态

### 6.1 当前结论

当前仓库处于“前端控制台和后端主链路已具备，本地 live API/SSE 联调路径已经基本打通，但生产级会话、真实部署和真实资金链路尚未闭环”的状态。

### 6.2 已经具备的部分

- 前端读取统一走 `src/server/api/*`
- 前端写操作统一走 `src/server/actions/*`
- 前端 live API 已统一请求 `/api/v1/...`
- 前端已有 SSE proxy / mock stream 机制，并可代理 Rust `/api/v1/stream/{channel}`
- 前端 live fetch 已能发送本地 dev-auth headers 或签名内部 JWT
- 后端已有 `v1` REST API、worker 和交易/回写相关主链路
- 后端已有审批、风险告警、风险桶的一等只读资源端点，前端不再依赖 `live-console-derived.ts` 派生这些资源

### 6.3 当前明确存在的缺口

1. SSE 仍是 snapshot-backed stream：后端按间隔生成当前 signals/risk/events 快照消息，尚不是持久化事件总线或 Redis/Postgres outbox 驱动的精确增量流。
2. 前端权限当前仍是 `off | mock-session`，不是生产级真实会话体系。
3. 签名内部 JWT 链路已具备代码路径，但仍需要在真实环境配置 Ed25519 key rotation、会话来源和撤销策略。
4. Polymarket live 模式已有 connector/worker 骨架，但仍需要真实凭证、真实账户、小额演练和运维 runbook 才能视为生产交易链路。

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
cargo run -p polyedge-worker -- drain-execution-queue
```

如果需要轮询或对账，可使用：

```bash
cargo run -p polyedge-worker -- reconcile-paper-fills
cargo run -p polyedge-worker -- poll-paper-order-statuses
cargo run -p polyedge-worker -- poll-polymarket-order-statuses
cargo run -p polyedge-worker -- reconcile-polymarket-fills
```

## 8. 关键入口文件

### 8.1 前端

- `packages/front/src/server/api/base.ts`
- `packages/front/src/server/actions/approval-actions.ts`
- `packages/front/src/server/actions/risk-actions.ts`
- `packages/front/src/app/api/stream/[channel]/route.ts`
- `packages/front/src/proxy.ts`
- `packages/front/src/components/shared/console-realtime-provider.tsx`

### 8.2 后端

- `packages/backend/apps/api/src/lib.rs`
- `packages/backend/apps/api/src/main.rs`
- `packages/backend/apps/worker/src/main.rs`
- `packages/backend/crates/application/src/lib.rs`
- `packages/backend/crates/infrastructure/src/runtime.rs`
- `packages/backend/crates/infrastructure/src/settings.rs`
- `packages/backend/config/default.toml`

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
