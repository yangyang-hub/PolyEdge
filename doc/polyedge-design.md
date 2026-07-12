# PolyEdge 设计文档

> **状态（2026-07-12）**：本文是系统总体早期设计，事件驱动 research/signals/approval 等目标不等同于当前能力。当前产品焦点已收敛到 Polymarket rewards market maker；市场、事件/新闻与 Funding 作为支撑，旧钱包类和独立研究模块已移除。当前状态以 [../AGENTS.md](../AGENTS.md) 和 [modules/](modules/README.md) 为准。

## 1. 文档目标

本文档用于定义 PolyEdge 的系统目标、架构设计、模块边界、关键数据流、风控约束与 MVP 落地路径，作为后续工程实现和迭代的基础。

PolyEdge 是一个面向 Polymarket 的事件驱动交易系统，核心目标是将外部世界中的增量信息转化为对事件概率的更新，并在市场价格偏离真实概率时执行交易。

一句话概括：

> PolyEdge = 把“世界信息”转化为“概率差”，再转化为“风险可控的交易收益”的系统

### 1.1 配套子文档

1. [polyedge-frontend-design.md](./polyedge-frontend-design.md)
   前端控制台设计，基于 Next.js App Router。
2. [polyedge-backend-design.md](./polyedge-backend-design.md)
   后端系统设计，基于 Rust 异步服务架构。
3. [polyedge-api-contract.md](./polyedge-api-contract.md)
   前后端 API、DTO、错误码和实时推送契约。
4. [polyedge-auth-design.md](./polyedge-auth-design.md)
   认证、会话、权限和高风险操作保护设计。
5. [polyedge-llm-governance.md](./polyedge-llm-governance.md)
   LLM 调用边界、Prompt 版本、结构化输出和降级治理。
6. [polyedge-prototype-design.md](./polyedge-prototype-design.md)
   面向 Figma/线框图的前端原型页面设计说明。
7. [polyedge-frontend-ui-stack.md](./polyedge-frontend-ui-stack.md)
   基于原型确定的前端 UI 框架、依赖和 skill 建议。
8. [polyedge-domain-enums-and-decimals.md](./polyedge-domain-enums-and-decimals.md)
   后端与 API 共用的枚举、定点数精度和序列化规范。
9. [polyedge-storage-schema.md](./polyedge-storage-schema.md)
   PostgreSQL 落库细节，补充审计、幂等、outbox 和 LLM 调用表设计。
10. [polyedge-internal-auth-token-spec.md](./polyedge-internal-auth-token-spec.md)
   Next.js 到 Rust 的内部鉴权 token 格式、TTL、验签和轮换协议。
11. [polyedge-polymarket-connector-design.md](./polyedge-polymarket-connector-design.md)
   Polymarket 的 CLOB WebSocket、下单、Data API、CTF 链上交互和 Safe 代理执行设计。
12. [polyedge-backend-implementation-plan.md](./polyedge-backend-implementation-plan.md)
   后端从零起骨架的实施里程碑、首批 backlog、依赖关系和验收条件。
13. [polyedge-frontend-implementation-plan.md](./polyedge-frontend-implementation-plan.md)
   前端基于现有 Next.js 控制台骨架继续实现时的里程碑、契约收敛、页面优先级和首批 backlog。

---

## 2. 项目目标与边界

### 2.1 核心目标

PolyEdge 专注于以下能力：

1. 快速接收并整理市场与外部信息。
2. 判断信息是否会影响特定预测市场。
3. 将信息转化为概率更新，而不是情绪判断。
4. 识别市场价格与内部估值之间的偏差。
5. 在严格风控约束下执行交易。

### 2.2 非目标

当前阶段不做以下事项：

1. 高频做市。
2. 纯跟单策略。
3. 无约束的自动化交易。
4. 复杂跨交易所资金调度。
5. 追求超大吞吐量的通用事件平台。

### 2.3 核心交易公式

```text
Edge = Fair Probability - Market Implied Probability
```

当 `|Edge|` 足够大，且满足置信度、流动性与风险条件时，系统才生成可执行信号。

---

## 3. 核心原则

1. 不做高频，重点做认知差与定价差。
2. 不依赖单一数据源，避免信息偏差。
3. 不因为“有消息”就交易，只因为“有 edge”才交易。
4. 风控优先于收益，任何阶段都允许放弃交易机会。
5. 先研究、再模拟、后小资金，最后才自动化。
6. 系统输出必须可解释，至少能回答“为什么判断相关”“为什么认为概率变化”“为什么此时执行”。
7. 任何交易判断都必须基于清晰的结算语义，避免“看对新闻、做错题目”。
8. 策略参数、模型版本、Prompt 版本和研究结论必须可追踪，避免无意识策略漂移。

---

## 4. 系统总览

### 4.1 分层模型

```text
数据层 -> 认知层 -> 决策层 -> 执行层 -> 风控层 -> 监控层
```

### 4.2 总体架构

```text
                +----------------------+
                |     外部信息源       |
                | X / 新闻 / 公告 / 宏观 |
                +----------+-----------+
                           |
                           v
                +----------------------+
                |      数据处理层       |
                | 采集 / 清洗 / 去重 / 标准化 |
                +----------+-----------+
                           |
                           v
                +----------------------+
                |      事件识别层       |
                | 规则 + LLM + 实体映射  |
                +----------+-----------+
                           |
                           v
                +----------------------+
                |      概率估计层       |
                | 规则模型 + LLM 辅助    |
                +----------+-----------+
                           |
                           v
                +----------------------+
                |      信号生成层       |
                | edge / 阈值 / 置信度   |
                +----------+-----------+
                           |
                           v
                +----------------------+
                |       执行引擎        |
                | 下单 / 撤单 / 跟踪 / 回报 |
                +----------+-----------+
                           |
                           v
                +----------------------+
                |      风控与监控       |
                | 限额 / kill switch / 告警 |
                +----------------------+
```

### 4.3 关键设计思想

系统以“事件”为驱动，以“市场”为落点，以“概率更新”为核心中间变量。

也就是说，PolyEdge 并不直接把新闻映射成买卖，而是走完以下链路：

```text
原始信息 -> 有效事件 -> 影响市场 -> 概率变化 -> 定价偏差 -> 交易信号 -> 风控审批 -> 执行
```

### 4.4 结算语义优先

对于预测市场，事件理解本身并不足够，系统还必须理解 market 的结算条件、官方判定来源和边界场景。

因此 PolyEdge 在设计上应遵循：

```text
先理解 market 如何结算，再判断信息是否影响结算概率
```

这意味着即使某条新闻在现实世界中很重要，只要它不改变该 market 的结算路径，就不应被视为有效交易信号。

### 4.5 先验-证据-后验框架

PolyEdge 内部建议使用“先验概率 + 证据增量 + 后验概率”的更新框架，而不是每次从零开始估值：

```text
Prior -> Evidence Update -> Posterior -> Edge
```

其中：

1. `Prior` 通常来自市场当前隐含概率和历史状态。
2. `Evidence` 表示对结算路径有明确支持或反驳作用的信息。
3. `Posterior` 是综合证据后得到的内部公平价格。

这样可以让系统更容易解释、回测和校准。

---

## 5. 模块设计

### 5.1 数据层

#### 5.1.1 目标

统一采集 Polymarket 市场数据与外部信息流，形成可被后续模块消费的标准事件流和市场快照。

#### 5.1.2 输入数据

##### 市场侧

1. 市场列表。
2. 市场状态与元数据。
3. Orderbook 深度。
4. 最新成交与成交序列。
5. 持仓、账户与用户画像数据。

##### 外部侧

1. X 白名单账号动态。
2. 新闻 RSS。
3. 官方公告。
4. 宏观事件日历。
5. 辅助价格源与波动率数据。

#### 5.1.3 子模块

1. `market_ingestor`
   负责同步市场列表、价格、盘口、成交。
2. `news_ingestor`
   负责抓取新闻、公告、社媒内容。
3. `normalizer`
   负责统一字段、时间、实体格式。
4. `deduplicator`
   负责内容去重与事件聚合。
5. `stream_router`
   将标准化后的数据投递给下游模块。

#### 5.1.4 关键要求

1. 所有数据必须带 `source`、`ingested_at`、`event_time`、`raw_payload`。
2. 采集失败不能阻塞全局，必须支持单源降级。
3. 外部信息要支持去重，避免同一新闻触发多次交易。
4. 市场数据与外部事件的时钟必须统一为 UTC。

#### 5.1.5 数据质量与 SLA

MVP 阶段就应为关键数据定义最基本的质量标准：

1. 行情快照若超过设定阈值未更新，应标记为 `stale_market_data`。
2. 新闻或公告若采集延迟超过阈值，应降低对应信号权重。
3. 每个数据源都应维护 `health_score`，并在信号 recompute 时作为事件源可靠性的动态降权乘数。
4. 数据源异常时系统应进入降级模式，而不是继续使用陈旧数据强行交易。

建议首版至少定义以下阈值：

1. 市场价格快照最大允许延迟。
2. 外部事件最大允许延迟。
3. Orderbook 最大允许缺口时间。
4. 单源连续失败次数阈值。
5. 触发降级和恢复的条件。

---

### 5.2 事件识别层

#### 5.2.1 目标

判断一条原始信息是否构成“可交易事件”，并识别其与哪些 market 相关、影响方向如何、强度多大。

#### 5.2.2 输出

```json
{
  "event_id": "evt_xxx",
  "relevant": true,
  "related_markets": ["mkt_1", "mkt_2"],
  "impact_direction": "positive",
  "impact_strength": 0.64,
  "confidence": 0.78,
  "reason": "official announcement changes baseline odds"
}
```

#### 5.2.3 识别流程

1. 规则预筛选。
2. 实体识别与标准化。
3. 市场映射。
4. LLM 判断相关性和影响方向。
5. 事件聚合与冲突消解。

#### 5.2.4 规则层职责

适合做高召回、低成本的第一道筛选：

1. 关键词命中，如 `breaking`、`announce`、`ban`、`lawsuit`。
2. 实体命中，如人物、公司、国家、机构、地区。
3. 来源打分，如官方公告权重高于匿名社媒。
4. 时效过滤，过旧信息直接降权。

#### 5.2.5 LLM 层职责

适合做高语义理解：

1. 判断新闻是否真正改变事件基础概率。
2. 区分“噪音信息”和“结构性信息”。
3. 判断影响方向和影响幅度。
4. 给出自然语言解释，便于审计。

#### 5.2.6 事件有效性标准

一条信息被认定为有效事件，至少满足以下条件中的大部分：

1. 来源可信。
2. 与某个 market 的结算条件相关。
3. 对未来结果概率有增量影响。
4. 不是市场已经充分反映的旧信息。
5. 能输出可解释的影响理由。

#### 5.2.7 结算规则层

为避免“语义误判”，事件识别层内部建议加入独立的结算规则解析子层，负责回答以下问题：

1. 该 market 的结算条件是什么。
2. 官方或默认的 resolution source 是什么。
3. 哪些边界条件会导致 market 按 YES 或 NO 结算。
4. 当前外部信息是否真的影响上述结算条件。
5. 该 market 是否存在高歧义或高争议条款。

建议输出结构：

```json
{
  "market_id": "mkt_xxx",
  "resolution_source": "official government release",
  "resolution_criteria": ["must occur before deadline", "source must be official"],
  "ambiguity_level": "medium",
  "tradable": true,
  "review_required": false
}
```

#### 5.2.8 高歧义市场处理

对歧义 market，系统不应默认继续交易，而应引入明确的治理策略：

1. `LOW`：允许自动交易。
2. `MEDIUM`：允许研究和模拟，实盘需人工确认。
3. `HIGH`：默认禁止自动交易，进入灰名单。

典型高歧义特征包括：

1. 题目 wording 含糊。
2. 结算来源不唯一。
3. 时间边界不清晰。
4. 是否发生存在多种解释口径。
5. 官方结算历史上出现争议。

---

### 5.3 概率估计层

#### 5.3.1 目标

对每个候选 market 生成内部估值，即 `fair_price` 或 `fair_probability`。

#### 5.3.2 输出结构

```json
{
  "market_id": "mkt_xxx",
  "fair_price": 0.58,
  "confidence": 0.70,
  "time_horizon": "short",
  "model_version": "v1_rule_llm_hybrid",
  "reason_codes": ["official_source", "material_update", "market_underreaction"]
}
```

#### 5.3.3 估值方法

##### 方法 A：LLM 直接估计

适用于复杂、语义密集、缺少明确结构化因子的事件。

优点：

1. 上手快。
2. 对复杂语境适应性较好。

缺点：

1. 稳定性与一致性较弱。
2. 可回测性较差。
3. 难以做严格的风险归因。

##### 方法 B：规则模型估计

适用于存在可量化驱动因子的市场。

常见输入：

1. 当前市场价格。
2. 盘口深度和冲击成本。
3. 历史波动率。
4. 类似事件统计特征。
5. 距离结算时间。
6. 来源权重与事件强度。

##### 方法 C：混合估值

推荐作为主路径：

```text
fair_price = base_market_price + event_adjustment + model_adjustment
```

其中：

1. `base_market_price` 来自当前市场共识。
2. `event_adjustment` 来自事件识别层的影响估计。
3. `model_adjustment` 用于修正市场过度或不足反应。

#### 5.3.4 置信度定义

置信度不是事件真假概率，而是系统对本次估值可靠性的主观评分，用于决定是否允许下单和允许多大仓位。

建议由以下因素构成：

1. 来源可信度。
2. 信息新鲜度。
3. 市场映射清晰度。
4. 模型一致性。
5. 盘口流动性。
6. 是否接近结算。

#### 5.3.5 证据模型

为了让概率更新过程可解释，建议在事件和估值之间引入 `evidence` 层。

每条证据不是简单的“有新闻”，而是“某个信息对某个 market 的某条结算路径提供了多强的支持或反驳”。

建议字段：

```json
{
  "evidence_id": "evd_xxx",
  "market_id": "mkt_xxx",
  "event_id": "evt_xxx",
  "direction": "supports_yes",
  "strength": 0.35,
  "source_reliability": 0.90,
  "novelty": 0.80,
  "half_life_minutes": 240,
  "contradiction_group": "candidate_health",
  "supports_resolution_path": true
}
```

建议将证据拆成以下维度：

1. `direction`：支持 YES、支持 NO，或仅提供背景。
2. `strength`：该证据理论上的影响强度。
3. `source_reliability`：来源可信度。
4. `novelty`：相对市场当前共识的新信息程度。
5. `half_life`：证据影响随时间衰减的速度。
6. `contradiction_group`：便于管理互相冲突的证据簇。

#### 5.3.6 概率更新机制

系统不建议每次“凭感觉重估 fair_price”，而应使用持续更新机制：

```text
posterior = prior + sum(weighted_evidence_delta) + structural_adjustment
```

其中：

1. `prior` 默认来自市场当前隐含概率。
2. `weighted_evidence_delta` 来自各条证据的加权影响。
3. `structural_adjustment` 用于处理市场反应不足、流动性差或已知行为偏差。

建议的加权逻辑：

```text
evidence_weight =
strength
* source_reliability
* novelty
* resolution_relevance
* freshness_decay
```

在实现上应注意：

1. 冲突证据不一定直接相互抵消，更常见的处理是降低整体置信度。
2. 同一来源的重复表达应合并，而不是重复加权。
3. 结算路径不明确的证据应直接降权或丢弃。
4. 接近结算时可降低自由裁量空间，提高规则权重。

#### 5.3.7 事件衰减与失效机制

不是所有有效事件都应该持续影响估值，因此需要单独定义衰减和失效逻辑。

建议规则：

1. 每条证据都带 `ttl` 或 `half_life`。
2. 若事件被确认、证伪或被新证据覆盖，应主动失效。
3. 若市场已经快速重定价，旧证据的 `novelty` 应下降。
4. 若事件只影响短期情绪而不影响结算路径，应快速衰减。

可参考的处理流程：

```python
if evidence.expired or evidence.invalidated:
    remove_from_posterior()
elif new_conflicting_evidence:
    reduce_confidence()
    recompute_posterior()
```

---

### 5.4 信号生成层

#### 5.4.1 目标

将内部估值与市场价格比较，生成标准化可执行信号。

#### 5.4.2 核心逻辑

```python
edge = fair_price - market_price

if abs(edge) > threshold and confidence >= min_confidence:
    generate_signal()
```

#### 5.4.3 信号字段

```json
{
  "signal_id": "sig_xxx",
  "market_id": "mkt_xxx",
  "action": "BUY",
  "side": "YES",
  "market_price": 0.52,
  "fair_price": 0.58,
  "edge": 0.06,
  "confidence": 0.70,
  "urgency": "medium",
  "ttl_sec": 180,
  "reason": "official update implies underpriced YES contracts"
}
```

#### 5.4.4 信号过滤条件

只有同时满足以下条件才允许进入执行层：

1. `abs(edge)` 高于最小阈值。
2. `confidence` 高于最小阈值。
3. 流动性满足要求。
4. 风险敞口未超限。
5. 信号在有效期内。
6. 非灰名单或禁用市场。

#### 5.4.5 阈值策略

阈值不应固定不变，建议动态调整：

```text
effective_threshold =
base_threshold
+ slippage_buffer
+ uncertainty_buffer
+ low_liquidity_penalty
```

这样可以避免“理论有 edge，实际交易后没有 edge”的问题。

#### 5.4.6 信号生命周期

信号不应只分“生成”和“执行”，而应具有完整生命周期，便于撤单、降仓和反转。

建议状态：

```text
NEW -> ACTIVE -> WEAKENED -> EXECUTED
                   \-> INVALIDATED
                   \-> REVERSED
                   \-> EXPIRED
```

状态说明：

1. `NEW`：刚生成，尚未通过所有检查。
2. `ACTIVE`：允许执行或挂单管理。
3. `WEAKENED`：edge 或置信度下降，应减弱执行优先级。
4. `INVALIDATED`：原假设失效，应撤单或减仓。
5. `REVERSED`：新证据使方向翻转，需重新评估甚至反手。
6. `EXPIRED`：超过有效期，自动失效。

---

### 5.5 执行引擎

#### 5.5.1 目标

将信号转化为受控订单，并持续管理订单状态，直到成交、撤单、过期或被 kill switch 中断。

#### 5.5.2 核心职责

1. 下单。
2. 撤单。
3. 改单。
4. 订单状态跟踪。
5. 成交回报处理。
6. 执行后结果上报。

对于 Polymarket，执行层应继续拆分为：

1. CLOB API 交易接口。
2. CLOB WebSocket 行情流。
3. Data API 辅助查询。
4. CTF 链上 `split` / `merge` / `redeem`。
5. GnosisSafe / proxy wallet 特殊执行路径。

具体实现边界见 [polyedge-polymarket-connector-design.md](./polyedge-polymarket-connector-design.md)。

#### 5.5.3 执行原则

1. 优先限价单。
2. 不追单，不因短期波动盲目抬价。
3. 明确控制滑点。
4. 多笔拆单优于一次性冲击市场。
5. 订单必须带来源信号与风险上下文。

#### 5.5.4 执行流程

```text
信号进入 -> 风控预检查 -> 计算目标价格/数量 -> 下单 -> 跟踪状态
       -> 部分成交则继续管理 -> 超时则撤单 -> 回写成交结果
```

#### 5.5.5 订单定价建议

下单价格建议基于以下因素综合计算：

1. 最优买卖价。
2. 目标 edge 留存空间。
3. 盘口深度。
4. 预计滑点。
5. 紧急程度。

示例：

```python
target_price = min(
    fair_price - required_residual_edge,
    best_ask if action == "BUY" else best_bid
)
```

#### 5.5.6 订单状态机

```text
NEW -> SUBMITTED -> OPEN -> PARTIALLY_FILLED -> FILLED
                         \-> CANCELED
                         \-> EXPIRED
                         \-> REJECTED
```

---

### 5.6 风控系统

#### 5.6.1 目标

确保任何单一市场、单一事件、单日损失或系统异常都不会导致不可控风险。

#### 5.6.2 基础风控规则

```python
max_position_per_market = 0.05
max_daily_loss = 0.10
max_single_trade = 0.02
max_open_orders_per_market = N
```

以上数值为初始建议，最终以实盘前回测和模拟结果校准。

#### 5.6.3 风控分层

##### 交易前

1. 单市场仓位检查。
2. 单笔交易规模检查。
3. 流动性检查。
4. 最低剩余 edge 检查。
5. 接近结算时间检查。
6. 模型置信度检查。

##### 交易中

1. 订单超时撤销。
2. 异常波动暂停追加。
3. API 错误触发保护。
4. 盘口快速恶化时重新评估。

##### 交易后

1. 持仓聚合检查。
2. 当日 PnL 检查。
3. 连续亏损检查。
4. 事件失效后主动减仓。

#### 5.6.4 动态风控

以下情形下自动降风险：

1. 波动率升高。
2. 流动性下降。
3. 距离结算变短。
4. 信息冲突增加。
5. 模型置信度下降。

#### 5.6.5 Kill Switch

以下情形必须立即进入保护模式：

1. 市场数据异常。
2. 执行 API 异常。
3. 风控服务不可用。
4. 账户净值异常波动。
5. 订单状态无法确认。

动作：

```python
if abnormal_market or api_error or risk_service_down:
    cancel_all_orders()
    disable_new_signals()
    alert_operator()
```

#### 5.6.6 组合级风控

除单市场限制外，系统还必须控制组合层面的相关风险。

建议增加以下维度的风险桶：

1. 主题桶，如美国大选、加密监管、宏观数据。
2. 事件桶，如同一候选人或同一政策事件。
3. 来源桶，如同一官方公告链条或同一社媒源。
4. 时间桶，如同一天内集中结算的市场。

组合级约束建议包括：

1. 同主题市场总敞口上限。
2. 同一事件簇总敞口上限。
3. 高相关市场的合并仓位限制。
4. 单一来源驱动仓位限制。
5. 组合层最大回撤阈值。

这样可以避免“单市场看起来分散，实际却押注同一个结果”的伪分散问题。

---

### 5.7 监控与审计层

#### 5.7.1 目标

保证系统可观测、可解释、可追责。

#### 5.7.2 监控指标

##### 数据侧

1. 各数据源延迟。
2. 各数据源成功率。
3. 去重率。
4. 事件吞吐量。

##### 策略侧

1. 每日事件数。
2. 有效事件转化率。
3. 信号生成数。
4. 信号成交率。
5. 理论 edge 与实际实现 edge。
6. event-to-market mapping 准确率。
7. 概率校准指标，如 Brier Score。
8. 费后、滑点后净 alpha。
9. 信号失效率与反转率。

##### 风控侧

1. 当前总敞口。
2. 单市场敞口。
3. 当日损益。
4. 连续亏损次数。
5. kill switch 触发次数。

#### 5.7.3 审计要求

每笔交易都应能够追溯到：

1. 原始数据。
2. 事件判断结果。
3. 概率估计输出。
4. 信号生成参数。
5. 风控决策结果。
6. 最终订单与成交记录。

#### 5.7.4 研究评估闭环

监控不应只服务生产运行，也应服务策略研究。

建议形成如下闭环：

```text
原始数据 -> 事件识别 -> 证据抽取 -> 概率更新 -> 信号 -> 执行结果
       -> 事后评估 -> 指标归因 -> 参数/规则修正 -> 新版本上线
```

研究阶段最重要的是能够区分：

1. 是事件没识别对。
2. 还是 market mapping 错了。
3. 还是估值模型不准。
4. 还是执行吃掉了 edge。
5. 还是风控限制过松或过紧。

---

## 6. 数据模型设计

### 6.1 存储组件

#### PostgreSQL

存储结构化主数据、策略结果、订单与交易记录。

#### Redis

存储实时行情、短期事件流、最近信号、风控热状态。

### 6.2 核心表建议

#### `markets`

记录市场基础信息。

建议字段：

1. `id`
2. `platform_market_id`
3. `question`
4. `description`
5. `status`
6. `close_time`
7. `resolve_time`
8. `category`
9. `tags`
10. `resolution_source`
11. `resolution_rules_json`
12. `ambiguity_level`
13. `tradability_status`
14. `metadata_json`
15. `created_at`
16. `updated_at`

#### `market_resolution_rules`

记录市场结算语义和可交易性判断。

建议字段：

1. `id`
2. `market_id`
3. `question_text`
4. `resolution_source`
5. `resolution_deadline`
6. `criteria_json`
7. `edge_cases_json`
8. `ambiguity_level`
9. `review_required`
10. `review_status`
11. `review_notes`
12. `updated_at`

#### `market_snapshots`

记录市场价格、盘口和流动性快照。

建议字段：

1. `id`
2. `market_id`
3. `best_bid`
4. `best_ask`
5. `mid_price`
6. `last_trade_price`
7. `spread`
8. `depth_json`
9. `volume_24h`
10. `captured_at`

#### `raw_events`

存储外部原始信息。

建议字段：

1. `id`
2. `source`
3. `source_type`
4. `title`
5. `content`
6. `url`
7. `author`
8. `published_at`
9. `ingested_at`
10. `hash`
11. `raw_payload`

#### `events`

存储系统识别后的标准化事件。

建议字段：

1. `id`
2. `raw_event_id`
3. `event_type`
4. `summary`
5. `entities_json`
6. `importance`
7. `relevance_score`
8. `impact_direction`
9. `impact_strength`
10. `confidence`
11. `reason`
12. `contradiction_group`
13. `status`
14. `expires_at`
15. `created_at`

#### `evidences`

记录事件对 market 结算路径的具体支持或反驳证据。

建议字段：

1. `id`
2. `event_id`
3. `market_id`
4. `direction`
5. `strength`
6. `source_reliability`
7. `novelty`
8. `resolution_relevance`
9. `half_life_minutes`
10. `contradiction_group`
11. `status`
12. `reason`
13. `created_at`
14. `expires_at`

#### `event_market_links`

关联事件与市场。

建议字段：

1. `id`
2. `event_id`
3. `market_id`
4. `relation_type`
5. `relevance_score`
6. `created_at`

#### `probability_estimates`

记录每次概率估计结果。

建议字段：

1. `id`
2. `market_id`
3. `event_id`
4. `prior_price`
5. `posterior_price`
6. `fair_price`
7. `market_price`
8. `edge`
9. `confidence`
10. `time_horizon`
11. `model_version`
12. `evidence_summary_json`
13. `features_json`
14. `created_at`

#### `signals`

记录策略信号。

建议字段：

1. `id`
2. `market_id`
3. `event_id`
4. `estimate_id`
5. `action`
6. `side`
7. `price`
8. `size`
9. `edge`
10. `confidence`
11. `status`
12. `expires_at`
13. `lifecycle_state`
14. `invalidated_reason`
15. `reason`
16. `version_snapshot_json`
17. `created_at`

#### `signal_transitions`

记录信号生命周期变化，便于回放和审计。

建议字段：

1. `id`
2. `signal_id`
3. `from_state`
4. `to_state`
5. `trigger_type`
6. `trigger_payload`
7. `created_at`

#### `orders`

记录订单生命周期。

建议字段：

1. `id`
2. `signal_id`
3. `market_id`
4. `external_order_id`
5. `side`
6. `price`
7. `quantity`
8. `filled_quantity`
9. `avg_fill_price`
10. `status`
11. `submitted_at`
12. `updated_at`

#### `trades`

记录成交明细。

建议字段：

1. `id`
2. `order_id`
3. `market_id`
4. `side`
5. `price`
6. `quantity`
7. `fee`
8. `executed_at`

#### `positions`

记录聚合持仓。

建议字段：

1. `id`
2. `market_id`
3. `net_qty`
4. `avg_cost`
5. `mark_price`
6. `unrealized_pnl`
7. `realized_pnl`
8. `updated_at`

#### `risk_state`

记录全局风控快照。

建议字段：

1. `id`
2. `date`
3. `nav`
4. `daily_pnl`
5. `gross_exposure`
6. `net_exposure`
7. `kill_switch`
8. `notes`
9. `updated_at`

#### `research_runs`

记录回放、模拟和参数实验结果，支撑策略迭代。

建议字段：

1. `id`
2. `run_type`
3. `strategy_version`
4. `risk_version`
5. `model_version`
6. `prompt_version`
7. `dataset_version`
8. `start_time`
9. `end_time`
10. `metrics_json`
11. `notes`
12. `created_at`

---

## 7. 核心数据流

### 7.1 外部事件驱动流程

```text
抓取原始新闻/X/公告
-> 清洗标准化
-> 去重
-> 解析 market 结算语义
-> 事件识别
-> 抽取 evidence
-> 关联 market
-> 概率估计
-> 生成信号
-> 风控检查
-> 提交订单
-> 回写订单与成交
-> 更新持仓与监控
```

### 7.2 市场驱动流程

```text
监听市场价格/盘口变化
-> 更新市场快照
-> 检查已有估值是否失效
-> 重新计算 edge
-> 更新信号状态
-> 决定保留/撤销/替换订单
```

### 7.3 定时重估流程

```text
定时任务触发
-> 扫描未结算市场
-> 获取最新事件与价格
-> 重算 fair_price
-> 更新风险参数
-> 产出新的信号或撤销旧信号
```

### 7.4 证据衰减与信号失效流程

```text
定时扫描 active evidences/signals
-> 判断 ttl / half-life / 冲突证据
-> 重算 posterior
-> 更新 signal state
-> 必要时撤单 / 减仓 / 反转
```

---

## 8. 服务划分建议

为降低系统复杂度，MVP 阶段建议拆成以下服务或模块。

### 8.1 `collector`

负责市场数据和外部信息采集。

### 8.2 `event-engine`

负责事件识别、实体提取、市场映射和结算语义解析。

### 8.3 `pricing-engine`

负责 evidence 聚合、概率更新与 edge 计算。

### 8.4 `signal-engine`

负责信号生成与信号生命周期管理。

### 8.5 `execution-engine`

负责订单管理与成交同步。

### 8.6 `risk-engine`

负责风险规则校验、限额、kill switch。

### 8.7 `api/dashboard`

负责策略状态展示、人工确认与管理入口。

### 8.8 `research-evaluator`

负责回放、指标评估、版本比较与实验归因。

MVP 可以先实现为一个 Rust 模块化单体，按模块分层和 workspace 组织；后续再按职责拆分为独立服务。

---

## 9. 技术栈建议

### 9.1 前端

1. Next.js（App Router）
2. TypeScript
3. React Server Components + Server Actions
4. Tailwind CSS 或同类原子化样式方案
5. SSE / WebSocket 用于实时模块

### 9.2 后端

1. Rust
2. Axum
3. Tokio
4. SQLx
5. Redis
6. Tracing / Metrics / Audit Logging

### 9.3 数据与消息

1. PostgreSQL
2. Redis
3. 可选消息队列，用于后续解耦事件流

### 9.4 数据接入

1. `reqwest`
2. WebSocket 客户端
3. RSS 解析工具

### 9.5 模型层

1. OpenAI 或本地模型用于事件理解
2. 规则模型用于稳定估值和风控输入

### 9.6 运维与观测

1. Docker
2. 基础日志聚合
3. 指标监控与告警

---

## 10. 配置设计

建议将策略参数外置，避免硬编码。

### 10.1 运行模式

系统至少应支持以下运行模式：

1. `research`
   只采集、识别、估值，不生成可执行订单。
2. `paper_trade`
   生成信号和模拟订单，但不触发真实交易。
3. `manual_confirm`
   生成真实订单草稿，需人工确认后提交。
4. `live_auto`
   自动执行，但必须受完整风控和 kill switch 约束。

不同模式下应明确：

1. 是否允许下单。
2. 是否允许自动撤单和改单。
3. 是否允许自动开新仓。
4. 哪些模式切换需要人工审批。

### 10.2 配置与版本治理

所有关键决策都应绑定版本快照，至少包括：

1. `strategy_version`
2. `risk_version`
3. `model_version`
4. `prompt_version`
5. `dataset_version`
6. `feature_version`

要求：

1. 每个信号和订单都能追溯到对应版本。
2. 回放结果必须和版本快照绑定。
3. 参数修改应支持灰度验证，而不是直接覆盖生产配置。
4. Prompt 或模型变更应视为策略变更的一部分。

示例配置：

```yaml
mode: research

trading:
  enabled: false
  min_confidence: 0.65
  base_edge_threshold: 0.03
  required_residual_edge: 0.015

risk:
  max_position_per_market: 0.05
  max_single_trade: 0.02
  max_daily_loss: 0.10
  max_open_orders_per_market: 3
  disable_near_resolution_minutes: 60
  max_theme_exposure: 0.20
  max_event_cluster_exposure: 0.12

data_sources:
  x_enabled: true
  rss_enabled: true
  official_feed_enabled: true
  market_stale_sec: 15
  news_stale_sec: 300
  source_fail_threshold: 5

llm:
  provider: openai
  model: gpt-5.4
  timeout_sec: 20

versioning:
  strategy_version: v1
  risk_version: v1
  prompt_version: event-v1
  dataset_version: snapshot-2026-04-15
```

---

## 11. MVP 落地路径

### 11.1 Phase 1：研究系统

目标：

1. 接入市场数据。
2. 接入至少一类外部信息源。
3. 建立事件识别与概率估计原型。
4. 不进行真实下单。

交付物：

1. 市场同步模块。
2. 原始事件入库。
3. 结算规则解析原型。
4. 证据和概率估计结果表。
5. 基础 dashboard 或命令行输出。

### 11.2 Phase 2：模拟交易

目标：

1. 记录信号。
2. 模拟成交与 PnL。
3. 验证 edge 是否可实现。

交付物：

1. 信号表和模拟订单表。
2. 回放与统计脚本。
3. 基本风控规则。
4. 校准和执行质量评估报表。

### 11.3 Phase 3：小资金实盘

目标：

1. 引入人工确认。
2. 小仓位验证执行质量。
3. 观察真实滑点与成交率。

交付物：

1. 手动确认开关。
2. 真实订单执行模块。
3. 订单与成交审计日志。

### 11.4 Phase 4：自动化

目标：

1. 自动执行。
2. 完整风控。
3. 完整监控与告警。

交付物：

1. 自动执行流程。
2. kill switch。
3. 生产化部署方案。

---

## 12. 测试与验证方案

### 12.1 需要验证的问题

1. 系统能否识别真正影响结算概率的信息。
2. 概率估计是否优于直接跟随市场价格。
3. 理论 edge 在扣除滑点和手续费后是否仍然成立。
4. 风控是否能限制尾部风险。

### 12.2 测试类型

#### 单元测试

针对：

1. 规则匹配。
2. 事件去重。
3. edge 计算。
4. 风控判定。
5. 结算语义解析。
6. 证据衰减逻辑。

#### 集成测试

针对：

1. 数据采集到事件生成全链路。
2. 信号到订单回写全链路。
3. kill switch 触发流程。
4. 证据失效后自动撤信号流程。

#### 回放测试

使用历史事件和市场价格数据进行准实时重放，观察系统在真实时间序列中的决策行为。

#### 纸面交易

不下单，只记录若当时交易会得到什么结果。

### 12.3 研究评估指标

研究阶段建议最少跟踪以下指标：

1. 事件识别准确率。
2. event-to-market mapping 准确率。
3. 结算语义解析正确率。
4. 概率校准指标，如 Brier Score、Calibration Error。
5. 信号命中率。
6. 理论 edge、可成交 edge、实现 edge 之间的偏差。
7. 成交率、撤单率、滑点和费后收益。
8. 单市场、单主题、组合级回撤。

### 12.4 实验治理与回溯

为保证策略迭代可控，建议所有实验都采用统一治理方式：

1. 每次回放都记录输入数据范围和版本快照。
2. 每次参数变更都记录变更原因和预期影响。
3. 生产信号表现恶化时，能够快速回溯到具体版本。
4. 重要策略变更先在 `research` 或 `paper_trade` 模式验证，再进入实盘。

---

## 13. 安全与可靠性要求

1. API Key、私钥和账户信息必须通过环境变量或密钥管理系统注入。
2. 真实交易与研究环境必须隔离。
3. 所有关键操作必须记录审计日志。
4. 外部接口异常不能直接导致重复下单。
5. 订单提交必须具备幂等保护。
6. 运行模式切换必须记录操作者和切换原因。
7. 人工确认和自动执行权限应分离。

---

## 14. 主要风险与缓解策略

### 14.1 市场题目和结算语义误判

风险：系统理解了新闻，但没有理解 market 真正的结算条件。

缓解：

1. 单独建设结算规则层。
2. 对高歧义市场设置人工复核或禁用。
3. 将 resolution source 和 edge case 持久化存储。

### 14.2 信息噪音过高

风险：系统对低价值新闻过度反应。

缓解：

1. 提高有效事件门槛。
2. 增加来源权重和二次确认。
3. 对低置信度事件只观察不交易。

### 14.3 LLM 输出不稳定

风险：相同输入输出差异较大，导致估值不稳定。

缓解：

1. 限制 LLM 只做事件理解，不直接主导仓位。
2. 引入规则模型兜底。
3. 存储 prompt、模型版本与输出结果，方便审计。

### 14.4 流动性不足

风险：理论 edge 无法转化为真实收益。

缓解：

1. 增加流动性筛选。
2. 下调低深度市场仓位上限。
3. 执行前预估冲击成本。

### 14.5 临近结算的跳变风险

风险：价格剧烈波动，模型来不及修正。

缓解：

1. 临近结算自动降仓。
2. 禁止在某些窗口内新开仓。
3. 提高近结算市场的信号阈值。

### 14.6 系统故障

风险：异常状态下重复交易或风险失控。

缓解：

1. kill switch。
2. 订单幂等。
3. 风控服务优先级高于执行服务。
4. 完整监控和人工干预入口。

---

## 15. 后续扩展方向

1. 多市场相关性分析与组合级风险控制。
2. 同一事件在不同市场间的套利识别。
3. LP rewards 参数优化。
4. dashboard 可视化与人工审阅工作台。
5. 历史案例库、结算语义模板库与证据模板库。

---

## 16. 推荐的首版实现重点

如果从零开始，建议优先做以下闭环：

1. 市场列表与价格快照采集。
2. 新闻或公告接入一条主数据源。
3. 结算规则解析原型。
4. 事件识别与 evidence 原型。
5. `fair_price` / posterior 估计原型。
6. 信号落库与模拟交易。
7. 基础风控、版本快照和审计日志。

这条路径能最快验证 PolyEdge 的核心假设：

> 系统是否真的能够通过“信息 -> 概率更新 -> 定价偏差”持续识别可交易机会。

---

## 17. 总结

PolyEdge 的本质不是一个单纯的量化交易脚本，而是一个“事件理解驱动的概率交易系统”。

它的竞争力不在于速度极限，而在于：

1. 比市场更快识别有效信息。
2. 比市场更准确地把信息映射成结算路径上的有效证据。
3. 比市场更准确地把证据更新为概率变化。
4. 比普通交易系统更严格地控制风险与执行质量。

最终目标不是“每条新闻都交易”，而是“只在少数真正有 edge 的时刻出手，并在可控风险下稳定积累收益”。
