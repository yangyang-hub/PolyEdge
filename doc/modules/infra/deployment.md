# 部署（Docker + Nginx + Scripts）

最后更新：2026-07-12

## 概述

部署体系基于 Docker Compose，包含 3 个服务：`polyedge-api`（API + 内嵌 worker runtime）、`polyedge-orderbook`、`polyedge-front`。前端是静态站点，浏览器通过 `NEXT_PUBLIC_POLYEDGE_API_BASE_URL` 直连 Rust API；API 使用 exact-origin CORS allowlist，支持 front/API 分别部署在不同主机，同时拒绝未授权网站跨域调用。

当前部署只保留市场数据、新闻/事件、LP rewards、Funding、settings 和执行/对账相关服务配置。旧钱包类与独立研究 worker/env 已从模板中删除。

## 默认生产排查环境

| 服务 | 地址 | 说明 |
|---|---|---|
| Frontend Rewards 工作台 | `http://192.168.31.5:33002/rewards` | 浏览器入口 |
| API 服务 | `http://100.87.45.72:38001` | 前端 `NEXT_PUBLIC_POLYEDGE_API_BASE_URL` 指向该地址 |
| Orderbook 服务 | `http://100.87.45.72:38002` | 盘口 HTTP、stats、内部 stream |

同一 Compose 项目内，API 访问 orderbook 可使用 `http://polyedge-orderbook:38002`。跨服务器或宿主机排查时使用实际内网地址，容器内不能用 `localhost` 指向另一个服务。

## 架构与关键文件

| 文件 | 职责 |
|---|---|
| `deploy/docker-compose.yml` | 服务编排 |
| `deploy/api.Dockerfile` | API/Worker 合并镜像，复制 `bin/polyedge-api` |
| `deploy/orderbook.Dockerfile` | Orderbook 镜像，复制 `bin/polyedge-orderbook` |
| `deploy/.env.api.example` | API + 内嵌 worker 环境变量模板 |
| `deploy/.env.orderbook.example` | Orderbook 环境变量模板 |
| `deploy/.env.front.example` | 前端端口和 build-time public API URL 模板 |
| `scripts/deploy.sh` | 部署脚本（auto/manual） |
| `scripts/build-backend-bin.sh` | Rust backend 二进制构建脚本 |
| `packages/front/Dockerfile` | 静态前端镜像 |
| `packages/front/nginx.conf.template` | Nginx 静态文件配置 |
| `packages/front/src/app/layout.tsx` + `globals.css` | 根布局与系统字体栈；构建时不下载远程字体 |

## 服务架构

```text
polyedge-front (nginx static)
    -> browser calls NEXT_PUBLIC_POLYEDGE_API_BASE_URL
    -> polyedge-api (API + WorkerRuntime)
    -> polyedge-orderbook (market sync + orderbook cache)
    -> PostgreSQL
```

## polyedge-api

- 端口：`0.0.0.0:38001 -> container:38001`。
- 健康检查：`GET /healthz`。
- 环境变量：`.env.api`。
- `POLYEDGE_CORS__ALLOWED_ORIGINS` 必须列出浏览器实际 frontend origin；production 为空、包含 `*` 或带路径时 API 拒绝启动。
- 通过 `POLYEDGE_ORDERBOOK__SERVICE_URL` 访问 orderbook 服务。
- `POLYEDGE_ORDERBOOK__WRITE_TOKEN` 必须与 `.env.orderbook` 一致，用于 worker 注册 token。
- 启动时内嵌 `WorkerRuntime`，后台任务由 `POLYEDGE_WORKER__*` 和业务 settings 控制。
- 模板默认开启新闻采集和数据库维护，rewards live、info-risk、execution drain、paper/live 对账和 Polymarket 用户事件等任务默认关闭，需要真实凭证和运营准备后再开启。
- Polymarket live、Deposit Wallet、Funding API 和 AI provider 相关密钥只放 `.env.api`，不得放入 front/orderbook。
- Funding API 使用同一后端私钥和 Polygon RPC 执行真实 USDC/USDT Bridge 入金转账；`FUNDER` 优先作为 Polymarket 入账地址，未配置时回退 `ACCOUNT_ID`。

## polyedge-orderbook

- 端口：`0.0.0.0:38002 -> container:38002`。
- 健康检查：`GET /healthz`。
- 环境变量：`.env.orderbook`。
- 职责：Gamma market sync、CLOB rewards catalog sync、price-history candle sync、CLOB WS + `/books` poll cache、HTTP 盘口 API、内部 `/orderbook/stream`、token registry。
- 启动时先 bind HTTP 暴露健康检查，再后台执行初始同步，避免外部 API 慢响应阻塞容器健康。
- register/ingest/delete、内部 stream，以及带 `refresh_if_stale_ms` 的 batch 请求要求 `POLYEDGE_ORDERBOOK__WRITE_TOKEN`；cache-only 盘口/batch、stats 和 health 不鉴权，仍需依赖内网边界。
- Polymarket market-channel 默认目标 chunk 为 500 token、最多 8 条连接；有效 chunk 会自动放大以满足连接预算，连接按 500ms 错峰启动，SDK 以 30-120 秒退避重连，降低同一出口 IP 触发 Cloudflare 429/1015 的风险。

## polyedge-front

- 端口：`0.0.0.0:33002 -> container:80`。
- 本机先 `yarn build` 生成 `out/`，镜像只复制静态文件到 nginx。
- `NEXT_PUBLIC_POLYEDGE_API_BASE_URL` 在 build time 写入静态 JS bundle；修改 API 地址后必须重建前端。
- `NEXT_PUBLIC_POLYEDGE_CONSOLE_AUTH` 当前唯一支持值为 `off`；静态前端尚无登录/session/Bearer token 获取链路。
- 字体使用本机/容器系统字体栈，`next build` 不访问 Google Fonts，内网构建不需要公共字体网络。
- 当前内网免鉴权模式不需要设置 dev-auth header。
- `scripts/deploy.sh` build 前会清理 `.next/` 和 `out/`，并给 HTML 中的 static 资源引用追加 front hash query，避免浏览器复用旧 bundle。

## Nginx 配置

| 路径 | 行为 |
|---|---|
| `/healthz` | 返回 200 |
| `/_next/static/` | 静态资源，`Cache-Control: no-cache, must-revalidate` |
| `/` | 静态文件服务，fallback 到 `$uri.html` 和 `/404.html` |

API 请求不经过前端 nginx 反代；跨域由 Rust API 的精确 allowlist 处理。同源部署可保持空列表，但 production 模板要求显式配置，以避免前端/API 拆分后静默不可用。

## deploy.sh

Auto 模式：

1. 获取部署锁，避免重叠执行。
2. `git fetch` + fast-forward。
3. 无镜像变更且容器运行中则跳过。
4. API/orderbook/front 按变更独立重建和重启。
5. 容器未运行但镜像 hash 未变化时只 `up -d` 启动已有镜像。

Manual 模式：

- `scripts/deploy.sh all`
- `scripts/deploy.sh api`
- `scripts/deploy.sh orderbook`
- `scripts/deploy.sh front`
- 支持组合：`api orderbook`、`api,front` 等。

环境变量验证：

- `POLYEDGE_POSTGRES__URL` 不能包含 `change-me`。
- production 的 `POLYEDGE_CORS__ALLOWED_ORIGINS` 不能为空且不能包含通配符。
- API 与 orderbook 的 `POLYEDGE_ORDERBOOK__WRITE_TOKEN` 必须存在且一致。
- `POLYEDGE_AUTH__DISABLED=false` 时，`POLYEDGE_AUTH__KEYS_JSON` 必须包含至少一个真实 Ed25519 公钥；仅配置 step-up code 无效，因为 production JWT 路径不读取它。
- `.env.front` 的 `NEXT_PUBLIC_POLYEDGE_CONSOLE_AUTH` 应保持 `off`；前端类型和运行时会把该设置收敛为 `off`，不能据此认为已接入会话体系。
- production 使用 `POLYEDGE_AUTH__DISABLED=true` 时必须显式设置 `POLYEDGE_AUTH__ALLOW_INSECURE_PRIVATE_DEPLOY=true`；API 和部署脚本都会拒绝缺少该确认的配置。该确认不提供安全能力，仍必须使用 VPN、私网 ACL 或可信访问代理隔离 API。
- 未关闭鉴权时，dev bypass 仅允许 local environment。

## 必需环境变量

| 变量 | 位置 | 说明 |
|---|---|---|
| `POLYEDGE_POSTGRES__URL` | `.env.api` / `.env.orderbook` | PostgreSQL 连接字符串 |
| `POLYEDGE_ORDERBOOK__WRITE_TOKEN` | `.env.api` / `.env.orderbook` | orderbook 写接口共享密钥 |
| `POLYEDGE_ORDERBOOK__SERVICE_URL` | `.env.api` | API/worker 访问 orderbook 的 HTTP 地址 |
| `NEXT_PUBLIC_POLYEDGE_API_BASE_URL` | `.env.front` | 前端 build-time API 地址 |
| `NEXT_PUBLIC_POLYEDGE_CONSOLE_AUTH` | `.env.front` | 当前固定为 `off`；真实会话模式尚未接入 |

## 常用可选环境变量

| 变量/前缀 | 位置 | 说明 |
|---|---|---|
| `POLYEDGE_RUNTIME__ENVIRONMENT` | `.env.api` / `.env.orderbook` | 部署模板为 `production` |
| `POLYEDGE_CORS__ALLOWED_ORIGINS` | `.env.api` | 逗号分隔的 frontend exact-origin allowlist |
| `POLYEDGE_AUTH__DISABLED` / `POLYEDGE_AUTH__ALLOW_INSECURE_PRIVATE_DEPLOY` / `POLYEDGE_AUTH__STEP_UP_CODE` / `POLYEDGE_AUTH__KEYS_JSON` | `.env.api` | 鉴权配置；关闭 production 鉴权需显式私网风险确认，step-up code 仅 local dev，production 使用 JWT claims |
| `POLYEDGE_API_BIND` / `POLYEDGE_API_PORT` | `.env.api` | API 宿主机暴露地址和端口 |
| `POLYEDGE_FRONT_BIND` / `POLYEDGE_FRONT_PORT` | `.env.front` | 前端宿主机端口 |
| `POLYEDGE_ORDERBOOK_STREAM__MAX_TOKENS` / `WS_CHUNK_SIZE` / `WS_MAX_CONNECTIONS` / `MAX_LEVELS_PER_SIDE` / `STALE_THRESHOLD_MS` | `.env.orderbook` | orderbook token 容量、WS 连接预算、盘口深度和 stale reconcile 调参 |
| `POLYEDGE_NEWS__ENABLED` / `POLYEDGE_NEWS__SOURCES_JSON` | `.env.api` | 新闻采集开关和 source 列表 |
| `POLYEDGE_WORKER__POLL_NEWS` / `PROMOTE_NEWS_EVENTS` / `DATABASE_MAINTENANCE` / `POLL_REWARD_BOT` / `POLL_REWARD_INFO_RISKS` | `.env.api` | API 内嵌 worker 循环开关 |
| `POLYEDGE_WORKER__DRAIN_EXECUTION_QUEUE` / `POLL_*ORDER_STATUSES` / `RECONCILE_*FILLS` / `CONSUME_POLYMARKET_USER_EVENTS` | `.env.api` | 执行/对账 worker 开关 |
| `POLYEDGE_REWARDS__ENABLED` / `POLYEDGE_REWARDS__AI_*` / `POLYEDGE_REWARDS__INFO_RISK_*` | `.env.api` | LP rewards 和 provider 配置 |
| `POLYEDGE_POLYMARKET__ACCOUNT_ID` / `SIGNATURE_TYPE` / `FUNDER` / `PRIVATE_KEY` / `API_*` / `POLYGON_RPC_URL` | `.env.api` | Polymarket live、Funding 和链上余额配置 |
| `POLYEDGE_API_IMAGE` / `POLYEDGE_ORDERBOOK_IMAGE` / `POLYEDGE_FRONT_IMAGE` | 对应 env | 镜像 tag 覆盖 |
| `POLYEDGE_ALLOW_IN_MEMORY_DEPLOY` | `.env.api` / `.env.orderbook` | 仅演示环境允许无数据库启动 |

## Polymarket live / Funding

真实凭证默认在模板中注释。Front/Orderbook 不持有 Polymarket 私钥或 AI provider key；余额、positions、托管订单、AI advisory 和信息风险结果都由 API 内嵌 worker/数据库链路提供。

Deposit Wallet 路径要求钱包已部署、已入金 pUSD 并完成必要 approval。当前系统不会执行 relayer wallet-create、pUSD 包装或 approval 批处理；connector 在下单前会调用 CLOB `balance-allowance/update`。

Rewards live worker 在 Postgres 路径持有一个 advisory lease 连接来串行化命令、full tick 和 reconcile；`POLYEDGE_POSTGRES__MAX_CONNECTIONS` 必须至少为 2，默认 20。

## 后端二进制构建

```bash
./scripts/build-backend-bin.sh
git add bin/polyedge-api bin/polyedge-orderbook
```

只构建单个服务：

```bash
POLYEDGE_BACKEND_BINARY=polyedge-orderbook ./scripts/build-backend-bin.sh
```

## 当前状态

- 当前静态前端没有登录/session/Bearer token 获取链路，模板默认保持免鉴权以保证部署可用，同时要求 exact-origin CORS；production API 会记录安全告警，部署必须依赖 VPN、私网 ACL 或可信反向代理限制 API 访问。CORS 本身不是鉴权。
- 默认生产排查入口为 Frontend Rewards `http://192.168.31.5:33002/rewards`、API `http://100.87.45.72:38001`、Orderbook `http://100.87.45.72:38002`。
- Orderbook 部署默认使用 `WS_CHUNK_SIZE=500`、`WS_MAX_CONNECTIONS=8`；即使 runtime config 残留旧的 100-token chunk，有效 chunk 仍会按连接预算自动收敛。
- Compose 使用窄构建上下文：后端只上传 `bin/`，前端只上传 `packages/front/`。
- `polyedge-front` 可独立运行，浏览器按 build-time API URL 访问后端。
- 前端生产构建不依赖 Google Fonts 或其他远程字体下载。
- `deploy/.env.api.example` 默认启用 database maintenance，清理 raw events、AI/info-risk cache、reward candles、控制命令、outbox/dedup、LLM/audit 等可增长表。
- Rewards poll loop 启动时会同时启动 Postgres-only durable action executor；无需也没有单独的 Compose worker/executor 服务开关。CLI 的 `poll-reward-action-executor` 主要用于独立排障。
- 若关闭免鉴权，必须先接入真实身份签发网关、前端短时 request-bound JWT 传输和 key rotation；不能把私钥或长期 JWT 放入静态前端 bundle。production step-up 由 JWT claims 表达。

## 修改检查清单

- [ ] 新增服务时更新 `docker-compose.yml`。
- [ ] 新增环境变量时更新对应 `deploy/.env.{api,orderbook,front}.example` 和 `deploy.sh` 验证逻辑。
- [ ] 修改 nginx 路由时更新 `nginx.conf.template`。
- [ ] 修改构建流程时更新 Dockerfile 和构建脚本。
- [ ] 部署后验证所有容器健康检查通过。
