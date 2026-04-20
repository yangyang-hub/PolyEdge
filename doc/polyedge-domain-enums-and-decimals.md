# PolyEdge 枚举与定点数规范

## 1. 文档目标

本文档定义 PolyEdge 在 API、Rust domain model 和 PostgreSQL schema 之间共用的：

1. 枚举字段取值。
2. 定点数字段口径。
3. 序列化、舍入和比较规则。

目标不是把所有字段再重复一遍，而是避免出现：

1. 前端把 `0.5` 当字符串，后端当浮点。
2. 数据库把 `edge` 存成 `NUMERIC(12, 6)`，业务层却按更高精度比较。
3. 同一个状态字段在不同模块使用不同拼写或不同语义。

---

## 2. 基本原则

1. Domain 与 contract 层禁止使用二进制浮点作为真值类型。
2. Rust 业务层统一使用十进制定点类型，如 `rust_decimal::Decimal`。
3. PostgreSQL 统一使用 `NUMERIC(p, s)` 持久化数值。
4. API 中的价格、概率、比例、金额和数量统一使用 JSON 字符串。
5. API 响应禁止科学计数法，如 `1e-6`。
6. 枚举统一使用 `snake_case` 字符串。
7. PostgreSQL 首版优先使用 `TEXT + CHECK`，不优先引入原生 `ENUM`，以降低迁移成本。

---

## 3. 定点数类型

### 3.1 类型族

| 类型族 | 典型字段 | API 形式 | Rust 建议 | PostgreSQL 建议 | 合法范围 |
| --- | --- | --- | --- | --- | --- |
| `probability_price` | `best_bid`、`best_ask`、`mid_price`、`last_trade_price`、`prior_price`、`posterior_price`、`fair_price`、`market_price`、`avg_fill_price`、`mark_price`、`avg_cost` | 字符串 | `Decimal` | `NUMERIC(12,6)` | `[0, 1]` |
| `unit_interval_score` | `confidence`、`relevance_score`、`source_reliability`、`novelty`、`resolution_relevance`、`strength` | 字符串 | `Decimal` | `NUMERIC(12,6)` | `[0, 1]` |
| `signed_edge` | `edge` | 字符串 | `Decimal` | `NUMERIC(12,6)` | `[-1, 1]` |
| `exposure_ratio` | `gross_exposure`、`net_exposure`、`max_position_per_market`、`max_daily_loss`、`max_single_trade` | 字符串 | `Decimal` | `NUMERIC(12,6)` | `[0, 10]` |
| `quantity_cash` | `size`、`quantity`、`filled_quantity`、`net_qty`、`volume_24h`、`fee`、`daily_pnl`、`realized_pnl`、`unrealized_pnl`、`nav` | 字符串 | `Decimal` | `NUMERIC(24,8)` | 由业务字段约束 |

说明：

1. `probability_price` 专用于预测市场价格与概率表达，默认视为 `[0, 1]` 闭区间。
2. `unit_interval_score` 是“评分”而不是价格，但仍使用同样的 6 位小数精度。
3. `exposure_ratio` 使用 NAV 归一化比例，不是百分数字符串，也不是美元金额。
4. `quantity_cash` 比价格类字段保留更多小数位，用于数量、成交额、费用和 PnL。

### 3.2 序列化规则

1. API 统一输出十进制字符串，如 `0`、`0.05`、`0.470001`。
2. API 不输出科学计数法。
3. `-0` 必须规范化为 `0`。
4. 至少保留一位整数位，小于 1 的值写成 `0.05`，不写 `.05`。
5. 服务端可接受带尾随零的输入，如 `0.050000`，但响应应输出规范化形式。

### 3.3 舍入规则

1. 业务计算阶段保留 `Decimal` 原始精度，不提前截断。
2. 持久化到 PostgreSQL 前，按对应类型族 scale 做舍入。
3. API 返回前，按对应类型族 scale 做舍入。
4. 首版统一采用 `round half even`。

### 3.4 比较规则

1. 风控阈值、幂等请求判等和状态迁移判断使用业务层 `Decimal` 比较，不转浮点。
2. 同一字段的持久化值和 API 值必须来自同一轮舍入结果。
3. 只有在写入数据库或返回 API 时才允许“定标”，不要在中间计算链路多次 round。

---

## 4. Rust 与数据库映射

### 4.1 Rust

建议：

1. `contracts` crate 中的数值字段使用字符串序列化。
2. `domain` crate 中统一使用 `Decimal` 值对象，不暴露裸字符串。
3. 对概率、价格、edge、数量分别封装值对象，避免跨语义误用。

示例：

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Probability(Decimal);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Quantity(Decimal);
```

### 4.2 PostgreSQL

建议：

1. 所有 `probability_price`、`unit_interval_score` 和 `signed_edge` 字段使用 `NUMERIC(12,6)`。
2. 所有 `quantity_cash` 字段使用 `NUMERIC(24,8)`。
3. 所有比例和分数类字段增加 `CHECK` 约束，避免越界写入。

示例：

```sql
fair_price NUMERIC(12,6) NOT NULL CHECK (fair_price >= 0 AND fair_price <= 1)
edge       NUMERIC(12,6) NOT NULL CHECK (edge >= -1 AND edge <= 1)
quantity   NUMERIC(24,8) NOT NULL CHECK (quantity >= 0)
```

---

## 5. 枚举编码规则

1. API、SSE 和 WebSocket payload 统一使用 `snake_case` 字符串。
2. Rust `enum` 建议使用 `#[serde(rename_all = "snake_case")]`。
3. PostgreSQL 首版将枚举字段定义为 `TEXT NOT NULL` 并配合 `CHECK`。
4. 新增枚举值前必须先更新本文档和 API 契约，再更新后端实现。

---

## 6. 核心枚举定义

### 6.1 市场相关

| 字段 | 允许值 | 说明 |
| --- | --- | --- |
| `market.status` | `open`、`closed`、`resolved`、`suspended` | `closed` 表示不再交易但未完成最终结算；`resolved` 表示已结算。 |
| `market.ambiguity_level` | `low`、`medium`、`high` | 对应结算语义歧义等级。 |
| `market.tradability_status` | `tradable`、`manual_review`、`observe_only`、`blocked` | `manual_review` 表示研究/模拟可用，实盘需人工确认。 |
| `market_resolution_rules.review_status` | `pending`、`approved`、`rejected`、`not_required` | 结算规则人工复核状态。 |

### 6.2 事件与证据相关

| 字段 | 允许值 | 说明 |
| --- | --- | --- |
| `raw_event.source_type` | `news`、`social`、`official`、`calendar`、`market` | 原始输入源类型。 |
| `event.status` | `active`、`expired`、`invalidated`、`superseded` | `superseded` 表示被更新证据覆盖。 |
| `event_market_links.relation_type` | `direct`、`contextual`、`candidate`、`rejected` | `candidate` 表示仅进入候选映射，尚未形成稳定关联。 |
| `evidence.direction` | `supports_yes`、`supports_no`、`background` | `background` 只提供背景，不直接推动概率方向。 |
| `evidence.status` | `active`、`expired`、`invalidated`、`superseded` | 与事件状态保持同一语义体系。 |
| `probability_estimates.time_horizon` | `intraday`、`short`、`medium`、`until_resolution` | 仅表示影响持续期，不表示持仓时长承诺。 |

### 6.3 信号与订单相关

| 字段 | 允许值 | 说明 |
| --- | --- | --- |
| `signal.action` | `buy`、`sell` | 首版不引入 `hold` 作为可执行信号。 |
| `signal.side` | `yes`、`no` | 对应预测市场合约方向。 |
| `signal.lifecycle_state` | `new`、`active`、`weakened`、`executed`、`invalidated`、`reversed`、`expired` | 与后端状态机一致。 |
| `order.status` | `new`、`submitted`、`open`、`partially_filled`、`filled`、`canceled`、`expired`、`rejected` | 不允许跳过状态机直接覆盖。 |

### 6.4 系统与风控相关

| 字段 | 允许值 | 说明 |
| --- | --- | --- |
| `system.mode` | `research`、`paper_trade`、`manual_confirm`、`live_auto`、`kill_switch_locked` | `kill_switch_locked` 为保护模式，不等同于普通运行模式。 |
| `audit.result` | `accepted`、`succeeded`、`rejected`、`failed` | 用于审计日志结果字段。 |
| `outbox.status` | `pending`、`published`、`failed`、`dead_letter` | 用于内部事件投递状态。 |
| `idempotency.status` | `started`、`completed`、`failed` | 用于幂等键表状态。 |

---

## 7. 变更规则

1. 新增枚举值属于兼容性变更，但必须先更新文档和 API 契约。
2. 删除枚举值或改变其语义属于 breaking change。
3. 调整数值 scale 或舍入规则属于 breaking change。
4. 任何影响概率、价格、PnL 口径的修改，都应视为策略层或契约层变更。
