# PolyEdge

最后更新：2026-07-15

PolyEdge V3 是人工配置的 Polymarket 做市执行系统。Operator 明确录入市场、YES/NO token、rewards 条款、quote slots、每槽数量与定价方式，再把同一策略批量应用到多个钱包。系统持续维护目标订单，并保留 deterministic 撤单和价格变化 cancel-replace。

当前事实来源：

- [AGENTS.md](./AGENTS.md)
- [V3 设计](./doc/designs/manual-market-maker-v3.md)
- [模块文档](./doc/modules/README.md)

V3 不包含 events/news/evidences、AI/info-risk、fair value、Gamma/rewards 全市场扫描、candidate prewarm、price-history candles 或独立 API/worker/orderbook/provider 服务。新部署使用空数据库，不兼容旧数据。

## 架构

```text
polyedge-front (Next.js static + Nginx)
    -> polyedge-server:38001
         ├── Axum API + Postgres store
         ├── targeted CLOB REST orderbook cache
         ├── environment wallet-secret resolver
         └── multi-wallet execution coordinator
    -> PostgreSQL / Polymarket CLOB
```

控制台 V3 主路径为 `/dashboard`、`/strategies`、`/wallets`、`/operations`、`/settings`，另保留 `/login`、`/unauthorized` 外壳。旧 `/markets`、`/events`、`/rewards`、`/rewards/fair-value` 与 Funding 路由/feature/API/DTO 已删除。

## 核心语义

- 人工策略版本固化 condition、YES/NO token、rewards terms 和多个 quote slots。
- slot 支持 YES-only、NO-only、双边或多个同侧订单，数量在录入时确定。
- pricing 支持 fixed 和 book-rank + offset，并配置价格边界/post-only。
- 同一策略统一选择多个钱包；钱包间并发、单钱包串行。
- open-like `wallet + quote slot` 唯一；unknown submission 继续占用 slot。
- place/replace 前检查 kill switch、钱包开关、freshness、post-only、余额与钱包风险上限。
- clean deploy 默认 kill switch locked/trading disabled。

## 仓库结构

```text
PolyEdge/
├── doc/                         # V3 设计与模块文档
├── deploy/                      # 双服务 Compose、Dockerfile、env 模板
├── scripts/                     # 构建与部署脚本
├── bin/                         # 预构建 polyedge-server
└── packages/
    ├── backend/
    │   ├── server/              # 唯一后端应用
    │   ├── crates/{domain,contracts,connectors}/
    │   └── migrations_v2/ + init.sql
    └── front/                   # Next.js 控制台
```

## 本地验证

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

默认地址：后端 `http://127.0.0.1:38001`，前端 Docker 映射 `http://127.0.0.1:33002`。`NEXT_PUBLIC_POLYEDGE_API_BASE_URL` 在前端 build time 写入。

## 数据库

V3 唯一 baseline 为：

- `packages/backend/migrations_v2/0001_manual_trading_schema.sql`
- `packages/backend/init.sql`

两者必须字节一致。旧 `packages/backend/migrations/` 不得与 V3 混用。

Server 只接受 `POLYEDGE_POSTGRES__URL` 作为数据库连接配置，不再读取旧项目遗留的 `DATABASE_URL` 回退。

## 部署

```bash
./scripts/build-backend-bin.sh
cp deploy/.env.server.example deploy/.env.server
cp deploy/.env.front.example deploy/.env.front
./scripts/deploy.sh all
```

Compose 只运行 `polyedge-server` 和 `polyedge-front`。后端 secret 通过每钱包 credential locator 解析，不能写入数据库或前端环境。

## 当前缺口

- targeted orderbook 当前是 REST poll，不是 market-channel WS；
- 钱包 job 已同步 CLOB 余额/开放订单与 Data API positions；账户范围外部订单持续同步尚未完成；
- SELL exit、merge、Funding 与独立 fills 账本已从 schema/API/前端/connector/runtime 删除，不属于 V3 范围；
- 生产级登录/session UX 尚未完成；
- 实盘需要已 funded/approved 钱包、独立凭证、小额演练与运维 runbook。
