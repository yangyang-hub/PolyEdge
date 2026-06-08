# 部署（Docker + Nginx + Scripts）

最后更新：2026-06-08

## 概述

部署体系基于 Docker Compose，包含 4 个服务（API、Orderbook、Worker、Frontend）。前端是静态站点，浏览器通过 `NEXT_PUBLIC_POLYEDGE_API_BASE_URL` 直连后端 API；API 使用 permissive CORS，支持 front/API 分别部署在不同内网服务器。

## 架构与关键文件

| 文件 | 职责 |
|---|---|
| `deploy/docker-compose.yml` | 服务编排 |
| `deploy/api.Dockerfile` | API/Worker 部署镜像（debian:trixie-slim + `bin/polyedge-api` / `bin/polyedge-worker`） |
| `deploy/orderbook.Dockerfile` | Orderbook 独立部署镜像（debian:trixie-slim + `bin/polyedge-orderbook`） |
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
| `packages/front/nginx.conf.template` | Nginx 静态文件配置模板 |

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
│  (API image)     │     │  port:38002      │
│                  │     │  (WS + HTTP)     │
└──────────────────┘     └──────────────────┘
```

### polyedge-api

- 镜像：`debian:trixie-slim` + 预构建 `bin/polyedge-api` 二进制
- 端口：`0.0.0.0:38001 → container:38001`
- 健康检查：`curl /healthz`（15s 间隔，10 次重试，20s 启动期）
- Compose 不声明启动依赖，可独立部署；需要盘口的 API 路由通过 service URL 访问 orderbook
- 通过 `POLYEDGE_ORDERBOOK__SERVICE_URL` 连接 orderbook 服务读取盘口数据
- 环境变量：`.env` + `.env.api`
- `extra_hosts: host.docker.internal:host-gateway`（访问宿主机数据库）
- Rewards snapshot 只读取 worker 写入数据库的账户快照、托管订单和持仓；API 不需要 Polymarket 私钥、CLOB API 凭证或 funder 配置。

### polyedge-orderbook

- 独立 `deploy/orderbook.Dockerfile` 镜像，只复制 `bin/polyedge-orderbook`
- 端口：`0.0.0.0:38002 → container:38002`
- 健康检查：`curl /healthz`（15s 间隔，10 次重试，20s 启动期）
- Compose 不声明启动依赖，可单独部署在盘口服务器
- 职责：HTTP API（健康检查、盘口读取、token 注册）、后台市场同步（Gamma + CLOB → Postgres）、WS + poll 盘口流（→ 进程内缓存）
- 启动顺序：先 bind HTTP 并暴露 `/healthz`，随后后台执行 initial/periodic market sync，避免外部 Polymarket API 慢响应导致容器启动健康检查失败
- register/ingest/delete 写接口要求 `.env.orderbook` 中的 `POLYEDGE_ORDERBOOK__WRITE_TOKEN`，读盘口、stats 和健康检查不需要该 token
- 环境变量：`.env` + `.env.orderbook`

### polyedge-worker

- 同 API 镜像，command 覆盖为 `polyedge-worker`
- 无端口暴露
- Compose 不声明启动依赖，可独立部署；启用需要盘口的任务前必须保证 orderbook service URL 可用
- Docker 部署中所有 `POLYEDGE_WORKER__...` 后台任务按代码默认值为 `false`，需要在 `deploy/.env.worker` 显式改为 `true` 才会启动对应任务
- 通过 `POLYEDGE_ORDERBOOK__SERVICE_URL` 连接 orderbook 服务读取盘口数据和注册 token
- `.env.worker` 中的 `POLYEDGE_ORDERBOOK__WRITE_TOKEN` 必须与 orderbook 服务一致；API/front 不需要该密钥
- 环境变量：`.env` + `.env.worker`
- Polymarket live / Deposit Wallet 配置示例见 `deploy/.env.polymarket.example`；建议只把私钥放入 `.env.worker`，避免进入 API/Front 容器环境。Rewards 账户余额由 worker 同步到数据库，资金钱包地址优先使用 `FUNDER`，CLOB balance 为 0/失败时会用链上 pUSD 余额回填 snapshot

### polyedge-front

- 镜像：本机 `yarn build` 预编译静态文件到 `out/`，Docker 镜像仅 `COPY out/` 到 nginx（无容器内编译）
- 端口：`0.0.0.0:33002 → container:80`
- 健康检查：`wget /healthz`
- 通过 `NEXT_PUBLIC_POLYEDGE_API_BASE_URL` 指向内网 API 地址，浏览器直连后端
- 当前内网免鉴权模式不需要设置 `NEXT_PUBLIC_POLYEDGE_INTERNAL_AUTH_DEV_BYPASS`
- `scripts/deploy.sh` 在 `yarn build` 前会读取 `deploy/.env.front` 并导出 `NEXT_PUBLIC_*`，这些值会被写入静态 JS bundle；修改 API 地址后必须重建前端镜像
- `envsubst` 将环境变量注入 nginx 静态文件配置模板

## Nginx 配置

| 路径 | 行为 |
|---|---|
| `/healthz` | 返回 200 "ok" |
| `/_next/static/` | 静态资源，1 年不可变缓存 |
| `/` | 静态文件服务，fallback 到 `$uri.html` 和 `/404.html` |

API 请求不再经过前端 nginx 反向代理；跨域由 Rust API 的 `CorsLayer::permissive()` 处理。当前纯内网部署通过 `POLYEDGE_AUTH__DISABLED=true` 关闭 API 权限校验。

## 部署脚本（deploy.sh）

### Auto 模式（默认，适合 cron/CI）

1. 获取部署锁（默认 `/tmp/polyedge-deploy.lock`），避免 cron/CI 重叠执行
2. `git fetch` + fast-forward merge
3. 无镜像变更且所有容器运行中 → 跳过
4. api/worker 二进制任一变化 → 重建共享 API 镜像并重启目标 API/Worker；orderbook 二进制变化 → 独立重建 orderbook 镜像
5. 前端文件或 `deploy/.env.front` 变更 → 重建前端镜像，立即写入 `.deploy-state`，再重启 Frontend
6. 容器未运行但镜像 hash 未变化 → 只 `up -d` 启动已有镜像，不强制 rebuild

### Manual 模式

- `scripts/deploy.sh all` — 全量重建
- `scripts/deploy.sh orderbook` — 重建 orderbook 镜像，只重启 Orderbook
- `scripts/deploy.sh api` — 重建 API/Worker 共享镜像，只重启 API
- `scripts/deploy.sh worker` — 重建 API/Worker 共享镜像，只重启 Worker
- `scripts/deploy.sh front` — 重建前端
- 支持组合：`api worker`、`api,front` 等

### 环境变量验证

- `POLYEDGE_POSTGRES__URL` 不能包含 "change-me"
- 部署 orderbook/worker 时，各自服务 env 中的 `POLYEDGE_ORDERBOOK__WRITE_TOKEN` 必须存在、不能包含 "change-me"；同次部署两项服务时脚本还会校验两端值一致
- `POLYEDGE_AUTH__DISABLED=false` 时，`POLYEDGE_AUTH__STEP_UP_CODE` 不能为空或 "change-me"
- `POLYEDGE_AUTH__DISABLED=true` 时，deploy.sh 不要求 step-up code，API 也不要求前端发送权限头
- 未关闭鉴权时，`POLYEDGE_INTERNAL_AUTH_DEV_BYPASS=1` 仅在 `POLYEDGE_RUNTIME__ENVIRONMENT=local` 时允许
- `deploy/.env*.example` 中每个变量上一段注释说明了变量含义、适用服务和安全注意事项

## 必需环境变量

| 变量 | 说明 |
|---|---|
| `POLYEDGE_POSTGRES__URL` | PostgreSQL 连接字符串 |
| `POLYEDGE_ORDERBOOK__WRITE_TOKEN` | Orderbook 内部写接口共享密钥；仅放 `.env.orderbook` / `.env.worker`，两端值必须一致 |

## 可选环境变量

| 变量 | 默认值 | 说明 |
|---|---|---|
| `POLYEDGE_REDIS__URL` | — | Redis URL（可选） |
| `POLYEDGE_AUTH__DISABLED` | `false`（代码默认；部署模板为 `true`） | 纯内网免鉴权开关，开启后 API 注入内部 admin 上下文 |
| `POLYEDGE_AUTH__STEP_UP_CODE` | — | `POLYEDGE_AUTH__DISABLED=false` 时用于敏感操作提权 |
| `POLYEDGE_FRONT_BIND` | `0.0.0.0` | 前端绑定地址 |
| `POLYEDGE_FRONT_PORT` | `33002` | 前端宿主机端口 |
| `POLYEDGE_API_BIND` | `0.0.0.0` | API 绑定地址 |
| `POLYEDGE_API_PORT` | `38001` | API 宿主机端口 |
| `POLYEDGE_API_IMAGE` | `polyedge-api:local` | API/Worker 共享镜像名 |
| `POLYEDGE_ORDERBOOK_IMAGE` | `polyedge-orderbook:local` | Orderbook 独立镜像名 |
| `NEXT_PUBLIC_POLYEDGE_API_BASE_URL` | — | 前端浏览器直连 API 地址，例如 `http://192.168.31.5:38001` |
| `POLYEDGE_ORDERBOOK__SERVICE_URL` | `http://localhost:38002` | API/Worker 访问 orderbook 服务的地址 |
| `POLYEDGE_ALLOW_IN_MEMORY_DEPLOY` | — | 设为 1 允许无数据库部署（仅演示） |
| `POLYEDGE_LOG_FILE` | `$HOME/polyedge-deploy.log`（cron） | deploy 脚本日志文件；无法写入时回退到 stdout/stderr |
| `POLYEDGE_DEPLOY_LOCK_FILE` | `/tmp/polyedge-deploy.lock` | deploy 脚本互斥锁 |
| `COMPOSE_PARALLEL_LIMIT` | `1` | Docker Compose 构建并发，低配服务器默认串行构建 |
| `POLYEDGE_WORKER__POLL_MARKET_SYNC` | `false`（代码默认） | 部署 worker 是否同步 markets/reward_markets；daemon 市场同步已迁移到 orderbook 服务 |
| `POLYEDGE_WORKER__CONSUME_ORDERBOOK_STREAM` | `false`（代码默认） | 部署 worker 是否消费 orderbook stream；daemon 盘口流已迁移到 orderbook 服务 |
| `POLYEDGE_WORKER__POLL_REWARD_BOT` | `false`（代码默认） | 部署 worker 是否运行 rewards full tick + fast reconcile loop |
| `POLYEDGE_POLYMARKET__POLYGON_RPC_URL` | `https://polygon-bor-rpc.publicnode.com` | Worker 读取资金钱包链上 pUSD 余额的 Polygon JSON-RPC 地址 |
| `POLYEDGE_ORDERBOOK_STREAM__MAX_TOKENS` | `3000` | orderbook stream 订阅 token 上限，过低会导致 rewards 覆盖不全，过高会增加 WS/poll 内存占用 |
| `POLYEDGE_ORDERBOOK_STREAM__MAX_LEVELS_PER_SIDE` | `100` | 每个 token 在 orderbook 进程内缓存和 HTTP ingest 中最多保留的 bid/ask 深度档数 |
| `POLYEDGE_ORDERBOOK_STREAM__STALE_THRESHOLD_MS` | `15000` | poll reconcile 盘口年龄阈值；0 只关闭年龄检查，TTL 过期仍生效 |

## Polymarket live 配置示例

`deploy/.env.polymarket.example` 提供 EOA、Proxy/Gnosis Safe、Deposit Wallet（`poly_1271`）三类账户示例，以及 rewards live worker 开关示例。真实凭证默认全部注释，执行链路按账户类型复制到 `deploy/.env.worker`。API 不持有 Polymarket 私钥；余额、positions 和托管订单都由 worker 同步到数据库后供 API 返回。`POLYEDGE_POLYMARKET__POLYGON_RPC_URL` 可替换为自有或有 SLA 的 Polygon RPC，用于链上 pUSD 余额回填。

Deposit Wallet 路径要求钱包已经部署、已入金 pUSD 并完成必要 approval。当前系统不会执行 relayer wallet-create、pUSD 包装或 approval 批处理；connector 在下单前会调用 CLOB `balance-allowance/update`。

Rewards live worker 在 Postgres 路径持有一个 advisory lease 连接来串行化命令、full tick 和 reconcile；`POLYEDGE_POSTGRES__MAX_CONNECTIONS` 必须至少为 2，默认 20。

## 后端二进制构建

```bash
./scripts/build-backend-bin.sh   # cargo build --release → bin/
git add bin/polyedge-api bin/polyedge-worker bin/polyedge-orderbook
```

## 当前状态

- 部署模板适合原型/内网共享环境
- Compose 部署使用窄构建上下文：后端只上传 `bin/`，前端只上传 `packages/front/`，避免扫描 Rust `target/`、前端 `node_modules/`、`.next/` 等大目录
- `polyedge-front` 不再依赖 API 健康后才启动；前端静态 Nginx 可独立运行，浏览器按 `NEXT_PUBLIC_POLYEDGE_API_BASE_URL` 访问 API
- `scripts/deploy.sh` 已防止重叠执行；前端变更 hash 包含 `packages/front/` 和 `deploy/.env.front`，会 prune `node_modules`、`.next`、`out` 等大目录；服务按目标独立部署，容器 down 且 hash 未变化时直接启动已有镜像，不会因其他服务健康失败而重复 rebuild
- 当前部署模板默认 `POLYEDGE_AUTH__DISABLED=true`，API/front 内网交互不做权限校验；API CORS 为 permissive
- Orderbook 服务 HTTP register/batch/ingest 入口按 `max_tokens` 和 `max_levels_per_side` 控制请求规模与缓存深度，写入时先排序再裁剪最优档位，registry source 固定上限为 32 个并在写锁内原子校验；register/ingest/delete 写接口还要求仅配置在 orderbook/worker 服务 env 的共享写 token，register 使用原子 source 替换
- 生产前需要：关闭 `POLYEDGE_AUTH__DISABLED`、接入真实会话体系、签名 JWT、key rotation

## 修改检查清单

- [ ] 新增服务时更新 `docker-compose.yml`
- [ ] 新增环境变量时更新 `.env.example` 和 `deploy.sh` 的验证逻辑
- [ ] 修改 nginx 路由时更新 `nginx.conf.template`
- [ ] 修改构建流程时更新 Dockerfile 和构建脚本
- [ ] 部署后验证所有容器健康检查通过
