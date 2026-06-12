# connectors（外部连接器层）

最后更新：2026-06-12

## 概述

`polyedge_connectors` crate 实现所有外部系统的适配器：Polymarket CLOB（交易）、Gamma API（市场数据）、Data API（钱包活动）、Order Book（盘口）、Rewards API（奖励市场）、RSS 新闻源，以及内置的 Paper Trading 执行器。

## 设计目标

- 封装所有外部 HTTP/WebSocket 调用，隔离外部 API 细节
- 提供统一的接口给 worker 和 application 层使用
- 内置 Paper Trading 执行器用于本地模拟

## 架构与关键文件

| 文件/目录 | 职责 |
|---|---|
| `lib.rs` | 模块入口 + Paper Trading 执行器定义（~316 行） |
| `polymarket/` | Polymarket 多 API 集成子目录 |
| `polymarket/live.rs` | 认证 CLOB 连接器：`LivePolymarketConnector` |
| `polymarket/live/trade_reconciliation.rs` | CLOB 订单/成交终态映射与 order-specific fill 对账 helper |
| `polymarket/live/trade_sync.rs` | CLOB 托管订单成交同步、关联 trade 单查和账户 trade 历史回退 |
| `polymarket/gamma.rs` | 公共市场元数据：`PolymarketGammaConnector`；Gamma keyset 分页 guard |
| `polymarket/data_api.rs` | 钱包活动与账户持仓 API：`PolymarketDataApiConnector` |
| `polymarket/chain.rs` | Polygon JSON-RPC ERC20 余额读取：`PolymarketChainConnector` |
| `polymarket/book.rs` | 盘口快照：`PolymarketBookConnector` |
| `rewards.rs` + `rewards/orderbooks.rs` | 奖励市场目录与 CLOB 批量盘口：`PolymarketRewardsConnector` |
| `orderbook.rs` | 独立 orderbook 服务 HTTP 客户端：读盘口、原子注册 token、内部写 token |
| `polymarket/models.rs` | 共享数据模型 |
| `polymarket/normalizers.rs` | WebSocket 消息规范化函数 |
| `polymarket/helpers.rs` | 共享辅助函数 |
| `news.rs` | RSS 新闻连接器：`RssNewsConnector` |

## 核心数据结构

### Polymarket Gamma（公共市场数据）

- **`PolymarketGammaConnector`**：`gamma_host` + `reqwest::Client`
- **`PolymarketGammaMarket`**：id、slug、question、category、status、best_bid/ask/mid_price、volume_24h、ambiguity_level、tradability_status、condition_id、yes/no_asset_id 等
- **`GammaMarketPage`**：分页响应（markets + next_cursor）
- 常量：`GAMMA_MARKETS_PATH = "markets/keyset"`、`GAMMA_TIMEOUT = 15s`、`GAMMA_LAST_CURSOR = "LTE="`、`GAMMA_MAX_PAGES = 1000`
- `fetch_markets()` 对 Gamma keyset 分页做防御：记录已请求 cursor、遇到重复 `next_cursor` 或末页 sentinel 即停止，并按 market id 去重，避免上游重复游标导致进程内存持续增长
- 用途：`market_sync.rs` worker 的主要数据源，`arbitrage.rs` 的回退数据源

### Polymarket Data API（钱包活动）

- **`PolymarketDataApiConnector`**：`data_api_host` + `reqwest::Client`，无需认证
- **`PolymarketWalletActivity`**：proxy_wallet、kind（TRADE/SPLIT/MERGE/REDEEM/REWARD）、side、asset、condition_id、price、size、usdc_size、timestamp 等
- **`PolymarketWalletPosition`**：当前持仓
- **`PolymarketClosedPosition`**：已结算持仓
- **`PolymarketTrade`**：单笔交易记录
- 常量：`MAX_DATA_API_LIMIT = 500`、`MAX_DATA_API_POSITION_PAGES = 100`、`DATA_API_TIMEOUT = 15s`
- `fetch_wallet_positions()` 使用 `sizeThreshold=0`、limit/offset 分页和 asset 去重读取完整账户持仓；超过最大页数返回错误，不把不完整快照交给下游替换。
- 用途：`copytrade.rs` worker 检测跟踪钱包的新成交，以及 rewards worker 同步外部账户持仓

### Polymarket Chain（资金钱包余额）

- **`PolymarketChainConnector`**：`polygon_rpc_url` + `reqwest::Client`
- `fetch_pusd_balance(wallet_address)`：通过 Polygon JSON-RPC `eth_call` 读取 Polymarket pUSD ERC20 `balanceOf`，按 6 位小数转换为美元 Decimal
- 用途：rewards worker 同步账户状态时，若 CLOB `balance-allowance` 返回 0 或失败，但资金钱包链上 pUSD 余额大于 0，则用链上余额回填 snapshot，避免 Deposit Wallet / `POLY_1271` 缓存或签名路径导致前端余额显示为 0

### Polymarket Book（盘口）

- **`PolymarketBookConnector`**：包装非认证 `ClobClient`
- `fetch_binary_book(market_refs)`：获取 YES+NO 双侧盘口
- `fetch_token_book(asset_id)`：获取单 token 盘口
- **`PolymarketBinaryBookSnapshot`**：condition_id + yes/no book + observed_at
- 用途：arbitrage scanner、rewards bot、orderbook stream

### Polymarket Live（认证交易）

- **`LivePolymarketConnector`**：client、private_key、chain_id、account_id、ws_host
- `connect(config)`：创建 `LocalSigner` → `ClobClient` → `auth_builder.authenticate()`
- 签名类型支持 `eoa`、`proxy`、`gnosis_safe`、`poly_1271`；`poly_1271` 对应 Polymarket Deposit Wallet / `POLY_1271`，需要把 `funder` 配成 deposit wallet 地址。
- `connect_user_ws()`：创建认证 WebSocket 客户端（订单/成交通道）
- `balance()`：查询认证资金账户的 collateral balance
- `orders_scoring()`：通过 CLOB `POST /orders-scoring` 批量查询 managed orders 是否正在参与奖励计分
- `reward_earnings_today_usd()`：读取 CLOB `GET /rewards/user/total` 的 UTC 当日账户级 maker rewards 聚合结果，并按每项 `asset_rate` 换算为 USD；该口径与 Polymarket `/rewards` 页面顶部 Daily Rewards 一致
- `list_open_orders()`：分页读取认证账户全部开放订单；遇到空 cursor、`LTE=`、空页、重复 cursor 或 1000 页 guard 时停止
- `find_matching_open_token_order()`：按 token/side/price/size 严格匹配唯一开放订单，用于 rewards 提交响应丢失后的恢复；多个匹配会返回冲突而不是猜测归属
- `post_order` 返回订单 ID 时，无论状态为 `live` / `matched` / `delayed` / `unmatched` / `canceled` / 未知值，connector 都保留为 accepted 供后续成交和订单状态对账；成功响应缺少订单 ID 会按提交结果未知处理
- `post_order` 返回 HTTP 4xx 时视为 CLOB 明确拒单，不进入提交结果未知锁；网络中断、5xx 或成功响应缺少订单 ID 仍按结果未知处理
- `submit()`：兼容 execution pipeline 的 YES/NO 买单提交
- `submit_token_order()`：按 token_id 直接提交 buy/sell；post-only 使用 GTC，非 post-only flatten 使用 FAK；提交前把价格收敛到最多 2 位小数，并返回实际提交 quantity，供 rewards live maker 使用
- `cancel_order()`：按 Polymarket order id 撤销单笔订单
- `poll_order_status()` / `collect_trade_updates()`：优先通过 CLOB 单订单接口查询；单订单返回 404，或订单返回的关联 trade 无法按 ID 单独查询时，会按 token/time 分页扫描认证账户 trades，并按 external order id 精确匹配。仅返回 `CONFIRMED` trade 供入账，live / 普通 GTC unmatched 状态可立即按 open 返回，并且只有预期关联 trade 全部达到终态后才返回取消、matched 或 FAK unmatched 终态。404 与关联 trade 回退失败分别使用 `POLYMARKET_MISSING_ORDER_TRADE_QUERY_FAILED`、`POLYMARKET_ASSOCIATED_TRADE_FALLBACK_FAILED`，worker 会隔离单笔失败并继续其余订单对账。
- **`LivePolymarketTradeSyncOutcome`**：包含 confirmed `updates`、安全可应用的 `order_status` 和 `order_not_found`；pending/mined/retrying trade 会阻止 terminal status 提前返回，404 fallback 不会伪造取消状态
- **`PolymarketMatchedOrderHint`**：当 worker 的认证 trade 回退仍失败时，重新读取单订单并只暴露 terminal matched/canceled 订单的 token、价格和 matched size，供 worker 与 Data API 钱包交易做严格最终核验
- **`PolymarketOpenOrder`**：隔离 SDK 的开放订单类型，供 live 订单恢复与对账使用
- 用途：live 模式下的订单管理、rewards live maker 和 copytrade 实盘骨架
- 当前范围：支持已有、已 funded、已 approve 的 Deposit Wallet 通过 CLOB V2 下单/撤单；`poly_1271` 查询余额前会调用 CLOB balance allowance update，刷新失败会直接返回错误，不再继续读取可能陈旧的账户状态。成交同步按 maker order 聚合同一 trade 中重复出现的全部 `matched_amount`，使用 size-weighted price/fee，避免把整笔 taker trade size 误记到单个 maker 订单或漏记重复 maker entry。尚未实现 relayer 建钱包、pUSD 入金/approval 或 deposit wallet 生命周期管理。

### Polymarket Rewards（奖励市场）

- **`PolymarketRewardsConnector`**：`clob_host` + `reqwest::Client`
- **`PolymarketRewardMarket`**：condition_id、question、market_slug、rewards_max_spread、rewards_min_size、total_daily_rate、tokens
- **`PolymarketRewardOrderBook`**：token_id、bids、asks、observed_at
- 常量：`ENRICH_TIMEOUT = 10s`、`ENRICH_MAX_RETRIES = 3`、`ENRICH_RETRY_BASE_DELAY = 500ms`、`MAX_REWARD_MARKET_PAGES = 1000`
- `fetch_current_markets()` 对分页做重复 cursor / 最大页数 / condition id 去重保护；token 会按 ID 去重。仅当原始 market 缺少两个 token 时才要求详情补全，补全后仍不完整或目录为空会返回错误，调用方保留上一版 catalog；详情请求失败但原始记录已完整时不阻断目录替换。
- `fetch_order_books(token_ids)` 优先使用 CLOB `POST /books` 批量拉取盘口；批量请求失败或遗漏 token 时，再使用固定并发窗口逐个调用 `GET /book` 补齐。每个盘口必须携带可解析的 CLOB 毫秒 `timestamp`，并作为 `observed_at` 传给缓存；整批无可用盘口时返回 dependency error，部分失败会记录告警。
- 用途：`market_sync.rs` 填充 `reward_markets` 表

### Orderbook HTTP

- **`OrderbookHttpClient`**：API/Worker 读取独立 orderbook 服务缓存；Worker 通过 `register_tokens()` 原子替换来源 token 集合
- `get_books()` 使用 `POST /orderbook/batch` 一次读取多个 token；rewards full/reconcile 不再逐 token 发 HTTP 请求
- register/ingest/delete 写请求携带 `x-polyedge-orderbook-token`，值来自 `POLYEDGE_ORDERBOOK__WRITE_TOKEN`
- HTTP 注册和注销失败会返回 `Result` 错误，不再静默吞掉非成功响应
- 单盘口读取只把 404 映射为 `None`；其他非成功 HTTP 状态会作为 dependency error 返回，不尝试把错误响应解码成盘口

### News（RSS/Atom 新闻）

- **`NewsSource`** trait：`async fn fetch(&self) -> Result<Vec<ConnectorNewsItem>>`
- **`RssNewsConnector`**：`config` + `reqwest::Client`，User-Agent: `polyedge-news-ingestor/0.1`
- **`ConnectorNewsItem`**：source、external_id、title、url、author、published_at、content_snippet、raw_payload
- 用途：`news.rs` worker 从多个 RSS 源采集新闻

### Paper Trading（内置模拟执行器）

- **`PaperExecutor`**（无状态）：`submit()`、`reconcile_fill()`、`poll_order_status()`
- **`PaperOrderRequest`**/**`PaperOrderAcceptance`**/**`PaperOrderRejection`**：提交/接受/拒绝
- **`PaperFillRequest`**/**`PaperFillReceipt`**：成交流通
- **`PaperExecutionOutcome`**：`Submitted(PaperOrderAcceptance)` | `Rejected(PaperOrderRejection)`
- 常量：`PAPER_EXECUTOR_NAME`、`PAPER_ACCOUNT_ID`、`PAPER_MIN_NOTIONAL_USD`

## 依赖关系

- **上游**：`domain`（AppError、枚举、数值类型）、`application`（部分 trait）
- **下游**：`apps/orderbook`（market sync、WS/poll）、`apps/worker`（arbitrage、copytrade、rewards、news）

## 当前状态

- 已实现当前系统使用的 Polymarket 公共市场、盘口、Data API、Rewards API、订单 scoring、当日 maker earnings 和 CLOB V2 交易 connector；Deposit Wallet relayer 生命周期接口尚未接入
- Gamma keyset 分页已具备重复 cursor / 末页 sentinel / 最大页数保护，并按 market id 去重，避免外部 API 游标异常导致 market sync 无限累积内存
- Rewards markets 分页和 enrichment 已具备完整性保护，不再把部分补全结果作为完整目录写入
- Rewards 盘口连接器优先走 CLOB 批量 `/books`，并对失败或遗漏项使用单 token `/book` 回退
- Data API positions 已按完整快照分页读取；不完整或失败的响应不会被 rewards worker 用于替换持仓
- Paper Trading 执行器已完整实现
- Live connector 已具备 CLOB V2 认证、余额查询、开放订单全量分页、用户 WS、订单提交、按 token_id 的 rewards buy/sell 提交和单笔撤单能力；post-only 使用 GTC，immediate flatten 使用 FAK，订单价格当前统一收敛到 0.01 精度，更粗的 per-market tick-size 尚未接入；订单/关联成交优先通过单订单接口对账，关联 trade 单查失败和 missing-order 都会按 token/time 扫描账户 trades 精确回退，轮询路径仅在 trade `CONFIRMED` 后返回成交，任一订单回退失败可被 worker 单订单隔离；签名类型已覆盖 EOA、Proxy、Gnosis Safe 和 Deposit Wallet (`poly_1271`)，其 balance allowance refresh 失败会传播给调用方；Polygon pUSD 余额 connector 已作为 rewards snapshot 的链上余额回退；订单 acceptance 返回实际提交 quantity，trade/WS 成交归一化按订单自身成交量入账；仍需要真实凭证和小额账户验证
- RSS connector 支持 Atom/RSS 两种格式

## 修改检查清单

- [ ] 新增 connector 时在 `lib.rs` 添加模块声明和 `pub use` 导出
- [ ] 修改 Polymarket API 调用时，检查对应的 worker 是否需要更新
- [ ] 新增/修改数据模型时，同步更新 `market_sync.rs` 中的转换函数
- [ ] WebSocket 相关修改后，检查 `orderbook_stream.rs` 和 `polymarket_events.rs`
- [ ] 运行 `cargo check --workspace --tests`
