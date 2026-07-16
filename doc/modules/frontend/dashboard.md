# Dashboard（仪表盘）

最后更新：2026-07-16

`/dashboard` 是认证后的控制台入口，并行读取当前 actor 可见的策略、钱包、订单和执行批次，展示已配置策略、启用钱包、开放订单和待处理批次数量。普通用户只看到自己的租户数据；管理员由后端获得全局视图。快捷入口目前仍只指向 `/strategies`、`/wallets`、`/operations`，尚未加入 following/admin 卡片。

关键文件为 `src/app/(console)/dashboard/page.tsx` 和 `src/features/dashboard/components/dashboard-overview.tsx`。API 不可用时指标降级为占位符，不阻塞静态页面渲染。
