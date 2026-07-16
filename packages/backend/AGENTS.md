# packages/backend/AGENTS.md

最后更新：2026-07-16

后端 Rust workspace 根为 [Cargo.toml](Cargo.toml)，仓库状态快照见根 [AGENTS.md](../../AGENTS.md)。本文件适用于 `packages/backend/server` 与活动共享 crates。

## 活动 workspace 边界

| crate/app | 职责 | 依赖方向 |
|---|---|---|
| `crates/domain` | 身份/RBAC、交易领域模型、状态枚举、数值和 `AppError` | 不依赖上层 |
| `crates/contracts` | 身份、钱包、策略、跟随、执行 HTTP DTO | `domain` |
| `crates/connectors` | Polymarket CLOB/Data API 协议适配 | `domain` |
| `server` | Cookie-session API、Postgres store、钱包加密、targeted orderbook、subscription 执行 runtime | 上述 crates |

V4 不再设置独立 `application`、`infrastructure`、`common`、API app、worker app、orderbook app 或 replay app。后端可部署产物只有 `polyedge-server`；不要重新引入进程间 orderbook/provider HTTP client、Gamma/news producer 或旧 Rewards service。

## 分层红线

- `domain` 不得依赖 HTTP、SQL、connector 或 server。
- 对外 HTTP DTO 放在 `contracts`；handler 不内联公开请求/响应类型。
- 外部 Polymarket 协议调用集中在 `connectors`；API handler 和策略纯函数不得直接发外部请求。
- SQL、迁移、幂等、审计和执行 lease 位于 `server/store` 与 `migrations_v2`。
- `server/orderbook` 只处理数据库已知的人工目标 token、open-like managed orders 和非零 positions，不发现市场、不全量扫描。
- 用户身份来自 opaque Cookie session，不接受共享 Bearer token或客户端伪造 actor；写请求只接受 `X-PolyEdge-CSRF-Token` 并校验 exact Origin，危险操作校验 recent authentication。
- 钱包 secret 只以 envelope ciphertext 存入数据库；`server/secrets.rs` 执行时解密，禁止明文进入 DTO、日志或管理员接口。浏览器 transport key 与数据库 storage key 必须分离。

## V4 执行约束

- quote slot 是 desired-order 稳定身份；一个钱包/slot 最多一张 open-like 订单。
- 钱包间并行、单钱包串行；数据库 job/action terminal write 必须携带 lease owner + epoch fencing。
- `unknown` submission/cancel 结果必须占用 slot 并 fail closed，不能自动补挂。
- place/replace 前重新检查 kill switch、钱包开关、余额、风险上限、盘口 freshness、post-only 和价格边界。
- fixed price 与 positive book rank 二选一；数量、方向、单边/双边均来自人工策略版本。
- 下调/上调重挂分别遵守确认时间，竞争性上调还受 cooldown 与 `max_replaces_per_cycle` 限制。
- runtime desired state 来自 owner/follower subscription 与其自有钱包绑定；源策略、订阅或绑定失效时只能保护性撤单。
- 策略只在 `[active_from, active_until)` 内允许 place/replace，到期必须持久化 cancel-only 工作。
- clean deploy 默认 kill switch 锁定且全局交易关闭。

## 代码组织

- crate 根 `lib.rs` 只做模块声明与 re-export。
- 同一逻辑单元超过阈值时优先拆成真实 `mod`；仅在现有 include 组织下延续职责清晰的 `include!`，不要为了规避边界把无关逻辑塞进同一作用域。
- 重复 SQL/row mapping 放到 `store/helpers.rs` 或职责明确的 store 子模块。
- identity/API、store、orderbook、execution、wallet crypto、secrets 和 config 保持独立职责；不要在 handler 内实现策略或数据库事务。

## 文件行数

- 软上限 500 行，超过时评估拆分。
- 硬上限 800 行，必须拆分；纯 DTO/生成代码/长 SQL 可按例外审查。
- 函数建议不超过 80 行。
- 当前没有活动生产文件超过 800 行硬上限。`server/src/api/mod.rs`、`execution.rs`、`store/execution.rs` 已超过 500 行软上限并接近硬上限；后续触碰应按身份/API 资源、runtime/venue action 和 store query/write 职责拆分。

## 测试与验证

- 纯决策逻辑使用模块内单元测试覆盖 fixed/book-rank、freshness、post-only、risk、keep/cancel/replace 与 unknown fencing。
- Store 变更应使用 PostgreSQL 集成测试或最小 clean-schema smoke，特别验证 `SKIP LOCKED`、lease fencing、partial unique open-slot 约束和幂等重放。
- 禁止使用真实私钥、真实资金或真实下单作为默认测试前置。

```bash
cd packages/backend
cargo fmt --all
cargo check --workspace --tests
cargo test --workspace
cargo clippy --workspace --tests
cargo run -p polyedge-server
```

数据库变更还必须保持以下文件字节一致，并在空 PostgreSQL 上执行：

```bash
cmp packages/backend/migrations_v2/0001_manual_trading_schema.sql packages/backend/init.sql
```

## 文档同步

- 修改 `server`：更新 `doc/modules/backend/server-app.md`。
- 修改 connector：更新 `doc/modules/backend/connectors.md`。
- 修改 domain/contracts：更新对应模块文档和前端 DTO。
- 修改 schema：更新 `doc/modules/infra/database.md`。
- 修改 env、端口、镜像或运行命令：更新根 `AGENTS.md`、README 和部署文档。
