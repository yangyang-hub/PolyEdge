# PolyEdge 鉴权与会话设计文档

> **状态（2026-06-14）**：本文是鉴权设计背景，不代表当前生产级会话已完成。当前仓库只保留前端 `off` 会话模式；仓库状态以 [../AGENTS.md](../AGENTS.md)、[../README.md](../README.md) 和 [modules/](modules/README.md) 为准。

## 1. 文档目标

本文档定义 PolyEdge 的身份认证、会话管理、权限模型、高风险操作保护和审计要求。

目标是让 Next.js 控制台和 Rust 后端之间形成一条清晰、可审计、可扩展的身份链路，而不是临时拼装 cookie、header 和页面权限判断。

---

## 2. 设计目标

1. 浏览器不直接持有任何交易密钥。
2. Rust 后端始终是权限真值来源。
3. Next.js 负责承载用户会话和页面体验。
4. 高风险操作必须具备二次保护。
5. 所有敏感操作都能追溯到具体用户和会话。

---

## 3. 角色模型

### 3.1 基础角色

1. `viewer`
   只读访问。
2. `operator`
   可以审批、撤单、查看风险详情。
3. `risk_admin`
   可以切换模式、触发/释放 kill switch、调整运行限制。
4. `admin`
   可以管理系统配置和用户授权。

### 3.2 权限原则

1. 权限按最小必要集分配。
2. 写权限和读权限分离。
3. 高风险能力单独授权，不随普通 `operator` 默认下发。

---

## 4. 会话架构

推荐采用：

```text
Browser <-> Next.js Session Layer <-> Rust API
```

### 4.1 浏览器与 Next.js

1. 浏览器只持有安全 session cookie。
2. cookie 设置：
   - `HttpOnly`
   - `Secure`
   - `SameSite=Lax` 或更严格
3. 前端页面和 Server Actions 通过该 session 获取用户身份。

### 4.2 Next.js 与 Rust

1. 浏览器不直接调用高权限 Rust 写接口。
2. Server Components 与 Server Actions 在服务端调用 Rust API。
3. Next.js 调 Rust 时必须携带短期服务端身份凭证和终端用户上下文。

### 4.3 推荐传递方式

建议由 Next.js 生成短期签名 token，声明：

1. `user_id`
2. `session_id`
3. `roles`
4. `auth_time`
5. `request_id`
6. `step_up_verified`

Rust 只信任签名合法且未过期的内部 token。
具体 header、claims、TTL、scope 和密钥轮换规则见 [polyedge-internal-auth-token-spec.md](./polyedge-internal-auth-token-spec.md)。

---

## 5. 认证链路

### 5.1 登录流程

```text
user login
-> Next.js 完成认证
-> 建立 session
-> 写入 session_id / user_id / roles / auth_time
-> 浏览器获得安全 cookie
```

### 5.2 页面读取流程

```text
browser request page
-> Next.js Server Component 读取 session
-> 调用 Rust 查询 API
-> Rust 校验 token 与角色
-> 返回资源数据
```

### 5.3 写操作流程

```text
browser submit action
-> Next.js Server Action 校验 session + CSRF
-> 生成 request_id + idempotency key
-> 调用 Rust 写 API
-> Rust 校验 token / role / step-up state
-> 执行业务操作并写 audit log
```

---

## 6. 高风险操作保护

以下操作必须视为高风险：

1. 信号人工放行。
2. 强制撤单。
3. 运行模式切换。
4. 触发或释放 kill switch。
5. 修改风险阈值。

### 6.1 保护要求

1. 角色必须满足要求。
2. 需要 `step_up_verified=true`。
3. 需要显式输入原因 `reason`。
4. 必须写入审计日志。

### 6.2 Step-up Auth 建议

对于高风险动作，建议增加二次校验：

1. 短期重新认证。
2. WebAuthn 确认。
3. 一次性确认 token。

该确认应具有短期有效期，例如 5 到 10 分钟，仅覆盖有限操作范围。

---

## 7. CSRF、重放与误触保护

### 7.1 CSRF

Server Actions 或 BFF 路由必须：

1. 校验同源来源。
2. 使用 session 绑定的 anti-CSRF token。
3. 拒绝缺失或不匹配请求。

### 7.2 重放保护

所有写操作都应携带：

1. `Idempotency-Key`
2. `X-Request-Id`
3. 可选 `Action-Nonce`

Rust 端应对短时间内的重复 key 进行去重。

### 7.3 误触保护

高风险按钮必须具备：

1. 明确文案。
2. 二次确认弹层。
3. 当前影响范围摘要。
4. 成功/失败的审计反馈。

---

## 8. Rust 后端鉴权模型

### 8.1 真值原则

Rust 后端应独立完成：

1. token 验签。
2. 角色校验。
3. step-up 状态校验。
4. 审计记录。

后端不能只依赖前端把按钮隐藏掉来做权限控制。

### 8.2 授权判定

授权判定建议由统一中间件或授权服务完成，避免散落在 handler 中。

最小校验字段：

1. `user_id`
2. `session_id`
3. `roles`
4. `auth_time`
5. `step_up_verified`
6. `token_exp`

---

## 9. Session 生命周期

### 9.1 会话字段

建议会话至少包含：

1. `session_id`
2. `user_id`
3. `roles`
4. `created_at`
5. `last_seen_at`
6. `auth_time`
7. `step_up_until`
8. `revoked_at`

### 9.2 生命周期规则

1. 长时间不活跃会话自动过期。
2. 高风险 step-up 状态短时过期。
3. 模式切换或管理操作应刷新审计上下文，不应无限复用旧 step-up。
4. 关键账号支持主动吊销所有 session。

---

## 10. 审计要求

所有敏感操作必须记录：

1. `user_id`
2. `session_id`
3. `request_id`
4. `action`
5. `resource_id`
6. `reason`
7. `result`
8. `trace_id`
9. `ip` 或来源标识
10. `user_agent` 摘要

### 10.1 审计事件示例

1. `auth.login`
2. `auth.logout`
3. `signal.approve`
4. `order.cancel`
5. `system.mode.switch`
6. `system.kill_switch.trigger`
7. `system.kill_switch.release`

---

## 11. 前后端职责划分

### Next.js 负责

1. 用户登录体验。
2. session cookie 承载。
3. Server Components / Server Actions 读取 session。
4. 发起到 Rust 的受控调用。

### Rust 负责

1. 权限真值判定。
2. 高风险动作最终放行。
3. 审计落库。
4. 幂等与状态机保护。

---

## 12. 首版推荐实现

首版不必一开始就做最复杂的企业 IAM，但至少应落地：

1. 安全 session cookie。
2. 角色模型。
3. Server Actions 写操作代理。
4. Rust 端统一鉴权中间件。
5. 高风险操作 step-up。
6. 审计日志落库。

这六项是从“能登录”升级到“能安全操作交易系统”的最小闭环。
