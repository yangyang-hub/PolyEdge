# PolyEdge

最后更新：2026-07-16

PolyEdge V4 是多用户、人工配置的 Polymarket 做市执行系统。管理员由环境变量 bootstrap，并创建其他用户；市场录入用户可以管理自己的加密钱包、录入带生效时段的策略，或使用自己的钱包跟随其他用户的 `followable` 策略。只读用户只能查看授权数据。

当前事实来源：

- [AGENTS.md](./AGENTS.md)
- [模块文档](./doc/modules/README.md)

V4 不包含 events/news/evidences、AI/info-risk、fair value、Gamma/rewards 全市场扫描、candidate prewarm、candles 或独立 API/worker/orderbook/provider 服务。新部署使用空数据库，不兼容旧数据。

## 架构

```text
browser
  -> polyedge-front:33002 (Next.js static + Nginx)
       └── same-origin /api proxy
  -> polyedge-server:38001
       ├── Cookie session + RBAC/CSRF API
       ├── Postgres V4 store
       ├── targeted CLOB REST orderbook cache
       ├── encrypted wallet-secret resolver
       └── subscription-based multi-wallet execution
  -> PostgreSQL / Polymarket CLOB / Data API
```

控制台主路径为 `/login`、`/activate`、`/dashboard`、`/strategies`、`/following`、`/wallets`、`/operations`、`/settings`，管理员另有 `/admin/users` 和 `/admin/finance`。

## 核心语义

- 环境管理员不可通过 API 禁用或降权；其他用户由管理员创建并通过一次性 token 激活。
- session 使用 opaque HttpOnly Cookie；写请求校验 CSRF 和 Origin，危险操作要求最近密码认证。
- 策略关联的 canonical market 固化 condition 与 YES/NO token；published version 固化 rewards 快照和 quote slots，并只在 `[active_from, active_until)` 内做市。
- 策略到期、暂停或跟随失效后停止 place/replace，并通过 durable cancel-only 工作撤销开放订单。
- follower 引用源策略 desired state，但始终使用自己的钱包、余额和风险限制；不会复制源钱包订单或资金结果。
- 浏览器先用 RSA-OAEP + AES-GCM 加密钱包私钥，后端再以独立 AES-GCM envelope 加密入库。
- open-like `wallet + quote slot` 唯一；unknown submission 继续占用 slot。
- clean deploy 默认 kill switch locked、全局交易 disabled。

## 仓库结构

```text
PolyEdge/
├── doc/                         # 设计与模块状态文档
├── deploy/                      # 双服务 Compose、Dockerfile、env 模板
├── scripts/                     # 构建与部署脚本
├── bin/                         # 预构建 polyedge-server
└── packages/
    ├── backend/
    │   ├── server/              # 唯一后端应用
    │   ├── crates/{domain,contracts,connectors}/
    │   └── migrations_v2/ + init.sql
    └── front/                   # Next.js 静态控制台
```

## 本地验证

```bash
cd packages/backend
cargo fmt --all
cargo check --workspace --tests
cargo test --workspace
cargo clippy --workspace --tests

cd ../front
yarn install
npx tsc --noEmit
yarn lint
yarn build
```

本地 `cargo run` 的 server 默认可通过 `http://127.0.0.1:38001` 访问，前端 Docker 位于 `http://127.0.0.1:33002`。Compose 不把 server 端口映射到宿主机；生产前端必须保持 `NEXT_PUBLIC_POLYEDGE_API_BASE_URL=`，通过 Nginx 同源 `/api` 代理访问 server。

## 数据库与部署

V4 唯一 baseline 是 `packages/backend/migrations_v2/0001_manual_trading_schema.sql`，并与 `packages/backend/init.sql` 保持字节一致。Server 只接受 `POLYEDGE_POSTGRES__URL`。

```bash
./scripts/build-backend-bin.sh
cp deploy/.env.server.example deploy/.env.server
cp deploy/.env.front.example deploy/.env.front
./scripts/deploy.sh all
```

部署前必须配置真实 Argon2id 管理员 hash、RSA 导入私钥文件和 32-byte base64 storage key。Compose 只运行 `polyedge-server` 和 `polyedge-front`。

## 当前缺口

- targeted orderbook 是 REST poll，不是 market-channel WebSocket；账户范围外部订单持续同步未完成。
- 管理员可人工录入外部资金流；managed 累计成交差额与 position 同步只产生操作性 fill/partial equity 数据，尚无权威 venue fill ingestion 或完整 mark-to-market，因此管理员财务页不能视为完整盈利核算。
- 管理员用户页已支持创建、角色/状态修改和 pending local 用户重签激活 token；following 页尚无 discover 选择器和暂停/停止编辑 UI。
- 前端已删除旧 step-up 输入；recent-auth 过期后尚未自动衔接 `/auth/reauth` 交互。
- 全局 runtime-state 与外部 cash-flow 写入均要求 admin role + recent-auth。
- 实盘仍需要 funded/approved 钱包、小额演练、密钥轮换与运维 runbook。
