# 共享组件

最后更新：2026-07-16

控制台复用现有组件。`AuthProvider` 读取 `/auth/me`、处理登录跳转、登出和管理员路由保护；导航新增 `/following`、`/admin/users` 与 `/admin/finance`，管理员项按角色过滤。策略/钱包写表单按 `admin|market_editor` 隐藏，只读用户仍可浏览。

页面保持静态导出兼容。`ActionDialog` 只收集操作备注和上下文，不再展示无效的 step-up code；危险操作的 recent-auth 由后端统一校验。operations/following 的写入口按 `admin|market_editor` 门控，settings 与 `/admin/*` 仅允许 admin，后端 RBAC 仍是最终写入边界。桌面和移动导航使用 Next `Link` 保持客户端路由。旧分页、占位展示组件和未使用 UI primitives 已删除。已删除的 Funding、Markets、Events、Rewards 路由不会出现在桌面或移动导航。
