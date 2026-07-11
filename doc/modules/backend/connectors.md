# connectors（外部连接器层）

最后更新：2026-07-11

## 概述

`polyedge_connectors` crate 封装所有外部系统调用：Polymarket Gamma、CLOB public/orderbook、CLOB authenticated live、Data API、Polygon RPC/Bridge、Rewards catalog、orderbook 服务客户端、Rewards combined provider、RSS/Atom 新闻源，以及内置 Paper Trading 执行器。

连接器只负责协议适配和响应解析。API handler 与策略代码不得在请求路径直接调用外部市场 API；市场数据由 orderbook/worker 后台 producer 写入 Postgres 或缓存后再被消费。

## 设计目标

- 隔离外部 API 细节、认证、分页、回退和解码错误。
- 为 worker/orderbook 服务提供明确的数据入口。
- 外部响应结构漂移时返回带短 preview 的 dependency error，便于排查。
- 不在 connector 内直接读写数据库或执行业务策略。

## 架构与关键文件

| 文件/目录 | 职责 |
|---|---|
| `lib.rs` | 模块入口和 Paper Trading 执行器 |
| `polymarket/gamma.rs` | Gamma `/markets` 全量/condition 定向查询 |
| `polymarket/book.rs` | CLOB public orderbook connector |
| `polymarket/data_api.rs` | Data API 钱包活动/持仓/交易读取；当前只作为 rewards 对账 fallback |
| `polymarket/live.rs` | 认证 CLOB connector：下单、撤单、余额、开放订单、订单状态、成交同步、用户 WS |
| `polymarket/live/raw.rs` | authenticated raw HTTP fallback：heartbeat、rewards earnings 宽容解析 |
| `polymarket/live/trade_reconciliation.rs` | CLOB 订单/成交终态映射和 order-specific fill 对账 helper |
| `polymarket/live/trade_sync.rs` | 托管订单成交同步和账户 trade 历史回退 |
| `polymarket/chain.rs` | Polygon ERC20 余额、Bridge 入金转账、Safe proxy CTF merge |
| `polymarket/models.rs`、`normalizers.rs`、`helpers.rs` | Polymarket 共享模型、WS 消息规范化和 helper |
| `rewards.rs` + `rewards/orderbooks.rs` + `rewards/price_history.rs` | CLOB rewards catalog、批量盘口和 `/prices-history` |
| `orderbook.rs` | 独立 orderbook 服务 HTTP/内部 WS client |
| `openai_compat.rs` | OpenAI-compatible helper：base URL 归一化、认证头、JSON 提取和模型差异处理 |
| `reward_provider.rs` | Rewards combined provider：一次请求可同时返回 AI advisory 与 info-risk section |
| `reward_ai.rs`、`reward_info_risk.rs` | Rewards provider 响应 schema/parser helper |
| `news.rs` | RSS/Atom 新闻 connector |
| `test_http.rs` | provider connector 单元测试 HTTP 捕获 helper，捕获 request line 与 JSON body |

## Polymarket Gamma

- `PolymarketGammaConnector` 读取 Gamma public market data。
- `fetch_markets()` 使用 offset 分页，按 active/open/non-archived 和 24h volume 拉取，遇到 422 offset 边界、空页或短页停止，并按 market id 去重。
- `fetch_markets_by_condition_ids()` 使用重复 `condition_ids` query 参数做小批量定向刷新，每批最多 50 个 condition。
- `PolymarketGammaMarket` 保存 id、slug、question、category、status、best bid/ask/mid、`liquidity_usd`、`volume_24h`、start/end time、event date candidates、`has_reviewed_dates`、ambiguity/tradability、condition id、YES/NO asset id 和原始 token 信息。
- 用途：`polyedge-orderbook` 的 full/priority market sync；worker 中的 market sync 仅是 CLI 兼容入口。

## Polymarket Data API

- `PolymarketDataApiConnector` 无需认证，封装 wallet activity、positions、closed positions、trades 和 public profile 等读取。
- `fetch_wallet_positions()` 使用 `sizeThreshold=0` 分页读取完整持仓；超过最大页数或失败不会把不完整快照交给下游替换。
- 当前运行路径仅用于 rewards live 对账 fallback：当认证 CLOB trade 明细无法解码或 missing-order 需要最终核验时，worker 可用 Data API 钱包活动做严格补账匹配。

## Polymarket Chain

- `PolymarketChainConnector` 使用 Polygon JSON-RPC 和 Polymarket Bridge。
- `fetch_pusd_balance()` 读取 pUSD ERC20 余额，作为 rewards snapshot 的链上余额回填。
- `fetch_funding_token_balance()` 读取 Funding allowlist 中 USDC/USDT 余额，供 Funding 页面展示。
- `funding_source_address()` 从后端私钥派生付款地址，只返回地址。
- `submit_funding_transfer()` 校验 chain id、token allowlist、金额精度和 Bridge supported-assets 后，生成 Bridge 入金地址并广播 ERC20 transfer。
- `submit_merge_positions()` 通过 Safe proxy wallet 执行 CTF `mergePositions`，供 BalancedMerge 自动执行使用。

## Polymarket Book

- `PolymarketBookConnector` 包装 public CLOB book 接口。
- `fetch_token_book()` 读取单 token 盘口。
- `fetch_binary_book()` 读取 YES/NO 双侧盘口。
- 用途：独立 orderbook 服务 WS/poll reconcile 和按需刷新。Rewards 策略通过 orderbook 服务读取缓存，不直接调用该 connector。

## Polymarket Live

- `LivePolymarketConnector` 负责 CLOB V2 认证、用户 WS、余额、开放订单、orders scoring、当日 maker rewards、heartbeat、下单、撤单、订单状态和成交同步。
- 签名类型支持 `eoa`、`proxy`、`gnosis_safe`、`poly_1271`；Deposit Wallet 需要把 `funder` 配成 deposit wallet 地址。
- `post_heartbeat()` 显式维护链式 heartbeat；SDK 请求失败时会使用 raw authenticated fallback。
- `list_open_orders()` 分页读取认证账户全部开放订单，并处理空 cursor、重复 cursor 和最大页数 guard。
- `submit_token_order()` 按 token id 提交 rewards buy/sell；post-only 使用 GTC，flatten 使用 FAK。
- `poll_order_status()` / `collect_trade_updates()` 优先按单订单查询，关联 trade 单查失败或订单 404 时按 token/time 扫描账户 trades 精确回退；只有 confirmed trade 会入账。
- 当前范围：支持已 funded、已 approve 的账户或 Deposit Wallet 下单/撤单；仍需要真实凭证和小额验证。

## Polymarket Rewards

- `PolymarketRewardsConnector` 读取 rewards catalog、批量盘口和 price history。
- `fetch_current_markets()` 对 cursor 分页、重复页、condition 去重和详情补全做保护；只有原始 market 缺唯一 YES/NO token 或 question 时才请求 CLOB market 详情。
- `fetch_order_books()` 优先使用 `POST /books` 批量拉取，失败或遗漏时用单 token `/book` 回退；整批不可用时返回 dependency error，部分失败记录告警。
- `fetch_price_history()` 读取 `/prices-history`，解析 seconds/millis 时间戳，丢弃 0-1 以外价格并排序去重。
- 用途：orderbook 服务同步 `reward_markets`、按需刷新缓存盘口和低频写入 `reward_market_candles`。

## Orderbook 服务客户端

- `OrderbookHttpClient` 供 API/worker 读取独立 orderbook 服务缓存，并由 worker 注册 token source。
- `OrderbookStreamClient` 连接内部 `GET /orderbook/stream`，接收 `OrderbookStreamEvent` 用于 worker 本地 cache 和 rewards fast reconcile wake。
- `get_books()` 使用 `POST /orderbook/batch` 批量读取缓存。
- `get_books_with_max_age()` 在请求体携带 `refresh_if_stale_ms`，由 orderbook 服务只刷新缺失或 `confirmed_at` 超龄 token。
- register/ingest/delete 写请求携带 `x-polyedge-orderbook-token`，值来自 `POLYEDGE_ORDERBOOK__WRITE_TOKEN`。
- 单盘口 404 映射为 `None`，其他非成功状态映射为 dependency error。

## Rewards Combined Provider

- `RewardProviderConnector` 用于 rewards worker 对单个市场评估 AI advisory 和/或 info-risk。
- 请求体 `RewardProviderRequest` 中 `advisory` 与 `info_risk` section 可独立为空；connector 只请求本轮需要补齐的 section。
- 支持 `openai_responses`、`openai_chat_completions` 和 `anthropic_messages`。OpenAI-compatible base URL 可配置根地址、`/v1` 或 versioned path；模型名包含 GLM/DeepSeek/Agnes 时按兼容路径归一。
- Advisory V2 schema 输出 `action=allow|reduce|stop_new`、`size_multiplier`、`edge_buffer_cents`、`confidence`、`reasons` 和 `metrics`；禁止输出 live price、side、rank 或绝对 notional。旧 `allow_quote` / `suitability` 响应只在 connector ingress 转换为 V2 action，legacy strategy hint 被忽略且不持久化。
- Info-risk V2 schema 输出 `action=allow|reduce|stop_new|cancel_yes|cancel_no|cancel_all`、risk taxonomy、direction、event time、confidence、summary、sources 和 metrics。`directional_risk` 明确定义为不安全的 resting-BUY outcome（不是预测赢家）并必须匹配 `cancel_yes/cancel_no`；connector 负责严格 JSON/范围解析，application 再验证 cancel 的方向、证据新鲜度与来源独立性。
- OpenAI Responses 仅在请求包含 info-risk 且 web search 显式开启时附加 `web_search_preview` tool。
- connector 不访问 Polymarket 或数据库。Advisory payload 只由稳定市场元数据和完成的粗粒度 candle 构造；info-risk payload 只含稳定市场身份、评估时间与证据搜索边界，不包含 orderbook、quote plan、账户或库存。

## News

- `RssNewsConnector` 支持 RSS 与 Atom。
- 输出 `ConnectorNewsItem`：source、external_id、title、url、author、published_at、content_snippet、raw_payload。
- 用途：news worker 采集 raw news events。

## Paper Trading

- `PaperExecutor` 是内置无状态模拟执行器，支持 submit、reconcile fill 和 poll order status。
- 常量：`PAPER_EXECUTOR_NAME`、`PAPER_ACCOUNT_ID`、`PAPER_MIN_NOTIONAL_USD`。
- 当前主要用于本地/测试执行链路；Rewards live bot 不使用 paper 模式。

## 当前状态

- 已实现当前系统使用的 Gamma、CLOB public/orderbook、CLOB live、Data API 对账 fallback、Rewards catalog、price history、Polygon Funding/merge、orderbook service client、Rewards provider 和 RSS connector。
- Gamma、CLOB rewards、order book、price-history 和 provider 解码失败会返回短 preview，便于定位 HTML、截断响应或上游结构漂移。
- Rewards catalog 分页和详情补全具备完整性保护，不会把明显不完整目录破坏性替换到 store。
- Orderbook 服务客户端支持 HTTP batch/bootstrap、按最大确认年龄刷新和内部 WS 推送。
- Provider 并发由 worker 按 `RewardBotConfig` 控制；connector 内不维护全局单飞闸门。
- Live connector 已具备显式 heartbeat、订单/成交对账回退、Deposit Wallet 签名路径、Polygon pUSD 余额回填、Funding 入金和 Safe proxy merge 支持。

## 修改检查清单

- [ ] 新增 connector 时在 `lib.rs` 添加模块声明和 `pub use`。
- [ ] 修改 Polymarket API 调用时检查对应 worker/orderbook producer。
- [ ] 新增/修改模型时同步检查 application、store、frontend DTO。
- [ ] WebSocket 相关修改后检查 orderbook 服务和 Polymarket 用户事件 worker。
- [ ] 运行 `cargo check --workspace --tests`。
