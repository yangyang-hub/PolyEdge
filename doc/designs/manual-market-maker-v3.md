# PolyEdge 人工市场多钱包做市 V3

最后更新：2026-07-15

## 1. 目标

V3 把 PolyEdge 收敛为一个可审计、失败关闭的人工做市执行系统：人决定做哪个市场、挂 YES/NO 哪一侧、挂几张、每张数量和定价规则；系统负责把稳定的目标订单状态批量应用到多个钱包，并持续撤单或按价格变化重挂。

核心目标：

- 市场与 rewards 条款人工录入，不做自动发现/筛选；
- quote slots 在策略录入时明确，支持 YES-only、NO-only、双边和多个同侧槽位；
- 同一策略统一分配多个钱包，钱包间并行、单钱包串行；
- 保留 deterministic 撤单和价格变化 cancel-replace；
- 单后端进程，保留前后端分离；
- clean deploy，不兼容旧数据。

## 2. 明确移除

- events/news/evidences 与所有来源采集；
- AI advisory、info-risk、LLM provider、内容/信息过滤；
- fair value、edge gate、历史和工作台；
- Gamma/rewards catalog 全市场扫描与自动市场优先级；
- candidate prewarm、price-history candles、独立 replay；
- 独立 API/provider/worker/orderbook 服务和内部 HTTP token registry；
- 旧 API、旧表和旧前端路由兼容。

这些能力不能以“未来可选 gate”的形式继续影响当前策略。人工策略配置与明确的账户/盘口/订单风险规则是唯一执行输入。

## 3. 总体架构

```text
Next.js static console
  -> REST /api/v1
polyedge-server
  ├── API/auth/idempotency/audit
  ├── Postgres V3 store
  ├── targeted orderbook poll/cache
  ├── wallet secret resolver
  └── execution coordinator
       ├── wallet A serial queue
       ├── wallet B serial queue
       └── wallet N serial queue
  -> PostgreSQL
  -> Polymarket CLOB
```

API handler 不抓外部数据。targeted orderbook 和执行协调器是同进程后台任务，共享数据库连接池与内存 cache。

## 4. 人工策略模型

一次策略创建包含：

- market：condition、slug、question、URL、YES token、NO token；
- rewards terms：minimum size、maximum spread、可选 daily rate；
- version：盘口 freshness、下调/上调重挂确认、cooldown、单轮替换上限；
- quote slots：slot key、outcome、quantity、pricing mode、price/offset、价格边界、post-only、enabled；
- wallet targets：统一选择的钱包 ID 集合。

每个 slot 表示一张持续维护的 desired order。slot key 在一个版本内唯一；新配置生成新不可变版本，已创建的 batch 固化原版本。

定价模式：

- `fixed`：使用人工指定价格；
- `book_rank`：读取对应 token bids 的第 N 档，加人工 offset，再应用价格边界。

系统不改变 outcome、不动态增加槽位、不根据 rewards/新闻/AI 重新分配数量。

## 5. Targeted orderbook

每轮 token universe 是以下集合的并集：

1. active strategy + active wallet target + enabled quote slot 的 token；
2. open-like managed order 的 token；
3. nonzero position 的 token。

集合按 token 去重并排序。超过上限时必须整体报错，不能截断后继续交易，因为被截断 token 可能对应风险撤单或已有持仓覆盖。

当前实现通过 CLOB REST `/books` 轮询，cache 只保存在进程内。`confirmed_at` 是执行 freshness 的依据；cache missing/stale 时允许撤单，不允许新 place。

## 6. 多钱包执行

提交一个 execution batch 时选择一个 strategy 和多个钱包。系统固化 published version，并为每个钱包创建独立 job。

- 钱包间：有界并发，避免一个钱包阻塞全部账户；
- 钱包内：进程 mutex + Postgres lease owner/epoch/expiry 双重串行；
- job/action：`FOR UPDATE SKIP LOCKED` claim，terminal write owner-fenced；
- 批次结果：允许 succeeded/failed/cancelled 混合，不用最慢或失败钱包回滚其他钱包已完成动作。

credential locator 与策略统一配置分离。每个 job 执行时才解析对应钱包 secret，数据库和 API 永远不返回 secret。

## 7. Desired-state 对账

对每个 slot：

```text
target unavailable/blocked + current order -> cancel
target unavailable/blocked + no order      -> keep empty
target == current                           -> keep
target changed + confirmation/cooldown pass -> cancel then place new generation
target changed + throttle not pass          -> keep current
target exists + no current order             -> risk check then place
```

place 前检查：

- global kill switch 与 global trading；
- wallet active/trading enabled；
- strategy/market/version/slot active；
- book exists/fresh、价格位于 `(0,1)`、post-only 不穿 ask；
- max open orders、max order notional、open BUY、market position、total position、available collateral。

venue-first match 用于识别已存在的同 token/side/price/quantity 订单。managed order 从 open set 消失后按 external id 精确查询并落库 partial/filled/cancelled/rejected/expired；查询错误、多匹配、submission unknown、cancel unknown 或未知终态才进入 `unknown`，继续占用 slot，禁止自动补单。

## 8. 撤单与重挂

保留的撤单原因包括：

- operator cancellation batch；
- kill switch / wallet / strategy / market 禁用；
- slot 删除或 disabled；
- 盘口缺失、过旧或目标穿价；
- fixed/book-rank 目标越界；
- 目标价格或数量变化。

价格下降使用 `downward_reprice_confirm_ms`，价格上升使用 `upward_reprice_confirm_ms`，两者都受 `reprice_cooldown_ms`，每轮最多 `max_replaces_per_cycle`。操作员强制撤单批次进入 cancel-only，不在同轮重新 place。

## 9. 数据库与审计

V3 schema 以钱包、人工市场、不可变策略版本、quote slots、批次/job/action、managed orders/positions 为核心。重要约束：

- 一个策略最多一个 published version；
- 一个钱包/slot 最多一张 open-like order；
- action idempotency key 全局唯一；
- API write 使用 request hash 幂等；
- order transitions 与 audit logs 追加写；
- default runtime state locked/off。

不包含 events/news/fair-value/AI/provider/catalog/candle/replay 表。

## 10. API 与安全

API 资源为 wallets、market-strategies、execution-batches、cancellation-batches、orders、positions 和 system/runtime-state。SELL exit、merge、Funding 与独立 fills 账本已从 V3 schema、API、前端和 runtime 删除。

所有写请求要求 `Idempotency-Key`。启用钱包交易、批量执行、强制撤单和 kill switch 分别使用最小 step-up scope，并同时校验 step-up code；production 强制配置至少 16 字符的 `POLYEDGE_AUTH__STEP_UP_CODE`。生产若暂时关闭 Bearer auth，仍必须在可信私网边界内运行；CORS 不是认证。

secret resolver 当前使用环境变量 JSON。locator 规范化后映射到独立变量，错误只报告 locator/字段名，不回显 secret。

## 11. 部署

Compose 只有 `polyedge-server` 和 `polyedge-front`。后端端口为 38001，前端静态端口默认 33002。构建产物只有 `bin/polyedge-server`。

部署前必须：

1. 创建空 PostgreSQL；
2. 配置 exact CORS、auth/private-network 边界；
3. 逐钱包注入 secret；
4. 保持 kill switch locked 和钱包 trading disabled 完成只读 smoke；
5. 只用小额、已 funded/approved 钱包做首轮演练。

## 12. 当前实现边界

已实现：V3 schema/domain/contracts、单 server API/store、targeted REST orderbook、多钱包 BUY quote place/cancel/replace、批量撤单、CLOB 余额/开放订单读取、Data API positions 全量替换、幂等/审计/step-up、双服务部署和 V3 控制台主路由。

未完成：market-channel WS、账户范围外部订单持续同步、生产 session UX。SELL exit、merge、Funding 与独立 fills 账本不属于 V3 待办。
