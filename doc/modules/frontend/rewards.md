# Rewards（奖励机器人）

最后更新：2026-06-13

## 概述

`/rewards` 页面管理做市奖励机器人的生命周期：配置策略参数、向 worker 提交运行/取消/重置命令、查看订单/持仓/事件。仅支持实盘（live）模式。

## 架构与关键文件

| 文件 | 职责 |
|---|---|
| `src/app/(console)/rewards/page.tsx` | 路由页面 |
| `src/features/rewards/components/rewards-workbench.tsx` | 主工作台编排：状态/操作区、指标条、活动/配置/风控 tabs |
| `src/features/rewards/components/rewards-overview-cards.tsx` | 顶部执行概览、操作中心和关键指标条 |
| `src/features/rewards/components/rewards-config-panel.tsx` | 分组策略配置面板（执行、市场筛选、报价构造、盘口选择、AI 建议、库存与控制） |
| `src/features/rewards/components/rewards-advanced-config.tsx` | 盘口选择与 AI advisory 配置子面板 |
| `src/features/rewards/components/rewards-config-fields.tsx` | 配置面板共享字段、区块和提示组件 |
| `src/features/rewards/components/rewards-tables.tsx` | 表格组件（订单/持仓等） |
| `src/features/rewards/components/rewards-events-panel.tsx` | 事件面板 |
| `src/features/rewards/components/number-input.tsx` | 数值输入组件 |
| `src/features/rewards/loaders/rewards-page-data.ts` | 服务端数据装配 |
| `src/features/rewards/lib/rewards-helpers.ts` | 辅助函数 |
| `src/features/rewards/types.ts` | 类型定义（~26 行） |

## 核心类型（types.ts）

- **`NumberConfigKey`**：数值输入参数的字符串联合类型 — `max_markets`、`max_open_orders`、`per_market_usd`、`quote_size_usd`、`min_daily_reward`、`min_market_liquidity_usd`、`min_market_volume_24h_usd`、`min_hours_to_end`、`max_market_spread_cents`、`max_market_data_age_minutes`、dominant 单边阈值、盘口集中度阈值、AI advisory TTL、`account_capital_usd` 等；`quote_bid_rank`、quote/selection mode 和 AI provider/request format 使用受限下拉框，不进入该联合类型
- **`EventCategory`**：`"all" | "placements" | "cancels" | "fills" | "rewards"`

## API 依赖

- `src/lib/api/rewards.ts` — `readRewardBotSnapshot`、`updateRewardBotConfig`、`runRewardBotOnce`、`cancelRewardBotOrders`、`resetRewardBot`
- `readRewardBotSnapshot()` 会传递计划/订单分页、搜索、状态和排序 query；首屏明确请求 `plans_eligible=true`，与默认选中的“可挂”页签一致。后端分页结果和 `orders_page` 都描述本地 managed orders，不再用 Polymarket live open orders 覆盖
- 后端 snapshot 不返回全量 reward markets；页面只使用 `status.markets_tracked`、`status.eligible_markets` 和 `quote_plans` 展示市场覆盖与候选计划
- snapshot 的 `available_usd` / `positions` 来自 worker 写入数据库的账户快照；API 不持有 Polymarket 私钥，也不直接请求外部账户数据。`available_usd` 优先使用 CLOB `balance-allowance`，当 CLOB 返回 0 或失败但资金钱包链上 pUSD 余额大于 0 时，worker 使用链上 pUSD 回填

## 关键交互

- **Run** → `runRewardBotOnce()` → API 写入 `run_once` 控制命令，worker 领取后执行一轮 live 策略
- **Cancel open orders** → `cancelRewardBotOrders()` → API 写入 `cancel_all` 控制命令，worker 领取后撤销 Polymarket live 托管订单
- **Reset** → `resetRewardBot()` → API 写入 `reset` 控制命令，worker 领取后按 cancel-all 撤销 live 订单
- **Config 编辑** → `updateRewardBotConfig(patch)` → 即时更新配置
- **挂单档位** → `quote_bid_rank=1|2|3` → 分别选择买一/买二/买三；任一 YES/NO 盘口缺少所选档位时本轮不挂单
- **盘口选择** → `quote_mode=double|auto` + `selection_mode=observe|enforce` → 默认只保留双边报价；auto/enforce 可让后端基于一边倒概率、退出深度和盘口集中度执行单边或跳过
- **AI 建议配置** → 保存 provider、request format 和 TTL；worker 启用且环境变量配置 provider key 后，会在 full tick 中低频调用模型并缓存 advisory
- **市场质量** → 可配置最低流动性、最低 24h 成交量、最短剩余结算时间、最大 Gamma spread 和最大目录同步年龄；后端还固定拒绝高歧义和非唯一 YES/NO 市场
- 事件面板支持按 `EventCategory` 过滤
- 页面默认展示活动视图：左侧候选报价计划，右侧托管订单与本地库存，下方事件/成交流；策略配置和风控配置通过 tabs 切换，减少实盘盯盘时的配置噪音。
- 筛选刷新使用单调请求序号，只接收最新 REST 响应，避免快速搜索/翻页时旧请求覆盖新状态；读取失败会进入页面反馈栏，不产生未处理 Promise。

## 数据流

所有 mutation 通过 Server Actions。配置保存会立即返回更新后的 `RewardBotSnapshotDto`；Run / Cancel / Reset 只表示命令已入队，返回的是入队后的当前 snapshot，实际外部订单变化会在 worker 处理命令后由数据库 snapshot 反映。

## i18n

使用 `rewards` 命名空间字典。

## 当前状态

- 完整的 Run / Cancel / Reset 入队交互
- 顶部执行概览展示实盘模式、启停/运行状态、市场就绪度、钱包余额/策略上限比例、最近扫描/运行时间和事件触发计数；策略上限直接读取当前 `snapshot.config.account_capital_usd`，不再使用可能保留历史初始值的账户账本字段，也不代表链上钱包余额。
- 操作中心集中 Run / Save / Cancel / Reset，文案提醒当前命令可能提交或取消 Polymarket 实盘订单。
- 配置编辑按执行、市场筛选、报价构造、盘口选择、AI 建议、库存与控制分组，包含数值参数、布尔开关、受限下拉框和成交后策略。
- 市场筛选面板公开质量硬门槛；通过门槛的市场由后端继续按奖励、流动性、成交量、剩余时长和奖励 spread 综合排序。
- 报价构造使用“挂单档位”下拉框选择买一/买二/买三，不再提供中间价“报价偏移”；默认买一。
- 盘口选择公开 quote/selection mode、dominant 单边概率区间、退出深度、top1/top3 买盘集中度、HHI 和偏好分类评分加成；默认 `double + observe` 不改变既有双边挂单。
- AI 建议面板保存 OpenAI/Anthropic provider、请求格式和 TTL；API key、base URL、模型名、请求超时和每轮最大判断数只来自 worker 环境变量，不会出现在前端配置或 snapshot。报价计划表展示 AI suitability、推荐 quote mode、confidence 和首条 reason。
- `per_market_usd` 表示 YES + NO 两腿合计资金上限；后端先保障按 CLOB 成本精度对齐后的两腿最小份额，再在剩余额度内靠近 `quote_size_usd` 单腿目标，页面提示与该联合预算语义一致。
- 配置不包含 `execution_mode` 选择器（始终为 live）。提示说明 `max_markets=0`、`max_open_orders=0`、`quote_size_usd=0` 都会停止新挂单。
- 报价计划默认展示可挂市场，本地支持全部/可挂/不可挂切换，并用状态标记说明每个当前候选计划是否符合最终过滤要求。
- Managed orders 表格发送后端分页/搜索/状态过滤/排序 query（默认每页 15 条），表格数据与 `orders_page` 均来自本地 managed-order 查询。
- 报价计划和订单搜索框使用独立防抖输入组件；外部 query 重置通过组件 key 同步，不在 React effect 中同步 setState。
- 首屏不加载全量 reward markets，避免奖励市场数量过大时长时间停留在 loading skeleton。
- Wallet balance、Positions 和 Orders 表格展示 worker 同步到数据库的 rewards 账户视图；余额显示资金钱包 pUSD，资金钱包地址优先使用 `POLYEDGE_POLYMARKET__FUNDER`，未配置时使用 `ACCOUNT_ID`。
- 今日已赚奖励展示 worker 同步到 `account.reward_earned_usd` 的 UTC 当日 maker rewards 值；worker 优先读取认证 CLOB `GET /rewards/user/total` 聚合端点，聚合端点为空、为 0 或不可用时回退分页读取 `GET /rewards/user` 明细并按 `earnings * asset_rate` 求和。前端不直接访问 Polymarket，账户快照停更或认证配置缺失时不会自行回退官网数据。
- 事件分类视图（挂单/撤单/吃单/奖励）
- live worker 已接入 post-only 买单、撤单、confirmed 成交同步、成交后卖出/平仓、本地账本更新、managed order 计分状态、账户开放买单总 notional 观测和 UTC 当日账户级 maker rewards 同步（聚合端点优先、明细端点 fallback）；账户范围外开放订单明细与奖励结算对账仍是后端缺口
- 页面不再暴露仅用于旧模拟逻辑或可能错误释放对账锁的配置；历史 critical event 不会永久占用 `status.error`，当前错误只反映活跃对账锁
- API 不直连 Polymarket 私有账户；账户余额、完整 positions 和本系统托管订单都从数据库读取。`status.open_orders` / `status.positions` 描述本地 managed state。

## 修改检查清单

- [ ] 新增配置参数时同步更新 `NumberConfigKey` 类型
- [ ] 新增事件类别时同步更新 `EventCategory`
- [ ] 修改后人工 smoke `/rewards` 页面（Run/Cancel/Reset、配置编辑、事件过滤）
