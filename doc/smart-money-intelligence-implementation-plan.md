# Smart Money Intelligence 实施计划

最后更新：2026-06-27

## 状态

本文是聪明钱跟单重构方案和实施计划，描述目标能力和分阶段落地路径。当前仓库的 `copytrade` 仍是只读钱包跟踪与分析：手工维护 tracked wallets，worker 读取这些钱包的 Polymarket Data API activity/positions，记录 source trades 并更新钱包分析统计；不会下单或撤单。Smart Money Intelligence 已落地 Phase 1 foundation，并继续落地 Phase 2 的确定性信号流和 advisory 缓存/provider 基础：`scan-smart-money-once` 和可选定时扫描可从 Polymarket Data API leaderboard 与 active copytrade tracked wallets 派生候选，再按 tracked/watch/candidate 状态扫描候选钱包，读取 Data API activity/positions/closed positions/trades，写入近端样本画像、确定性评分和源交易；worker 会对未处理 source trades 读取 orderbook 服务缓存，按年龄、盘口、方向价格、滑点和最优档深度生成 `observe` 或 `rejected` 信号，并写入 `stage=deterministic_gate` 的 signal decision 审计记录；application/store 已能读写 `smart_signal_advisories` 缓存并在 snapshot DTO 中返回最近 signal advisory，application 已能构造结构化 signal advisory provider payload 和稳定 input_hash，worker 在 `signal_advisory_enabled=true` 时会按 Smart Money 配置中的 provider/request-format/model 和 `POLYEDGE_SMART_MONEY__SIGNAL_ADVISORY_*` env-only key/base URL 为近期 observe 信号补齐源交易/profile/score 上下文、构造 advisory request/input_hash、检查缓存，并在 provider key 存在时调用 `SmartSignalAdvisoryConnector` 保存 `allow|observe|reject` 三态 advisory；API 已支持配置保存和候选钱包状态更新，可把候选晋级为 watch/tracked 或标记 blocked/rejected；`/copy-trading` 前端已接入 Smart Money snapshot，并提供配置保存（含 signal advisory provider/request format/model）、候选池查看、状态操作、基础信号流和最近 signal advisory 展示；定时扫描默认关闭，需同时开启 `POLYEDGE_WORKER__POLL_SMART_MONEY=true` 和 Smart Money config `enabled=true`；已实现的是 leaderboard 种子发现、基础配置、候选管理、非执行信号/decision 流、signal advisory 缓存层、request builder、独立 provider 配置和 provider refresh，不是完整全网 discovery，仍不会生成可执行跟随订单，也不会执行纸面或实盘交易。

## 目标

把现有 `copytrade` 重构为 `Smart Money Intelligence`：自动发现优质 Polymarket 钱包，构建可审计的钱包画像和评分，生成受控跟随信号，先做观察与纸面模拟，再按显式配置进入小额受控实盘。

核心目标：

- 自动发现候选钱包，而不是只依赖手工添加。
- 用确定性指标评估钱包质量和可跟性。
- 使用大模型做解释、信息风险识别和 advisory，不让模型直接决定下单。
- 所有外部数据抓取由后台 worker 执行，API 和前端只读数据库或写控制命令。
- 每个信号、拒绝原因、模型建议和执行结果都可回放、可审计。
- 实盘执行必须经过价格、盘口、延迟、资金和敞口硬风控。

## 非目标

- 第一阶段不实现自动实盘跟单。
- 不在 API handler 或前端请求中临时抓取 Polymarket、CLOB、Data API 或链上 RPC。
- 不把大模型输出作为唯一资金动作依据。
- 不恢复旧 copytrade 模拟账户 UI 作为产品主路径；新纸面模拟应围绕信号质量和可跟窗口验证。

## 设计原则

1. **Worker-first 数据生产**：发现、画像、评分、信号、LLM advisory、纸面模拟和实盘执行都由 worker 消费数据库状态后推进。
2. **确定性核心**：ROI、胜率、回撤、成交量、滑点、盘口深度、延迟和敞口限制必须由代码计算。
3. **LLM 只做 advisory**：模型输出 `allow / observe / reject`、风险标签和解释；硬规则仍可 fail closed。
4. **先验证再执行**：必须先跑 observe/paper，证明扣除延迟和滑点后仍有正期望，再开放 live guarded。
5. **来源可追踪**：候选钱包来源、源交易、信号、决策和执行记录都必须有 trace id、raw payload 和版本字段。

## 目标架构

```text
External producers
  Polymarket Data API / leaderboard / recent trades
  Polygon chain indexer (phase 2)
  Gamma markets + orderbook service cache

Worker tasks
  smart_wallet_discovery
  smart_wallet_profiler
  smart_wallet_scorer
  smart_signal_detector
  smart_wallet_advisory_refresh
  smart_signal_advisory_refresh
  smart_signal_simulator
  smart_guarded_executor (phase 4)

Application service
  SmartMoneyService
  SmartMoneyStore trait
  deterministic scoring and gating helpers

Postgres
  candidates / profiles / scores / trades
  signals / decisions / advisories
  paper executions / live intents

API + Frontend
  candidate leaderboard
  wallet detail
  signal feed
  paper performance
  config and control commands
```

## 数据来源

### Phase 1 来源

- Polymarket Data API leaderboard：用于种子钱包发现。
- Polymarket Data API recent/user trades：用于候选钱包交易历史和 tracked wallet 增量交易。
- Polymarket Data API positions/closed positions/value/traded：用于钱包画像。
- 本地 `markets` 表：用于市场问题、分类、状态、流动性和结束时间。
- Orderbook 服务：用于当前价格、盘口深度和跟随窗口判断。

### Phase 2 来源

- Polygon 链上事件索引：用于校验 Data API 发现的钱包、识别 proxy wallet、补充资金流和交易事件。
- CTF token transfer / exchange fill 事件：用于更完整的链上成交归因。

链上索引不建议作为第一阶段核心路径。它能提升可信度，但会显著增加 proxy wallet、合约事件归因、成交价格还原和结算 PnL 复杂度。

## 数据库设计

新增迁移建议从 `0049_smart_money_intelligence.sql` 开始。字段使用 `TEXT` + `CHECK` 表达业务状态，避免过早创建数据库 enum；金额和价格使用 `NUMERIC`；事件时间使用 `TIMESTAMPTZ`；可变 provider 输出和 raw payload 使用 `JSONB`。

### `smart_money_config`

Key-value 配置表，延续 rewards/copytrade 配置风格。

```sql
CREATE TABLE smart_money_config (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
```

关键配置：

- `enabled`
- `mode`：`observe | paper | approval | live_guarded`
- `discovery_enabled`
- `wallet_advisory_enabled`
- `signal_advisory_enabled`
- `signal_advisory_provider`
- `signal_advisory_request_format`
- `signal_advisory_model`
- `min_trade_count`
- `min_settled_trade_count`
- `min_total_volume_usd`
- `min_copyability_score`
- `max_signal_age_ms`
- `max_price_slippage_cents`
- `min_orderbook_depth_usd`
- `max_wallet_exposure_usd`
- `max_market_exposure_usd`
- `max_daily_notional_usd`

### `smart_wallet_candidates`

自动发现的钱包候选池。

```sql
CREATE TABLE smart_wallet_candidates (
    id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    wallet_address TEXT NOT NULL,
    source TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'candidate'
        CHECK (status IN ('candidate', 'watch', 'tracked', 'blocked', 'rejected')),
    first_seen_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_seen_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_analyzed_at TIMESTAMPTZ,
    promoted_at TIMESTAMPTZ,
    rejected_at TIMESTAMPTZ,
    reason TEXT,
    raw JSONB NOT NULL DEFAULT '{}',
    UNIQUE (wallet_address, source)
);

CREATE INDEX smart_wallet_candidates_status_seen_idx
    ON smart_wallet_candidates (status, last_seen_at DESC);
CREATE INDEX smart_wallet_candidates_wallet_idx
    ON smart_wallet_candidates (wallet_address);
```

### `smart_wallet_profiles`

钱包滚动画像，作为评分输入。该表会被 profiler 周期更新，避免把高频交易明细查询压到 API。

```sql
CREATE TABLE smart_wallet_profiles (
    wallet_address TEXT PRIMARY KEY,
    trade_count BIGINT NOT NULL DEFAULT 0,
    settled_trade_count BIGINT NOT NULL DEFAULT 0,
    total_volume_usd NUMERIC NOT NULL DEFAULT 0,
    realized_pnl_usd NUMERIC NOT NULL DEFAULT 0,
    roi NUMERIC NOT NULL DEFAULT 0,
    win_rate NUMERIC NOT NULL DEFAULT 0,
    max_drawdown_usd NUMERIC NOT NULL DEFAULT 0,
    avg_trade_usd NUMERIC NOT NULL DEFAULT 0,
    median_trade_usd NUMERIC NOT NULL DEFAULT 0,
    avg_hold_secs BIGINT,
    active_days BIGINT NOT NULL DEFAULT 0,
    markets_traded BIGINT NOT NULL DEFAULT 0,
    category_concentration_score NUMERIC NOT NULL DEFAULT 0,
    market_concentration_score NUMERIC NOT NULL DEFAULT 0,
    low_liquidity_trade_ratio NUMERIC NOT NULL DEFAULT 0,
    stale_copy_window_ratio NUMERIC NOT NULL DEFAULT 0,
    last_trade_at TIMESTAMPTZ,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX smart_wallet_profiles_updated_idx
    ON smart_wallet_profiles (updated_at DESC);
```

### `smart_wallet_scores`

确定性评分结果。总分不是简单盈利榜，重点是可跟性。

```sql
CREATE TABLE smart_wallet_scores (
    wallet_address TEXT PRIMARY KEY,
    total_score NUMERIC NOT NULL,
    profit_score NUMERIC NOT NULL,
    consistency_score NUMERIC NOT NULL,
    risk_score NUMERIC NOT NULL,
    liquidity_score NUMERIC NOT NULL,
    recency_score NUMERIC NOT NULL,
    copyability_score NUMERIC NOT NULL,
    tier TEXT NOT NULL CHECK (tier IN ('blocked', 'candidate', 'watch', 'approved')),
    explanation JSONB NOT NULL DEFAULT '{}',
    scoring_version TEXT NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX smart_wallet_scores_tier_score_idx
    ON smart_wallet_scores (tier, total_score DESC);
```

### `smart_wallet_trades`

标准化源钱包交易。第一阶段由 Data API 生成，第二阶段可加入 chain source 校验字段。

```sql
CREATE TABLE smart_wallet_trades (
    id TEXT PRIMARY KEY,
    wallet_address TEXT NOT NULL,
    source TEXT NOT NULL,
    condition_id TEXT NOT NULL,
    token_id TEXT,
    side TEXT NOT NULL CHECK (side IN ('buy', 'sell')),
    outcome TEXT,
    price NUMERIC NOT NULL CHECK (price >= 0 AND price <= 1),
    size NUMERIC NOT NULL CHECK (size >= 0),
    notional_usd NUMERIC NOT NULL CHECK (notional_usd >= 0),
    tx_hash TEXT,
    source_timestamp TIMESTAMPTZ NOT NULL,
    discovered_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    raw JSONB NOT NULL DEFAULT '{}'
);

CREATE INDEX smart_wallet_trades_wallet_time_idx
    ON smart_wallet_trades (wallet_address, source_timestamp DESC);
CREATE INDEX smart_wallet_trades_condition_time_idx
    ON smart_wallet_trades (condition_id, source_timestamp DESC);
CREATE INDEX smart_wallet_trades_discovered_idx
    ON smart_wallet_trades (discovered_at DESC);
```

### `smart_signals`

源交易转化出的跟随信号。信号不等于订单，必须再经过 decision。

```sql
CREATE TABLE smart_signals (
    id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    source_trade_id TEXT NOT NULL REFERENCES smart_wallet_trades(id),
    wallet_address TEXT NOT NULL,
    condition_id TEXT NOT NULL,
    token_id TEXT,
    side TEXT NOT NULL CHECK (side IN ('buy', 'sell')),
    source_price NUMERIC NOT NULL CHECK (source_price >= 0 AND source_price <= 1),
    current_price NUMERIC CHECK (current_price >= 0 AND current_price <= 1),
    price_slippage_cents NUMERIC,
    latency_ms BIGINT,
    source_notional_usd NUMERIC NOT NULL DEFAULT 0,
    consensus_wallet_count BIGINT NOT NULL DEFAULT 1,
    score NUMERIC NOT NULL DEFAULT 0,
    status TEXT NOT NULL DEFAULT 'new'
        CHECK (status IN ('new', 'rejected', 'observe', 'paper', 'approval_required', 'live_ready', 'executed', 'expired')),
    reason TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX smart_signals_status_created_idx
    ON smart_signals (status, created_at DESC);
CREATE INDEX smart_signals_condition_created_idx
    ON smart_signals (condition_id, created_at DESC);
CREATE INDEX smart_signals_wallet_created_idx
    ON smart_signals (wallet_address, created_at DESC);
```

### `smart_signal_decisions`

每次 deterministic gate 或 LLM advisory 后的决策记录。一个信号可以有多条 decision，用于审计和回放。

```sql
CREATE TABLE smart_signal_decisions (
    id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    signal_id BIGINT NOT NULL REFERENCES smart_signals(id),
    decision TEXT NOT NULL CHECK (decision IN ('allow', 'observe', 'reject')),
    stage TEXT NOT NULL,
    mode TEXT NOT NULL CHECK (mode IN ('observe', 'paper', 'approval', 'live_guarded')),
    rejection_reason TEXT,
    risk_checks JSONB NOT NULL DEFAULT '{}',
    decided_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX smart_signal_decisions_signal_idx
    ON smart_signal_decisions (signal_id, decided_at DESC);
CREATE INDEX smart_signal_decisions_decision_idx
    ON smart_signal_decisions (decision, decided_at DESC);
```

### `smart_wallet_advisories`

LLM 对钱包的低频 advisory。输入必须是结构化画像，不允许模型自行抓外部数据。

```sql
CREATE TABLE smart_wallet_advisories (
    id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    wallet_address TEXT NOT NULL,
    provider TEXT NOT NULL,
    request_format TEXT NOT NULL,
    model TEXT NOT NULL,
    input_hash TEXT NOT NULL,
    recommendation TEXT NOT NULL CHECK (recommendation IN ('allow', 'observe', 'reject')),
    confidence NUMERIC NOT NULL CHECK (confidence >= 0 AND confidence <= 1),
    risk_tags JSONB NOT NULL DEFAULT '[]',
    summary TEXT NOT NULL DEFAULT '',
    reasons JSONB NOT NULL DEFAULT '[]',
    raw_output JSONB NOT NULL DEFAULT '{}',
    expires_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (wallet_address, provider, request_format, model, input_hash)
);

CREATE INDEX smart_wallet_advisories_lookup_idx
    ON smart_wallet_advisories (wallet_address, expires_at DESC);
```

### `smart_signal_advisories`

LLM 对高价值信号的准实时 advisory，主要判断信息风险、是否错过窗口、源交易是否可能不可复制。

```sql
CREATE TABLE smart_signal_advisories (
    id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    signal_id BIGINT NOT NULL REFERENCES smart_signals(id),
    provider TEXT NOT NULL,
    request_format TEXT NOT NULL,
    model TEXT NOT NULL,
    input_hash TEXT NOT NULL,
    recommendation TEXT NOT NULL CHECK (recommendation IN ('allow', 'observe', 'reject')),
    confidence NUMERIC NOT NULL CHECK (confidence >= 0 AND confidence <= 1),
    risk_tags JSONB NOT NULL DEFAULT '[]',
    summary TEXT NOT NULL DEFAULT '',
    reasons JSONB NOT NULL DEFAULT '[]',
    raw_output JSONB NOT NULL DEFAULT '{}',
    expires_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (signal_id, provider, request_format, model, input_hash)
);

CREATE INDEX smart_signal_advisories_signal_idx
    ON smart_signal_advisories (signal_id, expires_at DESC);
```

### `smart_paper_executions`

纸面跟随执行结果，用于验证策略质量。

```sql
CREATE TABLE smart_paper_executions (
    id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    signal_id BIGINT NOT NULL REFERENCES smart_signals(id),
    account_id TEXT NOT NULL,
    side TEXT NOT NULL CHECK (side IN ('buy', 'sell')),
    token_id TEXT,
    planned_price NUMERIC NOT NULL,
    filled_price NUMERIC,
    size NUMERIC NOT NULL,
    notional_usd NUMERIC NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('planned', 'filled', 'expired', 'closed')),
    realized_pnl_usd NUMERIC NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX smart_paper_executions_signal_idx
    ON smart_paper_executions (signal_id);
CREATE INDEX smart_paper_executions_account_created_idx
    ON smart_paper_executions (account_id, created_at DESC);
```

## 评分模型

总分建议：

```text
total_score =
  0.25 * profit_score
+ 0.20 * consistency_score
+ 0.20 * copyability_score
+ 0.15 * risk_score
+ 0.10 * liquidity_score
+ 0.10 * recency_score
```

评分说明：

- `profit_score`：已结算 ROI、realized PnL、正收益样本占比。
- `consistency_score`：盈利是否来自多个市场/多段时间，而非单笔极端收益。
- `copyability_score`：源成交后是否仍有可跟价格窗口，跟随滑点是否可控。
- `risk_score`：最大回撤、集中度、追高频率、低流动性交易占比。
- `liquidity_score`：交易市场是否具备足够盘口深度和退出能力。
- `recency_score`：最近 7/30/90 天是否仍活跃且表现未明显衰减。

强制 fail 条件：

- 成交样本小于配置下限。
- 已结算样本不足。
- 主要收益来自单个市场或单笔极端交易。
- 高频买入低流动性市场且跟随窗口长期不可用。
- 钱包已在 blocked 列表。

## 信号准入

`smart_signal_detector` 从 `smart_wallet_trades` 生成信号，但默认只进入 `observe` 或 `paper`。进入 `live_ready` 必须满足：

- 钱包 tier 为 `approved`。
- `copyability_score` 高于阈值。
- 源交易金额高于最小 notional。
- 信号年龄未超过 `max_signal_age_ms`。
- 当前价格相对源价格劣化不超过 `max_price_slippage_cents`。
- orderbook 服务返回的盘口未 stale，深度大于 `min_orderbook_depth_usd`。
- 同 wallet、condition、token 的日内敞口未超过上限。
- LLM signal advisory 未返回 reject。
- 全局 kill switch 未触发。

## LLM 设计

### 角色

LLM 不负责计算分数或下单。LLM 负责：

- 钱包风格解释。
- 异常盈利归因。
- 市场语义风险与信息不对称风险。
- 多个钱包同向交易的可复制性判断。
- 给候选钱包和信号输出 `allow / observe / reject` advisory。

### Wallet Advisory 输入

输入只包含本地计算出的结构化摘要：

- profile 指标。
- score 维度。
- 最近交易摘要。
- 类别/市场集中度。
- 可跟窗口统计。
- 大额异常交易列表。

输出：

```json
{
  "recommendation": "observe",
  "confidence": 0.72,
  "risk_tags": ["thin_liquidity", "single_market_concentration"],
  "summary": "Wallet is profitable but gains are concentrated in a few early entries.",
  "reasons": [
    "high realized ROI",
    "copy windows often deteriorate after source trade",
    "recent trade count is below approval threshold"
  ]
}
```

### Signal Advisory 输入

输入只包含单个信号和当前本地上下文：

- 源交易价格、规模、时间。
- 当前盘口、滑点和深度。
- 市场问题、分类、结束时间、状态。
- 同向钱包共识。
- 钱包 tier 和关键评分。

输出与 Wallet Advisory 相同。`reject` 只会让信号 fail closed；`allow` 也不能绕过硬规则。

### Provider 复用

实现时优先复用 rewards 已有 OpenAI-compatible / Anthropic connector 设计、`llm_calls` 审计表、fallback provider 和 JSON schema 校验模式。新 connector 应只接收 application 构建的 payload，不直接访问 Polymarket 或链上数据。

## Application 设计

新增或重构为：

- `packages/backend/crates/application/src/smart_money.rs`
- `smart_money/models.rs`
- `smart_money/service.rs`
- `smart_money/scoring.rs`
- `smart_money/signal.rs`（当前已实现确定性 source trade → observe/rejected signal gate + deterministic decision）
- `smart_money/advisory_payload.rs`（当前已实现 signal advisory provider payload + 稳定 input_hash；不调用 provider）
- `smart_money/advisory_models.rs`
- `smart_money/paper.rs`

`SmartMoneyStore` 端口：

- Config：`load_config`、`save_config`
- Candidates：upsert/list/update status
- Profiles：upsert/get/list stale profiles
- Scores：upsert/list leaderboard
- Trades：insert dedup/list recent/source lookup
- Signals/Decisions：create/list/update status，deterministic decision 写入
- Decisions：append/list
- Advisories：signal advisory `latest/save/list` 已接入；wallet advisory 与 provider refresh 待实现
- Paper executions：create/update/list performance
- Control commands：enqueue/claim/complete/fail

`SmartMoneyService`：

- 读写配置。
- 候选钱包晋级/拉黑。（已接入 foundation API）
- 构建 snapshot。
- 计算 deterministic scores。
- 生成信号和 gate 决策。
- 构造 signal advisory request payload 和 input_hash。
- 聚合纸面表现。

## Worker 设计

### `smart_wallet_discovery`

周期发现候选钱包：

1. 从 Data API leaderboard/recent trades 拉取地址。
2. 标准化 EVM address。
3. 写入 `smart_wallet_candidates`。
4. 对 blocked 钱包跳过。
5. 输出发现数量、去重数量、失败数量。

当前实现状态：worker 已接入 Data API leaderboard 种子发现，单轮读取总体榜单最多 50 条，过滤正 PnL 且成交量不低于 `min_total_volume_usd` 的钱包，写入 `source=polymarket_leaderboard` 候选；任一来源已标记 `blocked` 或 `rejected` 的钱包会被 seed 和扫描跳过；recent trades discovery 和独立发现 report 仍待补齐。

### `smart_wallet_profiler`

周期处理 stale candidates/tracked wallets：

1. 读取 activity、positions、closed positions、trades、portfolio value。
2. 标准化并写入 `smart_wallet_trades`。
3. 计算 `smart_wallet_profiles`。
4. 出错时保留旧 profile，不写空快照覆盖。

### `smart_wallet_scorer`

读取 profile，计算 score 和 tier：

1. 应用硬性样本过滤。
2. 计算六个维度分。
3. 写入 `smart_wallet_scores`。
4. 对达到阈值的钱包标为 `watch`，不自动进入 live tracked。

### `smart_signal_detector`

增量读取 tracked/approved 钱包的新交易：

1. 按 deterministic trade id 去重。
2. 从 orderbook 服务读取当前盘口。
3. 计算信号延迟、滑点、深度和共识。
4. 写入 `smart_signals`。
5. 追加 deterministic decision。

### `smart_wallet_advisory_refresh`

对高分候选钱包补 LLM advisory：

1. 只处理 deterministic 评分通过的钱包。
2. 构造稳定 input hash。
3. 命中未过期缓存则跳过。
4. 调 provider，写 `smart_wallet_advisories` 和 `llm_calls`。
5. provider 失败只记录 warning，不能把钱包自动晋级。

### `smart_signal_advisory_refresh`

对高价值信号补 LLM advisory：

1. 只处理硬规则未 reject 的新信号。
2. 构造包含当前盘口和市场语义的 payload。
3. provider 返回 reject 时更新信号为 `rejected`。
4. provider 返回 allow/observe 时仍交给 hard gate 决定最终模式。

### `smart_signal_simulator`

纸面跟随：

1. 只处理 `paper` 信号。
2. 按当时盘口和配置滑点模拟成交。
3. 周期读取当前 price/closed result 估算 PnL。
4. 聚合 wallet、category、market 和整体表现。

### `smart_guarded_executor`

第四阶段才实现：

1. 只处理 `live_ready` 且配置 `mode=live_guarded` 的信号。
2. 持久化 intent。
3. POST 前使用 orderbook 服务做 1 秒 max-age last-look。
4. 使用 live connector 小额提交。
5. 后续对账、撤单、退出规则必须独立于 signal detector。

## API 设计

新增路由前缀建议：`/api/v1/smart-money`。

读取接口：

- `GET /api/v1/smart-money`：snapshot。
- `GET /api/v1/smart-money/candidates`：候选钱包榜。
- `GET /api/v1/smart-money/wallets/{address}`：钱包详情。
- `GET /api/v1/smart-money/signals`：信号流。
- `GET /api/v1/smart-money/paper-performance`：纸面表现。

写接口：

- `POST /api/v1/smart-money/config`：保存配置。
- `POST /api/v1/smart-money/wallets/promote`：候选晋级 tracked/watch。
- `POST /api/v1/smart-money/wallets/block`：拉黑钱包。
- `POST /api/v1/smart-money/run-discovery`：入队发现命令。
- `POST /api/v1/smart-money/analyze-wallets`：入队分析命令。

API handler 只调用 `SmartMoneyService` 和控制命令队列，不直接访问 Polymarket Data API、CLOB、Gamma、orderbook 外部源或 LLM provider。

## Frontend 设计

`/copy-trading` 已接入第一版 Smart Money 配置和候选池面板；后续可以继续把该页面重构为完整 Smart Money 工作台，或新增 `/smart-money` 后逐步迁移导航。

建议四个 tab：

- **配置与候选钱包**：基础配置、自动发现候选、评分、来源、风险标签、晋级/拉黑操作。当前已在 `/copy-trading` 落地配置保存、基础候选表、profile/score 摘要和 watch/tracked/blocked/rejected 状态操作。
- **已跟踪钱包**：tracked/watch/blocked 钱包管理、profile 和 score 摘要。
- **信号流**：源交易、当前价格、滑点、延迟、共识、拒绝原因、LLM advisory。当前已在 `/copy-trading` 落地基础信号表，展示 deterministic observe/rejected 信号；LLM advisory 和可执行跟随状态仍未接入。
- **纸面表现**：整体收益、回撤、命中率、按钱包/分类/市场拆分表现。

UI 文案必须明确：

- observe/paper 不是实盘。
- LLM advisory 是风险提示，不是交易保证。
- live guarded 必须显式开启。

## 实施阶段

### Phase 0：准备和兼容

- 新增 `smart-money` 计划文档和模块文档。
- 保留现有 copytrade API/页面作为只读兼容。
- 定义新 DTO、Store trait 和 migration 草案。
- 明确 copytrade 旧 run/cancel/reset 仍为 no-op，避免产品文案冲突。

验收：

- 文档与当前状态不冲突。
- 新 schema 通过 sqlx migrate 本地验证。

### Phase 1：候选发现和画像

- 新增 migration：config、candidates、profiles、scores、trades。
- 新增 `SmartMoneyService` 和 Postgres/in-memory store。
- 新增 discovery/profiler/scorer worker。
- 新增 candidates/wallet detail API。
- 前端展示候选榜和钱包画像。

验收：

- 系统能自动填充候选钱包。
- 每个候选钱包有 profile 和 deterministic score。
- 失败的外部 API 不会用空数据覆盖已有 profile。

### Phase 2：信号流和 LLM advisory

- 新增 signals、decisions、wallet advisories、signal advisories。当前 signals 表、deterministic signal 写入、`deterministic_gate` decision 写入、signal advisory cache 读写、signal advisory request payload/input_hash builder、独立 Smart Money provider 配置和 worker provider refresh 已接入；wallet advisory 仍待补齐。
- 新增 signal detector worker。当前已接入 source trades + orderbook cache 的 deterministic signal detector、decision 审计和 observe 信号 advisory provider refresh。
- 新增 wallet/signal advisory connector 或复用 reward AI connector 模式。当前已新增 signal advisory connector，支持 OpenAI Responses、OpenAI-compatible Chat Completions 和 Anthropic Messages；wallet advisory connector 待实现。
- LLM 调用写入 `llm_calls`。当前 signal advisory provider 调用写入 `task_type=smart_signal_advisory`。
- 前端展示信号流、拒绝原因和 advisory。当前已展示信号流、拒绝原因和最近 signal advisory；wallet advisory 与纸面表现待实现。

验收：

- 高分钱包新交易能生成信号。
- 每个信号都有 deterministic decision。
- LLM reject 会 fail closed，但 LLM allow 不能绕过硬规则。

### Phase 3：纸面模拟

- 新增 paper executions。
- 新增 paper simulator worker。
- 前端展示纸面表现。
- 加入按钱包、分类、市场和时间窗口的绩效拆分。

验收：

- 至少运行 2 到 4 周 observe/paper。
- 有可复盘的收益、回撤、胜率和滑点统计。
- 能证明跟随窗口仍有效，否则不进入 Phase 4。

### Phase 4：小额 guarded live

- 新增 live intent/order 表或复用未来统一 execution pipeline。
- 实现 `smart_guarded_executor`。
- 接入 live connector、orderbook last-look、账户敞口限制和 kill switch。
- 默认关闭，必须手工开启。

验收：

- 单笔、小额、严格限速。
- 每个 live order 都能追溯到 signal、decision、advisory 和盘口 last-look。
- 任何 provider/cache/orderbook 不确定性都 fail closed。

## 配置建议

默认值应保守：

```text
enabled = false
mode = observe
discovery_enabled = true
wallet_advisory_enabled = false
signal_advisory_enabled = false
signal_advisory_provider = openai
signal_advisory_request_format = openai_responses
signal_advisory_model = gpt-4.1-mini
min_trade_count = 50
min_settled_trade_count = 20
min_total_volume_usd = 10000
min_copyability_score = 0.70
max_signal_age_ms = 60000
max_price_slippage_cents = 2
min_orderbook_depth_usd = 50
max_wallet_exposure_usd = 20
max_market_exposure_usd = 50
max_daily_notional_usd = 100
```

LLM 默认关闭，等 deterministic pipeline 和 paper 结果稳定后再开启。

## 测试计划

后端：

- scoring 单元测试：极端盈利、低样本、高集中度、低流动性、近期衰减。
- signal gate 单元测试：过期信号、滑点超限、盘口缺失、深度不足、LLM reject。
- store 集成测试：upsert 去重、分页、状态迁移、advisory cache 命中。
- worker 测试：外部 API 失败不覆盖旧数据；重复交易不重复生成信号。

前端：

- 候选榜分页/筛选。
- 钱包详情空状态和错误态。
- 信号流拒绝原因和 signal advisory 展示。
- 纸面表现无数据/有数据状态。

运维：

- 增加 database-maintenance retention，避免 trades/signals/advisories 无限增长。
- 增加 worker report 和日志字段。
- 监控 LLM 调用量、失败率和缓存命中率。

## 风险与对策

| 风险 | 对策 |
|---|---|
| 钱包历史收益不可复制 | copyability_score、paper 阶段、信号滑点和延迟统计 |
| Data API 限流或结构漂移 | worker 限速、错误 preview、保留旧 profile、不空写覆盖 |
| LLM 幻觉或格式漂移 | JSON schema、temperature=0、解析失败 fail closed、只做 advisory |
| 低流动性接盘 | min depth、max slippage、low liquidity ratio、observe-only 默认 |
| 单个钱包或市场过度集中 | wallet/market/category exposure cap |
| 实盘误触发 | live guarded 默认关闭、last-look、kill switch、持久化 intent 和审计 |

## 开发检查清单

- [ ] 新增模块文档并更新 `doc/modules/README.md`。
- [ ] 新增 migration 并更新 `doc/modules/infra/database.md`。
- [ ] 新增 application module、Store trait 和 tests。
- [ ] 新增 infrastructure Postgres/in-memory store。
- [ ] 新增 contracts DTO 和前端 TS DTO。
- [ ] 新增 API handler 和路由。
- [x] 新增 worker task、CLI 子命令和 runtime wiring（Phase 1：leaderboard/copytrade seed + 候选画像扫描）。
- [x] 新增 deterministic signal detector 基础（source trades + orderbook cache → observe/rejected signals + deterministic_gate decisions；不调用 LLM、不执行 paper/live）。
- [x] 新增 signal advisory cache 基础（application/store/Postgres/in-memory/snapshot DTO；不调用 LLM、不影响执行决策）。
- [x] 新增 signal advisory request payload/input_hash builder（结构化 signal/source/profile/score/config payload；不调用 LLM）。
- [x] 新增 Smart Money signal advisory connector（三态 `allow|observe|reject`，OpenAI Responses/OpenAI-compatible Chat/Anthropic，含 GLM Chat Completions 请求测试）。
- [x] 新增 worker signal advisory provider refresh（近期 observe 信号 + 已入库上下文 → advisory request/input_hash/cache lookup；使用 Smart Money 独立 provider 配置和 env-only key/base URL，provider key 存在时调用 provider、保存 advisory、记录 `llm_calls`；无 key 时只统计待请求）。
- [x] 新增 Smart Money signal advisory 独立 provider 配置（`signal_advisory_provider` / `signal_advisory_request_format` / `signal_advisory_model` 保存到 smart_money_config，key/base URL/timeout 只从 `POLYEDGE_SMART_MONEY__SIGNAL_ADVISORY_*` 读取）。
- [x] 新增 frontend smart money foundation 入口（`/copy-trading` 内配置保存、基础候选表、状态操作、基础信号流和 signal advisory 展示；完整钱包详情、wallet advisory 和纸面表现未实现）。
- [x] 更新根 `AGENTS.md` 当前状态、关键文件、worker 子命令和数据架构说明。
- [x] 运行后端 `cargo check --workspace --tests`、Smart Money/connector/settings 相关测试，以及前端 `tsc --noEmit`。
