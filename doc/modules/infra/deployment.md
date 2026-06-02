# 部署（Docker + Nginx + Scripts）

最后更新：2026-06-01

## 概述

部署体系基于 Docker Compose，包含 4 个服务（API、Orderbook、Worker、Frontend），通过 Nginx 反向代理前端到后端的 API 请求。

## 架构与关键文件

| 文件 | 职责 |
|---|---|
| `deploy/docker-compose.yml` | 服务编排 |
| `deploy/backend.Dockerfile` | 后端部署镜像（debian:trixie-slim + `bin/` 预构建二进制） |
| `deploy/.env.example` | 公共环境变量模板；每个变量均有用途说明 |
| `deploy/.env.api.example` | API 服务环境变量模板；每个变量均有用途说明 |
| `deploy/.env.orderbook.example` | Orderbook 服务环境变量模板；每个变量均有用途说明 |
| `deploy/.env.worker.example` | Worker 服务环境变量模板；每个变量均有用途说明 |
| `deploy/.env.front.example` | Frontend 服务环境变量模板；每个变量均有用途说明 |
| `deploy/.env.polymarket.example` | Polymarket CLOB V2 live、Proxy/Gnosis Safe、Deposit Wallet、Rewards live worker 配置示例；每个变量均有用途说明 |
| `scripts/deploy.sh` | 部署脚本（auto + manual 模式） |
| `scripts/build-backend-bin.sh` | 后端二进制构建脚本 |
| `packages/backend/Dockerfile` | 后端镜像兼容模板（旧的仓库根 context 形式；Compose 部署不再使用） |
| `packages/front/Dockerfile` | 前端镜像（3 阶段：deps → builder → nginx:1.27-alpine；context 为 `packages/front/`） |
| `packages/front/.dockerignore` | 前端构建 context 排除规则 |
| `.dockerignore` | 仓库根构建 context 排除规则（兼容旧构建入口） |
| `packages/front/nginx.conf.template` | Nginx 配置模板 |

## 服务架构

```
┌──────────────────┐     ┌──────────────────┐
│   polyedge-front │────→│   polyedge-api   │────┐
│   nginx:80       │     │   port:38001     │    │
│   (static site)  │     │   (Rust Axum)    │    │
└──────────────────┘     └──────────────────┘    │
                                                  ↓
┌──────────────────┐     ┌──────────────────┐
│  polyedge-worker │────→│ polyedge-orderbook│
│  (same image)    │     │  port:38002      │
│                  │     │  (WS + HTTP)     │
└──────────────────┘     └──────────────────┘
```

### polyedge-api

- 镜像：`debian:trixie-slim` + 预构建 `bin/polyedge-api` 二进制
- 端口：`0.0.0.0:38001 → container:38001`
- 健康检查：`curl /healthz`（15s 间隔，10 次重试，20s 启动期）
- 依赖 orderbook 健康检查通过后启动
- 通过 `POLYEDGE_ORDERBOOK__SERVICE_URL` 连接 orderbook 服务读取盘口数据
- 环境变量：`.env` + `.env.api`
- `extra_hosts: host.docker.internal:host-gateway`（访问宿主机数据库）

### polyedge-orderbook

- 同 API 镜像，command 覆盖为 `polyedge-orderbook`
- 端口：`0.0.0.0:38002 → container:38002`
- 健康检查：`curl /healthz`（15s 间隔，10 次重试，20s 启动期）
- 后端链路中的基础服务，Compose 会先于 API 和 Worker 启动
- 职责：HTTP API（健康检查、盘口读取、token 注册）、后台市场同步（Gamma + CLOB → Postgres）、WS + poll 盘口流（→ 进程内缓存）
- 启动顺序：先 bind HTTP 并暴露 `/healthz`，随后后台执行 initial/periodic market sync，避免外部 Polymarket API 慢响应导致容器启动健康检查失败
- 环境变量：`.env` + `.env.orderbook`

### polyedge-worker

- 同 API 镜像，command 覆盖为 `polyedge-worker`
- 无端口暴露
- 依赖 API 和 orderbook 健康检查通过后启动
- Docker 部署中所有 `POLYEDGE_WORKER__...` 后台任务默认由 Compose 覆盖为 `false`，需要在 `deploy/.env.worker` 显式改为 `true` 才会启动对应任务
- 通过 `POLYEDGE_ORDERBOOK__SERVICE_URL` 连接 orderbook 服务读取盘口数据和注册 token
- 环境变量：`.env` + `.env.worker`
- Polymarket live / Deposit Wallet 配置示例见 `deploy/.env.polymarket.example`；建议只把私钥放入 `.env.worker`，避免进入 API/Front 容器环境

### polyedge-front

- 镜像：本机 `yarn build` 预编译静态文件到 `out/`，Docker 镜像仅 `COPY out/` 到 nginx（无容器内编译）
- 端口：`0.0.0.0:33002 → container:80`
- 健康检查：`wget /healthz`
- 入口脚本验证 `POLYEDGE_CONSOLE_STEP_UP_CODE` 已设置
- `envsubst` 将环境变量注入 nginx 配置模板

## Nginx 配置

| 路径 | 行为 |
|---|---|
| `/healthz` | 返回 200 "ok" |
| `/_next/static/` | 静态资源，1 年不可变缓存 |
| `/api/v1/stream/` | SSE 代理，完全禁用缓冲（`proxy_buffering off`、`proxy_read_timeout 1h`） |
| `/api/v1/` | 标准反向代理到后端，附带 step-up 认证头和硬编码身份（"Static Console"、admin 角色） |
| `/` | 静态文件服务，fallback 到 `$uri.html` 和 `/404.html` |

**Step-up 认证：** nginx 通过 `map` 指令比较请求头 `X-PolyEdge-Step-Up-Code` 与环境变量 `POLYEDGE_CONSOLE_STEP_UP_CODE`，设置 `$polyedge_step_up_verified` 为 "true" 或 "false"。

## 部署脚本（deploy.sh）

### Auto 模式（默认，适合 cron/CI）

1. 获取部署锁（默认 `/tmp/polyedge-deploy.lock`），避免 cron/CI 重叠执行
2. `git fetch` + fast-forward merge
3. 无镜像变更且所有容器运行中 → 跳过
4. 后端二进制变更 → 重建后端镜像，立即写入 `.deploy-state`，再按 orderbook → API → Worker 顺序重启
5. 前端文件变更 → 重建前端镜像，立即写入 `.deploy-state`，再重启 Frontend
6. 容器未运行但镜像 hash 未变化 → 只 `up -d` 启动已有镜像，不强制 rebuild

### Manual 模式

- `scripts/deploy.sh all` — 全量重建
- `scripts/deploy.sh orderbook` — 重建后端，重启 Orderbook
- `scripts/deploy.sh api` — 重建后端，重启 API
- `scripts/deploy.sh worker` — 重建后端，重启 Worker
- `scripts/deploy.sh front` — 重建前端
- 支持组合：`api worker`、`api,front` 等

### 环境变量验证

- `POLYEDGE_POSTGRES__URL` 不能包含 "change-me"
- `POLYEDGE_CONSOLE_STEP_UP_CODE` 不能为空或 "change-me"
- `POLYEDGE_INTERNAL_AUTH_DEV_BYPASS=1` 仅在 `POLYEDGE_RUNTIME__ENVIRONMENT=local` 时允许
- `deploy/.env*.example` 中每个变量上一段注释说明了变量含义、适用服务和安全注意事项

## 必需环境变量

| 变量 | 说明 |
|---|---|
| `POLYEDGE_POSTGRES__URL` | PostgreSQL 连接字符串 |
| `POLYEDGE_CONSOLE_STEP_UP_CODE` | 控制台提权认证码 |

## 可选环境变量

| 变量 | 默认值 | 说明 |
|---|---|---|
| `POLYEDGE_REDIS__URL` | — | Redis URL（可选） |
| `POLYEDGE_FRONT_BIND` | `0.0.0.0` | 前端绑定地址 |
| `POLYEDGE_FRONT_PORT` | `33002` | 前端宿主机端口 |
| `POLYEDGE_API_BIND` | `0.0.0.0` | API 绑定地址 |
| `POLYEDGE_API_PORT` | `38001` | API 宿主机端口 |
| `POLYEDGE_API_UPSTREAM` | `http://polyedge-api:38001` | 前端代理目标 |
| `POLYEDGE_ORDERBOOK__SERVICE_URL` | `http://polyedge-orderbook:38002` | API/Worker 访问 orderbook 服务的内部地址 |
| `POLYEDGE_ALLOW_IN_MEMORY_DEPLOY` | — | 设为 1 允许无数据库部署（仅演示） |
| `POLYEDGE_LOG_FILE` | `$HOME/polyedge-deploy.log`（cron） | deploy 脚本日志文件；无法写入时回退到 stdout/stderr |
| `POLYEDGE_DEPLOY_LOCK_FILE` | `/tmp/polyedge-deploy.lock` | deploy 脚本互斥锁 |
| `COMPOSE_PARALLEL_LIMIT` | `1` | Docker Compose 构建并发，低配服务器默认串行构建 |
| `POLYEDGE_WORKER__POLL_MARKET_SYNC` | `false`（Compose 覆盖） | 部署 worker 是否同步 markets/reward_markets |
| `POLYEDGE_WORKER__CONSUME_ORDERBOOK_STREAM` | `false`（Compose 覆盖） | 部署 worker 是否消费 orderbook stream |
| `POLYEDGE_WORKER__POLL_REWARD_BOT` | `false`（Compose 覆盖） | 部署 worker 是否运行 rewards full tick + fast reconcile loop |
| `POLYEDGE_ORDERBOOK_STREAM__ENABLED` | `true` | orderbook stream 功能开关；Compose 中还需打开 worker 任务 |
| `POLYEDGE_ORDERBOOK_STREAM__MAX_TOKENS` | `20000` | orderbook stream 订阅 token 上限，过低会导致 rewards 覆盖不全 |

## Polymarket live 配置示例

`deploy/.env.polymarket.example` 提供 EOA、Proxy/Gnosis Safe、Deposit Wallet（`poly_1271`）三类账户示例，以及 rewards live worker 开关示例。真实凭证默认全部注释，使用时按账户类型复制到 `deploy/.env.worker` 或公共 `.env`；私钥优先只放 `.env.worker`。

Deposit Wallet 路径要求钱包已经部署、已入金 pUSD 并完成必要 approval。当前系统不会执行 relayer wallet-create、pUSD 包装或 approval 批处理；connector 在下单前会调用 CLOB `balance-allowance/update`。

## 后端二进制构建

```bash
./scripts/build-backend-bin.sh   # cargo build --release → bin/
git add bin/polyedge-api bin/polyedge-worker bin/polyedge-orderbook
```

## 当前状态

- 部署模板适合原型/内网共享环境
- Compose 部署使用窄构建上下文：后端只上传 `bin/`，前端只上传 `packages/front/`，避免扫描 Rust `target/`、前端 `node_modules/`、`.next/` 等大目录
- `polyedge-front` 不再依赖 API 健康后才启动；前端静态 Nginx 可独立运行并在 API 恢复后继续代理请求
- `scripts/deploy.sh` 已防止重叠执行；前端变更 hash 会直接 prune `node_modules`、`.next`、`out` 等大目录；容器 down 时会按 orderbook → API → Worker 顺序启动已有后端镜像，不会因健康失败而重复 rebuild
- 认证使用内部 dev-auth 模式
- 生产前需要：真实会话体系、签名 JWT、key rotation

## 修改检查清单

- [ ] 新增服务时更新 `docker-compose.yml`
- [ ] 新增环境变量时更新 `.env.example` 和 `deploy.sh` 的验证逻辑
- [ ] 修改 nginx 路由时更新 `nginx.conf.template`
- [ ] 修改构建流程时更新 Dockerfile 和构建脚本
- [ ] 部署后验证所有容器健康检查通过
