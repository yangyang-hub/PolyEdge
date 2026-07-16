# Settings（设置）

最后更新：2026-07-16

`/settings` 读取和修改 `/api/v1/system/runtime-state`。页面展示全局 kill switch、交易开关、原因、版本与审计更新时间。锁定时强制关闭交易；释放时由操作员明确决定是否同时启用全局交易。

关键文件：`src/app/(console)/settings/page.tsx`、`src/features/settings/components/settings-workbench.tsx`、`src/lib/api/settings.ts`、`src/lib/api/actions/settings.ts`、`src/lib/contracts/dto/settings.ts`。

旧 runtime-config、Funding、新闻源健康和研究配置已删除。

当前页面仍要求输入旧 step-up code，后端实际以 admin role + recent-auth session 保护全局状态写入。后续可清理无效输入；最终授权始终由后端执行。
