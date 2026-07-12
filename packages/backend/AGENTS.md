# packages/backend/AGENTS.md

后端（Rust workspace）代码规范。Rust workspace 根为 [packages/backend/Cargo.toml](Cargo.toml)。仓库级状态快照见根 [AGENTS.md](../../AGENTS.md)；前端规范见 [packages/front/AGENTS.md](../front/AGENTS.md)。本文件的规则在写或改 `packages/backend/api/`、`packages/backend/order/`、`packages/backend/` 下任何 Rust 代码时必须遵守，违背即应拆分/重构而非沿用。

## 适用范围

`packages/backend` Rust workspace 下所有后端 crate 与 app：服务 crate `packages/backend/api`、`packages/backend/order`，以及 worker/replay app 和共享 crates。任何改变模块结构、分层依赖、公共抽象位置的改动，都要确认仍符合本文件，必要时同步更新本文件。

## 分层架构

crate 依赖**单向**，不可逆向：

| crate | 职责 | 依赖 |
|---|---|---|
| `domain` | 领域模型、错误（`AppError`）、数值 newtype（`Probability`/`Edge`/`UsdAmount`…） | 无（不依赖任何上层） |
| `application` | 用例服务 + port traits（`*Store`/`*Sink` 抽象）、领域编排 | `domain` |
| `connectors` | 外部数据源适配（Polymarket / news） | `domain` `application` |
| `infrastructure` | port 的具体实现：`catalog`(postgres/in-memory)、`stores`、`settings`、`auth`、`http`、`runtime` | `domain` `application` |
| `contracts` | HTTP API DTO（纯数据结构 + serde） | `domain` |
| `common` | 跨二进制复用的进程外壳 helper（监听地址、signal shutdown 等），不放业务逻辑 | `domain` |
| `packages/backend/api` / `packages/backend/order` / `packages/backend/apps/{worker,replay}` | 可执行入口，组装上述 crate | 全部 |

**红线：**
- `domain` 不得 `use` 任何上层；领域逻辑不下沉到 `infrastructure`。
- HTTP DTO 只放 `contracts`，不在 handler 内联定义请求/响应结构。
- 跨外部系统的交互走 `connectors`，不在 `application` 里直接发 HTTP/SQL。

## 模块化设计

1. **crate 根 `lib.rs` / 目录 `mod.rs`**：只做 `mod` 声明 + `pub use` 收敛对外 API（范例：`packages/backend/crates/application/src/lib.rs`）。不要在根文件堆实现。

2. **`include!` 拆分模式（项目核心惯例，全仓 100+ 处）**：当单个逻辑模块超过行数阈值，建一个「模块根文件」放共享 `use`/`const`/核心类型，按职责把实现拆到子目录文件，用 `include!("子目录/文件.rs")` 内联：
   - 被 `include!` 的子文件**不写自己的 `use`**，共享根文件作用域的导入；
   - 同一个 `impl T` 可拆成多个 `impl T { … }` 块分布在不同子文件；
   - 路径相对**根文件物理目录**解析，可多层嵌套（范例：`catalog/postgres/market_event/execution_updates/`）；
   - 子文件按**职责**命名（`fills.rs`/`quoting.rs`/`verifier.rs`/`parsers.rs`），不要按类型机械堆叠。

   范例：`rewards.rs`（根：共享导入 + `include!` 聚合）→ `rewards/{models,service,planner,helpers}.rs`。

3. **`mod` vs `include!`**：要对外暴露子命名空间（独立可见性边界）用真正的 `mod`（如 `settings::runtime_config`、`infrastructure` 的 `pub mod`）；同一逻辑单元的纯物理拆分用 `include!`。

## 文件行数规范

- **软上限 500 行**：超过应评估是否按职责拆分。
- **硬上限 800 行**：必须拆分。
- 函数体建议 ≤ 80 行；过长的 `impl` 拆成多个 `impl T { … }` 分文件。
- **例外（允许略超）：**
  - 单个 `impl Trait for Type` 块语言上不可跨文件拆（范例：`stores/rewards/postgres.rs` 的 `impl RewardBotStore`）。确需缩小时，把方法体委托给可拆分的 inherent helper。
  - 纯数据定义集合（DTO）、生成代码、长 SQL 字面量。
- **拆分纪律**：纯文本搬移、零逻辑改动；每拆一个文件立即 `cargo check`，编译器兜底正确性，不向后累积错误。

## 公共代码提取

- 重复逻辑提到 `helpers.rs`（范例：`rewards/helpers.rs`、`stores/helpers.rs`）。
- DB 行 ↔ 领域对象映射放 `*_rows.rs`（范例：`catalog/helpers/*_rows.rs`）。
- HTTP DTO ↔ 领域对象转换放 `mappers.rs`（范例：`packages/backend/api/src/handlers/mappers.rs`）。
- 仅本模块用的私有 helper 就近放模块内；跨 crate 业务复用下沉到 `domain` 或 `application`，跨二进制进程外壳复用放 `common`，不要在 app 之间复制。
- 禁止复制粘贴成片逻辑：同一段逻辑第二次出现即提取为共享函数。

## 测试组织

- 库 crate：模块内 `#[cfg(test)] mod tests { use super::*; … }`，可作为 `tests.rs` 被 `include!`（范例：`auth/tests.rs`、`settings/tests.rs`）。
- 二进制 crate：`src/tests/` 目录 + `src/tests.rs` 聚合（范例：`packages/backend/api/src/tests/`）。
- 测试与被测代码同 crate，通过 `super::` 访问私有项。

## 验证命令

```bash
cd packages/backend
cargo check --workspace --tests   # 编译（含测试目标），跨 crate 改动后必跑
cargo test --workspace            # 运行测试
cargo fmt --all                   # 统一格式（拆分/搬移后必跑）
cargo clippy --workspace --tests  # lint
```

## 现有文件长度债务（2026-07-12 快照）

以下生产代码物理文件已超过 800 行硬上限，属于存量拆分债务；后续触碰这些文件时应优先按职责拆分，不能继续扩大：

- `order/src/stream.rs`（~1965）
- `crates/infrastructure/src/stores/rewards/in_memory.rs`（~1837）
- `apps/worker/src/worker/rewards.rs`（~1745）
- `apps/worker/src/worker/rewards/live_orders.rs`（~1647）
- `crates/infrastructure/src/stores/rewards/postgres.rs`（~1629；单个 `impl RewardBotStore` 受语言限制，方法体应委托到 inherent helper）
- `apps/worker/src/worker/reward_action_executor.rs`（~1529）
- `crates/application/src/rewards/config_impl.rs`（~1352）
- `crates/infrastructure/src/stores/rewards/postgres_run_ledger.rs`（~1293）
- `crates/application/src/rewards/service.rs`（~1210）
- `crates/connectors/src/polymarket/chain.rs`（~1168）
- `apps/worker/src/worker/rewards/account_sync.rs`（~1121）
- `crates/application/src/rewards/planner.rs`（~1049）
- `apps/worker/src/worker/rewards/live_pending.rs`（~967）
- `crates/application/src/rewards/models.rs`（~905；纯数据定义可按例外评估）
- `crates/application/src/rewards/run_ledger_models.rs`（~903；纯数据定义可按例外评估）
- `crates/application/src/rewards/runtime_models.rs`（~892；纯数据定义可按例外评估）
- `apps/worker/src/worker/rewards/live_sync.rs`（~874）
- `crates/application/src/rewards/opportunity_metrics.rs`（~825）
- `crates/infrastructure/src/stores/helpers/reward_config.rs`（~820）
- `apps/worker/src/worker/rewards/info_risk.rs`（~820）
- `crates/infrastructure/src/catalog/postgres/market_event/queries.rs`（~811）

此外仍有多份测试文件超过硬上限，`catalog/postgres/market_event/execution_updates/fills.rs`（~688）和 `application/risk.rs`（~684）等生产文件处于软上限区间。行数以当前物理文件 `wc -l` 为准；重构后应同步刷新本节。
