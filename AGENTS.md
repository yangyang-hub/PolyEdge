# Agent Guidelines

最后更新：2026-07-15

## 维护规则

- **模块文档优先**：修改任何模块前，必须先查阅 `doc/modules/` 下对应模块文档（索引见 [doc/modules/README.md](./doc/modules/README.md)）；修改后同步更新日期、关键文件、数据结构、当前状态和已知缺口。
- 任何改变行为、路由、命令、环境变量、部署方式、依赖或集成状态的改动，都必须同步更新本文件。
- 不要把设计目标写成已实现能力。若本文件、README、页面文案或历史设计冲突，以本文件为仓库状态快照优先修正。
- V3 是破坏性 clean deploy，不兼容旧数据库，也不保留旧 API、worker、orderbook service 或前端路由兼容层。

## 当前聚焦

PolyEdge V3 是人工配置的 Polymarket 做市执行系统：operator 录入 condition、YES/NO token、rewards 条款、目标 quote slots、每槽数量与定价方式，再把同一策略统一分配到多个钱包批量执行。

系统已移除并不得重新引入：

- events、news、evidences 采集与页面；
- AI advisory、info-risk、LLM provider 与信息过滤；
- fair-value 估计、历史与工作台；
- Gamma/rewards catalog 全市场扫描、candidate prewarm 与 price-history candles；
- 成交后自动 SELL exit、BalancedMerge/链上 merge 与 Funding；
- 独立 `polyedge-api`、`polyedge-orderbook`、`polyedge-worker`、`polyedge-replay` 部署；
- 请求路径即时发现市场或自动选择 rewards market。

## 当前架构

前后端继续分离：

```text
polyedge-front (Next.js static export + Nginx)
    -> browser REST
polyedge-server:38001
    ├── Axum API
    ├── Postgres store / idempotency / audit
    ├── targeted orderbook REST poll + in-memory cache
    ├── multi-wallet execution coordinator
    └── environment wallet-secret resolver
    -> PostgreSQL
    -> Polymarket CLOB / Data API
```

后端只有一个活动可部署进程 `polyedge-server`。API、数据访问、目标盘口监督和执行 runtime 是同一进程内模块，不通过内部 HTTP 或 provider service 相互调用。

## 数据边界

### Single Source of Truth

- 人工市场、策略、quote slots、钱包目标、风险限制、批次、订单和持仓以 Postgres 为事实源。
- 实时盘口只存在于 `polyedge-server` 的进程内 targeted cache；策略与 API handler 不在请求时调用外部 API。
- targeted orderbook token 集只来自：启用策略的 quote slots、open-like managed orders、非零 positions。
- token 数超过 `POLYEDGE_TARGETED_ORDERBOOK__MAX_TOKENS` 时整轮失败关闭，不允许静默截断风险覆盖。
- `observed_at` 表示上游盘口内容时间，`confirmed_at` 表示本进程最近成功确认该快照的时间；实盘动作使用 `confirmed_at` freshness。

### 禁止模式

- 扫描 Gamma/CLOB 获取所有市场或 rewards 目录。
- API handler 或页面加载期间直接访问 Polymarket。
- 根据新闻、事件、AI、fair value 或自动评分决定市场/方向/数量。
- 在数据库中保存私钥、API secret/passphrase，或把 secret 返回给前端/日志。
- 为不同钱包复制策略参数；钱包只作为统一策略版本的执行目标。

## 策略与执行语义

- `strategy_quote_slots` 是稳定 desired-order identity；每个 slot 固化 `outcome`、`quantity`、`fixed|book_rank` 定价、offset、价格边界、post-only 和 enabled。
- YES-only、NO-only、双边和多槽位完全由人工录入决定，不做自动方向选择。
- 同一钱包同一 quote slot 最多一张 open-like managed order；open set 缺失订单按 external id 精确查询终态，只有不明确结果进入 `unknown` 并继续占用槽位，防止重复下单。
- 对账动作包括 keep/place/cancel/replace。价格变化可按下调/上调确认时间、cooldown 和单轮替换上限触发 cancel-replace。
- 全局 kill switch、钱包状态、钱包交易开关、盘口 freshness/post-only、订单数量、单单/开放 BUY/总仓位/单市场风险上限和可用余额均 fail closed。
- 钱包间按 `POLYEDGE_EXECUTION__WALLET_CONCURRENCY` 有界并行；同一钱包通过进程内 mutex 与数据库 lease/epoch fencing 串行。
- 批次固化 published strategy version，并为每个目标钱包创建独立 job；钱包失败不会抹去其他钱包结果。
- clean deploy 默认 `kill_switch_locked=true`、`trading_enabled=false`，必须显式提权解锁后才允许新 BUY。

当前 runtime 已覆盖人工 BUY quote 的 place/cancel/replace、操作员批量撤单、CLOB 余额/开放订单读取，以及 Data API 钱包持仓全量替换和风险名义金额刷新。SELL exit、merge、Funding 与独立 fills 账本不属于 V3 范围，相关 schema、API、前端、connector 与 runtime 已删除。

## 活动 API

所有业务路由位于 `/api/v1`：

- `GET/POST /wallets`，`GET/PATCH /wallets/{id}`
- `GET/POST /market-strategies`，`GET/PATCH /market-strategies/{id}`
- `GET/POST /execution-batches`，`GET /execution-batches/{id}`
- `POST /execution-batches/{id}/cancel`
- `POST /cancellation-batches`
- `GET /orders`、`GET /positions`
- `GET/PATCH /system/runtime-state`
- `GET /healthz`、`GET /readyz`

所有写请求必须携带 `Idempotency-Key`。启用钱包交易、批次提交、强制撤单和 kill-switch 变更分别使用 `wallet_trading_enable`、`execution_submit`、`order_cancel_force`、`system_kill_switch_trigger|release` step-up scope。

## 关键文件

| 文件 | 职责 |
|---|---|
| `packages/backend/server/src/main.rs` | 单后端进程入口与 graceful shutdown |
| `packages/backend/server/src/api/mod.rs` | Axum 路由、鉴权、step-up、幂等响应 |
| `packages/backend/server/src/store/` | V3 Postgres CRUD、批次、账本和执行 lease |
| `packages/backend/server/src/store/positions.rs` | Data API 钱包持仓全量替换与风险合计 |
| `packages/backend/server/src/store/order_reconciliation.rs` | managed order 终态持久化、transition 与 slot 释放 |
| `packages/backend/server/src/orderbook.rs` | 人工目标 token 集合与 CLOB REST 盘口缓存 |
| `packages/backend/server/src/execution.rs` | 多钱包 desired-state 对账与 keep/place/cancel/replace |
| `packages/backend/server/src/execution/planning.rs` | target pricing、重挂节流与钱包风险纯计算 |
| `packages/backend/server/src/execution/reconciliation.rs` | open set 缺失订单的 external id 精确终态对账 |
| `packages/backend/server/src/secrets.rs` | credential locator 到环境 secret 的运行时解析 |
| `packages/backend/crates/domain/src/manual_trading.rs` | V3 领域类型与状态机 |
| `packages/backend/crates/contracts/src/manual_trading.rs` | V3 HTTP DTO |
| `packages/backend/crates/connectors/src/polymarket.rs` | CLOB live 与 Data API 协议适配 |
| `packages/backend/crates/connectors/src/polymarket/order_reconciliation.rs` | CLOB 单订单状态到 managed lifecycle 的保守映射 |
| `packages/backend/migrations_v2/0001_manual_trading_schema.sql` | V3 唯一 clean-deploy baseline |
| `packages/backend/init.sql` | 与 baseline 字节一致的空库初始化快照 |
| `packages/front/src/app/(console)/strategies/page.tsx` | 人工市场与 quote slots 录入 |
| `packages/front/src/app/(console)/wallets/page.tsx` | 多钱包与风险配置 |
| `packages/front/src/app/(console)/operations/page.tsx` | 批量执行、撤单与账本查看 |
| `deploy/docker-compose.yml` | `polyedge-server` + `polyedge-front` 部署 |

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
POLYEDGE_SERVER_ENV_FILE=.env.server.example POLYEDGE_FRONT_ENV_FILE=.env.front.example \
  docker compose -f deploy/docker-compose.yml config
git diff --check
```

## 配置说明

- `polyedge-server` 默认监听 `0.0.0.0:38001`；不再有 38002 服务。
- Postgres 必需：`POLYEDGE_POSTGRES__URL`。
- server 请求体上限由 `POLYEDGE_SERVER__MAX_BODY_BYTES` 控制，默认 1 MiB；不再读取旧 `DATABASE_URL` 回退。
- CORS 使用 `POLYEDGE_CORS__ALLOWED_ORIGINS` 精确 origin；production 禁止空 allowlist。
- 当前鉴权实现为可选 Bearer API token：启用时 `POLYEDGE_AUTH__API_TOKEN` 至少 32 字符。Production 无论是否关闭 Bearer auth 都必须配置至少 16 字符的 `POLYEDGE_AUTH__STEP_UP_CODE`，危险操作同时校验最小 scope + code。静态前端尚无生产 session 获取链路；production 若关闭鉴权，必须设置 `POLYEDGE_AUTH__ALLOW_INSECURE_PRIVATE_DEPLOY=true` 并置于可信网络边界。
- targeted orderbook 使用 `POLYEDGE_TARGETED_ORDERBOOK__MAX_TOKENS`、`POLL_INTERVAL_MS`；每个策略版本的 `book_freshness_ms` 是实盘 freshness 唯一配置。
- 多钱包调度使用 `POLYEDGE_EXECUTION__WALLET_CONCURRENCY`、`RECONCILE_INTERVAL_MS`。
- 钱包持仓同步使用 `POLYEDGE_POLYMARKET__DATA_API_HOST`；`POLYEDGE_POLYMARKET__CHAIN_ID` 只用于 CLOB 签名配置。
- 钱包凭证由 `POLYEDGE_WALLET_SECRETS__ENV_PREFIX` 解析，例如 locator `maker-primary` 对应 `POLYEDGE_WALLET_SECRET__MAKER_PRIMARY` JSON。真实 secret 只允许由主机/编排 secret manager 注入。
- 部署只使用 `deploy/.env.server.example` 与 `deploy/.env.front.example`。

## 当前状态与已知缺口

- 新 schema、V3 domain/contracts、单 server API/store、targeted REST orderbook、多钱包执行协调器、部署双服务和新控制台路由已建立。
- 前端与后端 DTO/写请求必须保持逐字段一致；变更 Rust contract 时同步修改 TypeScript 镜像与表单。
- targeted orderbook 当前是 REST poll，不是 Polymarket market-channel WS。
- 允许新 BUY 的钱包 job 会刷新 CLOB 余额并全量替换已管理 token 的 Data API positions；所有 job 都核验开放订单，保护性撤单不依赖余额/持仓刷新。账户范围外部订单持续同步和生产级登录/session UX 尚未完成。
- SELL exit、merge、Funding 与独立 fills 账本已从 schema、路由、前端、DTO、connector 和 runtime 删除，不属于 V3 待办。
- 控制台只保证展示 PolyEdge managed orders；其他客户端创建的账户级开放订单可见性不完整。
- 真实实盘仍需要已注资/approve 的账户、逐钱包凭证、小额演练和运维 runbook。
- `server/src/store/execution.rs` 已接近 800 行硬上限；`server/src/api/mod.rs`、`execution.rs`、`store/strategies.rs` 等超过 500 行软上限。后续触碰应按 API 资源、runtime/venue action 与 store query/write 继续拆分；当前活动生产文件没有超过硬上限。
