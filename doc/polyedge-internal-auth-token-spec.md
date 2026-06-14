# PolyEdge 内部鉴权 Token 协议

> **状态（2026-06-14）**：本文是内部鉴权 token 协议草案。当前前端 `off` 会话不会签发可信令牌，生产级会话体系仍是缺口；当前状态以 [../AGENTS.md](../AGENTS.md) 和 [modules/backend/api-app.md](modules/backend/api-app.md) 为准。

## 1. 文档目标

本文档将 [polyedge-auth-design.md](./polyedge-auth-design.md) 中的“Next.js 向 Rust 传递短期签名 token”落到可实现协议，明确：

1. Token 格式。
2. 传输方式。
3. TTL 与时钟规则。
4. Rust 验签与授权校验步骤。
5. 密钥轮换与 session 吊销方式。

---

## 2. 适用范围

本协议仅用于：

1. Next.js Server Components 调 Rust 查询接口。
2. Next.js Server Actions 调 Rust 写接口。
3. 其他受信内部服务调 Rust API。

本协议不用于：

1. 浏览器直连 Rust API。
2. 第三方公开 API 访问。
3. 交易所或外部数据源认证。

---

## 3. 传输规则

### 3.1 必需请求头

每次 Next.js -> Rust 请求必须携带：

1. `Authorization: Bearer <internal_token>`
2. `X-Request-Id: <request_id>`

写操作额外必须携带：

1. `Idempotency-Key: <idempotency_key>`

### 3.2 客户端上下文转发

Next.js 应补充以下上下文头，供 Rust 审计使用：

1. `X-Client-IP`
2. `X-Client-User-Agent`

规则：

1. Rust 只在调用方来自受信 BFF 或内网网段时信任这些头。
2. 浏览器传入的同名头必须在 Next.js 侧重写，不允许原样透传。

---

## 4. Token 格式

### 4.1 选择

首版建议使用：

1. JWT Compact Serialization
2. JWS 签名
3. `alg=EdDSA`
4. Ed25519 密钥对

原因：

1. 足够轻量。
2. 前后端生态成熟。
3. 公钥分发和轮换相对简单。

### 4.2 Header

必须包含：

1. `alg`
2. `kid`
3. `typ`

示例：

```json
{
  "alg": "EdDSA",
  "kid": "bff-key-2026-04-16",
  "typ": "JWT"
}
```

### 4.3 Claims

#### 标准 claims

1. `iss`
   固定为受信 BFF 标识，如 `polyedge-nextjs`
2. `aud`
   固定为 Rust API 标识，如 `polyedge-rust-api`
3. `sub`
   终端用户 `user_id`
4. `iat`
   token 签发时间
5. `nbf`
   token 生效时间
6. `exp`
   token 失效时间
7. `jti`
   token 唯一 ID

#### 自定义 claims

1. `session_id`
2. `roles`
3. `auth_time`
4. `request_id`
5. `step_up_verified`
6. `step_up_scope`
7. `step_up_until`

示例 payload：

```json
{
  "iss": "polyedge-nextjs",
  "aud": "polyedge-rust-api",
  "sub": "usr_123",
  "iat": 1776295200,
  "nbf": 1776295200,
  "exp": 1776295260,
  "jti": "jit_abc123",
  "session_id": "sess_456",
  "roles": ["operator"],
  "auth_time": 1776291000,
  "request_id": "req_789",
  "step_up_verified": true,
  "step_up_scope": ["signal.approve"],
  "step_up_until": 1776295800
}
```

---

## 5. TTL 与时钟规则

### 5.1 Token TTL

首版建议：

1. 查询 token：`exp - iat <= 60s`
2. 普通写 token：`exp - iat <= 30s`
3. 高风险写 token：仍使用单请求短 token，不单独拉长 token 生命周期

### 5.2 Step-up 窗口

1. `step_up_verified=true` 只说明当前请求可声明 step-up 状态，不等于永久有效。
2. `step_up_until` 距离 step-up 发生时间不得超过 10 分钟。
3. BFF 必须为每次请求重新签发短 token，而不是复用长时间 token。

### 5.3 时钟容差

Rust 验证时允许：

1. `iat` / `nbf` / `exp` 的时钟漂移不超过 30 秒

超过该窗口应视为无效 token。

---

## 6. Rust 侧验证顺序

Rust 后端必须按以下顺序验证：

1. `Authorization` 头存在且格式正确。
2. `alg` 必须为 `EdDSA`，拒绝算法降级。
3. `kid` 存在且属于当前可接受公钥集合。
4. 签名有效。
5. `iss` 与 `aud` 精确匹配。
6. `nbf <= now <= exp`。
7. `sub`、`session_id`、`request_id`、`roles` 不为空。
8. 请求头 `X-Request-Id` 必须与 claim 中 `request_id` 一致。
9. `session_id` 未被吊销。
10. endpoint 所需角色必须包含在 `roles` 中。
11. 若为高风险操作，要求：
    - `step_up_verified=true`
    - `step_up_until >= now`
    - `step_up_scope` 包含当前动作

---

## 7. 动作与 Step-up Scope

首版 scope 名称固定如下：

1. `signal.approve`
2. `order.cancel.force`
3. `system.mode.switch`
4. `system.kill_switch.trigger`
5. `system.kill_switch.release`
6. `risk.threshold.update`

授权规则建议：

1. `viewer` 只允许查询，无 step-up 写能力。
2. `operator` 可执行 `signal.approve`、`order.cancel.force`，但必须满足 step-up。
3. `risk_admin` 可执行模式切换、kill switch 和风险阈值变更，且必须满足 step-up。
4. `admin` 可拥有全部 scope，但仍必须满足高风险 step-up。

---

## 8. Session 吊销

Token 有效不是充分条件，Rust 还必须检查 session 状态。

首版建议：

1. Next.js 登录态真值仍在 session 层。
2. Session 吊销事件同步到 Rust 可访问的 revocation store。
3. Revocation store 可由 PostgreSQL 真值表 + Redis 热缓存组成。

最小检查逻辑：

1. 若 `session_id` 已吊销，则拒绝请求。
2. 若 `auth_time` 早于账号强制重新认证时间，则拒绝请求。
3. 若 step-up 状态已失效，则即使 token 未过期也拒绝高风险操作。

---

## 9. 密钥管理与轮换

### 9.1 分工

1. Next.js 持有 Ed25519 私钥。
2. Rust 只持有公钥集合。
3. 私钥不得出现在浏览器、前端 bundle 或 Rust API 进程中。

### 9.2 `kid` 规则

1. 每个可用公钥必须有唯一 `kid`。
2. BFF 总是使用“当前 active key”签名。
3. Rust 必须至少同时接受“当前 key + 上一个 key”。

### 9.3 轮换窗口

建议：

1. 新 key 切换后，旧 key 至少保留 15 分钟可验证窗口。
2. 待旧 token 全部自然过期后，再从 Rust 验签列表移除旧 key。

---

## 10. 错误映射

Rust 建议按以下方式映射错误：

1. 签名无效或 `kid` 不存在：`AUTH_INVALID_INTERNAL_TOKEN`
2. `exp` 过期：`AUTH_TOKEN_EXPIRED`
3. `iss` / `aud` 不匹配：`AUTH_INVALID_AUDIENCE`
4. `session_id` 已吊销：`AUTH_INVALID_SESSION`
5. 角色不足：`PERMISSION_DENIED`
6. 缺少 step-up：`PERMISSION_STEP_UP_REQUIRED`
7. `step_up_scope` 不覆盖当前动作：`PERMISSION_STEP_UP_SCOPE_INVALID`

---

## 11. 首版实现建议

首版建议先落地以下闭环：

1. Next.js 每请求签发短 JWT。
2. Rust 统一鉴权中间件完成验签、claim 解析和 request context 注入。
3. `X-Request-Id` 与 claim 对齐校验。
4. 写请求叠加 `Idempotency-Key`。
5. 高风险动作按 `step_up_scope` 做细粒度校验。
6. Session 吊销信息同步到 Rust 可查询的 revocation store。
