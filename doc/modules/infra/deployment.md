# 部署（polyedge-server + polyedge-front）

最后更新：2026-07-21

## 概述

V4 仍只有两个容器：

- `polyedge-server`：Cookie-session API、Postgres store、targeted orderbook、钱包 envelope 和 subscription execution runtime；
- `polyedge-front`：Next.js 静态产物、Nginx 和同源 `/api/*` 反向代理。

不部署独立 API、worker、provider 或 orderbook 服务。两个容器可以在同一 Compose 主机运行，也可以分别部署在前端机和后端机；拆机不改变浏览器同源 Cookie/CSRF 边界。新部署必须使用空 PostgreSQL 和 V4 clean baseline，不兼容 V3 数据、共享 actor 或 credential locator。

## 架构

```text
browser -> polyedge-front:33002
             ├── static console
             └── same-origin /api/*
                   ├── same-host -> polyedge-server:38001
                   └── split-host -> backend-lan-ip:38001
                              -> PostgreSQL / Polymarket CLOB + Data API
```

同源代理是 Cookie/CSRF 唯一路径，`NEXT_PUBLIC_POLYEDGE_API_BASE_URL` 必须留空。默认同机 upstream 是 `http://polyedge-server:38001`；拆机时由 `POLYEDGE_FRONT_API_UPSTREAM` 指向后端内网 origin。浏览器不能直接跨 IP 调后端，因为 host-only `SameSite=Strict` session/CSRF Cookie 在 HTTP 跨站请求中不可用。

## 关键文件

| 文件 | 职责 |
|---|---|
| `deploy/docker-compose.yml` | 同机/拆机双服务编排、后端宿主端口和前端 upstream 注入 |
| `deploy/server.Dockerfile` | `polyedge-server` 最小镜像 |
| `deploy/.env.server.example` | Postgres、session/admin、wallet crypto、orderbook/execution 模板 |
| `deploy/.env.front.example` | 前端镜像/端口、远端 API upstream 与必须为空的 public API base |
| `scripts/build-backend-bin.sh` | 构建并复制唯一 server binary |
| `scripts/deploy.sh` | `auto|server|front|all`、env 校验、构建与 orphan 清理 |
| `packages/front/Dockerfile` + `nginx.conf.template` | 启动时 envsubst upstream、静态资源、安全 header、health 和 `/api` proxy |

## 端口与健康检查

- server 容器固定监听 `0.0.0.0:38001` 并保留 Compose `expose`；宿主发布默认 `127.0.0.1:38001`，拆机时可绑定后端内网 IPv4；`GET /healthz`、`GET /readyz`。
- front 默认 `0.0.0.0:33002 -> 80`，Nginx `/healthz` 返回静态健康状态。
- front `/healthz` 不检查远端 server；拆机部署还必须通过前端 `/api/v1/auth/me` 验证 upstream，未登录时预期返回后端 `401` 而不是 Nginx `502`。
- 不存在 38002 或独立 orderbook health endpoint。

## Server 环境变量

### HTTP、数据库与 identity

| 变量 | 说明 |
|---|---|
| `POLYEDGE_SERVER__HOST|PORT` | 通用 server 监听配置；标准 Compose 部署固定要求 `0.0.0.0:38001` |
| `POLYEDGE_SERVER_PUBLISH_BIND` | Compose 宿主发布 IPv4，默认 `127.0.0.1`；拆机时填写后端内网 IP |
| `POLYEDGE_SERVER_PUBLISH_PORT` | Compose 宿主发布端口，默认 `38001` |
| `POLYEDGE_SERVER__MAX_BODY_BYTES` | 请求体上限，默认 1 MiB |
| `POLYEDGE_POSTGRES__URL` | 必需的 V4 PostgreSQL URL |
| `POLYEDGE_POSTGRES__MAX_CONNECTIONS` | pool 上限，默认 20 |
| `POLYEDGE_RUNTIME__ENVIRONMENT` | 只接受 `local|production`；未知值启动失败，避免拼写错误降级安全策略 |
| `POLYEDGE_PUBLIC_ORIGIN` | 写请求 exact Origin；production 必须为 HTTPS |
| `POLYEDGE_CORS__ALLOWED_ORIGINS` | 可为空；非空时为逗号分隔 exact allowlist，始终拒绝 wildcard |
| `POLYEDGE_BOOTSTRAP_ADMIN__USERNAME` | 必需的环境管理员用户名 |
| `POLYEDGE_BOOTSTRAP_ADMIN__DISPLAY_NAME` | 可选显示名 |
| `POLYEDGE_BOOTSTRAP_ADMIN__PASSWORD_HASH` | 必需的 Argon2 PHC hash，不能放明文 |
| `POLYEDGE_BOOTSTRAP_ADMIN__CREDENTIAL_VERSION` | 正整数；增大才替换环境管理员 hash |
| `POLYEDGE_AUTH__SESSION_IDLE_SECONDS` | idle TTL，默认 1800 |
| `POLYEDGE_AUTH__SESSION_ABSOLUTE_SECONDS` | absolute TTL，默认 28800 |
| `POLYEDGE_AUTH__ACTIVATION_TTL_SECONDS` | 一次性激活 TTL，默认 86400 |
| `POLYEDGE_AUTH__RECENT_AUTH_TTL_SECONDS` | 危险操作 recent-auth TTL，默认 300 |

旧 `POLYEDGE_AUTH__API_TOKEN`、`AUTH__DISABLED`、`ALLOW_INSECURE_PRIVATE_DEPLOY`、`STEP_UP_CODE` 不再被 server 或部署脚本使用。管理员 bootstrap 在启动时执行；若 hash、RSA key 或 storage key 不合法，server fail closed。空 CORS 表示不开放浏览器跨源调用，不会关闭 `Origin == POLYEDGE_PUBLIC_ORIGIN` 或 CSRF 检查。

### Wallet cryptography

| 变量 | 说明 |
|---|---|
| `POLYEDGE_WALLET_CRYPTO__TRANSPORT_PRIVATE_KEY_PEM` | RSA 私钥 PEM 原文；`.env` 推荐单行并用 `\n` 表示换行；PKCS#8 或 PKCS#1，至少 2048 bit |
| `POLYEDGE_WALLET_CRYPTO__TRANSPORT_KEY_ID` | transport key id，默认 `wallet-import-rsa-v1` |
| `POLYEDGE_WALLET_CRYPTO__STORAGE_KEY_ID` | storage key id，默认 `wallet-storage-aes-v1` |
| `POLYEDGE_WALLET_CRYPTO__STORAGE_KEY` | 标准 base64，解码后精确 32 bytes（可用 `openssl rand -base64 32`） |
| `POLYEDGE_WALLET_CRYPTO__IMPORT_CONTEXT_TTL_SECONDS` | 默认 300，最大 3600 |
| `POLYEDGE_WALLET_CRYPTO__MAX_IMPORT_CONTEXTS` | 默认 1024，最大 100000 |

RSA PEM 与 storage key 直接写在 server `.env`（经 Compose `env_file` 注入），不得提交仓库、写入前端 public env 或打印到日志。旧 `*_FILE` / `POLYEDGE_WALLET_IMPORT_PRIVATE_KEY_FILE` / `POLYEDGE_WALLET_STORAGE_KEY_FILE` 与 Compose secret mount 已移除；若仍配置这些变量，deploy 与 server 都会 fail closed。部署脚本校验 PEM 形态与 storage key 长度；server 再校验 PEM、RSA 位数和密钥内容。当前 storage KEK 是单活动 key id，不是 KMS；没有在线旧 keyring 轮换。

### Orderbook、execution 与 Polymarket

- `POLYEDGE_TARGETED_ORDERBOOK__MAX_TOKENS`（默认 1000）和 `POLL_INTERVAL_MS`（默认 10000）。超限整轮失败；freshness 只由策略版本控制。
- `POLYEDGE_EXECUTION__WALLET_CONCURRENCY`（默认 4）和 `RECONCILE_INTERVAL_MS`（默认 2000）。跟随传播尚无 NOTIFY，延迟受 reconcile interval 影响。
- `POLYEDGE_POLYMARKET__CLOB_HOST|DATA_API_HOST|CHAIN_ID`。账户地址、funder、signature type 和加密 credential 来自数据库钱包。

## Front 环境变量与 Nginx

- `NEXT_PUBLIC_POLYEDGE_API_BASE_URL=`：生产部署的唯一允许值，浏览器请求相对 `/api/v1/*`，由 Nginx 转发。
- `POLYEDGE_FRONT_API_UPSTREAM`：Nginx 运行时 upstream，默认 `http://polyedge-server:38001`；拆机填写 `http://<后端内网 IP>:38001`，不允许 path/query/fragment。
- `packages/front/.env.example` 可为 Next.js 本地开发填写 exact backend origin，但必须同时匹配 server CORS 与 `POLYEDGE_PUBLIC_ORIGIN`；`deploy/.env.front` 不允许非空。
- Nginx 设置 CSP `connect-src 'self'`，因此生产静态镜像不应构建进跨 origin API URL。
- Next.js static export 会内联 hydration/Flight bootstrap；静态 Nginx 无法逐响应注入 nonce，因此当前 `script-src` 限定 `'self'` 并允许 `'unsafe-inline'`，同时设置 `object-src 'none'` 与 `frame-ancestors 'none'`。若改为动态前端，应使用逐请求 nonce 并移除 `'unsafe-inline'`。

## 构建与部署

同机部署：

```bash
./scripts/build-backend-bin.sh
cp deploy/.env.server.example deploy/.env.server
cp deploy/.env.front.example deploy/.env.front
# 在 .env.server 填入 RSA PEM、32-byte base64 storage key 与真实 Argon2 hash 后：
./scripts/deploy.sh all
```

拆机 HTTP 内网示例（`10.0.0.10` 为前端，`10.0.0.20` 为后端）：

```dotenv
# 后端机 deploy/.env.server
POLYEDGE_SERVER_PUBLISH_BIND=10.0.0.20
POLYEDGE_SERVER_PUBLISH_PORT=38001
POLYEDGE_RUNTIME__ENVIRONMENT=local
POLYEDGE_PUBLIC_ORIGIN=http://10.0.0.10:33002
POLYEDGE_CORS__ALLOWED_ORIGINS=
```

```dotenv
# 前端机 deploy/.env.front
POLYEDGE_FRONT_API_UPSTREAM=http://10.0.0.20:38001
NEXT_PUBLIC_POLYEDGE_API_BASE_URL=
```

```bash
# 后端机
./scripts/deploy.sh server
# 后端机若使用周期 auto
POLYEDGE_SKIP_SERVICES=front ./scripts/deploy.sh auto

# 前端机；脚本对 front-only 自动使用 Compose --no-deps
./scripts/deploy.sh front
# 前端机若使用周期 auto
POLYEDGE_SKIP_SERVICES=server ./scripts/deploy.sh auto
```

HTTP-only 内网必须使用 `local`，因为 `production` 强制 HTTPS public origin 和 Secure Cookie。`POLYEDGE_PUBLIC_ORIGIN` 必须是浏览器实际访问的前端 origin，不是后端地址。后端 38001 的主机防火墙必须只允许前端服务器访问；CORS 不是网络 ACL，后端还会将代理提供的来源地址用于认证限流。

部署脚本要求 Linux/Bash 4.3+，并校验固定容器监听、宿主 publish IPv4/端口、Postgres URL、`local|production`、production HTTPS public origin、可选 exact CORS、真实 Argon2 hash、wallet crypto env、前端 upstream，以及空的 public API base。Auto 模式使用文件锁、fast-forward、hash 增量构建和 `--remove-orphans`；front-only restart 使用 `--no-deps`，不会在前端机拉起本地 server。`deploy/.env.server` 内容进入 server hash，修改密钥会触发重启。

## 当前状态与缺口

- Compose、Nginx、env 模板和部署脚本支持同机或拆机，同时保持浏览器同源 session/RBAC 与 wallet envelope 边界。
- targeted orderbook 仍为 REST poll；账户范围外部订单持续同步未完成。
- managed order 累计成交差额与 position 同步会写入操作性 fill/partial equity，但没有权威 venue fill ingestion 或完整 valuation/PnL producer；管理员财务页不能视为完整盈利核算。
- 尚无 TLS termination 服务定义；生产 HTTPS 应由外层 ingress/reverse proxy 提供，并将公开 origin 配置为最终浏览器 origin。
- HTTP 内网拆机 hop 没有传输加密；其安全性依赖可信网络、后端 LAN bind 和只允许前端主机访问的防火墙。Nginx upstream 变更需要重启 front 容器。
- 默认宿主发布 `127.0.0.1:38001` 可能与已有进程冲突，可通过 `POLYEDGE_SERVER_PUBLISH_PORT` 调整；当前 upstream/publish 校验只覆盖 hostname/IPv4，不支持 IPv6。
- 尚无 RSA/storage key 自动生成、KMS、在线轮换、数据库备份恢复和实盘 runbook。

## 修改检查清单

- [ ] 修改 server env 时同步 `config.rs` / `wallet_crypto.rs`、`.env.server.example`、`deploy.sh` 和本文件；Compose 固定端口必须继续一致。
- [ ] 修改 Cookie/origin/proxy 时同时验证 Nginx、CORS、CSRF 和 production HTTPS。
- [ ] 修改端口、服务名、upstream、binary 或 health path 后同步 Compose/Dockerfile/scripts/root docs。
- [ ] 运行 `bash -n`、Compose config、前后端验证和两个容器 health smoke。
