# PolyEdge API 契约文档

## 1. 文档目标

本文档定义 PolyEdge 前端 `Next.js` 控制台与 Rust 后端之间的稳定契约，包括：

1. 资源命名规范。
2. DTO 结构。
3. 分页、过滤、排序协议。
4. 错误码体系。
5. 写操作语义。
6. SSE / WebSocket 实时消息格式。
7. 版本兼容策略。

本文件的目标不是罗列所有字段，而是定义一套可持续扩展、不会让前后端语义漂移的契约规则。

---

## 2. 契约原则

1. Rust 后端是业务真值来源。
2. API 契约优先于页面临时需要。
3. 写操作必须幂等。
4. 实时消息必须可重放、可去重。
5. 新字段只能向后兼容增加，不能无预警改变语义。

---

## 3. 命名与通用规范

### 3.1 资源命名

当前实现统一挂在 `/api/v1` 前缀下，资源使用复数名词：

1. `/api/v1/markets`
2. `/api/v1/events`
3. `/api/v1/evidences`
4. `/api/v1/signals`
5. `/api/v1/orders`
6. `/api/v1/positions`
7. `/api/v1/approvals`
8. `/api/v1/risk/state`
9. `/api/v1/risk/alerts`
10. `/api/v1/risk/buckets`
11. `/api/v1/system`
12. `/api/v1/news/source-health`
13. `/api/v1/stream/{channel}`

### 3.2 字段命名

1. JSON 字段统一使用 `snake_case`。
2. 时间统一使用 RFC 3339 UTC 字符串。
3. 金额、概率、价格统一使用字符串或定点小数字段约定，不使用不受控浮点展示值。
4. 枚举字段必须在契约中列出允许值。

### 3.3 通用元字段

关键资源建议统一包含：

1. `id`
2. `created_at`
3. `updated_at`
4. `version`
5. `trace_id`

详细枚举和值域、定点数 scale 与序列化规则见 [polyedge-domain-enums-and-decimals.md](./polyedge-domain-enums-and-decimals.md)。

---

## 4. 响应封装规范

### 4.1 查询响应

建议使用统一外层结构：

```json
{
  "data": {},
  "meta": {
    "request_id": "req_xxx",
    "trace_id": "trc_xxx",
    "generated_at": "2026-04-16T00:00:00Z"
  }
}
```

列表查询：

```json
{
  "data": [],
  "page": {
    "limit": 50,
    "next_cursor": "cursor_xxx",
    "has_more": true
  },
  "meta": {
    "request_id": "req_xxx",
    "trace_id": "trc_xxx",
    "generated_at": "2026-04-16T00:00:00Z"
  }
}
```

### 4.2 写操作响应

写操作建议返回操作结果而不是静默 `204`，便于前端刷新局部状态：

```json
{
  "data": {
    "accepted": true,
    "operation_id": "op_xxx",
    "resource_id": "sig_xxx",
    "status": "queued"
  },
  "meta": {
    "request_id": "req_xxx",
    "trace_id": "trc_xxx"
  }
}
```

---

## 5. 分页、过滤、排序协议

### 5.1 分页

统一采用 cursor pagination：

1. `limit`
2. `cursor`

示例：

```text
GET /api/v1/events?limit=50&cursor=evt_20260416_0001
```

### 5.2 过滤

过滤参数统一采用显式字段：

1. `status`
2. `market_id`
3. `event_id`
4. `signal_state`
5. `from`
6. `to`
7. `q`
8. `source_type`

多值过滤建议使用重复参数：

```text
GET /api/v1/signals?status=active&status=weakened
```

### 5.3 排序

统一使用：

1. `sort_by`
2. `sort_order=asc|desc`

例如：

```text
GET /api/v1/markets?sort_by=updated_at&sort_order=desc
```

---

## 6. 核心 DTO 建议

以下 DTO 只定义稳定字段骨架，细节字段后续可以扩展。

### 6.1 `MarketDto`

```json
{
  "id": "mkt_xxx",
  "question": "Will X happen before date Y?",
  "status": "open",
  "best_bid": "0.47",
  "best_ask": "0.49",
  "mid_price": "0.48",
  "volume_24h": "125000.00",
  "ambiguity_level": "medium",
  "tradability_status": "manual_review",
  "updated_at": "2026-04-16T00:00:00Z",
  "version": 12
}
```

### 6.2 `EventDto`

```json
{
  "id": "evt_xxx",
  "source": "reuters",
  "summary": "Official statement released",
  "relevance_score": "0.81",
  "confidence": "0.78",
  "status": "active",
  "related_market_ids": ["mkt_xxx"],
  "created_at": "2026-04-16T00:00:00Z",
  "version": 3
}
```

### 6.3 `EvidenceDto`

```json
{
  "id": "evd_xxx",
  "market_id": "mkt_xxx",
  "event_id": "evt_xxx",
  "direction": "supports_yes",
  "strength": "0.35",
  "source_reliability": "0.90",
  "novelty": "0.80",
  "resolution_relevance": "0.95",
  "status": "active",
  "expires_at": "2026-04-16T04:00:00Z",
  "version": 2
}
```

### 6.4 `SignalDto`

```json
{
  "id": "sig_xxx",
  "market_id": "mkt_xxx",
  "event_id": "evt_xxx",
  "action": "buy",
  "side": "yes",
  "market_price": "0.52",
  "fair_price": "0.58",
  "edge": "0.06",
  "confidence": "0.70",
  "lifecycle_state": "active",
  "reason": "official update implies underpriced yes",
  "updated_at": "2026-04-16T00:00:00Z",
  "version": 9
}
```

### 6.5 `RiskStateDto`

```json
{
  "id": "risk_state_global",
  "mode": "manual_confirm",
  "environment": "local",
  "kill_switch": false,
  "daily_pnl": "-125.50",
  "gross_exposure": "0.28",
  "net_exposure": "0.11",
  "open_alerts": 2,
  "daily_loss_limit": "5000.00",
  "daily_loss_used": "125.50",
  "updated_at": "2026-04-16T00:00:00Z",
  "version": 15
}
```

### 6.6 `ApprovalDto`

```json
{
  "id": "apr_signal_sig_xxx",
  "type": "signal",
  "severity": "warning",
  "owner": "Risk Engine",
  "resource_id": "sig_xxx",
  "summary": "Market requires manual confirmation.",
  "status": "pending",
  "requires_step_up_auth": true,
  "created_at": "2026-04-16T00:00:00Z",
  "updated_at": "2026-04-16T00:00:00Z",
  "version": 9
}
```

### 6.7 `RiskAlertDto`

```json
{
  "id": "alt_pending_signal_approvals",
  "severity": "warning",
  "reason": "1 signal approval item await operator review.",
  "target": "Approval Queue",
  "status": "watching",
  "created_at": "2026-04-16T00:00:00Z",
  "updated_at": "2026-04-16T00:00:00Z",
  "version": 9
}
```

### 6.8 `RiskBucketDto`

```json
{
  "id": "bucket_crypto",
  "name": "Crypto",
  "exposure": "0.28",
  "limit": "0.35",
  "utilization": "0.80",
  "status": "healthy",
  "updated_at": "2026-04-16T00:00:00Z",
  "version": 3
}
```

### 6.9 `NewsSourceHealthDto`

Returned by `GET /api/v1/news/source-health`.

```json
{
  "source": "sec_feed",
  "source_type": "official",
  "enabled": true,
  "reliability": "0.950000",
  "last_success_at": "2026-04-16T14:24:00Z",
  "last_error_at": null,
  "consecutive_failures": 0,
  "items_fetched": 18,
  "items_inserted": 12,
  "items_deduped": 6,
  "health_score": "0.950000",
  "last_error": null,
  "updated_at": "2026-04-16T14:24:00Z"
}
```

---

## 7. 错误码体系

### 7.1 错误响应结构

```json
{
  "error": {
    "code": "RISK_LIMIT_EXCEEDED",
    "message": "position exceeds configured market limit",
    "details": {
      "market_id": "mkt_xxx",
      "limit": "0.05"
    },
    "retryable": false
  },
  "meta": {
    "request_id": "req_xxx",
    "trace_id": "trc_xxx"
  }
}
```

### 7.2 错误码分类

1. `AUTH_*`
   认证失败，如 `AUTH_REQUIRED`、`AUTH_INVALID_SESSION`。
2. `PERMISSION_*`
   权限不足，如 `PERMISSION_DENIED`、`PERMISSION_STEP_UP_REQUIRED`。
3. `VALIDATION_*`
   请求格式错误，如 `VALIDATION_INVALID_FIELD`。
4. `RISK_*`
   风控拒绝，如 `RISK_LIMIT_EXCEEDED`、`RISK_KILL_SWITCH_ACTIVE`。
5. `STATE_*`
   状态机冲突，如 `STATE_INVALID_TRANSITION`。
6. `SYSTEM_*`
   系统错误，如 `SYSTEM_TIMEOUT`、`SYSTEM_DEPENDENCY_UNAVAILABLE`。
7. `LLM_*`
   模型治理错误，如 `LLM_SCHEMA_INVALID`、`LLM_TIMEOUT_FALLBACK_USED`。

### 7.3 前端处理原则

1. `AUTH_*` 引导重新登录。
2. `PERMISSION_*` 显示受限原因，不重试。
3. `RISK_*` 展示可解释的风险拒绝原因。
4. `SYSTEM_*` 允许安全重试，但写操作必须保留幂等键。

---

## 8. 写操作契约

### 8.1 幂等键

所有高价值写操作都必须带：

1. `Idempotency-Key`
2. `X-Request-Id`

典型写操作：

1. 审批信号。
2. 撤单。
3. 模式切换。
4. 触发或释放 kill switch。

### 8.2 条件写入

为避免脏写，建议写操作支持版本保护：

1. `If-Match-Version`
2. 或请求体中的 `expected_version`

### 8.3 写操作示例

```json
{
  "reason": "manual approval after ambiguity review",
  "expected_version": 9
}
```

---

## 9. SSE 契约

SSE 适合信号、风险、事件等控制台级实时状态流。

### 9.1 事件格式

```text
event: signal.updated
id: 2026-04-16T00:00:00Z_sig_xxx_9
data: {"signal_id":"sig_xxx","version":9,"lifecycle_state":"active","edge":"0.06"}
```

### 9.2 规则

1. `id` 必须全局可排序，支持断线续传。
2. `data` 内必须带 `resource_id` 或等价主键。
3. 实时 payload 只推送变更摘要，不推送超大对象。
4. 客户端可用 `Last-Event-ID` 断点续传。

### 9.3 推荐事件类型

1. `signal.created`
2. `signal.updated`
3. `signal.invalidated`
4. `risk.alerted`
5. `risk.mode_changed`
6. `approval.created`
7. `approval.updated`
8. `event.created`
9. `order.updated`

当前实现中的 `/api/v1/stream/{channel}` 是 snapshot-backed SSE：后端按间隔读取当前资源状态并输出稳定事件 ID，客户端应按 `id` 去重。后续若接入 outbox/Redis stream，应保持事件类型和 payload 兼容。

---

## 10. WebSocket 契约

WebSocket 主要保留给市场价格和盘口类高频流。

### 10.1 入站消息

```json
{
  "type": "subscribe",
  "channels": [
    {"name": "market_book", "market_id": "mkt_xxx"}
  ]
}
```

### 10.2 出站消息

```json
{
  "type": "market_book.update",
  "market_id": "mkt_xxx",
  "sequence": 10231,
  "best_bid": "0.47",
  "best_ask": "0.49",
  "updated_at": "2026-04-16T00:00:00Z"
}
```

### 10.3 客户端处理要求

1. 使用 `sequence` 去重和乱序保护。
2. 若发现断档，主动回退到 REST 快照拉取。
3. 不允许仅凭 WebSocket 增量长期维持真值而不校准。

---

## 11. 版本兼容策略

### 11.1 兼容规则

1. 新增字段允许。
2. 删除字段必须先废弃再移除。
3. 改变字段语义视为 breaking change。
4. breaking change 必须升级 API 版本或新 endpoint。

### 11.2 发布顺序

建议顺序：

1. 后端先发布兼容新旧字段的版本。
2. 前端切换到新字段。
3. 观测稳定后再清理旧字段。

### 11.3 契约来源

建议后续以 Rust `contracts` crate 作为后端源定义，再通过 OpenAPI 或代码生成同步前端类型。

---

## 12. 最小首版范围

首版最需要稳定的契约并不是全部接口，而是：

1. `markets`
2. `signals`
3. `risk/state`
4. `events`
5. `approveSignal`
6. `cancelOrder`
7. `switchMode`
8. `triggerKillSwitch`

把这些先稳定住，前后端就可以并行开工。
