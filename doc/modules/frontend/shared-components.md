# 共享组件

最后更新：2026-07-16

控制台复用现有组件。`AuthProvider` 读取 `/auth/me`、处理登录跳转、登出和管理员路由保护；导航新增 `/following`、`/admin/users` 与 `/admin/finance`，管理员项按角色过滤。策略/钱包写表单按 `admin|market_editor` 隐藏，只读用户仍可浏览。

页面保持静态导出兼容。`ActionDialog` 仍展示旧 step-up code 输入，尚未重构为 recent-auth 密码确认；operations/settings 也尚未完整按角色隐藏写按钮，安全边界由后端 RBAC 保证。已删除的 Funding、Markets、Events、Rewards 路由不会出现在桌面或移动导航。
