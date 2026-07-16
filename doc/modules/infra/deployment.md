# 部署（polyedge-server + polyedge-front）

最后更新：2026-07-16

## 概述

V4 仍只有两个容器：

- `polyedge-server`：Cookie-session API、Postgres store、targeted orderbook、钱包 envelope 和 subscription execution runtime；
- `polyedge-front`：Next.js 静态产物、Nginx 和同源 `/api/*` 反向代理。

不部署独立 API、worker、provider 或 orderbook 服务。新部署必须使用空 PostgreSQL 和 V4 clean baseline，不兼容 V3 数据、共享 actor 或 credential locator。

## 架构

```text
browser https://polyedge.example.com
  -> polyedge-front:80
       ├── static console
       └── /api/* proxy -> polyedge-server:38001
  -> PostgreSQL / Polymarket CLOB + Data API
```

同源代理是生产 Cookie/CSRF 主路径。`NEXT_PUBLIC_POLYEDGE_API_BASE_URL` 默认留空；server 的宿主 38001 映射主要供运维访问，可按网络边界收紧 `POLYEDGE_SERVER_BIND`。

## 关键文件

| 文件 | 职责 |
|---|---|
| `deploy/docker-compose.yml` | 双服务编排与 RSA PEM 只读挂载 |
| `deploy/server.Dockerfile` | `polyedge-server` 最小镜像 |
| `deploy/.env.server.example` | Postgres、session/admin、wallet crypto、orderbook/execution 模板 |
| `deploy/.env.front.example` | 前端端口与可选 API base |
| `scripts/build-backend-bin.sh` | 构建并复制唯一 server binary |
| `scripts/deploy.sh` | `auto|server|front|all`、env 校验、构建与 orphan 清理 |
| `packages/front/nginx.conf.template` | 静态资源、安全 header、health 和 `/api` proxy |

## 端口与健康检查

- server 容器监听 `38001`，宿主映射由 `POLYEDGE_SERVER_BIND|PORT` 控制；`GET /healthz`、`GET /readyz`。
- front 默认 `0.0.0.0:33002 -> 80`，Nginx `/healthz` 返回静态健康状态。
- 不存在 38002 或独立 orderbook health endpoint。

## Server 环境变量

### HTTP、数据库与 identity

| 变量 | 说明 |
|---|---|
| `POLYEDGE_SERVER__HOST|PORT` | 容器监听，默认 `0.0.0.0:38001` |
| `POLYEDGE_SERVER__MAX_BODY_BYTES` | 请求体上限，默认 1 MiB |
| `POLYEDGE_POSTGRES__URL` | 必需的 V4 PostgreSQL URL |
| `POLYEDGE_POSTGRES__MAX_CONNECTIONS` | pool 上限，默认 20 |
| `POLYEDGE_RUNTIME__ENVIRONMENT` | `local|production` 等环境名 |
| `POLYEDGE_PUBLIC_ORIGIN` | 写请求 exact Origin；production 必须为 HTTPS |
| `POLYEDGE_CORS__ALLOWED_ORIGINS` | exact allowlist；production 非空，不接受 wildcard |
| `POLYEDGE_BOOTSTRAP_ADMIN__USERNAME` | 必需的环境管理员用户名 |
| `POLYEDGE_BOOTSTRAP_ADMIN__DISPLAY_NAME` | 可选显示名 |
| `POLYEDGE_BOOTSTRAP_ADMIN__PASSWORD_HASH` | 必需的 Argon2 PHC hash，不能放明文 |
| `POLYEDGE_BOOTSTRAP_ADMIN__CREDENTIAL_VERSION` | 正整数；增大才替换环境管理员 hash |
| `POLYEDGE_AUTH__SESSION_IDLE_SECONDS` | idle TTL，默认 1800 |
| `POLYEDGE_AUTH__SESSION_ABSOLUTE_SECONDS` | absolute TTL，默认 28800 |
| `POLYEDGE_AUTH__ACTIVATION_TTL_SECONDS` | 一次性激活 TTL，默认 86400 |
| `POLYEDGE_AUTH__RECENT_AUTH_TTL_SECONDS` | 危险操作 recent-auth TTL，默认 300 |

旧 `POLYEDGE_AUTH__API_TOKEN`、`AUTH__DISABLED`、`ALLOW_INSECURE_PRIVATE_DEPLOY`、`STEP_UP_CODE` 不再被 server 或部署脚本使用。管理员 bootstrap 在启动时执行；若 hash、RSA key 或 storage key 不合法，server fail closed。

### Wallet cryptography

| 变量 | 说明 |
|---|---|
| `POLYEDGE_WALLET_IMPORT_PRIVATE_KEY_FILE` | Compose 宿主 RSA PEM 路径，默认 `deploy/secrets/polyedge-wallet-import-private.pem` |
| `POLYEDGE_WALLET_CRYPTO__TRANSPORT_PRIVATE_KEY_PEM_FILE` | 容器内 PEM 路径，模板为 `/run/secrets/polyedge-wallet-import-private.pem` |
| `POLYEDGE_WALLET_CRYPTO__TRANSPORT_KEY_ID` | transport key id，默认 `wallet-import-rsa-v1` |
| `POLYEDGE_WALLET_CRYPTO__STORAGE_KEY_ID` | storage key id，默认 `wallet-storage-aes-v1` |
| `POLYEDGE_WALLET_STORAGE_KEY_FILE` | Compose 宿主 storage key 文件路径 |
| `POLYEDGE_WALLET_CRYPTO__STORAGE_KEY_FILE` | 容器内 storage key 文件；标准 base64 解码后精确 32 bytes |
| `POLYEDGE_WALLET_CRYPTO__IMPORT_CONTEXT_TTL_SECONDS` | 默认 300，最大 3600 |
| `POLYEDGE_WALLET_CRYPTO__MAX_IMPORT_CONTEXTS` | 默认 1024，最大 100000 |

RSA PEM 和 storage key 必须由 secret manager/受控宿主文件只读注入，Unix 文件权限应为 `0600`，不得提交仓库、写入前端 public env 或打印到日志。当前 storage KEK 是单活动 key id，不是 KMS；没有在线旧 keyring 轮换。Compose 同时挂载 RSA PEM 与 storage key 文件。

### Orderbook、execution 与 Polymarket

- `POLYEDGE_TARGETED_ORDERBOOK__MAX_TOKENS`（默认 1000）和 `POLL_INTERVAL_MS`（默认 10000）。超限整轮失败；freshness 只由策略版本控制。
- `POLYEDGE_EXECUTION__WALLET_CONCURRENCY`（默认 4）和 `RECONCILE_INTERVAL_MS`（默认 2000）。跟随传播尚无 NOTIFY，延迟受 reconcile interval 影响。
- `POLYEDGE_POLYMARKET__CLOB_HOST|DATA_API_HOST|CHAIN_ID`。账户地址、funder、signature type 和加密 credential 来自数据库钱包。

## Front 环境变量与 Nginx

- `NEXT_PUBLIC_POLYEDGE_API_BASE_URL=`：推荐生产值，浏览器请求相对 `/api/v1/*`，由 Nginx 转发。
- 本地跨 origin 开发可填写 exact backend origin，但必须同时匹配 server CORS 与 `POLYEDGE_PUBLIC_ORIGIN`；Cookie session 的生产路径仍应同源。
- Nginx 设置 CSP `connect-src 'self'`，因此生产静态镜像不应构建进跨 origin API URL。
- Next.js static export 会内联 hydration/Flight bootstrap；静态 Nginx 无法逐响应注入 nonce，因此当前 `script-src` 限定 `'self'` 并允许 `'unsafe-inline'`，同时设置 `object-src 'none'` 与 `frame-ancestors 'none'`。若改为动态前端，应使用逐请求 nonce 并移除 `'unsafe-inline'`。
- 已删除旧 `NEXT_PUBLIC_POLYEDGE_CONSOLE_AUTH` 和 internal auth bypass 部署变量。

## 构建与部署

```bash
./scripts/build-backend-bin.sh
cp deploy/.env.server.example deploy/.env.server
cp deploy/.env.front.example deploy/.env.front
# 生成 RSA 私钥并设置真实 Argon2 hash/storage key 后：
./scripts/deploy.sh all
```

部署脚本校验 Postgres URL、production HTTPS public origin、exact CORS、真实 Argon2 hash、wallet crypto 路径和 32-byte base64 storage key；前端 API base 可为空。Auto 模式使用文件锁、fast-forward、hash 增量构建和 `--remove-orphans`。

## 当前状态与缺口

- Compose、Nginx、env 模板和部署脚本已切换到同源 session/RBAC 与 wallet envelope 配置。
- targeted orderbook 仍为 REST poll；账户范围外部订单持续同步未完成。
- schema 有 fills/equity 表，但部署中没有 fill/valuation/PnL worker；管理员财务页不能视为完整盈利核算。
- 尚无 TLS termination 服务定义；生产 HTTPS 应由外层 ingress/reverse proxy 提供，并将公开 origin 配置为最终浏览器 origin。
- 尚无 RSA/storage key 自动生成、KMS、在线轮换、数据库备份恢复和实盘 runbook。

## 修改检查清单

- [ ] 修改 server env 时同步 `config.rs`、`.env.server.example`、`deploy.sh` 和本文件。
- [ ] 修改 Cookie/origin/proxy 时同时验证 Nginx、CORS、CSRF 和 production HTTPS。
- [ ] 修改端口、服务名、binary 或 health path 后同步 Compose/Dockerfile/scripts/root docs。
- [ ] 运行 `bash -n`、Compose config、前后端验证和两个容器 health smoke。
