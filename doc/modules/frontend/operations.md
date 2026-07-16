# 执行运营

最后更新：2026-07-15

`/operations` 把一份统一策略批量应用到所选钱包，并查看批次、受管订单和持仓。

关键文件：`src/app/(console)/operations/page.tsx`、`src/features/operations/components/operations-workbench.tsx`、`src/lib/api/operations.ts`、`src/lib/api/actions/operations.ts`、`src/lib/contracts/dto/executions.ts`、`trading.ts`。

执行请求严格为 `strategy_id + wallet_ids + operator_note`，使用 `execution_submit` scope。批量撤单按可选 `wallet_ids + condition_ids` 过滤，使用 `order_cancel_force` scope。两类危险操作都通过确认对话框提交 step-up code；读请求并行加载 execution batches、orders 和 positions。
