# domain（V4 领域层）

最后更新：2026-07-16

## 概述

`polyedge_domain` 是活动后端的最底层 crate，定义统一错误、身份权限、精确数值和值对象，以及多用户人工市场、有效期策略、跨用户跟随、quote slot、执行批次和交易账本状态。它不依赖 SQL、HTTP、connector 或 server。

## 关键文件

| 文件 | 职责 |
|---|---|
| `src/lib.rs` | 活动领域模块入口与 re-export |
| `src/identity.rs` | `UserAccount`、role/status/auth source 与 `ActorScope` |
| `src/manual_trading.rs` | V4 钱包、市场、有效期策略、跟随、执行和账本模型 |
| `src/domain/error.rs` | `AppError`、`ErrorKind`、`Result<T>` |
| `src/domain/numeric.rs` | `Probability`、`Quantity`、`UsdAmount` 等精确数值类型 |

V4 活动领域不包含 events/news/evidences、signals、AI/info-risk、fair-value、Gamma/rewards catalog、Replay 或旧系统模式。

## 核心类型

- 身份：`UserAccount`、`UserRole(admin|market_editor|read_only)`、`UserStatus`、`UserAuthSource`、`ActorScope`。
- 钱包：`WalletAccount`、`WalletSecretMetadata`、`WalletRiskPolicy`、`WalletAccountState`。
- 人工市场：`ManagedMarket`、`ManagedMarketOutcome`。
- 策略：`MarketStrategy`（owner、visibility、`[active_from, active_until)`）、`StrategyVersion`、`StrategyRewardTerms`、`StrategyQuoteSlot`。
- 跟随：`StrategySubscription`、`StrategySubscriptionWallet`、`StrategyCommand` 及其状态枚举。
- 执行：`ExecutionBatch`、`WalletExecutionJob`、`ExecutionAction`。
- 账本：`ManagedOrder`、`ManagedPosition`。schema 虽预留 fill/equity 表，domain 尚无对应活动模型或采集状态机。

状态枚举统一以 snake_case 序列化，覆盖 wallet/market/strategy/version、outcome、pricing mode、batch/job/action/order 和 maker/taker role。

## 领域约束

- quote slot 明确 outcome、quantity、fixed/book-rank、offset、价格边界和 post-only。
- quantity/价格/名义金额使用 `Decimal` 或相应 newtype，不使用浮点数。
- `WalletSecretMetadata` 只表达 storage key id、secret version 和更新时间，不持有 ciphertext 或 secret。
- `unknown` managed-order 状态属于 open-like 状态，表示必须 fail closed 并阻止重复下单。
- 执行批次固化 strategy version；钱包目标与策略参数分离。
- reward 条款是 strategy version 的不可变快照，不再属于全局 market。
- follower subscription 引用源策略并只绑定 follower 自己的钱包；有效停止时间是源策略与 subscription 截止时间的较早值。

## 依赖关系

- 上游：无。
- 下游：`contracts`、`connectors`、`server`。

## 当前状态

V4 identity/manual-trading 类型已覆盖用户权限、钱包 owner/secret metadata、策略有效期、版本 reward snapshot、跨用户 subscription/command 和 owner-aware 执行账本。execution 与 orderbook 已切换到 subscription desired state；fills/equity 尚未形成 domain/runtime 闭环。

## 修改检查清单

- [ ] 枚举保持 snake_case serde 与 `FromStr`/`as_str` 一致。
- [ ] 数值保持精确范围校验，不引入 `f32/f64` 交易字段。
- [ ] domain 不依赖 server/SQL/HTTP/connector。
- [ ] 修改后运行 workspace tests 并同步 contracts/frontend。
