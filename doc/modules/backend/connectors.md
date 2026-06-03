# connectors（外部连接器层）

最后更新：2026-06-03

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
| `polymarket/gamma.rs` | 公共市场元数据：`PolymarketGammaConnector`；Gamma keyset 分页 guard |
| `polymarket/data_api.rs` | 钱包活动 API：`PolymarketDataApiConnector` |
| `polymarket/book.rs` | 盘口快照：`PolymarketBookConnector` |
| `rewards.rs` | 奖励市场：`PolymarketRewardsConnector` |
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
- 常量：`MAX_DATA_API_LIMIT = 500`、`DATA_API_TIMEOUT = 15s`
- 用途：`copytrade.rs` worker 检测跟踪钱包的新成交

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
- `submit()`：兼容 execution pipeline 的 YES/NO 买单提交
- `submit_token_order()`：按 token_id 直接提交 buy/sell GTC 订单，支持 post-only；返回实际提交 quantity，供 rewards live maker 使用
- `cancel_order()`：按 Polymarket order id 撤销单笔订单
- 用途：live 模式下的订单管理、rewards live maker 和 copytrade 实盘骨架
- 当前范围：支持已有、已 funded、已 approve 的 Deposit Wallet 通过 CLOB V2 下单/撤单；`poly_1271` 下单前会调用 CLOB balance allowance update。成交同步会对 maker 订单使用对应 maker order 的 `matched_amount`，避免把整笔 taker trade size 误记到单个 maker 订单。尚未实现 relayer 建钱包、pUSD 入金/approval 或 deposit wallet 生命周期管理。

### Polymarket Rewards（奖励市场）

- **`PolymarketRewardsConnector`**：`clob_host` + `reqwest::Client`
- **`PolymarketRewardMarket`**：condition_id、question、market_slug、rewards_max_spread、rewards_min_size、total_daily_rate、tokens
- **`PolymarketRewardOrderBook`**：token_id、bids、asks、observed_at
- 常量：`ENRICH_TIMEOUT = 10s`、`ENRICH_MAX_RETRIES = 3`、`ENRICH_RETRY_BASE_DELAY = 500ms`
- `fetch_order_books(token_ids)` 使用固定并发窗口（默认 10）拉取 `/book`，不会为全部 token 一次性创建任务。
- 用途：`market_sync.rs` 填充 `reward_markets` 表

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
- **下游**：`apps/worker`（market_sync、arbitrage、copytrade、rewards、news、orderbook_stream）

## 当前状态

- 已实现当前系统使用的 Polymarket 公共市场、盘口、Data API、Rewards API 和 CLOB V2 交易 connector；Deposit Wallet relayer 生命周期接口尚未接入
- Gamma keyset 分页已具备重复 cursor / 末页 sentinel / 最大页数保护，并按 market id 去重，避免外部 API 游标异常导致 market sync 无限累积内存
- Paper Trading 执行器已完整实现
- Live connector 已具备 CLOB V2 认证、用户 WS、订单提交、按 token_id 的 rewards buy/sell 提交和单笔撤单能力；签名类型已覆盖 EOA、Proxy、Gnosis Safe 和 Deposit Wallet (`poly_1271`)；订单 acceptance 返回实际提交 quantity，trade/WS 成交归一化按订单自身成交量入账；仍需要真实凭证和小额账户验证
- RSS connector 支持 Atom/RSS 两种格式

## 修改检查清单

- [ ] 新增 connector 时在 `lib.rs` 添加模块声明和 `pub use` 导出
- [ ] 修改 Polymarket API 调用时，检查对应的 worker 是否需要更新
- [ ] 新增/修改数据模型时，同步更新 `market_sync.rs` 中的转换函数
- [ ] WebSocket 相关修改后，检查 `orderbook_stream.rs` 和 `polymarket_events.rs`
- [ ] 运行 `cargo check --workspace --tests`
