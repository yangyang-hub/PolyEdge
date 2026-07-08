# Rewards 做市商生产前重构设计

最后更新：2026-07-08

状态：阶段 1 已落地，阶段 2 已收尾，阶段 3 第一层已落地。当前已实现 shadow strategy run ledger、quote plan 常用筛选列、只读 ledger API、`/rewards` Runs tab、ledger retention、application `RewardDecisionEngine` 对 pre-provider、post-provider 和最终 snapshot 计划变换的集中封装、独立 input builder（`RewardBotService::build_strategy_input`，单一读路径 + 单一注入 `now`）与可序列化 `RewardStrategyInput` tick 输入快照（`RewardLiveCycle::from_strategy_input` 桥接，engine 行为不变；provider cache 留待 Phase 4 v2），以及 application `RewardActionPlanner` 执行前 planned action proposal 写入。完整 `RewardLiveExecutor` 抽离、replay CLI 和完整 decision analytics 仍未实现。当前已实现状态以 `AGENTS.md` 和 `doc/modules/*` 为准。

## 背景

当前 Rewards market maker 已经具备完整 live 闭环：市场数据由 `polyedge-orderbook` 同步，策略只读 Postgres 和 orderbook cache，worker 执行 fair-value gate、opportunity scoring、AI/info-risk、事件窗口、下单前 last-look、成交后退出和 BalancedMerge，并把 full tick 的 run、decision、action 和订单状态变迁写入 shadow ledger。

问题不在于缺少单个风控条件，而在于生产前还需要持续建设三类能力：

- 决策可追责：阶段 1 已把 run、decision、action 和订单状态变迁串起来；阶段 3 第一层已在执行前写 planned action proposal。后续还需要把 worker side effect 执行完全改为 durable action executor 控制，而不是主要依赖现有 tick 流程。
- 回放可校准：selection score、fair-value 和 opportunity 权重是合理启发式；阶段 1 记录了 run 版本和决策快照，阶段 2 已集中主要计划变换入口，后续还需要稳定的 replay 输入模型与回放工具。
- 结构可维护：live tick 同时承担 snapshot 构建、外部同步、撤单、pending 提交、新挂单和持久化，安全但调试成本高；阶段 2 已把确定性 plan transform 收敛到 application `RewardDecisionEngine`，后续还需要副作用执行层。

## 目标

- 保持现有数据获取边界：API handler、前端和策略计算不直接访问 Polymarket 外部 API。
- 建立策略运行账本，把每轮 tick 的输入版本、配置、决策、动作和结果串起来。
- 把策略计算拆成纯计算层和副作用执行层，先做到行为等价，再做策略优化。
- 清理生产基线 schema，减少历史增量残留和兼容修复 SQL。
- 为回放、参数校准、小额实盘演练和运维排障提供稳定数据模型。

## 非目标

- 不在第一阶段改变做市策略的核心风控阈值和默认行为。
- 不把 LLM/provider 输出改成直接下单权限来源。
- 不把 orderbook cache 持久化成主盘口存储；orderbook service 仍是盘口缓存 owner。
- 不引入旧历史钱包、独立研究或已删除前端模块。

## 保持不变的边界

| 边界 | 设计要求 |
|---|---|
| 市场目录 | `polyedge-orderbook` 继续同步 Gamma markets 和 reward markets 到 Postgres |
| 盘口 | `polyedge-orderbook` 继续维护 WS/poll cache，worker/API 通过 orderbook service 读取 |
| 策略输入 | worker 只能读 Postgres、orderbook service、本地内存盘口历史和 provider cache |
| 外部私有账户 | 只在 worker/live connector 路径访问，API handler 不持有私钥 |
| 前端 mutation | 仍通过 API/Server Actions 写控制命令或配置，不直接执行外部订单 |

## 目标架构

```text
Market/orderbook/account producers
    -> Postgres + orderbook service cache
    -> RewardStrategyInputBuilder
    -> RewardDecisionEngine          (pure, deterministic)
    -> reward_strategy_runs          (run header + config snapshot)
    -> reward_strategy_decisions     (per condition/profile decision)
    -> RewardActionPlanner           (pure action proposals)
    -> reward_strategy_actions       (durable action ledger)
    -> RewardLiveExecutor            (CLOB cancel/submit/merge side effects)
    -> managed orders / fills / positions / events / action results
    -> Rewards API snapshot + run detail UI
```

### 后端模块拆分

建议在 `packages/backend/crates/application/src/rewards/` 下形成以下边界：

| 模块 | 职责 |
|---|---|
| `strategy_input.rs` | 定义一次 tick 的只读输入快照：config、candidate markets、books、book history、account、orders、positions、provider cache、event windows |
| `engine.rs` | 已存在：`RewardDecisionEngine` 纯函数入口，输入 `RewardStrategyInput`，输出 `RewardDecisionSet`，不访问 store、connector 或隐式 clock |
| `decision_models.rs` | run、decision、blocker、action proposal、metrics 等 typed model |
| `action_planner.rs` | 已存在第一层：把 worker 已确定的订单/merge intent 副作用候选转为 planned action proposals；后续继续上移 action selection |
| `run_ledger.rs` | application store trait：记录 run、decisions、actions、action result |
| `replay.rs` | 离线回放入口，复用 decision engine，不调用 live connector |

Worker 侧建议拆成：

| 模块 | 职责 |
|---|---|
| `worker/rewards/input.rs` | 装配 strategy input，集中读取 store/orderbook/local cache |
| `worker/rewards/ledger.rs` | 创建 run、写 decisions/actions/results |
| `worker/rewards/executor.rs` | 执行 cancel/submit/merge action，保证 idempotency 和未知状态保护 |
| `worker/rewards/reconcile.rs` | 外部订单、fills、positions、account 对账 |

第一步不需要移动所有代码，只需要先让现有 live tick 通过新 ledger 写出 run/decision/action 影子数据。

## 数据库设计

当前项目尚未生产部署，建议继续保持单 baseline 迁移，但把 schema 整理成“最终形态”，不要在 baseline 内保留历史 `ALTER TABLE` / `DROP CONSTRAINT` / 旧迁移注释。

### 当前快照表调整

`reward_quote_plans` 继续表示当前可见 quote plan snapshot，但增加运行版本和常用筛选列：

```sql
CREATE TABLE reward_quote_plans (
  condition_id TEXT NOT NULL REFERENCES reward_markets(condition_id) ON DELETE CASCADE,
  strategy_profile TEXT NOT NULL CHECK (strategy_profile IN ('standard', 'balanced_merge')),
  latest_run_id BIGINT,
  score NUMERIC(10, 4) NOT NULL CHECK (score >= 0),
  selection_score NUMERIC(10, 4) NOT NULL DEFAULT 0 CHECK (selection_score >= 0),
  eligible BOOLEAN NOT NULL DEFAULT false,
  quote_readiness TEXT NOT NULL CHECK (
    quote_readiness IN ('ready_to_quote', 'waiting_orderbook', 'provider_pending', 'blocked')
  ),
  quote_mode TEXT NOT NULL,
  reason_code TEXT NOT NULL DEFAULT '',
  reason TEXT NOT NULL,
  blocker_codes TEXT[] NOT NULL DEFAULT '{}',
  fair_value_passed BOOLEAN,
  event_window_status TEXT,
  ai_suitability TEXT,
  info_risk_level TEXT,
  quote_plan_json JSONB NOT NULL CHECK (jsonb_typeof(quote_plan_json) = 'object'),
  updated_at TIMESTAMPTZ NOT NULL,
  PRIMARY KEY (condition_id, strategy_profile)
);

CREATE INDEX reward_quote_plans_ready_selection_idx
  ON reward_quote_plans (eligible, quote_readiness, selection_score DESC, score DESC, updated_at DESC);

CREATE INDEX reward_quote_plans_profile_selection_idx
  ON reward_quote_plans (strategy_profile, eligible, selection_score DESC, updated_at DESC);

CREATE INDEX reward_quote_plans_blocker_codes_gin_idx
  ON reward_quote_plans USING GIN (blocker_codes);
```

说明：

- `quote_plan_json` 保留完整 DTO，便于快速迭代。
- `reason_code` / `blocker_codes` / gate 摘要列用于运维筛选和指标聚合。
- `latest_run_id` 指向最近产生该 plan 的策略运行。可先不加 FK，避免 run 清理影响 current snapshot；需要强一致时再加 FK。

### 策略运行账本

```sql
CREATE TABLE reward_strategy_runs (
  run_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
  account_id TEXT NOT NULL,
  trace_id TEXT NOT NULL,
  trigger_type TEXT NOT NULL CHECK (
    trigger_type IN ('poll', 'run_once', 'orderbook_event', 'control_command', 'replay')
  ),
  status TEXT NOT NULL CHECK (
    status IN ('running', 'completed', 'failed', 'cancelled')
  ),
  config_hash TEXT NOT NULL,
  config_json JSONB NOT NULL CHECK (jsonb_typeof(config_json) = 'object'),
  input_summary_json JSONB NOT NULL DEFAULT '{}'::jsonb CHECK (jsonb_typeof(input_summary_json) = 'object'),
  metrics_json JSONB NOT NULL DEFAULT '{}'::jsonb CHECK (jsonb_typeof(metrics_json) = 'object'),
  started_at TIMESTAMPTZ NOT NULL,
  completed_at TIMESTAMPTZ,
  error_code TEXT,
  error_message TEXT
);

CREATE INDEX reward_strategy_runs_account_started_idx
  ON reward_strategy_runs (account_id, started_at DESC);

CREATE INDEX reward_strategy_runs_status_started_idx
  ON reward_strategy_runs (status, started_at DESC);

CREATE INDEX reward_strategy_runs_trace_idx
  ON reward_strategy_runs (trace_id);
```

`input_summary_json` 只存摘要，不存完整盘口深度，避免表膨胀。建议包含：

- candidate count、book count、open order count、position count。
- orderbook newest/oldest `confirmed_at`。
- provider cache hit/miss/pending counts。
- account available/capital/external buy notional。

### 策略决策表

```sql
CREATE TABLE reward_strategy_decisions (
  run_id BIGINT NOT NULL REFERENCES reward_strategy_runs(run_id) ON DELETE CASCADE,
  condition_id TEXT NOT NULL,
  strategy_profile TEXT NOT NULL CHECK (strategy_profile IN ('standard', 'balanced_merge')),
  decision_rank INTEGER NOT NULL CHECK (decision_rank >= 0),
  eligible BOOLEAN NOT NULL,
  quote_readiness TEXT NOT NULL,
  quote_mode TEXT NOT NULL,
  score NUMERIC(10, 4) NOT NULL CHECK (score >= 0),
  selection_score NUMERIC(10, 4) NOT NULL CHECK (selection_score >= 0),
  reason_code TEXT NOT NULL,
  reason TEXT NOT NULL,
  blocker_codes TEXT[] NOT NULL DEFAULT '{}',
  planned_buy_notional_usd NUMERIC(18, 4) NOT NULL DEFAULT 0 CHECK (planned_buy_notional_usd >= 0),
  fair_value_passed BOOLEAN,
  fair_value_effective_edge_cents NUMERIC(12, 4),
  opportunity_score NUMERIC(10, 4),
  event_window_status TEXT,
  ai_suitability TEXT,
  info_risk_level TEXT,
  decision_json JSONB NOT NULL CHECK (jsonb_typeof(decision_json) = 'object'),
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  PRIMARY KEY (run_id, condition_id, strategy_profile)
);

CREATE INDEX reward_strategy_decisions_condition_created_idx
  ON reward_strategy_decisions (condition_id, created_at DESC);

CREATE INDEX reward_strategy_decisions_run_rank_idx
  ON reward_strategy_decisions (run_id, eligible DESC, selection_score DESC, decision_rank ASC);

CREATE INDEX reward_strategy_decisions_blocker_codes_gin_idx
  ON reward_strategy_decisions USING GIN (blocker_codes);
```

该表是回放和运维分析的核心，不用于交易路径强一致控制。

### 动作账本

```sql
CREATE TABLE reward_strategy_actions (
  action_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
  run_id BIGINT NOT NULL REFERENCES reward_strategy_runs(run_id) ON DELETE CASCADE,
  account_id TEXT NOT NULL,
  condition_id TEXT,
  token_id TEXT,
  managed_order_id TEXT,
  external_order_id TEXT,
  action_type TEXT NOT NULL CHECK (
    action_type IN (
      'place_buy',
      'submit_exit_sell',
      'cancel_order',
      'cancel_replace_exit',
      'record_fill',
      'create_merge_intent',
      'execute_merge',
      'skip'
    )
  ),
  status TEXT NOT NULL CHECK (
    status IN ('planned', 'executing', 'succeeded', 'failed', 'skipped', 'unknown')
  ),
  reason_code TEXT NOT NULL,
  reason TEXT NOT NULL,
  idempotency_key TEXT NOT NULL,
  request_json JSONB NOT NULL DEFAULT '{}'::jsonb CHECK (jsonb_typeof(request_json) = 'object'),
  result_json JSONB NOT NULL DEFAULT '{}'::jsonb CHECK (jsonb_typeof(result_json) = 'object'),
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE UNIQUE INDEX reward_strategy_actions_idempotency_idx
  ON reward_strategy_actions (idempotency_key);

CREATE INDEX reward_strategy_actions_run_idx
  ON reward_strategy_actions (run_id, action_id);

CREATE INDEX reward_strategy_actions_account_status_idx
  ON reward_strategy_actions (account_id, status, updated_at DESC);

CREATE INDEX reward_strategy_actions_order_idx
  ON reward_strategy_actions (managed_order_id, updated_at DESC)
  WHERE managed_order_id IS NOT NULL;
```

动作账本用于回答“系统打算做什么、实际做了什么、失败在哪里”。现有 `reward_managed_orders` 仍是订单状态事实表。

### 订单状态转移

当前订单 reason 会被覆盖，风险事件是补充日志。建议新增追加式状态转移表：

```sql
CREATE TABLE reward_order_transitions (
  transition_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
  run_id BIGINT,
  action_id BIGINT,
  managed_order_id TEXT NOT NULL,
  external_order_id TEXT,
  from_status TEXT,
  to_status TEXT NOT NULL,
  reason_code TEXT NOT NULL,
  reason TEXT NOT NULL,
  metadata_json JSONB NOT NULL DEFAULT '{}'::jsonb CHECK (jsonb_typeof(metadata_json) = 'object'),
  created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX reward_order_transitions_order_created_idx
  ON reward_order_transitions (managed_order_id, created_at DESC);

CREATE INDEX reward_order_transitions_run_idx
  ON reward_order_transitions (run_id, created_at DESC)
  WHERE run_id IS NOT NULL;
```

### FK 策略

- `reward_quote_plans`、`reward_strategy_decisions` 可以关联当前 reward catalog condition。
- `reward_positions` 和外部库存退出类 `reward_managed_orders` 不应强制 FK 到 `reward_markets`，因为账户可能持有当前 rewards catalog 外的 token。
- 对所有 FK 查询路径手动建索引，避免 PostgreSQL 父表删除/更新时锁等待放大。

### 保留与清理

建议默认保留：

| 表 | 默认保留 |
|---|---:|
| `reward_strategy_runs` | 90 天 |
| `reward_strategy_decisions` | 90 天，随 run cascade |
| `reward_strategy_actions` | 90 天，随 run cascade |
| `reward_order_transitions` | 180 天 |
| `reward_quote_plans` | 当前快照，不按时间清理 |

长期订单审计依赖 `reward_order_transitions` 和现有 fills/orders/positions 事实表。`reward_strategy_actions` 主要服务近期排障和 replay 对照，不作为长期账本唯一来源。

## Worker 重构流程

### 阶段 1：影子账本，不改变交易行为（已落地）

目标：现有 live tick 行为不变，只新增 run/decision/action 写入。

```text
run_reward_bot_live_tick()
  -> prepare cycle
  -> create reward_strategy_runs(status=running)
  -> existing scoring / fair-value / provider / readiness
  -> save current reward_quote_plans(latest_run_id)
  -> write reward_strategy_decisions from final plans
  -> persist existing live tick outcome
  -> derive reward_strategy_actions and reward_order_transitions from the same trace
  -> complete/fail reward_strategy_runs(metrics_json)
```

验收：

- quote plan snapshot 暴露 `latest_run_id`，可追溯到生成该 plan 的最近 run。
- 每个 eligible/blocked plan 都有 decision row。
- live tick outcome 会产生 action row 和 order transition row；当前 action 是 outcome 派生的 shadow 记录，不是执行前 durable action 控制。
- 行为测试不需要大改；主要新增 store/API/UI 编译校验。

### 阶段 2：抽纯策略引擎（第一层已落地）

目标：把现有计算函数组合成 `RewardDecisionEngine`，输入输出可序列化。当前已完成 pre-provider、post-provider 和最终 snapshot 三个 plan transform 入口；worker 仍负责装配 `RewardLiveCycle`、读取 provider cache、外部同步和 live 执行。

```rust
pub struct RewardStrategyInput {
    pub config: RewardBotConfig,
    pub candidate_markets: Vec<RewardCandidateMarket>,
    pub books: HashMap<String, RewardOrderBook>,
    pub book_history: HashMap<String, Vec<BookSnapshot>>,
    pub open_orders: Vec<ManagedRewardOrder>,
    pub positions: Vec<RewardPosition>,
    pub account: RewardAccountState,
    pub provider_cache: RewardProviderSnapshot,
    pub event_windows: Vec<RewardMarketEventWindow>,
    pub now: OffsetDateTime,
}

pub struct RewardDecisionSet {
    pub plans: Vec<RewardQuotePlan>,
    pub decisions: Vec<RewardStrategyDecision>,
    pub metrics: RewardStrategyRunMetrics,
}
```

约束：

- engine 不调用 DB、HTTP、connector。
- engine 不创建外部订单 ID。
- engine 可以使用传入的 `now`，避免隐藏 clock 影响回放。

### 阶段 3：动作规划与执行解耦（第一层已落地）

目标：把“想做的动作”和“执行外部副作用”分开。

```text
RewardActionPlanner
  input: decision set + current orders/positions/account
  output: action proposals

RewardLiveExecutor
  input: durable actions
  behavior: submit/cancel/merge with idempotency and unknown-result protection
```

当前第一层已新增 application `RewardActionPlanner`，并在 full tick 的 merge create/execute、cancel/cancel-replace、pending submit 和 placement submit 前写入 `reward_strategy_actions(status=planned)`。这些 action 使用与 outcome 派生 action 兼容的 idempotency key；后续 `apply_tick_outcome` 会更新同一 action 的状态/结果。fast reconcile 没有 strategy run 上下文，仍保持原路径。完整 executor 尚未抽离，live side effects 仍在 worker 原流程中执行。

需要保留现有保护：

- 撤单结果 unknown 时不提交 replacement。
- pending BUY 提交前继续 last-look 和 fair-value gate。
- SELL exit 不因事件窗口阻断。
- 外部对账不可靠时暂停新增 BUY。

### 阶段 4：回放工具

目标：用历史 `reward_strategy_runs` 输入摘要、quote plan history、fair-value history、candles、orderbook sampled snapshots 重跑 decision engine。

第一版可以不做完整盘口深度重建，只做配置/决策回放：

- 读取某天实际 runs。
- 对同一输入摘要和配置跑 engine。
- 比较 eligible count、top selection、blocker distribution、fair-value pass rate。

第二版再引入 orderbook snapshot sampling，用于评估：

- quote fill risk。
- exit cost。
- cancel churn。
- rewards density vs inventory occupation。

## 前端重构设计

`/rewards` 保持为当前控制台。新增或扩展三个视图：

| 视图 | 目的 |
|---|---|
| Run timeline | 最近 runs、状态、耗时、候选数、eligible 数、下单/撤单/失败数、provider miss、book stale |
| Run detail | 某次 run 的配置 hash、input summary、decision table、actions、order transitions；当前前端先展示 summary、decisions 和 actions |
| Decision analytics | blocker codes、fair-value fail reasons、info-risk/event-window/AI 拦截分布、selection score 分布；当前仍未单独实现 |

前端 DTO 不要直接依赖完整 `decision_json`。列表页使用摘要列；详情页再展开 JSON。

## API 设计

新增只读 API：

| 路由 | 用途 |
|---|---|
| `GET /api/v1/rewards-bot/runs` | 分页列出 strategy runs |
| `GET /api/v1/rewards-bot/runs/{run_id}` | run header + metrics + input summary |
| `GET /api/v1/rewards-bot/runs/{run_id}/decisions` | 分页/筛选该 run 的 decisions |
| `GET /api/v1/rewards-bot/runs/{run_id}/actions` | 分页/筛选该 run 的 actions |
| `GET /api/v1/rewards-bot/orders/{managed_order_id}/transitions` | 单订单状态时间线 |

写 API 仍保持现有 config/control commands，不新增直接下单 API。

## 实施顺序

1. 已完成：补上 run ledger 表和 quote plan 常用筛选列，当前仍保留单 baseline 文件。
2. 已完成：新增 application models/store trait 和 infrastructure Postgres/in-memory 实现。
3. 已完成：在现有 live tick 中接入影子 run ledger，保持交易行为不变。
4. 已完成：前端增加 Runs tab，只读展示 run timeline、summary、decisions 和 actions。
5. 已完成：抽出 `RewardDecisionEngine`（pre/post-provider/snapshot 纯决策变换）与可序列化 `RewardStrategyInput` tick 输入快照，新增独立 input builder（`RewardBotService::build_strategy_input`，单一读路径 + 单一注入 `now`）和 `RewardLiveCycle::from_strategy_input` 桥接，engine 行为不变；新增 application engine tests 与 strategy_input tests。注：builder 注入单一 `now` 替代原先 `prepare_live_cycle` 内多次 `now_utc()`，带来 plan `updated_at` 亚毫秒级差异；provider cache 未纳入快照（Phase 4 v2）。
6. 进行中：已新增 `RewardActionPlanner` 并在 full tick 执行前写 planned actions；后续继续把 placement/cancel/pending/merge 改为 durable action executor，逐步删除 worker 巨型流程中的混合职责。
7. 新增 replay CLI，先做决策一致性回放，再做盘口/退出成本回放。
8. 小额 live drill：单账户、低额度、只开 standard profile，记录 run/action/order transition 指标。

## 测试要求

- `cargo test -p polyedge-application rewards`：策略纯计算。
- `cargo test -p polyedge-worker rewards`：live 执行保护、unknown cancel、pending last-look、reselect。
- `cargo test -p polyedge-orderbook`：盘口缓存和 registry。
- `cargo check --workspace --tests`：重构阶段每次必须通过。
- 前端变更后运行 `yarn build`。
- 数据库基线清理后，用空库跑 `sqlx::migrate!` 和 `init.sql` 等价性检查。

新增重点测试：

- run ledger 在成功/失败/提前返回路径都能 close run。
- action idempotency key 重试不会重复提交外部订单。
- decision engine 对同一 input 多次输出一致。
- quote plan current snapshot 与 latest run decision 数量一致。
- 删除旧 run 不影响 current quote plans、open orders、positions。

## 生产前验收标准

- 任意一个 open order 能追溯到：run -> decision -> action -> order transition -> external order/fill。
- 任意一个 blocked plan 能按 reason code 聚合，并能看到对应 fair-value/event/AI/info-risk/opportunity 输入摘要。
- 一天内 runs 的 blocker/action/selection 指标能导出用于参数校准。
- 小额 live drill 中没有重复提交、未知撤单后补单、外部对账不可靠时继续新增 BUY 等高风险行为。
- baseline schema 是干净空库初始化脚本，不包含已删除模块或历史兼容修复语句。

## 主要风险

- 账本写入增加 DB 写压力：需要批量 insert decisions/actions，并避免高频更新宽 JSONB 行。
- run/decision 表增长快：阶段 1 已接入 90 天 run retention 和 180 天 order transition retention；后续若 tick 频率提高，仍需根据实盘写入量调优索引和保留窗口。
- 过早重写 live tick 可能引入交易风险：先影子账本，再抽纯函数，最后再改执行器。
- 前端如果直接展示完整 JSON 会变慢：列表页只用摘要列，详情页懒加载。
