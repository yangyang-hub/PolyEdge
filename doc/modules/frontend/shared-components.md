# 共享组件

最后更新：2026-07-15

控制台继续复用 shadcn UI primitives、`PageHeader`、Card、Button、Input、`ActionDialog` 和状态组件。V3 活动导航只提供 `/dashboard`、`/strategies`、`/wallets`、`/operations`、`/settings`。

页面保持静态导出兼容；交互状态只存在于策略、钱包、执行和设置工作台叶子组件。已删除的 Funding、Markets、Events、Rewards 路由不会出现在桌面或移动导航。
