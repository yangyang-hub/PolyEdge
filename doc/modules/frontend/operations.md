# 执行运营

最后更新：2026-07-16

`/operations` 把一份统一策略批量应用到所选钱包，并查看批次、受管订单和持仓。

关键文件：`src/app/(console)/operations/page.tsx`、`src/features/operations/components/operations-workbench.tsx`、`src/lib/api/operations.ts`、`src/lib/api/actions/operations.ts`、`src/lib/contracts/dto/executions.ts`、`trading.ts`。

执行请求严格为 `strategy_id + wallet_ids + operator_note`；后端验证策略/订阅与钱包 ownership，并把 batch 固化到当前 subscription/version。批量撤单按可选 `wallet_ids + condition_ids` 过滤并按 actor scope 隔离。读请求并行加载 execution batches、orders 和 positions；管理员可全局读取，普通用户只读自己的记录。

确认对话框不再要求无效的 step-up code；后端以 recent-auth session 授权操作。超过 recent-auth TTL 后页面尚不会自动弹出 `/auth/reauth`，这是当前 UX 缺口。只读用户的操作按钮尚未在此页面隐藏，但后端会拒绝写入。
