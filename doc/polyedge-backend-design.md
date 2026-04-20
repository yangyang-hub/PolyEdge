# PolyEdge 后端设计文档

## 1. 文档目标

本文档定义 PolyEdge 后端的运行架构、服务边界、模块设计、数据流、API 设计、并发与一致性策略，以及基于 Rust 的工程组织方式。

后端采用 Rust，目标不是单纯“高性能”，而是用强类型、异步并发和明确状态机支撑一个可审计、可回放、可实盘的事件驱动交易系统。

---

## 2. 后端目标

后端需要同时满足以下要求：

1. 正确性高于吞吐量。
2. 核心状态可追踪、可回放、可审计。
3. 采集、估值、信号、风控、执行之间边界清晰。
4. 既能先以模块化单体启动，也能后续拆成独立服务。
5. 对外提供稳定 API 给 Next.js 控制台使用。

后端当前不追求：

1. 超低延迟高频交易基础设施。
2. 复杂的分布式微服务矩阵。
3. 过早引入多种消息系统和多套存储。

---

## 3. 技术选型

### 3.1 核心栈

1. Rust
2. Tokio
3. Axum
4. SQLx
5. Redis
6. Serde
7. Tracing

### 3.2 推荐依赖方向

1. `reqwest`
   用于外部 HTTP 数据采集。
2. `tokio-tungstenite` 或同类库
   用于 WebSocket 数据源接入。
3. `sqlx`
   用于 PostgreSQL 访问和编译期 SQL 校验。
4. `redis`
   用于缓存、轻量事件流和热状态。
5. `tower`
   用于中间件、限流、超时、重试与鉴权。
6. `tracing` + metrics
   用于日志、链路追踪和指标上报。

---

## 4. 架构原则

1. 模块化单体优先。
   首版在一个 Rust workspace 内完成 API、worker 和 replay runner，后续按边界拆分。
2. PostgreSQL 是系统真值。
   交易、风控、状态机、审计记录都以 PostgreSQL 为准。
3. Redis 是热缓存和轻量分发层。
   不承载最终一致性真值。
4. 订单、信号、模式切换必须显式状态机化。
5. 所有关键操作都要具备幂等保护。

---

## 5. 系统拓扑

建议后端以一个 workspace 管理多个二进制入口：

```text
backend/
  Cargo.toml
  apps/
    api/
    worker/
    replay/
  crates/
    domain/
    application/
    infrastructure/
    connectors/
    contracts/
```

### 5.1 运行进程

1. `api`
   对前端提供查询、操作、实时推送和权限校验。
2. `worker`
   负责采集、去重、事件识别、证据生成、估值、信号、风险扫描和执行。
3. `replay`
   负责历史数据回放、研究实验和版本比较。

### 5.2 模块化单体的原因

1. 业务域边界已经明显，但调用链仍较紧密。
2. 过早拆分微服务会放大调试和一致性成本。
3. Rust workspace 很适合先做强边界模块，再按需要拆进程。

---

## 6. 领域模块设计

### 6.1 `market`

职责：

1. 同步市场基础信息。
2. 管理市场状态、流动性快照和可交易状态。
3. 持久化结算相关元数据。

### 6.2 `resolution`

职责：

1. 解析 market 题目和结算来源。
2. 维护 `market_resolution_rules`。
3. 输出歧义等级、review requirement 和 tradability。

### 6.3 `event`

职责：

1. 接收原始数据流。
2. 清洗、去重、实体提取。
3. 生成标准化事件。

### 6.4 `evidence`

职责：

1. 将事件映射到 market。
2. 生成支持或反驳结算路径的 evidence。
3. 处理证据冲突、合并、衰减和失效。

### 6.5 `pricing`

职责：

1. 维护 prior / posterior。
2. 聚合 evidence 生成 fair price。
3. 产出 confidence、reason codes 和 time horizon。

### 6.6 `signal`

职责：

1. 计算 edge。
2. 生成可执行信号。
3. 维护信号生命周期和状态迁移记录。

### 6.7 `risk`

职责：

1. 执行单市场、单事件、组合级风险检查。
2. 维护风险桶和全局运行模式。
3. 管理 kill switch。

### 6.8 `execution`

职责：

1. 订单提交、撤单、改单、成交同步。
2. 执行前后幂等校验。
3. 与账户、持仓、PnL 更新联动。

Polymarket 连接器、CLOB WebSocket、CTF 链上交互和 Safe 代理钱包执行细节见 [polyedge-polymarket-connector-design.md](./polyedge-polymarket-connector-design.md)。

### 6.9 `research`

职责：

1. 回放历史运行过程。
2. 记录实验版本和指标。
3. 比较不同参数和模型版本的效果。

---

## 7. 分层组织

建议采用典型的分层结构，而不是把 HTTP、数据库和业务逻辑混在一起。

### 7.1 `domain`

包含：

1. 实体，如 `Market`、`Event`、`Evidence`、`Signal`。
2. 值对象，如 `Probability`、`RiskBucket`、`SignalState`。
3. 纯业务规则和状态机。

### 7.2 `application`

包含：

1. Use Cases。
2. Command / Query handlers。
3. 编排事务边界和跨模块调用。

### 7.3 `infrastructure`

包含：

1. PostgreSQL 仓储实现。
2. Redis 缓存实现。
3. 外部 API 连接器。
4. 实时推送实现。

其中 Polymarket 相关连接器建议拆为：

1. `gamma_client`
2. `clob_ws_feed`
3. `clob_trading_client`
4. `data_api_client`
5. `ctf_client`
6. `safe_executor`

具体拆分见 [polyedge-polymarket-connector-design.md](./polyedge-polymarket-connector-design.md)。

### 7.4 `contracts`

包含：

1. 前后端共享的 API DTO。
2. 实时推送 payload。
3. 错误码和分页协议。

详细定义见 [polyedge-api-contract.md](./polyedge-api-contract.md)。
枚举和定点数字段规范见 [polyedge-domain-enums-and-decimals.md](./polyedge-domain-enums-and-decimals.md)。
存储补充 schema 见 [polyedge-storage-schema.md](./polyedge-storage-schema.md)。

---

## 8. 核心数据流

### 8.1 外部事件处理流

```text
collector ingest
-> normalize
-> deduplicate
-> resolution check
-> event create
-> evidence create/update
-> pricing recompute
-> signal generate/update
-> risk check
-> execution submit or queue approval
```

### 8.2 市场行情处理流

```text
market snapshot update
-> liquidity/spread recompute
-> posterior freshness check
-> edge recompute
-> signal weaken/invalidate/update
-> order adjust or cancel
```

### 8.3 定时任务流

```text
scheduler tick
-> expire evidences
-> recompute posterior
-> refresh risk state
-> reconcile orders
-> persist audit metrics
```

---

## 9. API 设计

### 9.1 API 分类

1. 查询 API
   给 Next.js 页面和 Server Components 使用。
2. 操作 API
   给 Server Actions 使用，如审批、撤单、模式切换。
3. 实时 API
   给前端实时面板使用，建议 SSE 为主。
4. 内部管理 API
   仅用于健康检查、指标和内部控制。

### 9.2 查询 API 示例

1. `GET /api/markets`
2. `GET /api/markets/:id`
3. `GET /api/events`
4. `GET /api/signals`
5. `GET /api/positions`
6. `GET /api/risk/state`
7. `GET /api/research/runs/:id`

### 9.3 操作 API 示例

1. `POST /api/signals/:id/approve`
2. `POST /api/orders/:id/cancel`
3. `POST /api/system/mode`
4. `POST /api/system/kill-switch/trigger`
5. `POST /api/system/kill-switch/release`

### 9.4 实时接口建议

1. `GET /api/stream/signals`
2. `GET /api/stream/risk`
3. `GET /api/stream/events`
4. `GET /ws/markets`

### 9.5 API 设计原则

1. 查询和操作分离。
2. 返回对象中尽量附带 `updated_at`、`version`、`trace_id`。
3. 所有写操作要求幂等键或操作 token。
4. 错误响应统一返回机器可读错误码。

---

## 10. 状态机设计

### 10.1 信号状态机

```text
NEW -> ACTIVE -> WEAKENED -> EXECUTED
                   \-> INVALIDATED
                   \-> REVERSED
                   \-> EXPIRED
```

状态迁移必须：

1. 明确触发原因。
2. 记录迁移前后状态。
3. 关联 trace_id、event_id、market_id。

### 10.2 订单状态机

```text
NEW -> SUBMITTED -> OPEN -> PARTIALLY_FILLED -> FILLED
                         \-> CANCELED
                         \-> EXPIRED
                         \-> REJECTED
```

执行服务不得绕过状态机直接覆盖状态。

### 10.3 运行模式状态

系统模式建议显式建模：

1. `research`
2. `paper_trade`
3. `manual_confirm`
4. `live_auto`
5. `kill_switch_locked`

模式切换应经过：

1. 权限检查。
2. 审计记录。
3. 依赖健康检查。

---

## 11. 并发、一致性与幂等

### 11.1 基本原则

1. PostgreSQL 事务负责关键写路径一致性。
2. Redis 只用于缓存和广播，不承担真值冲突解决。
3. 外部 API 重试必须幂等。
4. 订单提交必须绑定本地幂等键。

### 11.2 建议机制

1. 对订单和模式切换使用 `idempotency_key`。
2. 对高并发状态更新使用乐观锁或版本号。
3. 对跨系统写入使用 outbox 模式或等价机制。
4. 对外部回报做去重，避免重复成交回写。

### 11.3 典型事务边界

1. 生成 signal + 写入 audit log。
2. 审批通过 + 创建 order request。
3. 订单回报 + 更新持仓 + 更新风险快照。

---

## 12. 后台任务与调度

### 12.1 长运行任务

1. 市场同步。
2. 新闻/公告/X 抓取。
3. 证据衰减扫描。
4. 风险扫描。
5. 订单对账与补偿。

### 12.2 调度策略

建议使用两类调度：

1. 事件驱动任务
   对新事件、新行情、新回报立即响应。
2. 周期任务
   对衰减、对账、清理、指标归档定期执行。

### 12.3 内部消息机制

首版建议保持简单：

1. PostgreSQL 记录真值。
2. Redis Streams 或等价轻量队列分发内部事件。
3. 高价值事件同时写 audit log。

这样可以在不引入过多基础设施的前提下支持异步处理。

---

## 13. 鉴权与权限

完整认证与会话设计见 [polyedge-auth-design.md](./polyedge-auth-design.md)。

### 13.1 鉴权来源

后端应作为权限真值来源，前端只能作为会话承载层。

建议支持：

1. Session/Cookie 鉴权。
2. 内部服务 token。
3. 基于角色的访问控制。

### 13.2 权限边界

1. 读接口和写接口分开授权。
2. 高风险操作需要更高权限。
3. 模式切换、kill switch、手动执行必须单独审计。

---

## 14. LLM 调用治理

LLM 的工程治理、Prompt 版本和结构化输出规则见 [polyedge-llm-governance.md](./polyedge-llm-governance.md)。

---

## 15. 可观测性与审计

### 15.1 日志

建议统一使用结构化日志，关键字段至少包括：

1. `trace_id`
2. `market_id`
3. `event_id`
4. `signal_id`
5. `order_id`
6. `mode`

### 15.2 指标

1. 数据源延迟和失败率。
2. 事件吞吐。
3. posterior 重算次数。
4. signal 生成/失效/反转次数。
5. 风险拒绝次数。
6. 订单成交率和撤单率。

### 15.3 审计

所有关键动作都应能回溯到：

1. 触发输入。
2. 业务决策。
3. 调用者身份。
4. 版本快照。

---

## 16. 部署建议

### 16.1 初期部署

1. 一个 `api` 实例组。
2. 一个 `worker` 实例组。
3. 一个独立 PostgreSQL。
4. 一个独立 Redis。

### 16.2 扩展方向

当负载增长时，优先按职责扩展：

1. 将 `collector` 与 `execution` 从通用 worker 中拆出。
2. 将 `replay` 做成离线任务容器。
3. 将市场实时流和控制台 API 解耦。

---

## 17. 推荐工程结构

```text
backend/
  apps/
    api/
      src/
    worker/
      src/
    replay/
      src/
  crates/
    domain/
      src/
    application/
      src/
    infrastructure/
      src/
    connectors/
      src/
    contracts/
      src/
```

目录原则：

1. 外部适配器和业务规则分离。
2. 数据库模型和 HTTP DTO 分离。
3. 领域状态机集中管理，不散落在 handler 中。

---

## 18. MVP 后端落地顺序

更细的实施计划见 [polyedge-backend-implementation-plan.md](./polyedge-backend-implementation-plan.md)。

建议按以下顺序实现：

1. PostgreSQL/Redis 基础设施和 workspace 骨架。
2. `market`、`resolution`、`event` 三个基础域。
3. `evidence`、`pricing`、`signal`。
4. `risk` 与审批流。
5. `execution` 与订单对账。
6. `api` 查询接口与 SSE。
7. `replay` 与研究评估。

首版后端的关键目标不是“先把所有接口写完”，而是尽快打通：

```text
数据进入 -> 事件/证据 -> posterior -> signal -> risk -> audit
```

这条链路一旦稳定，再扩展执行与控制台会更稳。
