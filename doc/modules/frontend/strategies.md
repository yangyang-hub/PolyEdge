# 市场策略

最后更新：2026-07-16

`/strategies` 是多用户市场录入工作台。策略记录 owner、`private | followable` 可见性和 `active_from/active_until` 有效窗口；只读用户只能查看。`/following` 允许用户用自己的钱包跟随可跟随策略。

关键文件：`src/app/(console)/strategies/page.tsx`、`src/features/strategies/components/strategies-workbench.tsx`、`src/lib/api/strategies.ts`、`src/lib/api/actions/strategies.ts`、`src/lib/contracts/dto/strategies.ts`。

创建请求使用 `{ name, visibility, active_from, active_until, market, version, wallet_ids, operator_note }`；`version.quote_slots` 支持任意数量 YES/NO 槽位，顶层 `wallet_ids` 绑定自动创建的 owner subscription。默认有效期为五小时，列表展示 owner、visibility、截止时间、published version、slots 和当前用户 subscription 钱包数。

当前页面只实现创建和列表，没有策略更新、暂停/恢复或倒计时交互。`/following` 可按源策略 ID 创建/list subscription；discover API 已存在，但尚未做成策略浏览选择器，也没有暂停/停止编辑 UI。页面不扫描市场，不提供 AI、新闻、事件或 fair-value 配置。
