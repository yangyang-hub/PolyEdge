# 用户认证与管理员控制台

最后更新：2026-07-16

`/login` 使用 HttpOnly cookie session 登录，`/activate#token=...` 从 URL fragment 消费一次性令牌并设置至少 12 字符的密码；fragment 不会进入 Nginx 请求日志。控制台通过 `/auth/me` 获取 `admin`、`market_editor` 或 `read_only` 用户；`AuthProvider` 在无会话时跳转登录，并阻止非管理员进入 `/admin/*`。

关键文件：`src/app/(auth)/login`、`src/app/(auth)/activate`、`src/components/shared/auth-provider.tsx`、`src/features/admin/`、`src/lib/api/auth.ts`、`admin.ts`、`src/lib/contracts/dto/auth.ts`。

`/admin/users` 已实现创建、列表、角色/状态修改、一次性展示带 fragment 的 activation link，并可为 pending local 用户重新签发（旧令牌同步失效）；环境管理员的角色和状态不可修改。`/admin/finance` 聚合操作性 partial equity snapshot 与外部 reward/fee cash-flow，缺失时显示“不完整”，不是权威实时盈利核算。

写请求统一携带 Cookie、CSRF header 和 Origin。后端危险操作使用 recent authentication；前端已有 `/auth/reauth` client，但尚未提供 reauth 对话框，也仍保留旧 step-up code 输入。最终授权始终由后端执行。
