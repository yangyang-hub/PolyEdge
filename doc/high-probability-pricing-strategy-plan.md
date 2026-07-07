# 高概率定价模型设计与实现方案

最后更新：2026-07-07

## 状态

本文是策略研究与实施方案。当前仓库已落地 foundation：`HighProbabilityService` / `HighProbabilityStore`、Postgres/内存存储、`0050`–`0058` 迁移、`polyedge-worker import-high-probability-outcomes`、`build-high-probability-samples-once`、`refresh-high-probability-buckets-once`、`run-high-probability-backtest-once`、`observe-high-probability-once`、`refresh-high-probability-fair-values-once`、可选 `POLYEDGE_WORKER__POLL_HIGH_PROBABILITY_OBSERVE` / `POLYEDGE_WORKER__POLL_HIGH_PROBABILITY_FAIR_VALUES` 自动 poll loop、只读 `/api/v1/high-probability` / `/report` / `/backtests` / `/backtest-runs` / `/fair-values` API 和前端 `/high-probability` 研究页。当前可导入本地 outcome JSON 标签，并用本地 outcome 标签表 + 已入库 rewards price-history candles 构建 first-touch 样本（含结构化价格路径/流动性/时间/风险研究特征），对样本计算分桶统计、只读研究报告、即时 baseline walk-forward 回测和基础退出规则对比，可持久化 baseline 回测 run/trade/退出规则摘要，也可读取活跃 rewards 最新 candle 候选 + orderbook 服务缓存写入只读 observations，并可把 bucket stats + 当前盘口 + 风险标签融合成保守 `fair_yes_low/mid/high + confidence + uncertainty + reason_codes` 的 fair value 快照（`reward_market_fair_values`），供（未来的）Rewards 做市商 shadow/live 决策只读消费；自动 observe / fair value refresh 默认关闭，开启后按各自 interval 周期运行。全市场 price-history producer、outcome 自动同步、完整执行成本/多阶段退出回测、Phase 4 校准与漂移监控仍待实现；纸面模拟和实盘执行不再作为该模块目标。

按 2026-07-07 收敛方向，`sizing.rs`（Kelly 折扣/仓位上限/相关性折扣）与 `gates.rs`（入场硬过滤/退出过滤）的执行职责已迁出 `high_probability`，改由 Rewards 做市商的 inventory/risk engine 统一承担；该模块只输出可审计的定价输入，不再维护仓位或独立 kill switch。`features.rs` 已部分落地（价格路径/流动性/时间/风险标签特征），用于丰富历史样本和后续 Phase 2 可解释 ML；tier-depth 与 price-impact 特征列后续。

2026-07-07 设计方向收敛：`high_probability` 不再作为独立交易策略、paper 策略或 live guarded 策略推进。它应重构为 Rewards 做市商策略的 **fair value / base-rate pricing model provider**，负责历史样本、分桶统计、概率校准、模型漂移监控和只读研究解释；是否挂单、挂多大、如何退出和如何赚取 rewards 由 Rewards market maker 决策引擎统一处理。

该模块不能被简化为“价格达到 80% 就买入”。目标是建立一套可审计的概率定价系统：按市场类型、价格路径、流动性、剩余时间和事件风险估计保守胜率区间，为做市策略提供 `fair_probability_low/mid/high`、confidence、uncertainty 和 reason codes。

## 目标

核心目标：

- 从历史市场中统计高概率价格区间的真实胜率、反转率、最大回撤和资金占用收益。
- 针对不同市场类型和状态动态估计 `fair_probability`，而不是使用固定 80% 阈值。
- 输出可供 Rewards 做市商消费的公平概率区间、置信度、样本质量、回测误差和风险标签。
- 先做离线研究和 walk-forward 回测，再做 shadow calibration；不单独进入 paper/live 执行闭环。
- 所有外部数据由后台 producer 同步到数据库或 orderbook 服务缓存；策略、API 和前端只读取本地存储。

## 非目标

- 不直接让策略、API handler 或前端调用 Polymarket Gamma、CLOB 或 Data API。
- 不把 LLM 作为最终胜率或下单决策的唯一来源。
- 不用全市场平均反转率直接驱动实盘。
- 不作为独立下单模块；不新增 high_probability 专属 paper/live 订单、仓位、PnL 或实盘控制。
- 不承诺 80%、85% 或 90% 这类固定概率阈值长期有效。

## 定价假设

预测市场价格接近概率，但在部分细分场景下会有系统性偏差。例如：

- 结果已经基本发生、但市场尚未完全定价。
- 结算规则清晰、信息来源稳定、剩余时间较短。
- 高概率一侧经过持续成交确认，而不是被小额订单瞬间拉高。
- 同类型历史样本显示在当前价格和状态下，最终胜率显著高于可成交价格。

Rewards 做市商策略应优先利用这类“市场价格低于可验证真实概率”的机会；`high_probability` 只负责识别和量化这类概率偏差，不直接决定下单。

## 模型输出公式

对每个候选 token 计算保守定价输出：

```text
edge = fair_probability - executable_price
net_edge = edge - expected_slippage - fee_buffer - risk_margin
```

这些字段用于做市商决策引擎的 pricing component，不再作为独立入场条件。Rewards 做市商会把该定价 edge 与 rewards EV、退出成本、逆向选择成本和库存惩罚合并。

模型输出：

```text
fair_probability_low
fair_probability_mid
fair_probability_high
confidence
uncertainty_cents
calibration_error
recommended_max_entry_price
risk_tags
reason_codes
model_version
```

最高可接受买入价：

```text
max_entry_price = fair_probability
  - expected_slippage
  - fee_buffer
  - risk_margin
  - min_required_edge
```

历史回测仍可保留模拟仓位公式作为研究口径，但仓位不再由 `high_probability` 模块输出给执行器：

```text
position_size = capital
  * conservative_kelly_fraction
  * confidence_discount
  * liquidity_discount
  * correlation_discount
  * mode_cap
```

其中二元合约的简化 Kelly：

```text
kelly = (fair_probability - executable_price) / (1 - executable_price)
```

实际 live size 由 Rewards market maker 的 inventory/risk engine 决定。

## 目标架构

```text
Data producers
  polyedge-orderbook market sync
  price history / market state history sync
  settlement/outcome sync
  optional news/info-risk producer

Postgres
  markets / market_categories
  market price snapshots or candles
  market outcomes / settlement labels
  high-probability samples
  model versions / bucket stats
  backtest runs / simulated trades for calibration only
  fair value snapshots for Rewards market maker

Application layer
  HighProbabilityResearchService
  HighProbabilityModelService
  HighProbabilityFairValueProvider
  Store traits and pure scoring/gating helpers

Worker tasks
  build historical samples
  refresh bucket statistics
  run backtests
  export fair value snapshots
  monitor calibration drift

API + Frontend
  research report
  bucket statistics
  backtest results
  fair value diagnostics
  calibration and model quality
```

## 数据来源与缺口

### 可复用数据

- `markets`：市场问题、分类、状态、流动性、24h volume、结束时间、condition/token 映射。
- `reward_markets`：奖励市场的 rewards 参数和 token 目录。
- `reward_market_candles`：当前只覆盖 rewards token 的 5 分钟 price-history source candles，可用于 rewards 子集研究。
- Orderbook 服务缓存：当前盘口、top levels、确认时间和订阅流。
- Rewards AI / info-risk 结果：可作为事件风险和规则风险标签的参考，但不能直接替代统计模型。

### 必须补齐的数据

该策略如果要覆盖“所有市场”，需要新增历史数据生产链路：

1. **市场价格历史**
   - 保存 token 级价格 candles 或 snapshots。
   - 至少包含 close/mid/best_bid/best_ask、spread、sample_count、observed_at。
   - 不能只依赖当前 orderbook 进程内缓存，因为服务重启会丢失历史。

2. **市场最终结果**
   - 保存 condition/token 的 resolved outcome。
   - 区分 `won`、`lost`、`voided`、`ambiguous`、`unresolved`。
   - 记录 resolution source、resolved_at 和是否存在争议。

3. **市场类型与风险标签**
   - 规则分类：sports、politics、crypto、macro、company_event、weather、entertainment、official_confirmation、other。
   - 风险标签：ambiguous_rules、subjective_resolution、single_source_news、regulatory_or_court、long_horizon、thin_liquidity、event_already_occurred。

4. **执行可行性数据**
   - 买入时 ask 深度、卖出时 bid 深度。
   - 可成交金额、预计滑点、部分成交率。
   - 后续退出路径和最大浮亏。

## 样本定义

每个样本不是一个市场，而是一个“在某个时间点可交易的 token 状态”。

```text
sample = {
  condition_id,
  token_id,
  side,
  sampled_at,
  executable_price,
  price_bucket,
  market_type,
  risk_tags,
  time_to_resolution_bucket,
  liquidity_bucket,
  spread_bucket,
  path_features,
  outcome,
  realized_pnl,
  max_drawdown,
  hold_seconds
}
```

### 触发采样

初始版本建议在 token 进入以下价格区间时采样：

```text
0.55 - 0.60
0.60 - 0.65
0.65 - 0.70
0.70 - 0.75
0.75 - 0.80
0.80 - 0.85
0.85 - 0.90
0.90 - 0.95
0.95 - 0.99
```

同一个 token 同一个价格 bucket 可以保留：

- first touch 样本：首次进入该区间。
- sustained 样本：在该区间停留超过指定时间并有成交/盘口确认。
- re-entry 样本：跌出后重新进入，用于识别反复冲高回落风险。

### 标签定义

最终胜负：

```text
win = token 最终结算为 1
loss = token 最终结算为 0
```

反转标签：

```text
drawdown_10c = 买入后价格曾下跌 >= 0.10
drawdown_20c = 买入后价格曾下跌 >= 0.20
break_70 = 买入后跌破 0.70
break_60 = 买入后跌破 0.60
break_50 = 买入后跌破 0.50
```

收益标签：

```text
settlement_pnl = outcome - executable_price
exit_pnl = simulated_exit_price - executable_price
capital_days = notional * hold_seconds / 86400
return_per_capital_day = pnl / max(capital_days, epsilon)
```

## 特征设计

### 市场身份特征

- market_type
- category
- tags
- question length / keyword groups
- binary yes/no completeness
- ambiguity level
- settlement source type
- whether outcome has likely occurred

### 时间特征

- time_to_resolution
- market age
- weekday/hour
- time since last large price move
- time since first crossing target bucket

### 价格路径特征

- current price bucket
- price 5m/1h/6h/24h return
- realized volatility
- maximum run-up before sampling
- number of prior bucket crossings
- time spent above 70/80/90
- largest prior drawdown
- monotonic trend score

### 流动性与盘口特征

- bid/ask spread
- top-of-book depth
- depth within 1c/3c/5c
- executable size at target notional
- 24h volume
- liquidity_usd
- orderbook age / confirmed_at freshness
- price impact for entry and exit

### 事件风险特征

- ambiguous_rules
- subjective_resolution
- regulatory_or_court_dependency
- official_confirmation_pending
- single_source_news
- high_news_velocity
- source_conflict
- long_horizon

LLM 可以辅助打这些风险标签，但模型输出必须缓存、审计，并被确定性 gate 使用；不能让 LLM 单独决定 fair probability 或仓位。

## 模型方案

### Phase 1：分桶统计

先不用复杂模型，按以下维度聚合：

```text
market_type
price_bucket
time_to_resolution_bucket
liquidity_bucket
spread_bucket
risk_tag_group
path_shape
```

每个 bucket 输出：

- sample_count
- win_rate
- Wilson / beta-binomial confidence interval
- expected_pnl
- max_drawdown distribution
- break_70 / break_60 / break_50 probability
- average hold time
- return per capital day
- recommended max entry price

bucket 样本不足时必须回退到更粗层级：

```text
exact bucket
→ remove path_shape
→ remove spread_bucket
→ remove liquidity_bucket
→ market_type + price_bucket + time_to_resolution
→ global prior
```

回退后的 `fair_probability` 要增加 `risk_margin`，避免小样本过拟合。

### Phase 2：可解释机器学习

在分桶统计验证有效后，可增加：

- logistic regression：作为可解释基线。
- gradient boosted trees：捕捉非线性和交互项。
- calibrated probability model：Platt scaling / isotonic calibration。
- Bayesian hierarchical model：按市场类型共享先验，减少小类样本不稳定。

模型输出必须包含：

```text
fair_probability
confidence
calibration_error
top_positive_features
top_risk_features
model_version
training_window
```

### Phase 3：在线校准

实盘前需要持续监控：

- predicted probability vs realized outcome calibration。
- 不同 bucket 的胜率漂移。
- 最近 N 天/周是否低于训练期表现。
- 价格分布和流动性分布是否发生 regime shift。

发现模型漂移时，fair value provider 应降低 confidence、扩大 uncertainty，或停止向 Rewards 做市商输出可 live 使用的 fair value；是否停止新增订单由 Rewards 做市商统一执行。

## Fair Value 可用性规则

候选必须先通过模型可用性过滤：

- 市场 open/tradable。
- token 映射唯一且方向明确。
- orderbook 新鲜且非空。
- spread 不超过模型配置。
- 最小深度满足样本/估值质量要求。
- 结算规则不在强排除标签中。
- 历史 bucket 样本量、时间窗口和模型版本可用。

再输出概率定价：

```text
fair_probability >= min_fair_probability
net_edge >= min_required_edge
confidence >= min_confidence
```

推荐默认初始阈值：

```text
min_required_edge = 0.03
fee_buffer = 0.005
min_confidence = 0.60
max_spread_cents = 3
min_depth_usd = 50
```

这些只是研究初始值，必须由回测和 Rewards 做市 shadow decisions 校准。

## 退出规则研究

必须继续回测多种退出规则，不能只假设持有到结算。当前已在 baseline walk-forward 报告中加入基础退出规则对比：`settlement`、`take_profit_90`、`take_profit_95`、`stop_loss_70`、`stop_loss_60`。这些规则只用于校准 fair value、uncertainty 和风险标签，具体 SELL、merge、flatten 和库存退出由 Rewards 做市商 inventory manager 执行。

候选退出模式：

1. **hold_to_resolution**
   - 最简单，适合短周期、规则清晰、真实胜率极高的市场。

2. **profit_take**
   - 例如 `entry + 0.08` 或价格达到 0.95 时减仓。
   - 适合长周期市场降低资金占用。

3. **risk_stop**
   - 价格跌破动态置信阈值或出现新风险标签时退出。
   - 不能用固定止损替代统计验证。

4. **time_stop**
   - 持仓超过预期周期但 edge 消失时退出。

5. **liquidity_stop**
   - spread 或退出深度恶化到不可接受时停止加仓或减仓。

6. **event_risk_stop**
   - 规则争议、官方口径冲突、法院/监管新变量出现时退出或冻结新增。

回测输出必须同时包含持有到结算收益和各退出规则收益，用于评估做市商拿到库存后的退出风险，而不是驱动独立高概率执行器。

## 仓位研究与风控输入

### 单笔仓位

```text
base_fraction = min(conservative_kelly, max_single_trade_fraction)
size = capital * base_fraction
size = min(size, executable_depth_usd * depth_usage_ratio)
size = min(size, per_market_cap_usd)
```

建议初始研究配置：

```text
conservative_kelly_multiplier = 0.10
max_single_trade_fraction = 0.02
per_market_cap_usd = 25
depth_usage_ratio = 0.20
```

### 组合限制

- 单 condition / event cluster / market_type 的历史相关性标签。
- 同一主题/候选人/比赛/资产方向的 correlation group。
- 最大相关亏损场景损失估计。

这些输出交给 Rewards 做市商的 `InventoryRiskService` 使用；`high_probability` 不维护实际仓位。

### Tail risk 处理

高概率低赔率策略的主要风险是单次失败损失抵消多次盈利。必须定期统计：

- 最大连续亏损。
- 99% expected shortfall。
- 单日极端亏损。
- 同类事件相关失败。
- 市场突然 void/规则争议的损失。

## 回测设计

### 回测原则

- 只使用当时可见数据。
- 不使用结算后才知道的分类或标签，除非这些标签在样本时点已存在。
- 计入 bid/ask spread、可成交深度、滑点和部分成交。
- 训练集、验证集、测试集按时间切分。
- 使用 walk-forward，而不是随机打乱。
- 单独报告所有被过滤掉样本的数量和原因。

### Walk-forward 流程

```text
1. 用 T0-T1 训练 bucket/model。
2. 在 T1-T2 生成交易信号并模拟执行。
3. 滚动窗口前移。
4. 聚合所有 out-of-sample 结果。
```

### 对照基准

必须与以下 baseline 比较：

- 买入所有价格 >= 0.80 的市场。
- 买入所有 edge > 0 的模型市场，不做风险过滤。
- 只按市场类型过滤，不做价格路径过滤。
- 只持有到结算，不做退出。
- 不考虑滑点的理想成交。

如果模型不能稳定优于这些基准，它输出的 fair value 不应被 Rewards 做市商用于 live 决策，只能保留为研究/诊断信号。

## 数据库设计建议

新增迁移建议在当前最新迁移之后追加。字段使用 `TEXT` + `CHECK` 表达状态，金额和概率使用 `NUMERIC`，动态解释使用 `JSONB`。

### `high_prob_market_candles`

用于覆盖非 rewards 市场的 token 价格历史。若后续统一扩展现有 candle 表，也可以改为通用 `market_token_candles`。

```sql
CREATE TABLE high_prob_market_candles (
    condition_id TEXT NOT NULL,
    token_id TEXT NOT NULL,
    interval TEXT NOT NULL,
    bucket_start TIMESTAMPTZ NOT NULL,
    open NUMERIC NOT NULL CHECK (open >= 0 AND open <= 1),
    high NUMERIC NOT NULL CHECK (high >= 0 AND high <= 1),
    low NUMERIC NOT NULL CHECK (low >= 0 AND low <= 1),
    close NUMERIC NOT NULL CHECK (close >= 0 AND close <= 1),
    best_bid_close NUMERIC CHECK (best_bid_close >= 0 AND best_bid_close <= 1),
    best_ask_close NUMERIC CHECK (best_ask_close >= 0 AND best_ask_close <= 1),
    spread_cents_close NUMERIC,
    depth_bid_1c_usd NUMERIC,
    depth_ask_1c_usd NUMERIC,
    sample_count BIGINT NOT NULL DEFAULT 0,
    source TEXT NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (condition_id, token_id, interval, bucket_start)
);

CREATE INDEX high_prob_market_candles_token_recent_idx
    ON high_prob_market_candles (token_id, bucket_start DESC);
```

### `high_prob_market_outcomes`

```sql
CREATE TABLE high_prob_market_outcomes (
    condition_id TEXT PRIMARY KEY,
    status TEXT NOT NULL
        CHECK (status IN ('unresolved', 'resolved', 'voided', 'ambiguous')),
    winning_token_id TEXT,
    resolved_at TIMESTAMPTZ,
    resolution_source TEXT,
    dispute_status TEXT NOT NULL DEFAULT 'none'
        CHECK (dispute_status IN ('none', 'possible', 'active', 'settled')),
    raw JSONB NOT NULL DEFAULT '{}',
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
```

### `high_prob_market_labels`

```sql
CREATE TABLE high_prob_market_labels (
    condition_id TEXT PRIMARY KEY,
    market_type TEXT NOT NULL,
    risk_tags JSONB NOT NULL DEFAULT '[]',
    classifier_version TEXT NOT NULL,
    label_source TEXT NOT NULL,
    confidence NUMERIC NOT NULL DEFAULT 0 CHECK (confidence >= 0 AND confidence <= 1),
    raw JSONB NOT NULL DEFAULT '{}',
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX high_prob_market_labels_type_idx
    ON high_prob_market_labels (market_type);
```

### `high_probability_samples`

```sql
CREATE TABLE high_probability_samples (
    id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    condition_id TEXT NOT NULL,
    token_id TEXT NOT NULL,
    side TEXT NOT NULL CHECK (side IN ('yes', 'no')),
    sampled_at TIMESTAMPTZ NOT NULL,
    trigger_kind TEXT NOT NULL
        CHECK (trigger_kind IN ('first_touch', 'sustained', 're_entry')),
    executable_price NUMERIC NOT NULL CHECK (executable_price >= 0 AND executable_price <= 1),
    price_bucket TEXT NOT NULL,
    market_type TEXT NOT NULL,
    time_to_resolution_bucket TEXT,
    liquidity_bucket TEXT,
    spread_bucket TEXT,
    path_features JSONB NOT NULL DEFAULT '{}',
    risk_tags JSONB NOT NULL DEFAULT '[]',
    outcome TEXT CHECK (outcome IN ('win', 'loss', 'voided', 'unknown')),
    settlement_pnl NUMERIC,
    max_drawdown_cents NUMERIC,
    hold_seconds BIGINT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (condition_id, token_id, sampled_at, trigger_kind, price_bucket)
);

CREATE INDEX high_probability_samples_bucket_idx
    ON high_probability_samples (market_type, price_bucket, sampled_at DESC);
CREATE INDEX high_probability_samples_condition_idx
    ON high_probability_samples (condition_id);
```

### `high_probability_market_outcomes`

本地 outcome 标签表，当前由人工、脚本或后续 producer 写入。样本构建只消费该表中已有标签的 condition，避免从 `markets.status` 猜 winning token。

```sql
CREATE TABLE high_probability_market_outcomes (
    condition_id TEXT PRIMARY KEY,
    status TEXT NOT NULL
        CHECK (status IN ('unresolved', 'resolved', 'voided', 'ambiguous')),
    winning_token_id TEXT,
    resolved_at TIMESTAMPTZ,
    market_type TEXT NOT NULL DEFAULT 'unknown',
    risk_tags JSONB NOT NULL DEFAULT '[]',
    label_source TEXT NOT NULL DEFAULT 'manual',
    raw JSONB NOT NULL DEFAULT '{}',
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
```

### `high_probability_bucket_stats`

```sql
CREATE TABLE high_probability_bucket_stats (
    id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    model_version TEXT NOT NULL,
    bucket_key TEXT NOT NULL,
    bucket_dimensions JSONB NOT NULL,
    sample_count BIGINT NOT NULL,
    win_count BIGINT NOT NULL,
    win_rate NUMERIC NOT NULL,
    fair_probability NUMERIC NOT NULL CHECK (fair_probability >= 0 AND fair_probability <= 1),
    confidence_low NUMERIC,
    confidence_high NUMERIC,
    expected_pnl NUMERIC,
    avg_max_drawdown_cents NUMERIC,
    break_70_rate NUMERIC,
    break_60_rate NUMERIC,
    break_50_rate NUMERIC,
    avg_hold_seconds BIGINT,
    recommended_max_entry_price NUMERIC,
    computed_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (model_version, bucket_key)
);
```

### `high_probability_backtest_runs` / `high_probability_backtest_trades`

```sql
CREATE TABLE high_probability_backtest_runs (
    id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    run_at TIMESTAMPTZ NOT NULL,
    model_version TEXT NOT NULL,
    market_scope TEXT NOT NULL,
    sample_limit BIGINT NOT NULL,
    trade_count BIGINT NOT NULL,
    win_rate NUMERIC,
    total_pnl NUMERIC NOT NULL,
    roi NUMERIC,
    max_drawdown NUMERIC NOT NULL,
    exit_rule_reports JSONB NOT NULL DEFAULT '[]',
    notes JSONB NOT NULL DEFAULT '[]',
    config_json JSONB NOT NULL DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE high_probability_backtest_trades (
    id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    run_id BIGINT NOT NULL REFERENCES high_probability_backtest_runs(id) ON DELETE CASCADE,
    sample_id BIGINT NOT NULL REFERENCES high_probability_samples(id),
    condition_id TEXT NOT NULL,
    token_id TEXT NOT NULL,
    sampled_at TIMESTAMPTZ NOT NULL,
    bucket_key TEXT NOT NULL,
    executable_price NUMERIC NOT NULL,
    fair_probability NUMERIC NOT NULL,
    net_edge NUMERIC NOT NULL,
    outcome TEXT NOT NULL CHECK (outcome IN ('win', 'loss')),
    settlement_pnl NUMERIC NOT NULL,
    cumulative_pnl NUMERIC NOT NULL,
    drawdown NUMERIC NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX high_probability_backtest_trades_run_idx
    ON high_probability_backtest_trades (run_id, sampled_at, id);
```

### `high_probability_observations`

历史兼容表，用于记录只读 observation。新设计不再扩展为 high_probability 专属 paper/live 决策表；后续应新增或迁移到 fair value snapshot / market maker decision audit 表。

```sql
CREATE TABLE high_probability_observations (
    id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    observed_at TIMESTAMPTZ NOT NULL,
    condition_id TEXT NOT NULL,
    token_id TEXT NOT NULL,
    mode TEXT NOT NULL CHECK (mode IN ('observe', 'paper', 'live_guarded')), -- paper/live_guarded 为历史兼容值，不再作为新路线推进
    executable_price NUMERIC NOT NULL,
    fair_probability NUMERIC,
    net_edge NUMERIC,
    recommended_size_usd NUMERIC,
    decision TEXT NOT NULL CHECK (decision IN ('allow', 'reject', 'skip')),
    reasons JSONB NOT NULL DEFAULT '[]',
    model_version TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX high_probability_observations_recent_idx
    ON high_probability_observations (observed_at DESC);
CREATE INDEX high_probability_observations_condition_idx
    ON high_probability_observations (condition_id, observed_at DESC);
```

## 后端实现方案

### Application crate

新增模块建议：

```text
packages/backend/crates/application/src/high_probability/
```

核心文件：

- `models.rs`：已实现 outcome 标签、rewards candle 输入、样本、bucket、观察记录、决策、配置、build/refresh report、baseline backtest run/trade 和退出规则摘要模型。
- `bucket_model.rs`：已实现基础分桶统计和 beta(1,1) 保守 `fair_probability` 估计。
- `sample_builder.rs`：已实现从 rewards candle 输入构建 first-touch 样本、胜负标签、最大回撤、基础 bucket 特征和后续退出规则需要的 `min_future_close` / `max_future_close` 路径特征。
- `service.rs`：已实现 `HighProbabilityService`，支持读取配置、snapshot、记录样本、从 rewards candles 构建样本、刷新 bucket stats、生成即时 baseline 回测报告与基础退出规则对比、持久化 baseline 回测 run/trade 和读取历史 run/trade。
- `features.rs`：待实现价格路径、流动性、时间、风险标签特征计算。
- `sizing.rs`：待实现 Kelly 折扣、仓位上限和相关性折扣。
- `gates.rs`：待实现入场硬过滤和退出过滤。
- `service.rs`：当前包含 70/30 walk-forward baseline 回测 helper、基础退出规则对比和持久化 run/trade；完整 `backtest.rs`、订单簿执行成本模型和多阶段退出规则仍待后续实现。

Application 层只定义业务逻辑和端口，不直接访问外部 API。

### Infrastructure crate

新增 Postgres store：

```text
packages/backend/crates/infrastructure/src/stores/high_probability.rs
```

职责：

- 已实现读写 config / market outcomes / samples / bucket stats / baseline backtest runs/trades / `exit_rule_reports` / observations，并可读取 rewards candle sample inputs。
- 全市场 candles/outcomes/labels producer 待后续代码落地时新增。
- 提供分页和聚合查询。

### Worker app

新增 worker 子任务：

1. `high-prob-import-outcomes`
   - 从本地 JSON 文件导入/更新 `high_probability_market_outcomes`，用于人工或离线脚本准备结算标签。
   - 当前已以 `import-high-probability-outcomes <path>` CLI 名称实现；支持顶层数组或 `{ "outcomes": [...] }`，字段包括 `condition_id`、`status`、`winning_token_id`、`resolved_at`、`market_type`、`risk_tags`、`label_source`、`raw`。
   - `status=resolved` 必须提供 `winning_token_id` 和 RFC3339 `resolved_at`；该命令只读取本地文件，不调用 Polymarket/Gamma/CLOB/Data API。

2. `high-prob-build-samples-once`
   - 从 candles、markets、outcomes、labels 构建历史样本。
   - 当前已以 `build-high-probability-samples-once [limit]` CLI 名称实现，数据源为 `reward_market_candles` + `high_probability_market_outcomes` + `markets`。

3. `high-prob-compute-buckets-once`
   - 用已结算样本计算 bucket stats。
   - 当前已以 `refresh-high-probability-buckets-once` CLI 名称实现。

4. `high-prob-backtest-once`
   - 按指定训练/测试窗口跑 walk-forward 回测。
   - 当前已以 `run-high-probability-backtest-once` CLI 名称实现 baseline 70/30 回测、基础退出规则摘要和交易明细持久化。

5. `high-prob-observe-loop`
   - 历史兼容任务：周期扫描当前市场，只记录实时 observation，不交易。
   - 当前已以 `observe-high-probability-once [limit]` CLI 名称实现一次性只读扫描，并可通过 `POLYEDGE_WORKER__POLL_HIGH_PROBABILITY_OBSERVE=true` 接入 API 内嵌 worker runtime；默认关闭，间隔由 `POLYEDGE_WORKER__HIGH_PROBABILITY_OBSERVE_INTERVAL_SECS` 控制。
   - 新设计中该任务应逐步改为 `high-prob-fair-value-refresh`：为 rewards 候选 condition 写入 fair value snapshots，供 Rewards market maker shadow/live 决策读取。

6. `high-prob-fair-value-refresh`
   - 已实现。读取历史 bucket stats、当前 rewards 候选、candles/orderbook/cache 和风险标签，按 condition 融合成保守 `fair_yes_low/mid/high + confidence + uncertainty_cents + reason_codes + live_eligible` 快照，写入 `reward_market_fair_values`（`UNIQUE(condition_id, model_version)`，TTL 过期）。
   - bucket 解析走 5 维回退链（精确 → 丢 spread → 丢 liquidity → 丢 time → 仅 price → 全局先验），越粗化 uncertainty 越大、confidence 越低；`fair_yes` 优先用 YES token 价区间命中 bucket，否则用 NO token 价补数；`live_eligible` 是独立 gate 的纯 AND（confidence/uncertainty/样本量/粗化层级/排除风险标签/价差/盘口完整/深度/陈旧），每项失败 push 对应 reason。
   - 当前已以 `refresh-high-probability-fair-values-once [limit]` CLI 名称实现，并可通过 `POLYEDGE_WORKER__POLL_HIGH_PROBABILITY_FAIR_VALUES=true` 接入 API 内嵌 worker runtime；默认关闭，间隔由 `POLYEDGE_WORKER__HIGH_PROBABILITY_FAIR_VALUE_INTERVAL_SECS` 控制。`fair_value_enabled` 配置默认 false。
   - 不创建订单、不维护 paper fills、不调用 live connector。

所有任务只能读取数据库和 orderbook 服务，不能直接调用 Polymarket 外部 API。

### Orderbook / producer 扩展

如果要覆盖所有市场，需要在 producer 侧增加价格历史同步，而不是让策略临时抓取：

- 从已注册 token、活跃市场、候选 bucket 市场中选择 token。
- 低频写入 `high_prob_market_candles`。
- 对高价值候选可临时注册 orderbook source 以获得更高频盘口。
- 写入时保留 `source` 和 `sample_count`，区分 price-history provider、WS/poll midpoint 和 orderbook snapshot。

如果第一阶段只覆盖 rewards 子集，可以先复用 `reward_market_candles`，暂不扩展全市场 candle 表。

### API

当前已实现只读路由：

```text
GET /api/v1/high-probability
GET /api/v1/high-probability/buckets
GET /api/v1/high-probability/config
GET /api/v1/high-probability/report
GET /api/v1/high-probability/backtests
GET /api/v1/high-probability/backtest-runs
GET /api/v1/high-probability/backtest-runs/{run_id}/trades
GET /api/v1/high-probability/fair-values
```

后续可继续新增：

```text
PATCH /api/v1/high-probability/config
POST /api/v1/high-probability/commands
```

当前只读 API 只读取 `HighProbabilityService` / store，不执行 outcome 导入、样本构建、分桶刷新或回测写入。`/report` 由现有样本和当前模型版本 bucket stats 计算样本覆盖、胜负分布、合格 bucket 数、加权胜率/期望和数据提示；`/backtests` 当前返回即时 70/30 walk-forward baseline 报告，用较早已结算样本训练 bucket，用较晚样本按当前 edge/缓冲规则模拟是否入场，并返回基础退出规则对比；`/backtest-runs` 与 trades 子路由读取通过 worker CLI 持久化的历史 run/trade/退出规则摘要。后续写操作只入队研究/刷新命令；API 不抓外部数据、不下单。

### Frontend

当前已新增只读研究页 `/high-probability`，不是交易页。已实现内容：

- 策略总览：当前启用状态、模式、模型版本、bucket 数、总样本数、最低净边际和单笔上限。
- 研究报告：已结算样本、胜负分布、合格分桶数、正期望分桶数、加权胜率/期望、最佳/最差 bucket 和数据提示。
- Baseline 回测：训练/测试样本数、候选/交易/跳过数量、胜率、PnL、ROI、最大回撤、基础退出规则对比和数据提示。
- Bucket 表：按市场类型/价格区间/剩余时间/流动性/价差维度展示样本数、胜率、回撤、跌破阈值和推荐最高买入价。
- Observations / fair value diagnostics 表：读取已入库 observation 或后续 fair value snapshots，展示可成交价格、fair probability、net edge、置信度、uncertainty 和原因。
- 配置摘要：展示只读研究配置，不提供保存、命令或交易按钮。

后续可继续扩展：

- 持久化 Backtest 展示：按 run/trade 保存收益、最大回撤、胜率、交易明细和基础退出规则比较；完整执行成本和做市库存退出风险仍待后续补充。
- 配置页：model version、样本量门槛、confidence、uncertainty、市场类型开关；只能在后端 worker command queue 和写操作契约落地后添加。

前端文案必须明确：研究结果和 fair value 输出只是 Rewards 做市策略的定价输入，不是独立交易建议或已验证实盘收益。

## 配置建议

```text
enabled = false
mode = research
market_scope = rewards_only
model_version = bucket_v1
min_required_edge = 0.03
fee_buffer = 0.005
default_risk_margin = 0.02
min_confidence = 0.60
min_bucket_samples = 100
max_spread_cents = 3
min_depth_usd = 50
max_single_trade_usd = 25
max_single_market_exposure_usd = 50
max_market_type_exposure_usd = 150
max_daily_new_notional_usd = 100
conservative_kelly_multiplier = 0.10
excluded_risk_tags = ["ambiguous_rules", "subjective_resolution"]
```

## 实施阶段

### Phase 0：研究口径确认

输出：

- 市场范围：先 `rewards_only` 还是全市场。
- 样本触发规则。
- 反转和收益标签定义。
- 初始市场类型和风险标签 taxonomy。

验收：

- 文档化指标定义。
- 给出最小样本构建 SQL/服务接口设计。

### Phase 1：历史样本与 bucket report

输出：

- 新增历史样本构建任务。
- 新增 bucket stats 计算。
- 生成离线 markdown/JSON report 或 API snapshot；当前已提供 `/api/v1/high-probability` snapshot、`/buckets`、`/report`、`/backtests`、`/backtest-runs` 和 `/high-probability` 只读控制台页面。

验收：

- 至少输出每个 bucket 的样本量、胜率、回撤、期望收益。
- 样本不足 bucket 明确标记不可用。

### Phase 2：回测

输出：

- walk-forward backtest。
- 执行成本模型。
- 多种退出规则比较。
- baseline 对照。

验收：

- out-of-sample 净收益、最大回撤和资金日收益稳定优于 baseline。
- 明确哪些市场类型被剔除，以及剔除后收益是否仍有样本支撑。

### Phase 3：Fair Value Provider 集成

输出：

- 实时扫描 rewards 候选并写入 fair value snapshots。
- Rewards market maker shadow decisions 读取 fair value provider 输出。
- 控制台展示 fair probability 区间、confidence、uncertainty、bucket 样本和 reason codes。
- 不生成订单。

验收：

- 连续运行至少 2-4 周。
- fair value 输出与历史 bucket 分布一致，无明显模型漂移。
- Rewards market maker shadow decisions 能引用具体 fair value version/input hash。

### Phase 4：Calibration 与 Drift Monitoring

输出：

- 按模型版本持续记录 calibration error、bucket 漂移、样本覆盖和后验误差。
- 当模型漂移或样本不足时，降低 confidence、扩大 uncertainty 或停止输出 live-eligible fair value。
- 做市商实际成交后回填 outcome/PnL attribution，用于校准 fair value provider。

验收：

- 高 confidence bucket 的实际结果校准误差在阈值内。
- 模型不可用时 Rewards market maker 能 fail closed。
- 不存在 high_probability 独立 paper/live 执行路径。

## 主要风险

- **样本偏差**：只看到活跃或幸存市场会高估胜率。
- **结算标签漂移**：部分市场规则争议会污染训练标签。
- **执行偏差**：历史 price close 不等于当时可成交 ask。
- **小样本过拟合**：细分 bucket 胜率极不稳定。
- **相关性集中**：多个市场可能对应同一个真实事件风险。
- **资金效率低**：高概率低赔率持仓到结算可能年化不佳。
- **尾部亏损重**：一次 0.80 买入失败需要多次 0.80 胜利来弥补。

## 最小可验证版本建议

第一版建议不要覆盖所有市场，先做：

```text
scope = rewards_only
data = reward_market_candles + markets + reward_markets + 后续 outcome 标签
model = bucket_v1
execution = no live, report only
```

最小 report：

- 按市场类型和价格 bucket 的最终胜率。
- 价格达到 bucket 后跌破 70/60/50 的概率。
- 平均最大回撤。
- 平均持仓时间。
- 按可成交价格计算的期望收益。
- 推荐最高买入价。
- 样本量不足和规则风险剔除原因。

只有当该 report 在 out-of-sample 中稳定显示部分 bucket 有正 `net_edge`，才允许输出给 Rewards 做市商的 shadow decision 使用；进入 guarded live 仍由 Rewards 做市商统一验收。
