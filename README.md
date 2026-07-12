# PolyEdge

最后更新：2026-07-12

PolyEdge 是面向 Polymarket rewards market maker 的实盘策略与运维控制台。当前核心路径是 live 双边报价、market-implied fair value、资金与库存风控、成交后 maker/flatten 退出，以及 BalancedMerge 配对合并；市场、事件/新闻和 Funding 作为做市策略的支撑能力保留。

判断仓库当前真实状态时，优先阅读 [AGENTS.md](./AGENTS.md) 和 [模块文档索引](./doc/modules/README.md)。`doc/polyedge-*.md` 中的早期设计与计划仅作历史背景，不是当前能力清单。

## 当前状态

- 控制台主路由为 `dashboard / markets / events / rewards / rewards/fair-value / funding / settings`；另有 `/login`、`/unauthorized` 支撑路由，但仓库尚未提供生产级身份签发和 session 获取链路。
- 前端使用 Next.js 16、React 19、Tailwind CSS v4 和系统字体，只访问真实 Rust API，不含 mock-data 模式。
- Rust workspace 位于 `packages/backend/`，包含 `polyedge-api`、`polyedge-orderbook`、`polyedge-worker`、`polyedge-replay` 和共享 crates。
- 数据库采用单一干净部署基线：`packages/backend/migrations/0001_initial_schema.sql` 与 `packages/backend/init.sql` 表达当前 schema；旧部署不做历史表兼容迁移。
- `polyedge-orderbook` 独立负责 Gamma market、rewards catalog、reward candles、CLOB WS/poll 盘口缓存与 token registry。
- API handler 和策略代码只读 Postgres 或 orderbook service，不在请求路径直接调用 Polymarket 外部 API。
- Rewards bot 仅支持 live 路径；默认生产 drill 配置保持交易关闭，并以 1 个市场、最多 4 个开放订单和严格资金上限开始校准。
- 历史钱包、Copy-Trading 和独立研究模块已从路由、API、worker、服务、store、DTO、schema 和模块文档中移除。

## 仓库结构

```text
PolyEdge/
├── AGENTS.md                  # 当前仓库状态快照与维护规则
├── README.md                  # 项目入口说明
├── doc/
│   ├── modules/               # 当前模块文档，开发与排障优先阅读
│   ├── designs/               # 当前 Rewards 设计基线
│   └── polyedge-*.md          # 历史设计、契约和实施计划
├── deploy/                    # Compose、Dockerfile 与 env 模板
├── scripts/                   # 构建、部署和冒烟脚本
├── bin/                       # 部署镜像使用的预构建后端二进制
└── packages/
    ├── backend/               # Rust workspace、迁移与 init.sql
    └── front/                 # Next.js 控制台
```

代码与文档规范：

- [后端规范](./packages/backend/AGENTS.md)
- [前端规范](./packages/front/AGENTS.md)
- [模块文档索引](./doc/modules/README.md)

## 本地运行

后端：

```bash
cd packages/backend
cargo fmt --all
cargo check --workspace --tests
cargo test --workspace
cargo run -p polyedge-api
cargo run -p polyedge-orderbook
cargo run -p polyedge-worker
```

常用 worker 命令：

```bash
cargo run -p polyedge-worker -- ingest-news-once
cargo run -p polyedge-worker -- poll-news
cargo run -p polyedge-worker -- promote-news-events
cargo run -p polyedge-worker -- run-database-maintenance-once
cargo run -p polyedge-worker -- scan-rewards-once
cargo run -p polyedge-worker -- poll-reward-bot
cargo run -p polyedge-worker -- poll-reward-action-executor
cargo run -p polyedge-worker -- scan-reward-info-risks-once
cargo run -p polyedge-worker -- poll-reward-info-risks
cargo run -p polyedge-worker -- drain-execution-queue
cargo run -p polyedge-worker -- poll-polymarket-order-statuses
cargo run -p polyedge-worker -- reconcile-polymarket-fills
cargo run -p polyedge-worker -- consume-polymarket-user-events
```

Rewards 审计与确定性回放：

```bash
cargo run -p polyedge-replay -- --run-id <RUN_ID>
cargo run -p polyedge-replay -- --fixture <FIXTURE.json>
cargo run -p polyedge-replay -- --stored-run-id <RUN_ID>
```

前端：

```bash
cd packages/front
yarn install
yarn dev
yarn lint
yarn build
```

`NEXT_PUBLIC_POLYEDGE_API_BASE_URL` 必须指向 Rust API；静态部署时该值在构建阶段写入前端 bundle。

默认端口：

- API：`0.0.0.0:38001`
- Orderbook：`0.0.0.0:38002`
- Front Docker runtime：宿主机默认 `33002 -> container:80`

默认生产排查地址：

- Frontend Rewards：`http://192.168.31.5:33002/rewards`
- API：`http://100.87.45.72:38001`
- Orderbook：`http://100.87.45.72:38002`

## 数据获取架构

所有外部 API 数据必须由后台 producer 获取并写入 Postgres 或进程内缓存。策略、页面和 API handler 只能读取这些 store。

| 数据 | Producer | Source | Store |
|---|---|---|---|
| 通用市场 | `polyedge-orderbook` Gamma full/priority sync | Gamma `/markets` | `markets` |
| Rewards 市场 | `polyedge-orderbook` catalog sync | CLOB `/rewards/markets/current` | `reward_markets` |
| Order books | `polyedge-orderbook` WS + reconcile poll | CLOB WS、`/books`，回退 `/book` | `InMemoryOrderbookCache` |
| Rewards candles | `polyedge-orderbook` history sync | CLOB `/prices-history` | `reward_market_candles` |
| 账户、订单、成交、持仓 | `polyedge-worker` rewards loop | 认证 CLOB、Data API fallback、Polygon RPC | Rewards Postgres tables |
| Fair value | `polyedge-worker` rewards tick | orderbook service + 本地历史 | `reward_fair_values` / history |

Orderbook subscription 由 registry 聚合 `rewards_active`、`exec_orders`、`rewards_eligible`、`rewards_candidates`，并受 token 与 WS connection 上限约束。`observed_at` 表示盘口内容版本，`confirmed_at` 表示服务最近通过 WS/poll/ingest 确认过该盘口；Rewards 新挂单和撤单的 freshness 判断使用 `confirmed_at`。

## Rewards Bot

Rewards 计划先经过市场可交易性、catalog 流动性/成交量、事件窗口、盘口 freshness/结构、fair-value edge、退出深度、稳定性和资金风险门槛，再按 `selection_score` 分配资本。LP rewards 只以受限的 reward-density 次级权重参与排序，不能覆盖交易 edge 或风险门槛。

关键行为：

- Standard maker 从配置的 bid rank 开始，向更深档位搜索第一个满足 post-only 和 robust edge 的报价。
- Fair value 结合 YES/NO midpoint parity、microprice imbalance、历史、动态不确定性和可选 AI edge buffer。
- 下单 sizing 同时受 `maker_market_budget_usd`、钱包可用余额、per-outcome inventory、全局潜在敞口和 provider multiplier 约束；resting BUY 计入并发成交风险。
- AI advisory 只做慢速结构性风险审阅；info-risk 的取消动作必须满足可信来源、时间归因和置信度规则，provider 自报证据默认不可信。
- 成交后 standard profile 以成本/markup 目标退出，必要时受 `maker_max_exit_loss_cents` 控制 flatten floor；BalancedMerge 是独立 profile，链上广播使用 fail-closed fence。
- Full tick 记录 run/decision/action/order-transition ledger，并保存有大小与敏感字段保护的 replay fixture；Postgres durable action executor 对计划动作使用 lease、owner fence 和 venue-first reconciliation。

启用 live worker 至少需要：

```bash
POLYEDGE_REWARDS__ENABLED=true
POLYEDGE_WORKER__POLL_REWARD_BOT=true
POLYEDGE_ORDERBOOK__SERVICE_URL=http://127.0.0.1:38002
POLYEDGE_ORDERBOOK__WRITE_TOKEN=<shared-token>
```

真实下单还需要完整 Polymarket 凭证、资金钱包、Postgres 和正在运行的 orderbook service。独立异步 info-risk 扫描仅在未由 AI advisory 驱动 combined provider refresh 时需要 `POLYEDGE_WORKER__POLL_REWARD_INFO_RISKS=true`。

## API 安全边界

- 浏览器 CORS 使用 `POLYEDGE_CORS__ALLOWED_ORIGINS` 精确 origin；production 禁止通配符、带 path/query 的 origin 和空 allowlist。
- 当前静态前端没有生产级登录/session/Bearer token 获取链路。部署模板因此使用 `POLYEDGE_AUTH__DISABLED=true`，并要求 VPN、私网 ACL 或可信反向代理边界；production 还必须显式设置 `POLYEDGE_AUTH__ALLOW_INSECURE_PRIVATE_DEPLOY=true`。
- 开启鉴权时，production 需要非空 Ed25519 `POLYEDGE_AUTH__KEYS_JSON`；危险 Rewards 操作使用独立 step-up scopes。签名密钥和长期 JWT 不得进入前端 public env 或 bundle。

## Docker 部署

部署镜像从 `bin/` 复制预构建的 `polyedge-api` 和 `polyedge-orderbook`，服务器部署不编译 Rust。构建机或 CI 先运行：

```bash
./scripts/build-backend-bin.sh
```

Compose 编排三个服务：

- `polyedge-orderbook`：市场同步、candles、盘口 WS/poll、registry 与 HTTP API
- `polyedge-api`：HTTP API，并在同一进程内嵌 worker runtime
- `polyedge-front`：Nginx 静态站点

部署入口：

```bash
cp deploy/.env.api.example deploy/.env.api
cp deploy/.env.orderbook.example deploy/.env.orderbook
cp deploy/.env.front.example deploy/.env.front
# 填写 Postgres、同一 POLYEDGE_ORDERBOOK__WRITE_TOKEN、service URL、前端 API URL 和凭证
./scripts/deploy.sh all
```

同一 Compose 项目内 `POLYEDGE_ORDERBOOK__SERVICE_URL` 通常使用 `http://polyedge-orderbook:38002`。`deploy/.env.api.example` 默认关闭 rewards 和大多数会产生外部副作用的 worker 任务，启用前必须完成凭证、资金、风险配置和小额 drill。

## 已知缺口

- 生产级真实 session/auth UX 尚未完成；内部部署通常在受控网络边界内关闭 API 鉴权。
- Polymarket 私有任务需要真实凭证、已注资账户、小额实盘演练和正式运维 runbook 后才能投入生产。
- 控制台订单视图聚焦 PolyEdge managed orders，尚未完整展示账户范围内由其他客户端创建的开放订单。
- Rewards replay 已支持确定性决策重跑和 expected-plan 对比，但尚未模拟 fill risk、exit cost 和 cancel churn。
- 旧 arbitrage 表/迁移仍保留在当前 baseline 的必要部分，但旧 radar/signals/risk 控制台流程已不再暴露。
- Deposit Wallet 的 relayer 建钱包、pUSD 包装/入金和 approval 批处理仍需外部运维流程。
- 部分后端 Rewards/Orderbook 文件和前端 Rewards 配置面板仍超过仓库文件长度硬上限，存量清单见 package 级 `AGENTS.md`。

## 推荐阅读顺序

1. [AGENTS.md](./AGENTS.md)
2. [doc/modules/README.md](./doc/modules/README.md)
3. [Rewards Market Maker V2](./doc/designs/rewards-market-maker-v2.md)
4. [后端 worker 模块](./doc/modules/backend/worker-app.md)
5. [Orderbook 模块](./doc/modules/backend/orderbook-app.md)
6. [Rewards 前端模块](./doc/modules/frontend/rewards.md)
7. [部署模块](./doc/modules/infra/deployment.md)
