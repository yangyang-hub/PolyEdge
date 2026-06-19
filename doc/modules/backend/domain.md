# domain（领域层）

最后更新：2026-06-19

## 概述

`polyedge_domain` crate 是系统的最底层基础，定义了所有业务领域的值对象、枚举、错误类型和认证原语。零外部业务依赖，不依赖任何上层 crate。

## 设计目标

- 提供整个系统共享的**通用语言**（Ubiquitous Language）类型
- 保证类型安全：用 newtype 包装 `Decimal` 防止原始值混用
- 统一错误模型：所有 crate 使用同一个 `AppError`
- 枚举序列化与反序列化一致（`snake_case`）

## 架构与关键文件

| 文件 | 职责 |
|---|---|
| `packages/backend/crates/domain/src/lib.rs` | 模块入口，通过 `include!()` 内联所有子文件 |
| `domain/error.rs` | `AppError`、`ErrorKind`、`Result<T>` 类型别名、辅助函数 |
| `domain/numeric.rs` | 数值 newtype：`Probability`、`Edge`、`ExposureRatio`、`Quantity`、`UsdAmount`、`SignedUsdAmount` |
| `domain/market_enums.rs` | 18 个业务枚举（市场状态、订单状态、信号生命周期等） |
| `domain/auth.rs` | 认证原语类型 |
| `domain/tests.rs` | 单元测试 |

所有子文件通过 `include!()` 内联到 crate 根命名空间，外部直接使用 `polyedge_domain::Xxx`。

## 核心数据结构

### 错误模型（error.rs）

- **`ErrorKind`**（7 个变体）：`InvalidInput`、`Unauthorized`、`Forbidden`、`NotFound`、`Conflict`、`DependencyUnavailable`、`Internal`
- **`AppError`**：`kind` + `code`（静态错误码）+ `message`（可读描述）+ `retryable`
- 7 个构造函数：`invalid_input`、`unauthorized`、`forbidden`、`not_found`、`conflict`、`dependency_unavailable`（默认 retryable）、`internal`（默认 retryable）
- 辅助函数：`round_decimal`、`format_decimal`、`deserialize_decimal_str`

### 数值类型（numeric.rs，~306 行）

| 类型 | 有效范围 | 精度（SCALE） | 用途 |
|---|---|---|---|
| `Probability` | [0, 1] | 6 | 市场概率 |
| `Edge` | [-1, 1] | 6 | 交易边际 |
| `ExposureRatio` | [0, 10] | 6 | 组合敞口比 |
| `Quantity` | ≥ 0 | 8 | 订单数量 |
| `UsdAmount` | ≥ 0 | 2 | 非负美元金额 |
| `SignedUsdAmount` | 无限制 | 2 | 可负美元金额（如 PnL） |

共同模式：`new()` 带范围校验、`value()` 返回内部 `Decimal`、`api_string()` 返回格式化字符串、手动 `Serialize`/`Deserialize`/`Display` 实现。

### 业务枚举（market_enums.rs，~605 行）

| 枚举 | 变体数 | 用途 |
|---|---|---|
| `SystemMode` | 5 | 系统运行模式（Research/PaperTrade/ManualConfirm/LiveAuto/KillSwitchLocked） |
| `MarketStatus` | 3 | 市场状态（Open/Closed/Resolved） |
| `OrderStatus` | 8 | 订单生命周期（New→Submitted→Open→PartiallyFilled→Filled/Canceled/Expired/Rejected） |
| `SignalLifecycleState` | 7 | 信号生命周期（New/Active/Weakened/Executed/Invalidated/Reversed/Expired） |
| `SignalAction` | 2 | 买入/卖出 |
| `SignalSide` | 2 | Yes/No |
| `ExecutionRequestStatus` | 4 | 执行请求状态 |
| `OrderDraftStatus` | 4 | 草稿订单状态 |
| `EventStatus` | 4 | 事件状态 |
| `EvidenceStatus` | 3 | 证据状态 |
| `EvidenceDirection` | 3 | 证据方向 |
| `AmbiguityLevel` | 3 | 模糊度 |
| `TradabilityStatus` | 4 | 可交易性 |
| `TimeHorizon` | 3 | 时间范围 |
| `MarketSortField` | 2 | 排序字段 |
| `SortOrder` | 2 | 排序方向 |
| `UserRole` | — | 用户角色 |
| `StepUpScope` | — | 提权范围 |

所有枚举使用 `#[serde(rename_all = "snake_case")]`，大多数实现 `as_str()` 和 `FromStr`。

## 依赖关系

- **上游**：无（纯基础层）
- **下游被依赖**：`application`、`connectors`、`infrastructure`、`contracts`、`common`、`packages/api`、`packages/orderbook`、`packages/backend/apps/worker` — 所有上层 crate 都依赖 `domain`

## 当前状态

- 完全实现，作为系统通用语言的基础
- 所有类型在全部上层 crate 中广泛使用

## 修改检查清单

- [ ] 新增/修改枚举时，确保 `serde(rename_all = "snake_case")` 和 `FromStr` 实现一致
- [ ] 新增数值 newtype 时，遵循现有模式（`new()` 范围校验、`SCALE` 常量、序列化实现）
- [ ] 修改 `AppError` 时检查所有上层 crate 的错误处理路径
- [ ] 修改后运行 `cargo check --workspace --tests`
- [ ] 同步更新 `contracts` crate 中对应的 DTO 枚举（如果新增枚举）
- [ ] 同步更新前端 `src/lib/contracts/dto/` 中对应的 TypeScript 类型
- [ ] 同步更新 `src/lib/i18n/dictionaries/enums.ts` 中的枚举翻译
