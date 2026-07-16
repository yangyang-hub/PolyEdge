# 市场策略

最后更新：2026-07-15

`/strategies` 是 V3 人工市场录入工作台。用户明确填写 condition、slug、YES/NO token、奖励参数、稳定 quote slot、固定份数、固定价或盘口档位、价格边界、撤换节奏和目标钱包。

关键文件：`src/app/(console)/strategies/page.tsx`、`src/features/strategies/components/strategies-workbench.tsx`、`src/lib/api/strategies.ts`、`src/lib/api/actions/strategies.ts`、`src/lib/contracts/dto/strategies.ts`。

创建请求严格使用 `{ name, market, version, operator_note }`；`version.quote_slots` 支持任意数量 YES/NO 槽位，`version.wallet_ids` 统一绑定目标钱包。列表展示后端返回的 market/strategy/published version/slots/targets 嵌套结构。页面不扫描市场，不提供 AI、信息过滤、新闻、事件或 fair-value 配置。
