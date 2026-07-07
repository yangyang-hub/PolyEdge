# PolyEdge

PolyEdge 是面向 Polymarket 的事件、市场数据、市场策略研究和 LP rewards 自动化控制台。当前仓库已经包含前端、Rust 后端、数据库迁移、orderbook 服务和 Docker 部署入口。

如果要判断“代码现在真实实现到哪里”，优先看 [AGENTS.md](./AGENTS.md) 和 [doc/modules/](./doc/modules/README.md)。`doc/polyedge-*.md` 中的设计/计划文档保留为历史背景，不作为当前能力清单。

## 当前状态

- 前端控制台页面：`dashboard / markets / events / rewards / funding / settings`。未落地的 approvals 页面和 `/replay` 不再作为前端入口暴露。
- 前端只走真实 Rust API，不再提供 mock 数据模式；所有文案走 `@/lib/i18n/dictionaries` 中文字典。
- 后端 Rust workspace 根为 `packages/backend/Cargo.toml`：服务 crate、worker/replay 兼容 app、共享 crates、迁移和初始化 SQL 均位于 `packages/backend/`，其中 API 在 `packages/backend/api`，orderbook 服务在 `packages/backend/order`。
- 数据库迁移目前到 `0057_reward_merge_intent_execution.sql`；空库可用 `packages/backend/init.sql` 一次性初始化，运行时仍使用 `packages/backend/migrations/` 做 `sqlx` 迁移校验。
- 市场同步、rewards catalog 同步和 orderbook WS/poll 缓存由独立 `polyedge-orderbook` 服务负责。
- API 只读数据库或 orderbook 服务，不在 handler 中直接请求 Polymarket。Rewards 控制操作通过数据库命令队列交给 worker/runtime 执行。
- Rewards bot 仅支持 live 实盘路径：post-only 买单、撤单、confirmed fill 对账、成交后 sibling cancel、exit/flatten sell、账户余额/持仓快照同步、AI advisory 和异步信息风险缓存。
- 历史钱包类和独立研究模块已从前后端、worker、数据库和模块文档中移除；项目当前聚焦市场数据/事件基础设施和 LP rewards 策略。
- 当前控制台会话只保留 `off`，默认内网部署不是生产级真实会话/权限体系。

## 仓库结构

```text
PolyEdge/
├── AGENTS.md                  # 当前仓库状态快照和维护规则
├── README.md                  # 项目入口说明
├── doc/
│   ├── modules/               # 当前模块文档，开发时优先查阅
│   └── polyedge-*.md          # 历史设计、契约、计划和背景文档
├── deploy/                    # Docker Compose、Dockerfile 和 env 模板
├── scripts/                   # 构建、部署、冒烟脚本
├── bin/                       # 部署镜像复制的预构建后端二进制
└── packages/
    ├── backend/               # Rust 后端：api、order、worker/replay、共享 crates、迁移和 init.sql
    │   ├── Cargo.toml         # Rust workspace 根
    │   ├── Cargo.lock         # Rust workspace lockfile
    │   ├── rust-toolchain.toml
    │   ├── api/               # polyedge-api 服务 crate
    │   └── order/             # polyedge-orderbook 服务 crate
    └── front/                 # Next.js 16 + React 19 控制台
```

代码规范：

- 前端见 [packages/front/AGENTS.md](./packages/front/AGENTS.md)
- 后端见 [packages/backend/AGENTS.md](./packages/backend/AGENTS.md)
- 模块文档索引见 [doc/modules/README.md](./doc/modules/README.md)

## 本地运行

前端：

```bash
cd packages/front
yarn install
yarn dev
yarn lint
yarn build
```

`NEXT_PUBLIC_POLYEDGE_API_BASE_URL` 必须指向 Rust API，例如 `http://127.0.0.1:38001`。静态部署时该值在 build 阶段写入前端 bundle。

后端：

```bash
cd packages/backend
cargo check --workspace
cargo test --workspace
cargo run -p polyedge-api
cargo run -p polyedge-orderbook
cargo run -p polyedge-worker
```

常用 worker 维护/调试子命令：

```bash
cargo run -p polyedge-worker -- ingest-news-once
cargo run -p polyedge-worker -- poll-news
cargo run -p polyedge-worker -- promote-news-events
cargo run -p polyedge-worker -- scan-rewards-once
cargo run -p polyedge-worker -- poll-reward-bot
cargo run -p polyedge-worker -- scan-reward-info-risks-once
cargo run -p polyedge-worker -- poll-reward-info-risks
cargo run -p polyedge-worker -- drain-execution-queue
cargo run -p polyedge-worker -- poll-polymarket-order-statuses
cargo run -p polyedge-worker -- reconcile-polymarket-fills
cargo run -p polyedge-worker -- consume-polymarket-user-events
```

默认端口：

- API：`0.0.0.0:38001`
- Orderbook：`0.0.0.0:38002`
- Front Docker runtime：宿主机默认 `33002 -> container:80`

默认生产排查环境：

- Frontend Rewards 工作台：`http://192.168.31.5:33002/rewards`
- API 服务：`http://100.87.45.72:38001`
- Orderbook 服务：`http://100.87.45.72:38002`

除非另行说明，线上问题排查默认使用这组地址；前端静态构建的 `NEXT_PUBLIC_POLYEDGE_API_BASE_URL` 应指向 `http://100.87.45.72:38001`。

## 数据获取架构

外部市场数据必须由后台 producer 获取并写入数据库或缓存，策略、页面和 API handler 只读这些存储。

| 数据 | Producer | Source | Store |
|---|---|---|---|
| 通用市场 | `polyedge-orderbook` Gamma full/priority sync | Gamma `/markets` + `/markets?condition_ids=...` | Postgres `markets` |
| Rewards markets | `polyedge-orderbook` rewards catalog sync | CLOB `/rewards/markets/current` | Postgres `reward_markets` |
| Order books | `polyedge-orderbook` WS + poll | CLOB WS + `/books` batch，回退 `/book` | orderbook 服务进程内 `InMemoryOrderbookCache` |
| Rewards 账户状态 | worker rewards loop | 认证 CLOB / Data API / Polygon RPC | Postgres rewards tables |

关键约束：

- API handler、前端 loader 和策略代码不得直接调用 Polymarket Gamma/CLOB/Data API。
- Orderbook 写接口使用 `POLYEDGE_ORDERBOOK__WRITE_TOKEN`，读盘口接口应放在可信内网。
- Rewards worker 通过 orderbook HTTP batch 和内部 `/orderbook/stream` 读取盘口，本地 cache 缺失时才 bootstrap。

## Rewards Bot

Rewards bot 只从 `reward_markets` 和 `markets` 读取候选，硬过滤非 open/tradable、高歧义、低流动性、低 24h 成交量、临近结算、Gamma spread 过宽、数据过期/异常超前、非唯一 YES/NO token 的市场。通过门槛后按奖励、流动性、成交量、剩余时长和 rewards spread 综合排序。

启用 live worker 至少需要：

```bash
POLYEDGE_REWARDS__ENABLED=true
POLYEDGE_WORKER__POLL_REWARD_BOT=true
POLYEDGE_ORDERBOOK__SERVICE_URL=http://127.0.0.1:38002
```

要真实下单，还需要配置 Polymarket 凭证、签名类型、资金钱包和正在运行的 `polyedge-orderbook`。信息风险异步扫描另外需要：

```bash
POLYEDGE_WORKER__POLL_REWARD_INFO_RISKS=true
```

资金模型：本系统未成交 maker 买单不按全局资金硬锁，不同 condition 可复用资金；同一 condition 的已有 managed BUY 剩余 notional 与待补腿必须 fit 当前可用资金。账户级外部 BUY notional 中无法归属到本系统 managed order 的部分会被保守扣除。

主要缺口仍是账户范围外开放订单明细同步和奖励结算对账。

## Copy-Trading

Copy-trading 当前是只读跟踪和分析子系统：

- 管理多个 Polymarket 钱包地址
- 通过 Data API 扫描源钱包成交
- 记录 source trades 和事件日志
- 统计钱包胜率、ROI、成交量等分析指标
- 前端只保留启停跟踪、钱包管理和 Analyze

启用持续扫描：

```bash
POLYEDGE_COPYTRADE__ENABLED=true
POLYEDGE_WORKER__POLL_COPYTRADE=true
```

旧 Run / Cancel / Reset 模拟交易语义已移除；兼容命令在 worker 中是 no-op 或仅用于历史控制队列兼容。

## Docker 部署

部署镜像从 `bin/` 复制预构建后端二进制，服务器部署不编译 Rust。构建机/CI 先执行：

```bash
./scripts/build-backend-bin.sh
git add bin/polyedge-api bin/polyedge-orderbook
```

Compose 当前编排三个服务：

- `polyedge-orderbook`：独立市场同步、盘口 WS/poll、HTTP API
- `polyedge-api`：HTTP API，并在同一进程中内嵌 worker runtime；加载 `.env.api`
- `polyedge-front`：Nginx 静态站点

部署入口：

```bash
cp deploy/.env.api.example deploy/.env.api
cp deploy/.env.orderbook.example deploy/.env.orderbook
cp deploy/.env.front.example deploy/.env.front
# 在 .env.api 和 .env.orderbook 填入 PostgreSQL URL，并设置相同的 POLYEDGE_ORDERBOOK__WRITE_TOKEN
# 在 .env.api 设置 POLYEDGE_ORDERBOOK__SERVICE_URL，在 .env.front 设置 NEXT_PUBLIC_POLYEDGE_API_BASE_URL
./scripts/deploy.sh all
```

部署侧只有三个服务级 env：`deploy/.env.api`、`deploy/.env.orderbook`、`deploy/.env.front`。同一 Compose 项目中 `POLYEDGE_ORDERBOOK__SERVICE_URL` 通常是 `http://polyedge-orderbook:38002`；跨服务器部署时使用 orderbook 服务器实际地址，当前默认生产排查地址是 `http://100.87.45.72:38002`。`deploy/.env.front` 的 `NEXT_PUBLIC_POLYEDGE_API_BASE_URL` 当前默认生产值是 `http://100.87.45.72:38001`。`deploy/.env.api.example` 会显式关闭各 worker 后台任务，需要运行时再改为 `true`。

## 主要缺口

- 生产级真实会话体系未完成；当前前端只保留 `off`。
- 内部 JWT 签名 helper 已有代码路径，但当前不会从 `off` 签发可信令牌。
- 前端已移除 SSE 实时流，页面通过 REST API 初始加载。
- 新闻源可以抓取、去重、提升为 events/evidences，但尚未自动生成 signals。
- Rewards 仍缺账户范围外开放订单明细同步和奖励结算对账。
- Deposit Wallet 生命周期管理未实现，包括 relayer 建钱包、pUSD 入金/approval 批处理。

## 推荐阅读顺序

1. [AGENTS.md](./AGENTS.md)
2. [doc/modules/README.md](./doc/modules/README.md)
3. [doc/modules/backend/worker-app.md](./doc/modules/backend/worker-app.md)
4. [doc/modules/backend/orderbook-app.md](./doc/modules/backend/orderbook-app.md)
5. [doc/modules/frontend/data-layer.md](./doc/modules/frontend/data-layer.md)
6. [doc/modules/infra/deployment.md](./doc/modules/infra/deployment.md)

`README.md` 是入口说明，`AGENTS.md` 是当前状态快照。如果两者冲突，以 `AGENTS.md` 为准并修正 README。
