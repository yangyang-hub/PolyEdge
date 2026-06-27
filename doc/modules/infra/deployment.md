# 部署（Docker + Nginx + Scripts）

最后更新：2026-06-27

## 概述

部署体系基于 Docker Compose，包含 3 个服务（API 内嵌 Worker、Orderbook、Frontend）。前端是静态站点，浏览器通过 `NEXT_PUBLIC_POLYEDGE_API_BASE_URL` 直连后端 API；API 使用 permissive CORS，支持 front/API 分别部署在不同内网服务器。

## 默认生产排查环境

除非用户明确指定其他环境，线上/生产问题排查默认使用以下地址：

| 服务 | 地址 | 说明 |
|---|---|---|
| Frontend Rewards 工作台 | `http://192.168.31.5:33002/rewards` | 浏览器入口 |
| API 服务 | `http://100.87.45.72:38001` | 前端 `NEXT_PUBLIC_POLYEDGE_API_BASE_URL` 应指向该地址 |
| Orderbook 服务 | `http://100.87.45.72:38002` | 盘口 HTTP、stats、内部 stream 的服务地址 |

当前生产形态是前端和 API/orderbook 分别在不同内网地址上暴露；排查浏览器/API 连通性时按上表访问。API 容器内部如果和 orderbook 位于同一 Compose 项目，`POLYEDGE_ORDERBOOK__SERVICE_URL` 仍可使用 `http://polyedge-orderbook:38002`；从宿主机或跨服务器排查 orderbook 时默认使用 `http://100.87.45.72:38002`。

## 架构与关键文件

| 文件 | 职责 |
|---|---|
| `deploy/docker-compose.yml` | 服务编排 |
| `deploy/api.Dockerfile` | API/Worker 合并部署镜像（debian:trixie-slim + `bin/polyedge-api`，内嵌 worker runtime） |
| `deploy/orderbook.Dockerfile` | Orderbook 独立部署镜像（debian:trixie-slim + `bin/polyedge-orderbook`） |
| `deploy/.env.api.example` | API + 内嵌 worker runtime 环境变量模板：Postgres、鉴权、orderbook service URL、worker 开关、Polymarket/AI 可选凭证 |
| `deploy/.env.orderbook.example` | Orderbook 服务环境变量模板：Postgres、写 token、端口映射和盘口容量常用项 |
| `deploy/.env.front.example` | Frontend 服务环境变量模板：前端端口和 build-time public API URL |
| `scripts/deploy.sh` | 部署脚本（auto + manual 模式） |
| `scripts/build-backend-bin.sh` | 后端二进制构建脚本；从 `packages/backend/Cargo.toml` workspace 构建并复制 `packages/backend/target/...` 下的二进制到 `bin/` |
| `packages/backend/Cargo.toml` | Rust workspace 根；包含 `packages/backend/api`、`packages/backend/order` 和 `packages/backend/...` members |
| `packages/backend/api/` | `polyedge-api` 服务 crate |
| `packages/backend/order/` | `polyedge-orderbook` 服务 crate |
| `packages/backend/Dockerfile` | 后端镜像兼容模板（旧的仓库根 context 形式；Compose 部署不再使用；只复制默认构建产物 `polyedge-api` / `polyedge-orderbook`） |
| `packages/front/Dockerfile` | 前端静态镜像（本机/脚本先 `yarn build` 到 `out/`，镜像只 COPY 到 nginx:1.27-alpine；context 为 `packages/front/`） |
| `packages/front/.dockerignore` | 前端构建 context 排除规则 |
| `.dockerignore` | 仓库根构建 context 排除规则（兼容旧构建入口） |
| `packages/front/nginx.conf.template` | Nginx 静态文件配置模板 |

## 服务架构

```
┌──────────────────┐     ┌──────────────────┐
│   polyedge-front │────→│   polyedge-api   │────┐
│   nginx:80       │     │   port:38001     │    │
│   (static site)  │     │   (API + Worker) │    │
└──────────────────┘     └──────────────────┘    │
                                                  ↓
                                        ┌──────────────────┐
                                        │ polyedge-orderbook│
                                        │  port:38002      │
                                        │  (WS + HTTP)     │
                                        └──────────────────┘
```

### polyedge-api（内嵌 Worker runtime）

- 镜像：`debian:trixie-slim` + 预构建 `bin/polyedge-api` 二进制
- 端口：`0.0.0.0:38001 → container:38001`
- 健康检查：`curl /healthz`（15s 间隔，10 次重试，20s 启动期）
- Compose 不声明启动依赖，可独立部署；需要盘口的 API 路由通过 service URL 访问 orderbook
- 通过 `POLYEDGE_ORDERBOOK__SERVICE_URL` 连接 orderbook 服务读取盘口数据；同一 Compose 项目使用 `http://polyedge-orderbook:38002`，跨服务器使用实际地址，容器内不能用 `localhost` 指向另一个服务
- Compose 不再用宿主机变量展开覆盖 `env_file` 中的 `POLYEDGE_ORDERBOOK__SERVICE_URL`，`.env.api` 的配置会原样传入容器
- 环境变量：`.env.api`
- `extra_hosts: host.docker.internal:host-gateway`（访问宿主机数据库）
- API 服务启动时内嵌启动 `WorkerRuntime`，共享同一进程；worker 后台任务通过 `deploy/.env.api` 配置
- Docker 模板默认开启新闻采集和数据库维护，其他 worker 循环在 `deploy/.env.api` 中显式设为 `false`，需要运行新闻提升、rewards、copytrade 或私有对账任务时再改为 `true`；旧 signal recompute 和 arbitrage radar worker 已移除
- 数据库维护循环由 `POLYEDGE_WORKER__DATABASE_MAINTENANCE` 控制；生产模板默认开启并每 3600 秒清理一次历史/缓存/队列表，本地模板默认关闭
- `.env.api.example` 显式写入当前默认 RSS/Atom 新闻源 `POLYEDGE_NEWS__SOURCES_JSON`，并默认开启 `POLYEDGE_NEWS__ENABLED=true` 和 `POLYEDGE_WORKER__POLL_NEWS=true`
- High Probability observe 自动扫描由 `POLYEDGE_WORKER__POLL_HIGH_PROBABILITY_OBSERVE` 控制，部署模板和本地模板均默认关闭；开启后按 `POLYEDGE_WORKER__HIGH_PROBABILITY_OBSERVE_INTERVAL_SECS` 写入只读 observations，不下单
- `.env.api` 中的 `POLYEDGE_ORDERBOOK__WRITE_TOKEN` 必须与 orderbook 服务一致；front 不需要该密钥
- Polymarket live / Deposit Wallet / Funding API / AI provider 可选配置已合并到 `deploy/.env.api.example`；建议私钥和 AI provider key 只放 `.env.api`，避免进入 Front/Orderbook 容器环境。Rewards 与 Smart Money signal advisory 使用独立 provider key/base URL 环境变量。Rewards 账户余额由 worker 同步到数据库，资金钱包地址优先使用 `FUNDER`，CLOB balance 为 0/失败时会用链上 pUSD 余额回填 snapshot；Funding API 也会优先使用 `FUNDER` 作为 Polymarket 入账钱包，并使用同一私钥和 Polygon RPC 广播真实 USDC/USDT 入金转账

### polyedge-orderbook

- 独立 `deploy/orderbook.Dockerfile` 镜像，只复制 `bin/polyedge-orderbook`
- 端口：`0.0.0.0:38002 → container:38002`
- 健康检查：`curl /healthz`（15s 间隔，10 次重试，20s 启动期）
- Compose 不声明启动依赖，可单独部署在盘口服务器
- 职责：HTTP API（健康检查、盘口读取、内部 WS stream、token 注册）、后台市场同步（Gamma + CLOB → Postgres）、WS + poll 盘口流（→ 进程内缓存）
- 启动顺序：先 bind HTTP 并暴露 `/healthz`，随后后台执行 initial/periodic market sync，避免外部 Polymarket API 慢响应导致容器启动健康检查失败
- register/ingest/delete 写接口要求 `.env.orderbook` 中的 `POLYEDGE_ORDERBOOK__WRITE_TOKEN`；读盘口、batch、stats、内部 `/orderbook/stream` 和健康检查不需要该 token，需依赖内网边界限制访问
- 环境变量：`.env.orderbook`

### polyedge-front

- 镜像：本机 `yarn build` 预编译静态文件到 `out/`，Docker 镜像仅 `COPY out/` 到 nginx（无容器内编译）
- 端口：`0.0.0.0:33002 → container:80`
- 健康检查：`wget /healthz`
- 通过 `NEXT_PUBLIC_POLYEDGE_API_BASE_URL` 指向内网 API 地址，浏览器直连后端；默认生产排查环境下为 `http://100.87.45.72:38001`
- 当前内网免鉴权模式不需要设置 `NEXT_PUBLIC_POLYEDGE_INTERNAL_AUTH_DEV_BYPASS`
- `scripts/deploy.sh` 在 `yarn build` 前会读取 `deploy/.env.front` 并导出 `NEXT_PUBLIC_*`，这些值会被写入静态 JS bundle；修改 API 地址后必须重建前端镜像。build 前会删除旧 `.next/` 和 `out/`，并在 build 后给 HTML 中的 `/_next/static/*.js/css` 引用追加 front hash query，避免复用旧静态导出产物或旧浏览器缓存
- `envsubst` 将环境变量注入 nginx 静态文件配置模板

## Nginx 配置

| 路径 | 行为 |
|---|---|
| `/healthz` | 返回 200 "ok" |
| `/_next/static/` | 静态资源，`Cache-Control: no-cache, must-revalidate`，避免静态导出 chunk 文件名复用时浏览器长期运行旧 JS |
| `/` | 静态文件服务，fallback 到 `$uri.html` 和 `/404.html`，HTML 同样要求 revalidate |

API 请求不再经过前端 nginx 反向代理；跨域由 Rust API 的 `CorsLayer::permissive()` 处理。当前纯内网部署通过 `POLYEDGE_AUTH__DISABLED=true` 关闭 API 权限校验。

## 部署脚本（deploy.sh）

### Auto 模式（默认，适合 cron/CI）

1. 获取部署锁（默认 `/tmp/polyedge-deploy.lock`），避免 cron/CI 重叠执行
2. `git fetch` + fast-forward merge
3. 无镜像变更且所有容器运行中 → 跳过
4. API 二进制变化 → 重建 API 镜像并重启 API；orderbook 二进制变化 → 独立重建 orderbook 镜像
5. 前端文件或 `deploy/.env.front` 变更 → 重建前端镜像，立即写入 `.deploy-state`，再重启 Frontend
6. 容器未运行但镜像 hash 未变化 → 只 `up -d` 启动已有镜像，不强制 rebuild

### Manual 模式

- `scripts/deploy.sh all` — 全量重建
- `scripts/deploy.sh orderbook` — 重建 orderbook 镜像，只重启 Orderbook
- `scripts/deploy.sh api`（或 `worker`）— 重建 API 镜像并重启 API（`worker` 是兼容别名）
- `scripts/deploy.sh front` — 重建前端
- 支持组合：`api orderbook`、`api,front` 等

### 环境变量验证

- `POLYEDGE_POSTGRES__URL` 不能包含 "change-me"；API 和 orderbook 各自的 env 文件都会独立校验
- 部署 orderbook 时，`.env.orderbook` 的 `POLYEDGE_ORDERBOOK__WRITE_TOKEN` 必须存在、不能包含 "change-me"；部署 API 时，`.env.api` 中的同一 token 也必须存在且不能包含 "change-me"；同次部署两项服务时脚本还会校验两端值一致
- `POLYEDGE_AUTH__DISABLED=false` 时，`POLYEDGE_AUTH__STEP_UP_CODE` 不能为空或 "change-me"
- `POLYEDGE_AUTH__DISABLED=true` 时，deploy.sh 不要求 step-up code，API 也不要求前端发送权限头
- 未关闭鉴权时，`POLYEDGE_INTERNAL_AUTH_DEV_BYPASS=1` 仅在 `POLYEDGE_RUNTIME__ENVIRONMENT=local` 时允许
- 部署脚本只会自动创建目标服务需要的 `.env.api`、`.env.orderbook`、`.env.front`；高级调参优先使用前端 Settings/runtime_config 或代码默认值

## 必需环境变量

| 变量 | 说明 |
|---|---|
| `POLYEDGE_POSTGRES__URL` | PostgreSQL 连接字符串；API 和 orderbook env 都需要配置 |
| `POLYEDGE_ORDERBOOK__WRITE_TOKEN` | Orderbook 内部写接口共享密钥；仅放 `.env.orderbook` / `.env.api`，两端值必须一致 |
| `POLYEDGE_ORDERBOOK__SERVICE_URL` | API/内嵌 worker 访问 orderbook 服务的 HTTP 地址；放 `.env.api`，跨服务器排查默认 orderbook 地址为 `http://100.87.45.72:38002` |
| `NEXT_PUBLIC_POLYEDGE_API_BASE_URL` | 前端静态构建写入浏览器 bundle 的 API 地址；放 `.env.front`，默认生产值为 `http://100.87.45.72:38001` |

## 常用可选环境变量

精简模板不再列出所有代码默认值；部署侧只保留 `deploy/.env.api.example`、`deploy/.env.orderbook.example`、`deploy/.env.front.example` 三个服务级模板。更细的业务阈值和轮询参数优先通过 Settings/runtime_config 或代码默认值管理。

| 变量/前缀 | 放置位置 | 说明 |
|---|---|---|
| `POLYEDGE_RUNTIME__ENVIRONMENT` | `.env.api` / `.env.orderbook` | 部署模板为 `production`；非本地环境不要使用 `local` |
| `POLYEDGE_AUTH__DISABLED` / `POLYEDGE_AUTH__STEP_UP_CODE` / `POLYEDGE_AUTH__KEYS_JSON` | `.env.api` | 内网免鉴权或 JWT/step-up 鉴权配置 |
| `POLYEDGE_API_BIND` / `POLYEDGE_API_PORT` | `.env.api` | API 宿主机暴露地址和端口 |
| `POLYEDGE_FRONT_BIND` / `POLYEDGE_FRONT_PORT` / `NEXT_PUBLIC_POLYEDGE_API_BASE_URL` | `.env.front` | 前端宿主机端口和 build-time API 地址 |
| `POLYEDGE_ORDERBOOK__SERVICE_URL` | `.env.api` | API/内嵌 worker 访问 orderbook 服务的 HTTP 地址 |
| `POLYEDGE_ORDERBOOK_STREAM__MAX_TOKENS` / `MAX_LEVELS_PER_SIDE` / `STALE_THRESHOLD_MS` | `.env.orderbook` | orderbook 订阅容量、盘口深度和 stale reconcile 常用调参 |
| `POLYEDGE_NEWS__ENABLED` / `POLYEDGE_NEWS__SOURCES_JSON` / `POLYEDGE_REWARDS__ENABLED` / `POLYEDGE_COPYTRADE__ENABLED` | `.env.api` | 业务子系统总开关；新闻采集默认 `true`，rewards/copytrade 默认 `false`；新闻源列表在模板中显式写入当前默认 RSS/Atom 源 |
| `POLYEDGE_WORKER__POLL_*` / `POLYEDGE_WORKER__ANALYZE_*` | `.env.api` | API 内嵌 worker 后台循环开关；新闻 poll 默认 `true`，High Probability observe/Smart Money/rewards/copytrade 等策略循环默认 `false`；不再包含旧 signal recompute 或 arbitrage radar 开关 |
| `POLYEDGE_WORKER__DATABASE_MAINTENANCE` / `POLYEDGE_WORKER__DATABASE_MAINTENANCE_INTERVAL_SECS` | `.env.api` | 数据库历史/缓存/队列表自动清理；生产模板默认 `true` / `3600`，本地模板默认关闭 |
| `POLYEDGE_REWARDS__AI_*` / `POLYEDGE_REWARDS__INFO_RISK_*` | `.env.api` | Rewards AI advisory / 信息风险 provider 的 key、base URL、模型、置信度等可选配置；主 provider 仍只配置 OpenAI-compatible 或 Anthropic，GLM/DeepSeek 通过 OpenAI-compatible base URL 与模型名识别，fallback 同理；AI provider 单次请求默认超时 180 秒；AI advisory 每轮最大市场数环境变量已移除，信息风险旧 max markets 变量只兼容读取且不再限制每轮扫描数量 |
| `POLYEDGE_SMART_MONEY__SIGNAL_ADVISORY_*` | `.env.api` | Smart Money signal advisory provider 的独立 key、base URL 和请求超时；provider/request format/model 由 Smart Money 配置保存，密钥不进入数据库或前端 DTO |
| `POLYEDGE_POLYMARKET__ACCOUNT_ID` / `SIGNATURE_TYPE` / `FUNDER` / `PRIVATE_KEY` / `API_*` / `POLYGON_RPC_URL` | `.env.api` | Polymarket live 账户、Funding API 入金和凭证 |
| `POLYEDGE_API_IMAGE` / `POLYEDGE_ORDERBOOK_IMAGE` / `POLYEDGE_FRONT_IMAGE` | 对应服务 env | 可选镜像 tag 覆盖；deploy.sh 会导出给 Compose interpolation |
| `POLYEDGE_ALLOW_IN_MEMORY_DEPLOY` | `.env.api` / `.env.orderbook` | 仅演示环境允许无数据库启动 |

`deploy.sh` 仍支持 `POLYEDGE_LOG_FILE`、`POLYEDGE_DEPLOY_LOCK_FILE`、`COMPOSE_PARALLEL_LIMIT`、`POLYEDGE_SKIP_SERVICES` 等脚本级变量；它们不是常规应用配置，未放入精简模板。

## Polymarket live 配置示例

Polymarket live、Deposit Wallet（`poly_1271`）、Funding API、rewards live 和 Smart Money signal advisory provider 最小开关示例已合并到 `deploy/.env.api.example`。真实凭证默认全部注释，按账户类型在 `.env.api` 中启用。Front/Orderbook 不持有 Polymarket 私钥或 AI provider key；余额、positions、托管订单、AI advisory、Smart Money signal advisory 和信息风险结果都由 API 内嵌 worker/数据库链路提供。`POLYEDGE_POLYMARKET__PRIVATE_KEY` 对应后端资金钱包，Funding API 会用它签名真实 Polygon USDC/USDT 转账；`POLYEDGE_POLYMARKET__FUNDER` 优先作为 Polymarket 入账钱包，未配置时回退 `ACCOUNT_ID`。`POLYEDGE_POLYMARKET__POLYGON_RPC_URL` 可替换为自有或有 SLA 的 Polygon RPC，用于链上 pUSD 余额回填和 Funding API 广播 Polygon 转账。

Deposit Wallet 路径要求钱包已经部署、已入金 pUSD 并完成必要 approval。当前系统不会执行 relayer wallet-create、pUSD 包装或 approval 批处理；connector 在下单前会调用 CLOB `balance-allowance/update`。

Rewards live worker 在 Postgres 路径持有一个 advisory lease 连接来串行化命令、full tick 和 reconcile；`POLYEDGE_POSTGRES__MAX_CONNECTIONS` 必须至少为 2，默认 20。

## 后端二进制构建

```bash
./scripts/build-backend-bin.sh   # cargo build --release（workspace: packages/backend/）→ bin/
git add bin/polyedge-api bin/polyedge-orderbook
```

只构建单个服务时可仅设置二进制名，脚本会自动选择同名 Cargo package，避免复制未重新编译的旧二进制：

```bash
POLYEDGE_BACKEND_BINARY=polyedge-orderbook ./scripts/build-backend-bin.sh
```

## 当前状态

- 部署模板适合原型/内网共享环境
- 默认生产排查环境：Frontend Rewards 工作台 `http://192.168.31.5:33002/rewards`，API `http://100.87.45.72:38001`，Orderbook `http://100.87.45.72:38002`
- `scripts/build-backend-bin.sh` 的单服务模式会让 `POLYEDGE_BACKEND_BINARY` 同时作为默认 Cargo package，确保 worker/orderbook 定向构建不会误编译 API 后复制旧目标文件。
- Compose 部署使用窄构建上下文：后端只上传 `bin/`，前端只上传 `packages/front/`，避免扫描 Rust `packages/backend/target/`、前端 `node_modules/`、`.next/` 等大目录
- 兼容用 `packages/backend/Dockerfile` 不再要求 `bin/polyedge-worker`，只复制默认构建脚本产出的 `polyedge-api` 和 `polyedge-orderbook`；Compose 仍使用 `deploy/api.Dockerfile` 与 `deploy/orderbook.Dockerfile`
- `polyedge-front` 不再依赖 API 健康后才启动；前端静态 Nginx 可独立运行，浏览器按 `NEXT_PUBLIC_POLYEDGE_API_BASE_URL` 访问 API
- `scripts/deploy.sh` 已防止重叠执行；前端变更 hash 包含 `packages/front/` 和 `deploy/.env.front`，会 prune `node_modules`、`.next`、`out` 等大目录，实际 build 前也会清理 `.next/` 和 `out/`，并用同一 front hash 版本化 HTML 中的静态资源引用；服务按目标独立部署，容器 down 且 hash 未变化时直接启动已有镜像，不会因其他服务健康失败而重复 rebuild
- 旧 `scripts/smoke-arbitrage-radar.sh` 已删除；`/radar` 页面和 `/api/v1/arbitrage/*` 端点不再作为部署冒烟目标
- 当前 `.env.api` 模板默认 `POLYEDGE_RUNTIME__ENVIRONMENT=production` 且 `POLYEDGE_AUTH__DISABLED=true`，API/front 内网交互不做权限校验；API CORS 为 permissive
- `deploy/.env.api.example` 默认启用 `POLYEDGE_WORKER__DATABASE_MAINTENANCE=true`，清理 raw events、AI/info-risk cache、reward candles、控制命令、copytrade 历史、outbox/dedup、LLM/audit 等可增长表；`packages/backend/.env.example` 为本地开发默认关闭该循环。
- Orderbook 服务 HTTP register/batch/ingest 入口按 `max_tokens` 和 `max_levels_per_side` 控制请求规模与缓存深度，写入时先排序再裁剪最优档位，registry source 固定上限为 32 个并在写锁内原子校验；register/ingest/delete 写接口还要求仅配置在 `.env.orderbook` / `.env.api` 的共享写 token，register 使用原子 source 替换。`/orderbook/stream` 是内部 WS 推送接口，worker rewards loop 用它更新本地盘口 cache，缺失或重连时仍通过 HTTP batch bootstrap
- 生产前需要：关闭 `POLYEDGE_AUTH__DISABLED`、接入真实会话体系、签名 JWT、key rotation

## 修改检查清单

- [ ] 新增服务时更新 `docker-compose.yml`
- [ ] 新增环境变量时更新对应的 `deploy/.env.{api,orderbook,front}.example` 和 `deploy.sh` 的验证逻辑
- [ ] 修改 nginx 路由时更新 `nginx.conf.template`
- [ ] 修改构建流程时更新 Dockerfile 和构建脚本
- [ ] 部署后验证所有容器健康检查通过
