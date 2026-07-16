# Dashboard（仪表盘）

最后更新：2026-07-15

`/dashboard` 是 V3 控制台入口，并行读取策略、钱包、订单和执行批次，展示已配置策略、启用钱包、开放订单和待处理批次数量。快捷入口仅指向 `/strategies`、`/wallets`、`/operations`。

关键文件为 `src/app/(console)/dashboard/page.tsx` 和 `src/features/dashboard/components/dashboard-overview.tsx`。API 不可用时指标降级为占位符，不阻塞静态页面渲染。
