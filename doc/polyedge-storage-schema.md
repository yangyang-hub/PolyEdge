# PolyEdge 存储 Schema 补充规范

> **状态（2026-07-12）**：本文是早期 schema 补充规范，正文只作为审计/幂等/outbox 设计背景，部分表名、列和 SSE 投递规划不代表当前迁移。当前数据库使用 `0001_initial_schema.sql` 单 baseline，包含 rewards run/replay/action ledger、fair value、candles、event windows 和 BalancedMerge；准确状态以 [modules/infra/database.md](modules/infra/database.md) 和 [../packages/backend/migrations/](../packages/backend/migrations/) 为准。

## 1. 文档目标

本文档补充 PolyEdge 在 PostgreSQL 侧的实现级 schema 约束，重点覆盖：

1. 核心表未写明的公共列规则。
2. 审计、幂等、outbox 和外部回报去重所需的支撑表。
3. LLM 调用审计落库要求。
4. 与事务边界对应的最小持久化模型。

本文件默认与 [polyedge-design.md](./polyedge-design.md)、[polyedge-backend-design.md](./polyedge-backend-design.md)、[polyedge-auth-design.md](./polyedge-auth-design.md) 和 [polyedge-llm-governance.md](./polyedge-llm-governance.md) 一起使用。

---

## 2. 基本约定

### 2.1 主键与 ID

1. API-facing 资源首版统一使用应用生成的 `TEXT` 主键，如 `mkt_xxx`、`evt_xxx`、`sig_xxx`。
2. 不要求数据库为核心业务实体生成自增主键。
3. 对外部系统返回的 `external_*_id` 必须与内部主键分离。

### 2.2 时间与时区

1. 所有时间列统一使用 `TIMESTAMPTZ`。
2. 所有写入统一使用 UTC。
3. 业务层和数据库层都不允许存本地时区时间。

### 2.3 公共列

以下规则为首版强制约定：

1. 所有“可变资源表”必须包含 `version BIGINT NOT NULL DEFAULT 1`。
2. 所有“可变资源表”必须包含 `updated_at TIMESTAMPTZ NOT NULL`。
3. 所有“对外可回溯表”必须包含 `trace_id TEXT NOT NULL`。
4. 所有 JSON 扩展字段统一使用 `JSONB`。
5. 枚举字段首版统一使用 `TEXT NOT NULL + CHECK`。

可变资源表至少包括：

1. `markets`
2. `market_resolution_rules`
3. `events`
4. `evidences`
5. `signals`
6. `orders`
7. `positions`
8. `risk_state`

### 2.4 表类型划分

建议按写入模式区分：

1. Append-only：`raw_events`、`market_snapshots`、`signal_transitions`、`trades`、`audit_logs`、`llm_calls`
2. Mutable current-state：`markets`、`events`、`evidences`、`signals`、`orders`、`positions`、`risk_state`
3. Delivery support：`idempotency_keys`、`outbox_events`、`external_event_dedup`

---

## 3. 必备支撑表

### 3.1 `audit_logs`

用途：

1. 敏感操作审计。
2. 状态机关键迁移审计。
3. 模式切换和 kill switch 审计。

建议字段：

1. `id`
2. `occurred_at`
3. `request_id`
4. `trace_id`
5. `actor_user_id`
6. `actor_session_id`
7. `actor_roles_json`
8. `action`
9. `resource_type`
10. `resource_id`
11. `reason`
12. `result`
13. `error_code`
14. `ip`
15. `user_agent_summary`
16. `payload_json`
17. `version_snapshot_json`

建议索引：

1. `request_id`
2. `trace_id`
3. `(resource_type, resource_id, occurred_at DESC)`
4. `(actor_user_id, occurred_at DESC)`
5. `(action, occurred_at DESC)`

规则：

1. `audit_logs` 为 append-only，不做业务更新。
2. 审计记录与业务事务同事务提交。
3. 敏感操作失败同样要写审计。

### 3.2 `idempotency_keys`

用途：

1. 写接口重放保护。
2. 下单、审批、模式切换等高价值动作去重。

建议字段：

1. `scope`
2. `idempotency_key`
3. `request_hash`
4. `request_id`
5. `actor_user_id`
6. `actor_session_id`
7. `status`
8. `resource_type`
9. `resource_id`
10. `response_json`
11. `first_seen_at`
12. `last_seen_at`
13. `expires_at`

约束：

1. `UNIQUE(scope, idempotency_key)`

规则：

1. 同一个 `(scope, idempotency_key)` 只能对应同一个 `request_hash`。
2. 同 key 不同 payload 必须返回冲突错误。
3. `response_json` 可缓存成功响应，支持安全重放返回。

建议 `scope` 首版至少包含：

1. `signal.approve`
2. `order.cancel`
3. `system.mode.switch`
4. `system.kill_switch.trigger`
5. `system.kill_switch.release`
6. `execution.submit_order`

### 3.3 `outbox_events`

用途：

1. 业务事务与异步投递解耦。
2. 支撑 Redis Streams、SSE 广播、内部 worker 消费。

建议字段：

1. `id`
2. `event_id`
3. `aggregate_type`
4. `aggregate_id`
5. `event_type`
6. `payload_json`
7. `trace_id`
8. `status`
9. `delivery_attempts`
10. `next_attempt_at`
11. `published_at`
12. `last_error`
13. `created_at`

约束：

1. `event_id` 唯一。

规则：

1. 业务写入和 outbox 写入必须在同一事务中完成。
2. 外部发布器只消费 `status='pending'` 的记录。
3. 投递失败后通过 `next_attempt_at` 控制退避重试。

### 3.4 `external_event_dedup`

用途：

1. 对外部成交回报、订单状态回调、行情回补事件去重。
2. 避免重复成交写回和重复状态迁移。

建议字段：

1. `source_system`
2. `external_event_id`
3. `payload_hash`
4. `first_seen_at`
5. `processed_at`
6. `trace_id`

约束：

1. `UNIQUE(source_system, external_event_id)`

规则：

1. 外部事件进入订单回写事务前先检查本表。
2. 若外部系统没有稳定事件 ID，则退化为 `(source_system, payload_hash)` 去重。

### 3.5 `llm_calls`

用途：

1. 按照治理文档记录每次生产级 LLM 调用。
2. 支撑回放、成本统计和故障定位。

建议字段：

1. `id`
2. `task_type`
3. `model_version`
4. `prompt_version`
5. `input_hash`
6. `raw_output`
7. `parsed_output`
8. `validation_result`
9. `fallback_used`
10. `latency_ms`
11. `cost_estimate`
12. `trace_id`
13. `created_at`

建议索引：

1. `(task_type, created_at DESC)`
2. `(prompt_version, created_at DESC)`
3. `input_hash`
4. `trace_id`

### 3.6 `mode_transitions`

用途：

1. 显式记录系统运行模式变化。
2. 与 `audit_logs` 分工，保留系统状态演进链。

建议字段：

1. `id`
2. `from_mode`
3. `to_mode`
4. `reason`
5. `requested_by_user_id`
6. `requested_by_session_id`
7. `request_id`
8. `trace_id`
9. `created_at`

---

## 4. 对现有核心表的补充字段

### 4.1 `signals`

除现有建议字段外，建议补充：

1. `updated_at`
2. `trace_id`
3. `version`
4. `approved_by_user_id`
5. `approved_at`

### 4.2 `orders`

除现有建议字段外，建议补充：

1. `trace_id`
2. `version`
3. `request_id`
4. `idempotency_key`
5. `connector_name`
6. `account_id`
7. `order_type`
8. `time_in_force`
9. `rejection_code`
10. `last_external_sequence`

建议唯一约束：

1. `(connector_name, external_order_id)`，当 `external_order_id` 非空时唯一

### 4.3 `trades`

除现有建议字段外，建议补充：

1. `trace_id`
2. `connector_name`
3. `external_trade_id`

建议唯一约束：

1. `(connector_name, external_trade_id)`

### 4.4 `positions`

除现有建议字段外，建议补充：

1. `trace_id`
2. `version`
3. `account_id`

### 4.5 `risk_state`

除现有建议字段外，建议补充：

1. `mode`
2. `trace_id`
3. `version`

---

## 5. 关键唯一约束与索引

建议首版至少落地以下约束：

1. `raw_events(source, hash)` 唯一。
2. `event_market_links(event_id, market_id)` 唯一。
3. `idempotency_keys(scope, idempotency_key)` 唯一。
4. `outbox_events(event_id)` 唯一。
5. `external_event_dedup(source_system, external_event_id)` 唯一。
6. `orders(connector_name, external_order_id)` 条件唯一。
7. `trades(connector_name, external_trade_id)` 唯一。

建议首版至少落地以下查询索引：

1. `events(status, created_at DESC)`
2. `evidences(market_id, status, created_at DESC)`
3. `signals(market_id, lifecycle_state, updated_at DESC)`
4. `orders(signal_id, status, updated_at DESC)`
5. `market_snapshots(market_id, captured_at DESC)`
6. `audit_logs(request_id)`
7. `llm_calls(trace_id)`

---

## 6. 事务落地模式

### 6.1 生成信号

单事务内完成：

1. 插入或更新 `signals`
2. 插入 `signal_transitions`
3. 插入 `audit_logs`
4. 插入 `outbox_events`

### 6.2 审批并创建订单请求

单事务内完成：

1. 插入或确认 `idempotency_keys`
2. 更新 `signals`
3. 插入 `orders`
4. 插入 `audit_logs`
5. 插入 `outbox_events`

### 6.3 处理外部成交回报

单事务内完成：

1. 检查并插入 `external_event_dedup`
2. 更新 `orders`
3. 插入 `trades`
4. 更新 `positions`
5. 更新 `risk_state`
6. 插入 `audit_logs`

---

## 7. 首版迁移优先级

建议 migration 顺序：

1. 先建核心业务表。
2. 再建 `audit_logs`、`idempotency_keys`、`outbox_events`。
3. 再建 `external_event_dedup` 与 `llm_calls`。
4. 最后补充索引、条件唯一约束和冷热分层策略。
