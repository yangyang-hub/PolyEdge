# 部署（polyedge-server + polyedge-front）

最后更新：2026-07-15

## 概述

V3 保持前后端分离，但后端收敛为单进程。Docker Compose 只有：

- `polyedge-server`：Axum API、Postgres store、targeted orderbook 与多钱包执行 runtime；
- `polyedge-front`：Next.js 静态产物与 Nginx。

不再部署独立 API、worker、provider 或 orderbook 服务；不暴露 38002，不配置内部 orderbook URL/write token、candidate prewarm、Gamma/news/AI/fair-value/candle worker。

新部署必须使用空 PostgreSQL 和 V3 clean baseline，不兼容旧数据库。

## 架构

```text
browser
  -> polyedge-front:33002 (static)
  -> NEXT_PUBLIC_POLYEDGE_API_BASE_URL
  -> polyedge-server:38001
       ├── API + Postgres store
       ├── targeted CLOB REST poll cache
       ├── multi-wallet execution coordinator
       └── wallet secret resolver
  -> PostgreSQL / Polymarket CLOB
```

当前 server 不运行 Gamma/rewards catalog/full-market sync，也不运行 news/provider/fair-value。targeted supervisor 只轮询人工策略、managed open orders 和非零 positions 涉及的 token。

## 关键文件

| 文件 | 职责 |
|---|---|
| `deploy/docker-compose.yml` | `polyedge-server` 与 `polyedge-front` 编排 |
| `deploy/server.Dockerfile` | 从 `bin/` 复制 `polyedge-server` 的最小镜像 |
| `deploy/.env.server.example` | server/Postgres/CORS/auth/targeted orderbook/多钱包 secret 模板 |
| `deploy/.env.front.example` | 前端端口与 build-time API URL |
| `scripts/build-backend-bin.sh` | 只构建并复制 `polyedge-server` |
| `scripts/deploy.sh` | `auto|server|front|all` 部署、env 校验和 orphan 清理 |
| `packages/front/Dockerfile` | 静态前端镜像 |
| `packages/front/nginx.conf.template` | 静态资源与前端健康检查 |

## 端口与健康检查

- server 容器监听 `38001`；Compose 宿主映射由 `POLYEDGE_SERVER_BIND`、`POLYEDGE_SERVER_PORT` 控制。
- server 健康检查：`GET /healthz`；就绪检查：`GET /readyz`。
- front 默认映射 `0.0.0.0:33002 -> container:80`，Nginx 健康检查为 `/healthz`。
- 不存在 38002 服务或独立 orderbook health endpoint。

## Server 环境变量

### 必需/核心

| 变量 | 说明 |
|---|---|
| `POLYEDGE_SERVER__HOST` / `POLYEDGE_SERVER__PORT` | 容器内监听地址，默认 `0.0.0.0:38001` |
| `POLYEDGE_POSTGRES__URL` | V3 PostgreSQL 连接字符串 |
| `POLYEDGE_POSTGRES__MAX_CONNECTIONS` | 连接池上限 |
| `POLYEDGE_RUNTIME__ENVIRONMENT` | `local|production` 等环境名 |
| `POLYEDGE_CORS__ALLOWED_ORIGINS` | 浏览器 exact-origin allowlist；production 必须非空 |
| `POLYEDGE_SERVER__MAX_BODY_BYTES` | 请求体上限，默认 1 MiB |

### 鉴权

- `POLYEDGE_AUTH__DISABLED=false` 时，server 与部署脚本都要求至少 32 字符的 `POLYEDGE_AUTH__API_TOKEN`，请求使用 Bearer token。
- `POLYEDGE_AUTH__STEP_UP_CODE` 在 production 必须配置真实值且至少 16 字符；危险操作同时校验最小 scope header + code。该共享 code 仍只适合可信私网/身份代理后的当前部署形态。
- production 若 `POLYEDGE_AUTH__DISABLED=true`，必须设置 `POLYEDGE_AUTH__ALLOW_INSECURE_PRIVATE_DEPLOY=true`，并使用 VPN、私网 ACL 或可信访问代理形成边界。
- CORS 不是认证。静态前端目前没有生产级 session/token 获取流程。

部署脚本和 `.env.server.example` 已与 server 的 API-token 鉴权实现对齐；后续不得重新引入旧内部 JWT/orderbook token 配置。

### Targeted orderbook

| 变量 | 默认值 | 说明 |
|---|---:|---|
| `POLYEDGE_TARGETED_ORDERBOOK__MAX_TOKENS` | `1000` | 目标 token 总上限；超限整轮失败，不截断 |
| `POLYEDGE_TARGETED_ORDERBOOK__POLL_INTERVAL_MS` | `10000` | CLOB REST poll 周期 |

实际 quote freshness 只由每个策略版本的 `book_freshness_ms` 校验，不存在全局 stale-threshold 环境变量。

不再支持 `POLYEDGE_ORDERBOOK__SERVICE_URL`、`POLYEDGE_ORDERBOOK__WRITE_TOKEN`、WS chunk/connection、candidate cap 或 candle history 配置。

### 多钱包执行与 secrets

| 变量 | 默认值 | 说明 |
|---|---:|---|
| `POLYEDGE_EXECUTION__WALLET_CONCURRENCY` | `4` | 同时运行的钱包 job 上限 |
| `POLYEDGE_EXECUTION__RECONCILE_INTERVAL_MS` | `2000` | desired-state 对账周期 |
| `POLYEDGE_WALLET_SECRETS__ENV_PREFIX` | `POLYEDGE_WALLET_SECRET__` | environment secret 变量名前缀 |

locator `maker-primary` 规范化后读取：

```bash
POLYEDGE_WALLET_SECRET__MAKER_PRIMARY='{"private_key":"0x...","api_key":"...","api_secret":"...","api_passphrase":"..."}'
```

真实值不得提交到仓库、写入数据库、打印到部署日志或放入前端 public env。生产应由主机 secret manager/编排平台注入。

### Polymarket

- `POLYEDGE_POLYMARKET__CLOB_HOST`
- `POLYEDGE_POLYMARKET__DATA_API_HOST`：每钱包 positions 全量同步来源
- `POLYEDGE_POLYMARKET__CHAIN_ID`：CLOB signer chain id

账户地址、funder 与 signature type 来自数据库 wallet account；不再使用单账户全局 private key/account id。

## Front 环境变量

- `NEXT_PUBLIC_POLYEDGE_API_BASE_URL` 在 build time 写入静态 bundle，修改后必须重建。
- `NEXT_PUBLIC_POLYEDGE_CONSOLE_AUTH` 当前只支持 `off`；不要把长期 token 或 signing key 放入 public env。
- 浏览器直接访问 server，不经 Nginx API 反代。

## 构建与部署

```bash
./scripts/build-backend-bin.sh
cp deploy/.env.server.example deploy/.env.server
cp deploy/.env.front.example deploy/.env.front
./scripts/deploy.sh all
```

构建脚本固定执行 `cargo build --release -p polyedge-server`，输出 `bin/polyedge-server`。

部署目标：

```bash
./scripts/deploy.sh          # auto
./scripts/deploy.sh server
./scripts/deploy.sh front
./scripts/deploy.sh all
./scripts/deploy.sh server,front
```

Auto 模式获取文件锁、fast-forward、校验 env、按 hash 判断变化并只重建需要的服务。`up -d --remove-orphans` 会清理旧 `polyedge-api`、`polyedge-orderbook` 和其他不再属于 Compose 的容器。

## 当前状态与缺口

- Compose、Dockerfile、binary 和部署目标已统一为 `polyedge-server` + `polyedge-front`。
- 前端继续独立构建并通过 API base URL 直连后端。
- 部署模板已移除新闻、事件、AI/info-risk、fair-value、Gamma/full scan、candidate prewarm 和独立 orderbook 配置。
- server、env 模板与部署脚本在 enabled-auth 模式下统一校验至少 32 字符的 `POLYEDGE_AUTH__API_TOKEN`。
- server 启动时也会拒绝缺失或少于 16 个字符的 production `POLYEDGE_AUTH__STEP_UP_CODE`；部署脚本执行同等校验。
- targeted orderbook 当前为 REST poll；钱包 positions 已在 execution job 内同步，账户范围外部订单持续同步和生产 session UX 尚未部署。独立 fills、SELL exit、merge 与 Funding 不属于 V3 部署范围。

## 修改检查清单

- [ ] 修改 server env 时同步 `config.rs`、`.env.server.example`、`deploy.sh` 和本文件。
- [ ] 修改 frontend public env 后同步 `.env.front.example` 并重建静态产物。
- [ ] 修改端口、服务名、binary 或 health path 后同步 Compose/Dockerfile/scripts/root docs。
- [ ] 运行 `bash -n`、Compose config，并确认两个容器健康。
