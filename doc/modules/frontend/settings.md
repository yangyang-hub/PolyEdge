# Settings（设置）

最后更新：2026-07-15

`/settings` 只读取和修改 `/api/v1/system/runtime-state`。页面展示全局 kill switch、交易开关、原因、版本与审计更新时间，并用受保护确认对话框触发 `system_kill_switch_trigger` 或 `system_kill_switch_release`。锁定时强制关闭交易；释放时由操作员明确决定是否同时启用全局交易。

关键文件：`src/app/(console)/settings/page.tsx`、`src/features/settings/components/settings-workbench.tsx`、`src/lib/api/settings.ts`、`src/lib/api/actions/settings.ts`、`src/lib/contracts/dto/settings.ts`。

旧 runtime-config、Funding、新闻源健康和研究配置已删除。
