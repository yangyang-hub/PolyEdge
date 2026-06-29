# Rewards（奖励机器人）

最后更新：2026-06-29

## 概述

`/rewards` 页面管理做市奖励机器人的生命周期：配置策略参数、向 worker 提交运行/取消/重置命令、查看订单/持仓/事件。仅支持实盘（live）模式。

## 架构与关键文件

| 文件 | 职责 |
|---|---|
| `src/app/(console)/rewards/page.tsx` | 路由页面 |
| `src/features/rewards/components/rewards-workbench.tsx` | 主工作台编排：状态/操作区、指标条、活动/配置/风控 tabs |
| `src/features/rewards/components/rewards-overview-cards.tsx` | 顶部执行概览、每日大模型调用统计（总数为真实外部请求，AI/风控为 section 计数）、操作中心和关键指标条 |
| `src/features/rewards/components/rewards-config-panel.tsx` | 分组策略配置面板（执行、市场筛选、机会评分、报价构造、成交后合并、盘口选择、AI 建议/信息风险、库存与控制） |
| `src/features/rewards/components/rewards-opportunity-config.tsx` | 统一机会评分配置：竞争倍数、100U 日奖、资金占比、退出深度/滑点、盘口稳定性和评分权重 |
| `src/features/rewards/components/rewards-opportunity-summary.tsx` | Quote plan 表格中的机会评分、score adjustment、竞争倍数、100U 日奖、退出深度和样本数摘要 |
| `src/features/rewards/components/rewards-advanced-config.tsx` | 盘口选择、AI advisory strategy hint 和信息风险配置子面板 |
| `src/features/rewards/components/rewards-config-fields.tsx` | 配置面板共享字段、区块和提示组件 |
| `src/features/rewards/components/rewards-tables.tsx` | 报价计划和托管订单表格入口；展示 strategy profile、AI/info-risk 二值结果与 AI direction/rank/notional hint；重导出成交、持仓、事件表，合并服务端分页状态构造 |
| `src/features/rewards/components/rewards-fills-table.tsx` | 成交记录表格 |
| `src/features/rewards/components/rewards-positions-table.tsx` | 持仓表格与 PnL 展示 |
| `src/features/rewards/components/rewards-events-table.tsx` | 风险/操作事件表格 |
| `src/features/rewards/components/rewards-table-controls.tsx` | 表格排序指示、搜索输入和分页筛选 tabs 共享控件 |
| `src/features/rewards/components/rewards-events-panel.tsx` | 事件面板 |
| `src/features/rewards/components/number-input.tsx` | 数值输入组件 |
| `src/features/rewards/loaders/rewards-page-data.ts` | 服务端数据装配 |
| `src/features/rewards/lib/rewards-helpers.ts` | 辅助函数：readiness/tone、事件分类和 AI strategy hint metrics 解析 |
| `src/features/rewards/types.ts` | 类型定义（数值配置 key、事件类别） |

## 核心类型（types.ts）

- **`NumberConfigKey`**：数值输入参数的字符串联合类型 — `max_markets`、`max_open_orders`、`min_daily_reward`、`min_market_liquidity_usd`、`min_market_volume_24h_usd`、`min_hours_to_end`、`max_market_spread_cents`、`max_market_data_age_minutes`、统一机会评分 `opportunity_*` 数值阈值和权重、dominant 单边阈值、盘口集中度阈值、AI advisory TTL、AI strategy hint 最低置信度、信息风险 TTL、事件窗口 stop/cancel/resume 秒数、首单观察窗口、`account_capital_usd`、`requote_drift_cents` 及 drift 换价 guard 秒数/限速字段（`requote_drift_confirm_sec`/`cooldown_sec`/`max_cancels_per_cycle`，均在报价构造配置区可编辑）、BalancedMerge 的 `balanced_merge_*` 数值阈值等；`per_market_usd`、`quote_size_usd` 和 `low_competition_per_market_usd` 已从前端 `RewardBotConfigDto` 移除，仅后端兼容字段；`quote_bid_rank`、`balanced_merge_quote_bid_rank`、quote/selection mode、AI provider/request format、事件窗口置信度/模式和信息风险 mode/等级使用受限下拉框，不进入该联合类型
- **`EventCategory`**：`"all" | "placements" | "cancels" | "fills" | "rewards"`

## API 依赖

- `src/lib/api/rewards.ts` — `readRewardBotSnapshot`、`updateRewardBotConfig`、`runRewardBotOnce`、`cancelRewardBotOrders`、`resetRewardBot`
- `readRewardBotSnapshot()` 会传递计划/订单分页、搜索、状态和排序 query；首屏明确请求 `plans_eligible=true`，与默认选中的“可挂”页签一致。后端分页结果和 `orders_page` 都描述本地 managed orders，不再用 Polymarket live open orders 覆盖
- 后端 snapshot 不返回全量 reward markets；页面使用 `status.markets_tracked`、`status.eligible_markets`、`status.ready_quote_markets`、`status.waiting_orderbook_markets`、`status.provider_pending_markets`、`status.blocker_counts`、`quote_plans[].opportunity_metrics` 和 `llm_usage` 展示市场覆盖、最终可挂市场、真实可立即报价数量、等待 provider、资金不足、live 盘口验证、AI/信息风控拦截、竞争/奖励/退出/稳定性机会评分以及 combined provider 每日真实外部请求数与 AI/info-risk section 计数。`low_competition_report` 仍在 DTO 中兼容历史响应，但当前后端返回 `null`，页面不再渲染低竞争报告
- snapshot 的 `available_usd` / `positions` 来自 worker 写入数据库的账户快照；API 不持有 Polymarket 私钥，也不直接请求外部账户数据。`available_usd` 优先使用 CLOB `balance-allowance`，当 CLOB 返回 0 或失败但资金钱包链上 pUSD 余额大于 0 时，worker 使用链上 pUSD 回填

## 关键交互

- **Run** → `runRewardBotOnce()` → API 写入 `run_once` 控制命令，worker 领取后执行一轮 live 策略
- **Cancel open orders** → `cancelRewardBotOrders()` → API 写入 `cancel_all` 控制命令，worker 领取后撤销 Polymarket live 托管订单
- **Reset** → `resetRewardBot()` → API 写入 `reset` 控制命令，worker 领取后按 cancel-all 撤销 live 订单
- **Config 编辑** → `updateRewardBotConfig(patch)` → 即时更新配置
- **挂单档位** → `quote_bid_rank=1|2|3` → 分别选择买一/买二/买三；后端只在 live placement 准备挂单时用当前盘口验证目标档位，不在 quote plan 构建阶段提前淘汰市场
- **漂移换价** → `requote_drift_cents` 决定是否进入换价候选；`requote_drift_confirm_sec`（同向确认窗口）、`requote_drift_cooldown_sec`（订单冷却）和 `requote_drift_max_cancels_per_cycle`（单轮最大 drift 撤单数）现在都在报价构造配置区与 `requote_drift_cents` 一起可编辑，避免盘口档位抖动导致全量撤空后再重挂
- **盘口选择** → `quote_mode=double|auto` + `selection_mode=observe|enforce` → 默认只保留双边报价；auto/enforce 的初步计划只用概率区间决定单边/双边，退出深度、盘口集中度、双边点差/档位/安全边际和单腿回退在 live placement 阶段用当前 orderbook 验证
- **成交后合并策略** → `balanced_merge_enabled` 默认关闭；开启后后端用独立 `balanced_merge_*` 市场/订单/edge/库存阈值追加 `balanced_merge` profile 候选，同一 condition 与标准策略冲突时标准策略优先。该 profile 固定 YES/NO 双边买单，要求两侧买价合计满足 edge；一侧成交后不生成 SELL、不撤对侧 BUY，两侧库存可配对后只写入 `unsupported` merge intent，当前不自动链上 CTF merge。
- **AI/事件窗口配置** → 保存 provider（OpenAI-compatible/Anthropic）、request format、TTL、AI strategy hint 开关和最低置信度；GLM/DeepSeek 通过 OpenAI-compatible base URL 与模型名配置，后端按模型名归一为 Chat Completions；worker 启用且环境变量配置 provider key 后，会在 full tick 中按单 market 刷新 combined provider 缓存，同一 condition 的 AI advisory 与 info-risk 都到期时合并为一次外部请求。strategy hint 达标时会直接约束实盘方向、挂单挡位和 condition 金额上限，但仍不能突破后端硬风控。该面板也保存事件窗口开关、最低置信度、赛前停止新增、赛前撤 BUY、赛后恢复冷却、未知事件时间处理和 Gamma 未审核日期处理
- **信息风险配置** → 保存启用开关、observe/enforce、过滤等级、TTL、首单信息风险要求和首单观察窗口；AI advisory 开启时由 full tick 的 combined provider refresh 一并刷新，AI advisory 关闭时才由独立 info-risk worker 异步刷新。页面只展示二值“允许挂单/不允许挂单”、置信度和摘要；enforce 模式下缺少未过期风险缓存的计划会被后端置为不可挂，新 condition 首次 BUY 还可要求先命中信息风险缓存并观察一段时间
- **大模型调用统计** → 顶部执行概览读取 `snapshot.llm_usage`，展示 UTC 今日真实外部 provider 请求总数，以及最近 7 天 AI advisory section、info-risk section 和失败请求计数；该统计来自 worker 写入的实际外部 provider 调用，不包含缓存命中，combined 请求同时携带两个 section 时总数只加 1、两个 section 列各加 1
- **成交后策略** → 页面可选择 `exit_at_markup` / `hold_and_requote` / `flatten_immediately`；`exit_at_markup` 的退出加价相对被吃买单原价计算，`hold_and_requote`（持有并续挂）按被吃买单原价生成 SELL 退出 floor 并继续正常报价；后端提交 SELL 前只看当前 orderbook best bid，best bid 不低于 floor 时用非 post-only FAK/taker SELL 按 best bid 退出，best bid 低于 floor 时保留 intent 等待非亏损退出
- **市场质量** → 可配置最低流动性、最低 24h 成交量、最短剩余结算时间、最大 Gamma spread 和最大目录同步年龄；后端还固定拒绝高歧义、非唯一 YES/NO、FDV/launch/token/official-result 等高跳变事件风险市场
- **机会评分** → 页面提供统一 `opportunity_*` 配置，把竞争倍数、预估 100U 日奖、账户/单市场资金占比、退出深度、入场退出滑点、坏成交恢复天数、盘口样本、中点波动、top-of-book 跳变和评分权重交给同一套指标；quote plan 表格展示机会分、score adjustment、竞争倍数、100U 日奖、退出深度、样本数和警告数量。该配置适用于所有奖励市场，不再区分低竞争/普通市场，也不再提供 observe/enforce 低竞争报告。
- 事件面板支持按 `EventCategory` 过滤
- 页面默认展示活动视图：报价计划全宽优先，订单/库存下方分栏，事件/成交流使用独立卡片；策略配置和风控配置通过 tabs 切换，减少实盘盯盘时的配置噪音。
- 报价计划、订单原因、信息风险摘要、AI reason、事件消息和长账户/钱包字段允许换行，表格在窄屏使用横向滚动，避免关键长文本被一行省略和短卡片被高表格行强制拉伸。
- 筛选刷新使用单调请求序号，只接收最新 REST 响应，避免快速搜索/翻页时旧请求覆盖新状态；页面每 10 秒静默刷新当前 snapshot，让 worker 刚写入的 AI advisory、信息风险、订单和余额状态自动反映到表格；静默自动刷新读取失败时保留现有 snapshot 并等待下一轮，不进入页面反馈栏，用户主动筛选/操作触发的读取失败仍会反馈，且不会产生未处理 Promise。

## 数据流

所有 mutation 通过 Server Actions。配置保存会立即返回更新后的 `RewardBotSnapshotDto`；Run / Cancel / Reset 只表示命令已入队，返回的是入队后的当前 snapshot，实际外部订单变化会在 worker 处理命令后由数据库 snapshot 反映。

## i18n

使用 `rewards` 命名空间字典。

## 当前状态

- 完整的 Run / Cancel / Reset 入队交互
- 顶部执行概览展示实盘模式、启停/运行状态、实时可报价比例、钱包余额/策略上限比例、最近扫描/运行时间、事件触发计数和每日大模型调用统计；关键指标条把 `status.ready_quote_markets` 显示为“实时可报价”，把 `status.eligible_markets` 显示为“最终可挂”，并单独展示候选计划总量、已拦截计划、等待 AI/信息风险、资金不足、live 盘口验证和 AI/信息风控拦截数量，避免把资金或 provider gate 抖动误读成 reward 市场池大幅变化。策略上限直接读取当前 `snapshot.config.account_capital_usd`，不再使用可能保留历史初始值的账户账本字段，也不代表链上钱包余额。
- 操作中心集中 Run / Save / Cancel / Reset，文案提醒当前命令可能提交或取消 Polymarket 实盘订单。
- 配置编辑按执行、市场筛选、机会评分、报价构造、成交后合并、盘口选择、AI 建议、库存与控制分组，包含仍生效的数值参数、布尔开关、受限下拉框和成交后策略；退出加价提示明确 0 表示原价卖，合并策略文案明确当前只生成 intent、不自动链上执行。
- 市场筛选面板公开质量硬门槛；通过门槛的市场由后端继续按奖励、流动性、成交量、剩余时长和奖励 spread 综合排序。
- 低竞争市场 sleeve UI 已移除并合并为统一机会评分：前端新增 `RewardOpportunityMetricsDto`、`opportunity_*` 配置校验、机会评分配置面板和 quote plan 行内摘要；默认机会评分基线为 10U 探针、100U 日奖最低 0.75、竞争倍数上限 4、账户/单市场占用警告 1500/500 bps、退出深度至少 60U 或计划名义额 2.5 倍、入场退出滑点 2c、坏成交恢复 3 天、30 分钟观察窗口至少 30 个盘口样本、中点波动 3c、top-of-book 跳变 8 次，评分权重为 reward/competition/exit/stability = 35/30/25/10。保存配置时会把旧 `low_competition_mode` 强制为 `off`，独立市场/订单/全局占比置 0，并关闭/清零旧低竞争 liquidity/volume 过滤字段。DTO 中仍保留 `low_competition_*`、`strategy_bucket=low_competition`、`low_competition_metrics` 和 `low_competition_report` 以兼容历史后端响应，但页面不再提供低竞争配置、观察面板或专用表格摘要。
- 报价构造使用“挂单档位”下拉框选择买一/买二/买三，不再提供中间价“报价偏移”、`per_market_usd`“单市场额度”或 `quote_size_usd`“单腿金额”；默认买一。
- 成交后合并配置公开 `balanced_merge_enabled`、独立最大市场/订单数、最小 edge、最低评分、低成交量/流动性门槛、最大市场价差、独立挂单档位和单侧未配对库存上限；报价计划和托管订单表格用策略标签区分“标准”和“合并” profile，避免把两套撤单/成交语义混在一起。
- 盘口选择公开 quote/selection mode、dominant 单边概率区间、退出深度、top1/top3 买盘集中度、HHI 和偏好分类评分加成；默认 `double + observe` 不改变既有双边挂单。
- AI 建议面板保存 OpenAI-compatible/Anthropic provider、请求格式、advisory TTL、AI strategy hint 开关/最低置信度、信息风险启用、observe/enforce、过滤等级、信息风险 TTL、事件窗口配置、首单信息风险要求和首单观察窗口；GLM/DeepSeek 不作为独立下拉项，而是通过 OpenAI-compatible base URL 和模型名配置，后端会在模型名包含 `glm`/`deepseek` 时改用 Chat Completions + JSON object。API key、base URL、模型名、请求超时和 web search 开关只来自 worker 环境变量，不会出现在前端配置或 snapshot。AI advisory 与信息风险扫描由 worker 的 combined provider refresh 覆盖当前候选，优先开放订单、持仓和可挂 quote plan；后端对同一 condition 使用一次 provider 调用同时返回到期的 advisory/info-risk section，不再提供多市场 batch 配置。事件窗口配置默认启用，默认只对 high confidence event time 执行 hard gate；StopNewQuotes 阻断新增 BUY，CancelOpenBuys/InEventWindow/PostEventCooldown 触发 BUY 撤单。报价计划 DTO 已包含 `event_window` assessment，当前表格主要通过 reason/readiness 展示阻断结果；AI advisory 和信息风险都只展示“允许挂单/不允许挂单”、confidence 和首条 reason/summary，并在 advisory metrics 含 `strategy_hint` 时显示 AI 建议方向、挡位和金额上限；信息风险 enforce 且缓存缺失时，后端会把对应计划显示为不可挂；首单 gate 只影响没有开放订单/库存的新 condition。
- 后端不再用 `per_market_usd`、`quote_size_usd` 或 `low_competition_per_market_usd` 限制报价腿构造；live materializer 只保障按 CLOB 成本精度对齐后的 `rewards_min_size` 和 Polymarket 1 美元最小名义金额。新增报价是否可挂由后端按实际钱包余额、未归属外部 BUY notional 和同一 condition 已有 managed BUY notional 判断；待补最低 rewards size 腿放不下时，quote plan 会显示 funding 不可挂原因，等后续余额/开放订单同步后重新评估。
- 配置不包含 `execution_mode` 选择器（始终为 live）。`per_market_usd`、`quote_size_usd`、`low_competition_per_market_usd` 已从前端 `RewardBotConfigDto` 完全移除（不再展示、提交或出现在类型里），仅后端配置兼容历史快照与旧请求。提示说明 `max_markets=0` 或 `max_open_orders=0` 会停止新挂单。
- 报价计划默认展示当前通过非盘口依赖过滤且等待 live 盘口验证的可挂市场；每条计划携带 `quote_readiness=ready_to_quote|waiting_orderbook|provider_pending|blocked`，表格状态列优先展示“可报价 / 等待盘口 / 等待 AI/信息风险 / 已拦截”。若准备挂单时 `quote_bid_rank`、rewards spread、盘口集中度、退出深度或安全边际导致双边不可行，auto/enforce/dominant 会先尝试单腿回退；没有可行单腿时后端才会把计划标记为不可挂并返回原因和 12 小时 `live_skip_until`，到期后自动重新评估。
- Managed orders 表格发送后端分页/搜索/状态过滤/排序 query（默认每页 15 条），表格数据与 `orders_page` 均来自本地 managed-order 查询；“已成交”状态筛选包含 `filled_size > 0` 的部分成交订单。
- 报价计划和托管订单共享同一段服务端分页状态构造；成交、持仓和事件表已拆为独立文件，`rewards-tables.tsx` 保留原重导出入口，避免调用方重复 import 调整。
- 报价计划和订单搜索框使用独立防抖输入组件；外部 query 重置通过组件 key 同步，不在 React effect 中同步 setState。
- Rewards 工作台在保留当前搜索、筛选、排序和分页条件的前提下，每 10 秒通过 REST 重新读取 snapshot；自动刷新不显示过滤 loading 状态，短暂网络失败不覆盖页面反馈，手动筛选仍显示轻量刷新状态并反馈失败。
- 首屏不加载全量 reward markets，避免奖励市场数量过大时长时间停留在 loading skeleton。
- Wallet balance、Positions 和 Orders 表格展示 worker 同步到数据库的 rewards 账户视图；余额显示资金钱包 pUSD，资金钱包地址优先使用 `POLYEDGE_POLYMARKET__FUNDER`，未配置时使用 `ACCOUNT_ID`。
- 库存（持仓）表展示买一、卖一、盈亏金额（realized+unrealized）和盈亏百分比，托管订单表展示买一、卖一（用于和挂单价对比；开放订单未成交不展示盈亏）。这些值来自 snapshot 的 `token_quotes`（按 `token_id` 索引的 best_bid/best_ask/mark_price），由 API handler 在返回前 best-effort 从 orderbook 服务批量注入；orderbook 服务不可用或缺盘口时对应单元格显示 `—`，不阻断 snapshot。盈亏在前端按 `(mark-avg)*size + realized` 推导，百分比按总盈亏 / 成本基准计算。
- 今日已赚奖励展示 worker 同步到 `account.reward_earned_usd` 的 UTC 当日 maker rewards 值；worker 优先读取认证 CLOB `GET /rewards/user/total?sponsored=true` 聚合端点，以对齐 Polymarket `/rewards` 页面顶部 Daily Rewards 的 native+sponsored 口径。聚合端点为空、为 0 或不可用时回退分页读取 `GET /rewards/user` native 明细并合并 `sponsored=true` sponsored-only 明细，按 `earnings * asset_rate` 求和。前端不直接访问 Polymarket，账户快照停更或认证配置缺失时不会自行回退官网数据。
- 事件分类视图（挂单/撤单/吃单/奖励）
- live worker 已接入 post-only 买单、撤单、drift 换价 guard（历史同向确认、订单冷却、单轮限速）、confirmed 成交同步、成交后卖出/平仓、本地账本更新、managed order 计分状态、账户开放买单总 notional 观测、可映射 active rewards BUY 的 CLOB open-order 收养/重开和 UTC 当日账户级 maker rewards 同步（聚合端点优先、明细端点 fallback）；SELL、非 rewards 市场和无法唯一映射 token 的外部开放订单明细与奖励结算对账仍是后端缺口
- 页面不再暴露仅用于旧模拟逻辑或可能错误释放对账锁的配置；历史 critical event 和短暂 `awaiting final reconciliation` 不会占用 `status.error`，当前错误只反映活跃对账锁
- API 不直连 Polymarket 私有账户；账户余额、完整 positions 和本系统托管订单都从数据库读取。`status.open_orders` / `status.positions` 描述本地 managed state。

## 修改检查清单

- [ ] 新增配置参数时同步更新 `NumberConfigKey` 类型
- [ ] 新增事件类别时同步更新 `EventCategory`
- [ ] 修改后人工 smoke `/rewards` 页面（Run/Cancel/Reset、配置编辑、事件过滤）
