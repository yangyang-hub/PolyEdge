# PolyEdge

PolyEdge 是一个面向 Polymarket 的事件驱动交易系统，目标是把外部事件与证据转成概率更新、交易信号和风险受控的执行动作。

仓库包含三部分：

- `packages/front`：`Next.js 16 + React 19 + Tailwind v4 + shadcn/ui` 控制台前端
- `packages/backend`：Rust workspace，包含 `api / worker / orderbook / replay` apps，以及 `application / connectors / contracts / domain / infrastructure` crates
- `deploy/` + `scripts/`：Docker Compose 部署模板、环境变量示例和构建/部署/冒烟脚本

如果你想先了解"当前代码已经实现到哪一步"，优先看 [AGENTS.md](./AGENTS.md)。设计目标和长期方案在 `doc/` 目录。

## 当前状态

仓库已经不是纯文档状态：

- 前端控制台已有 `dashboard / markets / events / radar / rewards / copy-trading / signals / positions / risk / approvals / replay / settings` 页面
- 后端已有 Axum API、worker 子命令、独立 orderbook 服务、配置和数据库迁移（目前到 `0025`）
- 前端已移除 mock 数据模式，读写和 SSE 都要求连接真实 Rust 后端
- 市场同步和 orderbook 订阅已迁移到独立 `polyedge-orderbook` 服务
- Rewards bot 已接入 live 实盘 post-only 下单、撤单、成交同步和成交后退出
- 聪明钱跟单（copy-trading）已具备完整模拟子系统，`mode=live` 结构化支持但未接入真实下单
- Polymarket connector 已迁移到 CLOB V2，支持 `eoa` / `proxy` / `gnosis_safe` / `poly_1271` 签名
- 本地 API/SSE 联调路径已基本打通，生产化链路仍未闭环

当前最重要的现实判断：

1. 前端发布产物是静态文件，运行时由 Nginx 容器托管；浏览器通过 `NEXT_PUBLIC_POLYEDGE_API_BASE_URL` 直接访问 Rust API，不再经前端 Nginx 反向代理。
2. 当前部署模板默认 `POLYEDGE_AUTH__DISABLED=true`，前端会话只保留 `off`，不是生产级真实会话或权限体系。
3. 因此，这个仓库适合用于界面开发、契约收敛、后端能力建设和本地 live 联调，而不是直接当成已上线系统。

## 仓库结构

```text
PolyEdge/
├── AGENTS.md
├── README.md
├── doc/                    # 系统设计、API 契约、鉴权、存储、前后端计划等文档
├── deploy/                 # Docker Compose 部署模板和环境变量示例
├── scripts/                # 构建、部署、冒烟脚本
├── bin/                    # 部署镜像复制的预构建后端二进制
└── packages/
    ├── front/              # Next.js 控制台前端
    └── backend/            # Rust workspace
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
- 模块文档（`doc/modules/`）

### `packages/front/`

前端控制台，主要结构：

- `src/app/(console)`：页面路由
- `src/features/*`：按业务域拆分的 loader 和组件
- `src/lib/api/*`：浏览器侧 API 读取/写入层
- `nginx.conf.template`：静态资源和健康检查配置

前端代码规范见 [packages/front/AGENTS.md](./packages/front/AGENTS.md)。

### `packages/backend/`

Rust workspace，主要结构：

- `apps/api`：Axum HTTP API
- `apps/worker`：后台任务与执行/回写流程
- `apps/orderbook`：独立 orderbook 服务（WS + poll + HTTP API）
- `apps/replay`：研究/回放运行时骨架
- `crates/application`：用例编排
- `crates/domain`：领域模型与规则
- `crates/contracts`：HTTP/DTO 契约
- `crates/infrastructure`：配置、存储、鉴权、运行时
- `crates/connectors`：外部连接器

后端代码规范见 [packages/backend/AGENTS.md](./packages/backend/AGENTS.md)。

## 快速开始

这个仓库不是单一工具链的 monorepo。前端和后端要分别进入各自目录运行。

### 环境要求

建议本地至少具备：

- `Node.js 20+`
- `yarn`
- `Rust` 与 `cargo`
- `PostgreSQL`（持久化/多进程部署需要）和可选 `Redis`

## 前端运行

进入前端目录：

```bash
cd packages/front
```

安装依赖并启动：

```bash
yarn install
yarn dev
```

常用命令：

```bash
yarn lint
yarn build
```

默认环境变量见 [packages/front/.env.example](./packages/front/.env.example)：

```bash
HOSTNAME=0.0.0.0
NEXT_PUBLIC_POLYEDGE_API_BASE_URL=
NEXT_PUBLIC_POLYEDGE_CONSOLE_AUTH=off
NEXT_PUBLIC_POLYEDGE_INTERNAL_AUTH_DEV_BYPASS=1
```

说明：

1. Docker 部署必须在 `deploy/.env.front` 配置 `NEXT_PUBLIC_POLYEDGE_API_BASE_URL`；该值在 `yarn build` 时写入静态 bundle。
2. 浏览器会直接请求该 Rust API 地址，跨域由 API 的 permissive CORS 处理。
3. `NEXT_PUBLIC_POLYEDGE_CONSOLE_AUTH` 当前只保留 `off`；生产级 session provider 尚未接入。

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
cargo run -p polyedge-worker
cargo run -p polyedge-orderbook
```

`polyedge-api` 只负责 HTTP API / SSE / 前端交互，可以在负载均衡后面运行多个实例。`polyedge-orderbook` 是独立的 orderbook 服务，负责市场同步、WS + poll 盘口流和进程内缓存，暴露 HTTP API。后台任务统一由 `polyedge-worker` 常驻服务调度；是否启用各类任务由 `POLYEDGE_WORKER__...` 配置控制。

worker 仍保留以下维护/调试子命令，正常运行时不需要逐个手动执行：

```bash
cargo run -p polyedge-worker -- ingest-news-once
cargo run -p polyedge-worker -- poll-news
cargo run -p polyedge-worker -- promote-news-events
cargo run -p polyedge-worker -- scan-arbitrage-once
cargo run -p polyedge-worker -- poll-arbitrage-radar
cargo run -p polyedge-worker -- analyze-arbitrage-opportunities
cargo run -p polyedge-worker -- scan-rewards-once
cargo run -p polyedge-worker -- poll-reward-bot
cargo run -p polyedge-worker -- drain-execution-queue
cargo run -p polyedge-worker -- poll-polymarket-order-statuses
cargo run -p polyedge-worker -- reconcile-polymarket-fills
cargo run -p polyedge-worker -- consume-polymarket-user-events
cargo run -p polyedge-worker -- scan-copytrade-once
cargo run -p polyedge-worker -- poll-copytrade
cargo run -p polyedge-worker -- analyze-wallets-once
```

后端通过环境变量配置，示例见 [packages/backend/.env.example](./packages/backend/.env.example)。

建议先在 `packages/backend` 下创建本地配置：

```bash
cp .env.example .env
```

默认值对应当前开发环境：

- API 默认监听 `0.0.0.0:38001`
- runtime 默认模式是 `manual_confirm`
- Polymarket connector 没有 mock mode；市场列表走 Gamma 实时数据，私有订单/成交任务需要真实账户和私钥
- `postgres.url` 和 `redis.url` 默认仍为空

说明：

1. 环境变量命名采用 `POLYEDGE_<section>__<field>`，例如 `POLYEDGE_SERVER__PORT=38001`。
2. 留空的可选项会被视为未配置，例如 `POLYEDGE_POSTGRES__URL=`。
3. `POLYEDGE_AUTH__KEYS_JSON` 使用 JSON 数组格式配置验签公钥。

### Worker 常驻服务

```bash
POLYEDGE_ARBITRAGE__ENABLED=true
POLYEDGE_WORKER__POLL_ARBITRAGE_RADAR=true
POLYEDGE_WORKER__ANALYZE_ARBITRAGE_OPPORTUNITIES=true
cargo run -p polyedge-worker
```

可选开启新闻抓取：

```bash
POLYEDGE_WORKER__POLL_NEWS=true
POLYEDGE_NEWS__ENABLED=true
POLYEDGE_NEWS__SOURCES_JSON='[{"id":"sec_feed","source_type":"official","url":"https://example.com/sec.rss","reliability":"0.95","enabled":true}]'
```

可选开启 rewards bot 实盘：

```bash
POLYEDGE_REWARDS__ENABLED=true
POLYEDGE_WORKER__POLL_REWARD_BOT=true
cargo run -p polyedge-worker
```

可选开启跟单：

```bash
POLYEDGE_COPYTRADE__ENABLED=true
POLYEDGE_WORKER__POLL_COPYTRADE=true
cargo run -p polyedge-worker
```

可选开启执行/回写类任务：

```bash
POLYEDGE_WORKER__DRAIN_EXECUTION_QUEUE=true
POLYEDGE_WORKER__POLL_POLYMARKET_ORDER_STATUSES=true
POLYEDGE_WORKER__RECONCILE_POLYMARKET_FILLS=true
POLYEDGE_WORKER__CONSUME_POLYMARKET_USER_EVENTS=true
```

Polymarket 私有订单、成交和用户 websocket 任务会直接使用真实 connector；开启前必须配置真实 `POLYEDGE_POLYMARKET__ACCOUNT_ID` / `POLYEDGE_POLYMARKET__PRIVATE_KEY` 和必要 API 凭证。`POLYEDGE_POLYMARKET__SIGNATURE_TYPE` 支持 `eoa`、`proxy`、`gnosis_safe`、`poly_1271`；已有 Deposit Wallet 使用 `poly_1271`，并将 `POLYEDGE_POLYMARKET__FUNDER` 配成 deposit wallet 地址。

不要同时运行多个 `polyedge-worker` 常驻实例，除非先为具体任务加分布式锁/租约。API 可以多实例，worker 默认按单实例后台调度设计。

### Rewards bot

控制台 `/rewards` 已接入 Polymarket CLOB rewards 当前市场扫描和 live 实盘报价。Rewards bot 仅支持 `live` 模式，`execution_mode` 配置字段已移除；worker 通过 `OrderbookHttpClient` 读取独立 orderbook 服务盘口，通过 `LivePolymarketConnector` 提交 post-only GTC token 买单，成交后按配置撤 sibling legs 并执行 exit-at-markup 或 flatten sell。

手动运行一次：

```bash
cargo run -p polyedge-worker -- scan-rewards-once
```

说明：

1. `/rewards` 页面的 Run / Cancel / Reset 会写入数据库控制命令，由 worker 领取执行；API 服务本身不执行策略。
2. 要产生新挂单和 live 下单，需要配置真实 Polymarket 凭证并确保 `polyedge-orderbook` 服务正在运行并同步了 reward 市场数据。
3. 未成交 maker 买单不在本地按全局 notional 硬锁资金；`stale_book_ms=0` 只关闭盘口年龄检查，仍要求盘口存在且非空。
4. `/api/v1/rewards-bot` 只读取 worker 写入数据库的账户快照、托管订单和 positions；API 不持有 Polymarket 私钥或 CLOB 凭证。
5. worker 同步账户状态时，资金钱包地址优先使用 `POLYEDGE_POLYMARKET__FUNDER`，未配置时使用 `ACCOUNT_ID`；CLOB balance 为 0 或失败但链上 pUSD 余额大于 0 时，会用 Polygon pUSD 余额回填 `available_usd`。
6. 账户范围外开放订单同步、订单计分查询和奖励结算对账仍未完成。

### 聪明钱跟单（Copy-trading）

控制台 `/copy-trading` 已接入完整的模拟跟单子系统：跟踪多个 Polymarket 钱包地址、通过 Polymarket Data API 检测新成交、四种跟单仓位模式（`FixedUsd` / `ProportionalToSource` / `CapitalRatio` / `MirrorPortfolioWeight`）、钱包分析统计、per-wallet/per-market/total 风控和确定性模拟引擎。未处理 source trades 按时间顺序决策，同一 tick 内执行暂停钱包、wallet+token cooldown、日亏损和运行中 exposure cap；无本地持仓的 sell 不会产生模拟收益，crossed order 会完整成交并释放 reserve。

手动运行一次：

```bash
cargo run -p polyedge-worker -- scan-copytrade-once
```

说明：

1. `/copy-trading` 页面的 Run / Analyze / Cancel / Reset 会写入数据库控制命令，由 worker 领取执行。
2. `mode=live` 已结构化支持但未接入真实下单（记录警告回退模拟）。
3. 需要 `POLYEDGE_COPYTRADE__ENABLED=true` + `POLYEDGE_WORKER__POLL_COPYTRADE=true` 才会执行。

### 套利雷达 live 冒烟

本地验证真实盘口链路时，先确保已应用迁移到 `0015_reward_bot.sql`，再使用：

```bash
POLYEDGE_POSTGRES__URL=postgres://...
POLYEDGE_REDIS__URL=redis://...
POLYEDGE_ARBITRAGE__BOOK_SOURCE=polymarket
POLYEDGE_ARBITRAGE__SCAN_LIMIT=1
cargo run -p polyedge-api
cargo run -p polyedge-worker
```

前端 `/radar` 应能看到 scan、机会、validation、过期和 analysis 事件。`poll-arbitrage-radar` 会持续扫描，并按 `POLYEDGE_ARBITRAGE__EVENT_RETENTION_HOURS` 清理旧 outbox 事件。

也可以用本地冒烟脚本检查 API、worker、套利只读端点和可选前端 SSE 代理：

```bash
POLYEDGE_FRONT_BASE_URL=http://127.0.0.1:3001 \
./scripts/smoke-arbitrage-radar.sh
```

说明：

1. `packages/backend/.env` 中 `POLYEDGE_POSTGRES__URL` / `POLYEDGE_REDIS__URL` 留空时，后端会回退到内存 store；要验证持久化和 outbox，必须显式传真实连接串。
2. `POLYEDGE_ARBITRAGE__BOOK_SOURCE=polymarket` 会实时请求 Polymarket CLOB `/book`。live 冒烟时需要先把待测市场替换为真实 `polymarket_*` 引用，或把 `POLYEDGE_ARBITRAGE__BOOK_SOURCE` 改回 `market_snapshot`。
3. `scripts/smoke-arbitrage-radar.sh` 未设置 `POLYEDGE_SMOKE_BEARER_TOKEN` 时会发送本地 dev-auth header；如果后端没有关闭验签或没有启用 dev-auth，本脚本会跳过受保护端点或返回认证失败。

## Docker 部署

先在构建机或 CI 上生成 Linux 服务器可运行的后端二进制，并提交到仓库：

```bash
./scripts/build-backend-bin.sh
git add bin/polyedge-api bin/polyedge-orderbook
git commit -m "Build backend binaries"
```

服务器上执行：

```bash
cp deploy/.env.example deploy/.env
# 编辑 deploy/.env，填入外部 PostgreSQL URL；纯内网默认 POLYEDGE_AUTH__DISABLED=true，不需要 step-up code
# 在 deploy/.env.orderbook 和 deploy/.env.worker 设置相同的 POLYEDGE_ORDERBOOK__WRITE_TOKEN
# 各服务专属配置见 deploy/.env.{api,orderbook,worker,front}.example
# Polymarket live / Deposit Wallet 示例见 deploy/.env.polymarket.example
./scripts/deploy.sh all
```

`deploy/docker-compose.yml` 编排：

- `polyedge-orderbook`（独立 orderbook 服务，WS + poll + HTTP API）
- `polyedge-api`（通过 `POLYEDGE_ORDERBOOK__SERVICE_URL` 读取盘口）
- `polyedge-worker`
- `polyedge-front`

各服务无 Compose 启动依赖，可独立部署在不同服务器。PostgreSQL 不会由 compose 创建，需要在 `deploy/.env` 里配置已有服务地址；Redis 仅在需要时按需添加 `POLYEDGE_REDIS__URL`。

`scripts/deploy.sh` 只接受简单目标参数：

- 不传参数或 `auto`：拉取最新代码，per-service 检测二进制/前端 hash，只 rebuild 变化的镜像并启动变化或未运行的目标服务。
- `all`：重建所有本地具备所需二进制的镜像并重启对应服务。
- `api worker`：重建 API/Worker 共享镜像，并重启 API 与 worker。
- `api`：只重建 API/Worker 共享镜像并重启 API。
- `worker`：只重建 API/Worker 共享镜像并重启 worker。
- `orderbook`（或 `ob`）：重建独立 orderbook 镜像并重启 orderbook 服务。
- `front`：只重建前端镜像并重启前端。
- 支持组合，例如 `api front` 或 `api,worker`。

部署脚本默认使用 `/tmp/polyedge-deploy.lock` 防止 cron/CI 重叠执行，默认 `COMPOSE_PARALLEL_LIMIT=1` 串行构建镜像。API/Worker 共享镜像，orderbook 使用独立镜像；Compose 构建上下文已收窄：后端只上传 `bin/`，前端只上传 `packages/front/`，避免扫描本地 `packages/backend/target`、`node_modules`、`.next` 等大目录。

Docker 部署里 worker 后台任务按代码默认值均为关闭状态；需要新闻、套利、rewards 或 copytrade 时，在 `deploy/.env.worker` 显式设置对应 `POLYEDGE_WORKER__...=true`。市场同步和 orderbook 订阅由独立 `polyedge-orderbook` 服务管理，不需要在 worker 中启用。

更新指定分支：

```bash
POLYEDGE_GIT_BRANCH=main ./scripts/deploy.sh
```

如果是首次在空目录部署，也可以把脚本放到服务器后指定仓库地址：

```bash
POLYEDGE_DEPLOY_DIR=/opt/polyedge \
POLYEDGE_GIT_REPO=https://github.com/<owner>/<repo>.git \
POLYEDGE_GIT_BRANCH=main \
./scripts/deploy.sh
```

## 数据获取架构

目标架构要求所有外部 API 数据由后台 worker/orderbook 服务获取并写入数据库或内存缓存，策略、页面和 API handler 只从这些 store 读取，不直接调用外部 API。Rewards snapshot 已迁移为只读数据库账户快照，私有账户余额和完整持仓由 worker 同步。

| 数据 | 来源 | 存储 | 间隔 |
|------|------|------|------|
| 通用市场 | Gamma API `/markets/keyset` | `markets` 表（Postgres） | 5 分钟 |
| 奖励市场 | CLOB API `/rewards/markets/current` | `reward_markets` 表（Postgres） | 5 分钟 |
| 盘口 | CLOB WebSocket + `/book` poll | `InMemoryOrderbookCache`（orderbook 服务进程内，TTL 5 分钟） | WS 实时 + 30 秒 reconcile |

## 当前已实现能力

### 前端

- 控制台布局、顶部状态条、侧边导航和实时状态栏
- `dashboard / markets / events / radar / rewards / copy-trading / signals / positions / risk / approvals / replay / settings`
- feature loader + feature component 分层
- Server Actions 驱动的审批、风险控制、rewards bot 和跟单控制 UI 链路
- SSE live 代理与共享 realtime provider，`radar` 页面支持 arbitrage outbox 增量事件
- 前端仅支持中文，文案走字典导入

### 后端

- Axum API 已覆盖 markets、events、news、evidences、signals、orders、trades、positions、pricing、arbitrage、rewards bot、copytrade、risk、approvals、system、SSE、connector callback 和 orderbook 等主路径
- 独立 `polyedge-orderbook` 服务：市场同步、WS + poll 盘口流、进程内缓存和 token 注册 HTTP API
- worker 侧支持 news ingest、news promotion、arbitrage radar、rewards bot live 策略、copytrade 跟单、execution drain、Polymarket order/fill/user-event、orderbook token 注册
- worker 侧只读套利雷达，可扫描盘口、记录机会、校验、分析，不会创建 execution request 或订单
- 套利雷达 outbox-backed 增量 SSE，支持 `Last-Event-ID` 续传
- Rewards bot live 实盘：post-only GTC 下单、撤单、confirmed 成交同步、cash/库存/PnL 更新、资金钱包 pUSD 余额回填、sibling leg 撤单和 exit/flatten sell
- 聪明钱跟单：四种仓位模式、钱包分析统计、按时间顺序决策、运行中 exposure/cooldown 风控、确定性模拟引擎、控制命令队列
- Polymarket CLOB V2 connector，支持 `eoa` / `proxy` / `gnosis_safe` / `poly_1271` 签名、balance 查询、Polygon pUSD 余额读取和开放订单分页
- RSS/Atom 新闻源抓取、标准化、去重入库和 source health 记录
- PostgreSQL schema 迁移

## 当前主要缺口

1. 生产级真实会话体系未完成；前端只保留 `off` 模式。签名内部 JWT 代码路径已具备，但真实环境还需要 key rotation、会话来源和撤销策略。
2. `signals / risk / events` SSE 仍是 snapshot-backed stream；`arbitrage` 已是 outbox-backed 增量流，但尚未统一到全资源事件总线。
3. 新闻源可以抓取、去重、提升为 events/evidences，但尚未自动生成 signals。
4. Rewards live maker 已接入真实下单、成交同步、worker 账户余额/完整持仓快照和链上 pUSD 余额回填；仍未完成账户范围外开放订单同步、订单计分查询或奖励结算对账。
5. Rewards API snapshot 已改为只读数据库；API 不需要 Polymarket 私钥，但 worker 仍需要真实凭证和资金链路小额验证。
6. 聪明钱跟单 `mode=live` 已结构化支持但未接入真实下单。
7. Polymarket live 链路已具备 CLOB V2 SDK 和认证能力；仍未实现 Deposit Wallet 生命周期管理，且仍需真实资金链路小额验证。
8. 默认部署模板关闭 API 鉴权，只适合原型/内网共享环境。

## 推荐阅读顺序

如果你刚接手这个项目，建议按这个顺序阅读：

1. [AGENTS.md](./AGENTS.md)
2. [doc/polyedge-design.md](./doc/polyedge-design.md)
3. [doc/polyedge-frontend-design.md](./doc/polyedge-frontend-design.md)
4. [doc/polyedge-backend-design.md](./doc/polyedge-backend-design.md)
5. [doc/polyedge-api-contract.md](./doc/polyedge-api-contract.md)
6. [doc/polyedge-auth-design.md](./doc/polyedge-auth-design.md)

## 说明

- 根目录 `README.md` 用于项目入口说明。
- 根目录 [AGENTS.md](./AGENTS.md) 用于记录"当前仓库真实状态"和开发维护约定。
- 如果两者描述出现冲突，以 `AGENTS.md` 为准。
