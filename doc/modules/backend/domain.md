# domain（V3 领域层）

最后更新：2026-07-15

## 概述

`polyedge_domain` 是活动后端的最底层 crate，定义统一错误、精确数值和值对象，以及人工市场、多钱包、策略版本、quote slot、执行批次和交易账本状态。它不依赖 SQL、HTTP、connector 或 server。

## 关键文件

| 文件 | 职责 |
|---|---|
| `src/lib.rs` | 活动领域模块入口与 re-export |
| `src/manual_trading.rs` | V3 钱包、市场、策略、执行和账本模型 |
| `src/domain/error.rs` | `AppError`、`ErrorKind`、`Result<T>` |
| `src/domain/numeric.rs` | `Probability`、`Quantity`、`UsdAmount` 等精确数值类型 |

V3 活动领域不包含 events/news/evidences、signals、AI/info-risk、fair-value、Gamma/rewards catalog、Replay 或旧系统模式。

## 核心类型

- 钱包：`WalletCredentialRef`、`WalletAccount`、`WalletRiskPolicy`、`WalletAccountState`。
- 人工市场：`ManagedMarket`、`ManagedMarketOutcome`、`MarketRewardTerms`。
- 策略：`MarketStrategy`、`StrategyVersion`、`StrategyQuoteSlot`、`StrategyWalletTarget`。
- 执行：`ExecutionBatch`、`WalletExecutionJob`、`ExecutionAction`。
- 账本：`ManagedOrder`、`ManagedPosition`。独立 fill 模型不属于 V3。

状态枚举统一以 snake_case 序列化，覆盖 wallet/market/strategy/version、outcome、pricing mode、batch/job/action/order 和 maker/taker role。

## 领域约束

- quote slot 明确 outcome、quantity、fixed/book-rank、offset、价格边界和 post-only。
- quantity/价格/名义金额使用 `Decimal` 或相应 newtype，不使用浮点数。
- credential ref 只表达 provider/locator/key version，不持有 secret。
- `unknown` managed-order 状态属于 open-like 状态，表示必须 fail closed 并阻止重复下单。
- 执行批次固化 strategy version；钱包目标与策略参数分离。

## 依赖关系

- 上游：无。
- 下游：`contracts`、`connectors`、`server`。

## 当前状态

V3 manual-trading 类型已覆盖当前 schema 与 server execution runtime。删除或更改公开枚举/字段时必须同步 contracts、SQL row mapping、API、前端 DTO 和文档。

## 修改检查清单

- [ ] 枚举保持 snake_case serde 与 `FromStr`/`as_str` 一致。
- [ ] 数值保持精确范围校验，不引入 `f32/f64` 交易字段。
- [ ] domain 不依赖 server/SQL/HTTP/connector。
- [ ] 修改后运行 workspace tests 并同步 contracts/frontend。
