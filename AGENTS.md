# Agent Guidelines

最后更新：2026-07-21

## 维护规则

- **模块文档优先**：修改任何模块前，必须先查阅 `doc/modules/` 下对应模块文档（索引见 [doc/modules/README.md](./doc/modules/README.md)）；修改后同步更新日期、关键文件、数据结构、当前状态和已知缺口。
- 任何改变行为、路由、命令、环境变量、部署方式、依赖或集成状态的改动，都必须同步更新本文件。
- 不要把设计目标写成已实现能力。若本文件、README、页面文案或历史设计冲突，以本文件为仓库状态快照优先修正。
- V4 是破坏性 clean deploy，不兼容旧数据库，也不保留旧 API、共享 actor、credential locator、worker/orderbook service 或前端路由兼容层。

## 当前聚焦

PolyEdge V4 是多用户、人工配置的 Polymarket 做市执行系统。管理员通过环境变量 bootstrap；其他用户只能由管理员创建并通过一次性 token 激活。市场录入用户配置 condition、YES/NO token、rewards 快照、策略有效期、quote slots 和自有钱包，也可以使用自己的钱包跟随其他用户公开为 `followable` 的策略。

活动角色为：

- `admin`：用户管理、全局业务读取、系统状态和管理员资金汇总；环境管理员不能通过 API 禁用或降权。
- `market_editor`：管理自己的钱包和策略、跟随其他用户策略、执行自己的订阅目标。
- `read_only`：只能读取授权范围内数据，不能创建钱包、策略、跟随或执行写操作。

系统已移除并不得重新引入：events/news/evidences、AI/info-risk/LLM、fair-value、Gamma/rewards catalog 全市场扫描、candles、自动 SELL exit、merge、Funding，以及独立 API/orderbook/worker/replay 部署。

## 当前架构

```text
browser
  -> polyedge-front:33002 (Next.js static export + Nginx)
       └── same-origin /api/* reverse proxy
            ├── same-host Compose: polyedge-server:38001
            └── split-host: backend-lan-ip:38001
  -> polyedge-server:38001 (same host or dedicated backend host)
       ├── Axum Cookie-session API + RBAC/CSRF/idempotency/audit
       ├── Postgres V4 store
       ├── targeted orderbook REST poll + in-memory cache
       ├── subscription-based multi-wallet execution coordinator
       └── wallet envelope decryptor
  -> PostgreSQL / Polymarket CLOB / Data API
```

后端只有一个活动可部署进程 `polyedge-server`。API、数据访问、目标盘口监督和执行 runtime 位于同一进程，不通过内部 HTTP 相互调用。浏览器流量始终通过前端 Nginx 的同源 `/api` 代理；前后端既可位于同一 Compose 网络，也可分机部署。Compose 默认把 server 38001 发布到宿主 `127.0.0.1`；拆机时显式改为后端内网 IP，并必须用主机防火墙只允许前端服务器访问。

## 数据与安全边界

- 用户、市场、策略、quote slots、subscription、钱包、风险限制、批次、订单和持仓以 Postgres 为事实源。
- 实时盘口只存在于 server 进程内 targeted cache；API handler 不在请求时调用 Polymarket。
- targeted token 集来自有效 subscription 钱包对应的 slots、open-like managed orders 和非零 positions；超过 `POLYEDGE_TARGETED_ORDERBOOK__MAX_TOKENS` 时整轮失败，不静默截断。
- opaque session token、CSRF token 和 activation token 在数据库只保存 SHA-256 hash；密码保存 Argon2 PHC hash。生产 session cookie 使用 `Secure`、`HttpOnly`、`SameSite=Strict`。
- CORS allowlist 可以为空，表示不开放浏览器跨源访问；非空时只接受 exact origin 且拒绝 wildcard。无论 CORS 是否为空，业务写请求都必须满足 `Origin == POLYEDGE_PUBLIC_ORIGIN` 与 CSRF 校验。
- 浏览器用一次性 RSA-OAEP-256 + AES-256-GCM import context 上传钱包 secret；后端验证私钥推导地址后，用独立 AES-256-GCM storage KEK 和每钱包随机 DEK 写入 `wallet_secret_envelopes`。数据库、DTO、日志和管理员接口均不返回明文 secret。
- storage KEK 与 transport RSA 私钥当前通过 server `.env` 直接注入（`POLYEDGE_WALLET_CRYPTO__STORAGE_KEY` / `TRANSPORT_PRIVATE_KEY_PEM`），不是外部 KMS；导入 context 已按 owner 持久化到 `wallet_import_contexts` 并原子消费，server 内存不保存可重放明文 token。
- 管理员可以跨用户查看业务、余额和现有财务 snapshot 汇总，但不能查看或导出私钥。

## 策略、跟随与执行语义

- `managed_markets` 是全局 canonical 标的；`market_strategies`、钱包、subscription、批次、订单和持仓均带用户归属。
- 策略使用 `[active_from, active_until)`。到期后禁止 place/replace，并由周期 supervisor 持久化 cancel-only batch/action；已有持仓不自动 SELL/merge。
- `strategy_quote_slots` 是稳定 desired-order identity；每槽固化 outcome、quantity、fixed/book-rank 定价、offset、价格边界、post-only 和 enabled。
- 创建策略会创建 owner subscription；follower subscription 引用源策略并只绑定跟随者自己的钱包。源策略或订阅暂停/到期、钱包解绑等失效条件会进入保护性撤单，不复制源钱包订单、资金或故障。
- 跟随传播通过周期 reconciliation 和 durable commands 实现，尚未接入 PostgreSQL NOTIFY；延迟受 `POLYEDGE_EXECUTION__RECONCILE_INTERVAL_MS` 约束。
- 同一钱包同一 quote slot 最多一张 open-like managed order；缺失订单按 external id 精确查终态，只有不明确结果进入 `unknown` 并继续占槽。
- 全局 kill switch、钱包/订阅/策略状态、freshness、post-only、风险限制和余额均 fail closed。钱包间有界并行；同一钱包由进程 mutex 与数据库 lease/epoch fencing 串行。
- clean deploy 默认 `kill_switch_locked=true`、`trading_enabled=false`。危险操作不再使用共享 step-up code，而要求 session 在 `POLYEDGE_AUTH__RECENT_AUTH_TTL_SECONDS` 内完成过密码认证。

当前 runtime 已覆盖 BUY quote 的 keep/place/cancel/replace、批量撤单、CLOB 余额/开放订单读取、Data API positions 全量替换和 subscription desired state。管理员可录入带钱包时间边界校验的外部资金流；managed order 累计成交差额和 position 同步会产生操作性 fill/equity 数据，但成交价格、时间和费用并非权威 venue fill，因此管理员财务页仍需在数据不完整时标记不完整，不能视为完整盈利核算。

## 活动 API

业务路由位于 `/api/v1`：

- 身份：`POST /auth/login|logout|activate|reauth`、`GET /auth/me`
- 管理员：`GET/POST /admin/users`、`PATCH /admin/users/{id}`、`POST /admin/users/{id}/activation-token`、`GET /admin/finance`
- 钱包加密：`POST /security/wallet-import-contexts`
- 钱包：`GET/POST /wallets`、`GET/PATCH /wallets/{id}`
- 策略：`GET/POST /market-strategies`、`GET/PATCH /market-strategies/{id}`、`GET /market-strategies/discover`
- 跟随：`GET/POST /strategy-subscriptions`、`PATCH /strategy-subscriptions/{id}`
- 执行：`GET/POST /execution-batches`、`GET /execution-batches/{id}`、`POST /execution-batches/{id}/cancel`、`POST /cancellation-batches`
- 账本与系统：`GET /orders`、`GET /positions`、`GET /cash-flows`、管理员 `POST /cash-flows`、`GET/PATCH /system/runtime-state`
- 健康：`GET /healthz`、`GET /readyz`

业务写接口使用 `Idempotency-Key`、`X-PolyEdge-CSRF-Token` 和 Origin 校验；不接受 Bearer 身份或旧 CSRF header 别名。登录、激活、登出和重新认证不进入业务幂等层。管理员创建用户与重签激活令牌的明文 token 永不写入幂等响应，重放返回冲突。危险钱包启用/secret rotation、执行、强制撤单、用户管理、cash-flow 和 kill-switch 变更要求 recent authentication。

## 关键文件

| 文件 | 职责 |
|---|---|
| `packages/backend/server/src/api/mod.rs` + `api/identity.rs` | 路由、session、RBAC、CSRF、recent-auth、幂等 |
| `packages/backend/server/src/store/identity.rs` | 环境管理员、用户、激活、session 和管理员财务查询 |
| `packages/backend/server/src/wallet_crypto.rs` | 浏览器导入与数据库 envelope cryptography |
| `packages/backend/server/src/secrets.rs` | 从数据库 envelope 解密每钱包执行凭证 |
| `packages/backend/server/src/store/strategies/` | actor-scoped 策略、有效期、owner subscription 与 commands |
| `packages/backend/server/src/store/subscriptions.rs` | follower subscription 与钱包绑定 |
| `packages/backend/server/src/execution.rs` | subscription desired-state keep/place/cancel/replace |
| `packages/backend/server/src/orderbook.rs` | targeted CLOB REST 盘口缓存 |
| `packages/backend/crates/domain/src/{identity,manual_trading}.rs` | V4 身份和交易领域类型 |
| `packages/backend/crates/contracts/src/{identity,manual_trading}.rs` | V4 HTTP DTO |
| `packages/backend/migrations_v2/0001_manual_trading_schema.sql` | V4 唯一 clean-deploy baseline |
| `packages/backend/init.sql` | 与 baseline 字节一致的初始化快照 |
| `packages/front/src/components/shared/auth-provider.tsx` | 控制台 session 与管理员路由保护 |
| `packages/front/src/features/{admin,following,strategies,wallets}/` | 用户管理、财务、跟随、有效期与钱包导入 UI |
| `packages/front/nginx.conf.template` | 静态站点与可配置 upstream 的同源 `/api` 代理 |
| `deploy/docker-compose.yml` | 同机或拆机的 `polyedge-server` + `polyedge-front` 部署 |

## 命令

后端：

```bash
cd packages/backend
cargo fmt --all
cargo check --workspace --tests
cargo test --workspace
cargo clippy --workspace --tests
cargo run -p polyedge-server
```

前端：

```bash
cd packages/front
yarn install
npx tsc --noEmit
yarn lint
yarn build
```

部署与静态检查：

```bash
./scripts/build-backend-bin.sh
bash -n scripts/deploy.sh scripts/build-backend-bin.sh
POLYEDGE_SERVER_ENV_FILE=.env.server.example \
  docker compose -f deploy/docker-compose.yml config
cmp packages/backend/migrations_v2/0001_manual_trading_schema.sql packages/backend/init.sql
git diff --check
```

## 配置说明

- Postgres：`POLYEDGE_POSTGRES__URL`；不读取旧 `DATABASE_URL`。
- runtime：`POLYEDGE_RUNTIME__ENVIRONMENT` 只接受 `local|production`；production 强制 HTTPS public origin 和 Secure Cookie；可信内网 HTTP 必须使用 `local`。
- 身份：`POLYEDGE_PUBLIC_ORIGIN`、`POLYEDGE_BOOTSTRAP_ADMIN__USERNAME|DISPLAY_NAME|PASSWORD_HASH|CREDENTIAL_VERSION`、`POLYEDGE_AUTH__SESSION_IDLE_SECONDS|SESSION_ABSOLUTE_SECONDS|ACTIVATION_TTL_SECONDS|RECENT_AUTH_TTL_SECONDS`。
- CORS：`POLYEDGE_CORS__ALLOWED_ORIGINS` 可为空；非空值只接受 exact origin，且始终拒绝 wildcard。即使同源 Nginx 是主路径，server 仍执行写请求 Origin 检查。
- Compose server 发布：`POLYEDGE_SERVER_PUBLISH_BIND|PUBLISH_PORT` 默认 `127.0.0.1:38001`；拆机时 bind 到后端内网 IPv4，不能把 CORS 当作网络 ACL。
- 钱包加密：`POLYEDGE_WALLET_CRYPTO__TRANSPORT_PRIVATE_KEY_PEM|TRANSPORT_KEY_ID|STORAGE_KEY_ID|STORAGE_KEY|IMPORT_CONTEXT_TTL_SECONDS|MAX_IMPORT_CONTEXTS`。RSA PEM 与 32-byte base64 storage key 直接写在 server `.env`；PEM 推荐单行并用 `\n` 表示换行。旧 `*_FILE` / Compose secret mount 变量已移除。
- targeted orderbook：`POLYEDGE_TARGETED_ORDERBOOK__MAX_TOKENS|POLL_INTERVAL_MS`；freshness 只由策略版本 `book_freshness_ms` 决定。
- 执行：`POLYEDGE_EXECUTION__WALLET_CONCURRENCY|RECONCILE_INTERVAL_MS`。
- Polymarket：`POLYEDGE_POLYMARKET__CLOB_HOST|DATA_API_HOST|CHAIN_ID`。
- 前端部署必须保持 `NEXT_PUBLIC_POLYEDGE_API_BASE_URL=`，使用 Nginx 同源 `/api` 代理；`POLYEDGE_FRONT_API_UPSTREAM` 默认 `http://polyedge-server:38001`，拆机时指向后端内网 origin；非空 public API base 只用于 `packages/front` 本地开发。

## 当前缺口

- targeted orderbook 是 REST poll，不是 market-channel WebSocket。
- 账户范围外部订单持续同步未完成。
- cash-flow 由管理员人工录入并校验钱包创建时间及未来时间上限；managed 累计成交差额和 position 同步仅生成操作性 fill/partial equity 数据，尚无权威 venue fill ingestion 与完整 mark-to-market，不能宣称完整盈利核算。
- 前端管理员用户页支持创建、列表、角色/状态修改和 pending local 用户重新签发激活 token；环境管理员不可被降权或禁用。
- `/following` 已能按源策略 ID 创建和列出订阅，但尚未把 discover API 做成可浏览选择器，也没有完整暂停/停止编辑 UI。
- 前端危险操作已移除无效 step-up code 输入和兼容 header；后端仍只认 recent-auth session，尚未把 recent-auth 过期自动衔接到 `/auth/reauth` 交互。
- 前端已移除未使用的 toast/decimal 校验运行时依赖及历史占位组件；新增依赖需先确认被活动路由或数据层实际引用。
- `PATCH /system/runtime-state` 与 cash-flow 写入均显式要求 admin role + recent-auth。
- 内网 HTTP 拆机链路没有 TLS，管理员密码、session 和钱包导入公钥依赖可信网络与主机防火墙；前端 `/healthz` 只证明静态 Nginx 存活，不证明远程 backend 可达。
- 默认宿主发布 `127.0.0.1:38001` 可能与已有进程冲突；拆机 upstream/publish 当前只支持 hostname/IPv4，不支持 IPv6。
- 真实实盘仍需要 funded/approved 账户、小额演练、storage key/RSA key 轮换方案和运维 runbook。
