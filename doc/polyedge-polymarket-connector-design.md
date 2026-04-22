# PolyEdge Polymarket 连接器设计

## 1. 文档目标

本文档定义 PolyEdge 与 Polymarket 交互时的实现边界，覆盖：

1. CLOB REST / WebSocket 接入。
2. Gamma / Data API 辅助数据接入。
3. CLOB 下单、撤单、订单查询和余额查询。
4. 链上 CTF `split` / `merge` / `redeem`。
5. GnosisSafe / proxy wallet 场景下的代理执行。

本文件补充 [polyedge-backend-design.md](./polyedge-backend-design.md) 中 `connectors` 与 `execution` 的实现细节，不替代风控、信号或 API 契约文档。

---

## 2. 设计原则

1. Polymarket 接入层只负责“连接器语义”，不负责策略判断。
2. 链上交互与 CLOB API 交互分层，不混在同一个 executor 中。
3. WebSocket 行情、CLOB 下单、链上赎回、Data API 查询分别建独立适配器。
4. 连接器输出必须转为 PolyEdge 内部统一模型，不直接把 SDK response 透传给业务层。
5. 所有外部状态变更都要经过去重、状态映射和审计落库。

---

## 3. 建议模块划分

建议在 `backend/crates/connectors/polymarket` 或等价 crate 下拆分：

1. `gamma_client`
   市场发现、市场元数据、token 映射。
2. `clob_ws_feed`
   订单簿实时流订阅、重连、resubscribe。
3. `clob_trading_client`
   CLOB 鉴权、下单、撤单、订单查询、余额查询。
4. `data_api_client`
   持仓、活动、辅助校验。
5. `ctf_client`
   `split` / `merge` / `redeem` / `redeem_neg_risk`。
6. `safe_executor`
   GnosisSafe 代理钱包执行。
7. `mapper`
   外部 DTO -> PolyEdge 内部模型映射。

建议再由 `infrastructure` 层提供一个组合根：

```text
PolymarketConnector
├── gamma_client
├── clob_ws_feed
├── clob_trading_client
├── data_api_client
├── ctf_client
└── safe_executor (optional)
```

---

## 4. 配置模型

建议在配置中增加独立 `polymarket` 节：

```toml
[polymarket.chain]
chain_id = 137
rpc_url = "https://..."
rpc_fallbacks = ["https://...", "https://..."]

[polymarket.clob]
host = "https://clob.polymarket.com"
ws_host = "wss://ws-subscriptions-clob.polymarket.com"
signature_type = 2
proxy_wallet = "0x..."
ws_max_instruments = 350
first_message_warn_secs = 15
stale_reconnect_secs = 60

[polymarket.gamma]
host = "https://gamma-api.polymarket.com"

[polymarket.data_api]
host = "https://data-api.polymarket.com"

[polymarket.execution]
min_order_usdc = 1.0
default_order_type = "fok"
post_only_enabled = false
```

字段语义：

1. `signature_type`
   `0=eoa`、`1=proxy`、`2=gnosis_safe`
2. `proxy_wallet`
   当资金托管在 Polymarket proxy / Safe 中时，用于余额和仓位查询
3. `ws_max_instruments`
   单连接最大订阅 token 数
4. `first_message_warn_secs`
   创建 stream 后迟迟收不到首包时的诊断阈值
5. `stale_reconnect_secs`
   已建立流后无消息强制重连阈值

参考实现：

1. `/home/yangyang/workspace/polygon/PolyAlpha/crates/pa-core/src/config.rs`
2. `packages/backend/.env.example`

---

## 5. 账户模型

### 5.1 支持的账户类型

首版建议显式支持三种账户模型：

1. `eoa`
   直接由本地私钥签名
2. `proxy`
   邮箱钱包 / magic wallet 之类的代理签名模式
3. `gnosis_safe`
   浏览器钱包常见模式，资产和代币持仓在 Safe / proxy wallet 中

### 5.2 配置要求

每个交易账户建议包含：

1. `account_id`
2. `label`
3. `signature_type`
4. `private_key_ref`
5. `proxy_wallet`
6. `safe_address`
7. `enabled`

规则：

1. `signature_type=eoa` 时，`proxy_wallet` 可为空。
2. `signature_type=proxy` 或 `gnosis_safe` 时，`proxy_wallet` 必须显式配置。
3. `gnosis_safe` 时，若需要链上赎回，必须额外配置 `safe_address`。

### 5.3 余额语义

PolyEdge 需要区分三类余额：

1. `wallet_native_balance`
   EOA 的 MATIC 余额，用于 gas
2. `clob_collateral_balance`
   CLOB / proxy wallet 中可用 USDC
3. `ctf_position_balance`
   链上持有的 ERC-1155 outcome token

业务层不得把这三类余额混为同一个“账户余额”。

---

## 6. CLOB WebSocket 设计

### 6.1 订阅对象

首版 WebSocket 主要用于：

1. 订单簿更新
2. 最优买卖价变化
3. 盘口深度变化

建议订阅粒度为 `token_id`，不是 `market_id`。

### 6.2 本地组件

`clob_ws_feed` 建议内部持有：

1. `ws_client`
2. `orderbook_cache`
3. `update_tx`
4. `cancel_token`
5. `tasks`
6. `ws_connected`
7. `ws_last_message_unix`

### 6.3 连接生命周期

建议流程：

```text
build ws stream
-> wait first message
-> mark connected
-> update cache
-> publish local update event
-> stale watchdog
-> reconnect with exponential backoff
```

必须具备：

1. 首包确认
2. 首包迟到告警
3. 消息停滞 watchdog
4. 指数退避
5. resubscribe
6. 主动 shutdown

### 6.4 订阅上限

1. 单 WebSocket 连接的订阅数不得超过配置阈值。
2. 当市场数量超过阈值时，首版允许截断并告警。
3. 后续若需要更大规模，可扩展为多连接分片。

### 6.5 本地缓存规则

收到订单簿更新后：

1. 转换为内部 `OrderBook`
2. bids 按价格降序排序
3. asks 按价格升序排序
4. 写入本地 `OrderBookCache`
5. 广播 `OrderBookUpdate`

### 6.6 失败与恢复

必须覆盖：

1. stream 创建失败
2. stream 中途报错
3. stream 正常结束
4. 建连成功但长时间无消息

恢复策略：

1. 标记 `ws_connected=false`
2. 增加 `ws_reconnect_count`
3. 退避后重建订阅
4. 重连后由业务层按需做 REST 快照校准

参考实现：

1. `/home/yangyang/workspace/polygon/PolyAlpha/crates/pa-market-data/src/ws_feed.rs`

---

## 7. CLOB 鉴权与交易接口

### 7.1 鉴权

首版建议继续使用 `polymarket-client-sdk` 的 CLOB client 进行鉴权：

1. 根据配置创建 unauthenticated client
2. 通过 signer + `signature_type` 完成 authentication builder
3. 输出 authenticated client 供下单和查询使用

### 7.2 支持的能力

`clob_trading_client` 首版至少提供：

1. `buy_fok`
2. `sell_fok`
3. `buy_limit_post_only`
4. `sell_limit_post_only`
5. `cancel_order`
6. `cancel_orders`
7. `cancel_all`
8. `get_balance`
9. `get_orders_by_market`

### 7.3 订单类型

首版推荐：

1. 方向性信号默认使用 `FOK`
2. 流动性提供或挂单管理场景使用 `GTC + post_only`

### 7.4 精度与最小金额规则

Polymarket CLOB 有两个关键限制：

1. 订单 `size` 需要符合 lot size 精度
2. `price * size` 对应的 USDC cost 需要满足 2 位小数精度

因此连接器层必须负责：

1. 对 size 做 2 位小数 round
2. 对 `price * size` 做 cost precision 修正
3. 对买单执行 `$1.00` 最低成本检查
4. 对修正后 size 为 0 的订单直接拒绝

这部分必须放在连接器层，而不是散落在策略层。

### 7.5 订单响应映射

连接器应把外部 response 映射到内部统一状态：

| Polymarket 状态 | 内部订单状态建议 |
| --- | --- |
| `matched` | `filled` |
| `delayed` | `filled` |
| `live` | `open` |
| API success=false | `rejected` |
| 未成交但已接受 | `submitted` 或 `open`，按 response 细节映射 |

内部还应补齐：

1. `external_order_id`
2. `request_id`
3. `idempotency_key`
4. `connector_name='polymarket'`
5. `tx_hashes`

### 7.6 余额查询

`get_balance` 返回的是 proxy wallet / CLOB 可用抵押品余额，不是 EOA 钱包原生余额。

业务层必须明确：

1. 下单能力由 `clob_collateral_balance` 决定
2. 链上 gas 能力由 `wallet_native_balance` 决定

参考实现：

1. `/home/yangyang/workspace/polygon/PolyAlpha/crates/pa-execution/src/clob_executor.rs`

---

## 8. Data API 与辅助查询

Data API 建议用于：

1. 持仓和活动辅助查询
2. 钱包画像或对账辅助
3. 与链上事件监控交叉确认

不建议将 Data API 当作唯一真值来源。

首版原则：

1. 下单和撤单真值仍以 CLOB API / 回报为准
2. 链上赎回真值仍以链上 receipt 为准
3. Data API 作为观测与校准层

参考实现：

1. `/home/yangyang/workspace/polygon/PolyAlpha/crates/pa-market-data/src/wallet_tracker.rs`

---

## 9. 链上 CTF 交互

### 9.1 适用操作

`ctf_client` 首版支持：

1. `split`
2. `merge`
3. `redeem`
4. `redeem_neg_risk`

### 9.2 常量与链参数

首版应固定：

1. Chain ID：`137`
2. USDC：`0x2791Bca1f2de4661ED88A30C99A7a9449Aa84174`
3. ConditionalTokens：`0x4D97DCd97eC945f40cF65F87097ACe5EA0476045`

NegRisk 相关地址应作为配置或常量显式维护，不要在业务层写死。

### 9.3 前置条件

执行链上操作前要满足：

1. `split` 前 USDC allowance 已足够
2. `merge` / `redeem` 前 outcome token 已存在
3. 若经 Safe 代理执行，则调用必须从 Safe 发起

### 9.4 返回值

链上操作统一返回：

1. `tx_hash`
2. `block_number`

并在内部补写：

1. `account_id`
2. `request_id`
3. `trace_id`
4. `connector_name='polymarket_ctf'`

参考实现：

1. `/home/yangyang/workspace/polygon/PolyAlpha/crates/pa-execution/src/ctf_executor.rs`

---

## 10. GnosisSafe / Proxy Wallet 特殊处理

### 10.1 问题背景

当 `signature_type=2` 时，资产常常不在 EOA，而在 Safe / proxy wallet 中。

此时：

1. CLOB 签名仍可由 EOA 参与
2. 但链上 `redeemPositions()` 的 `msg.sender` 必须是 Safe

因此链上赎回不能简单由 EOA 直接调用。

### 10.2 解决方式

建议单独实现 `safe_executor`：

1. 编码目标合约 calldata
2. 读取 Safe nonce
3. 计算 Safe `getTransactionHash()`
4. 使用 `eth_sign` 模式签名
5. 调用 `execTransaction()`

### 10.3 启动期检查

系统启动时建议执行：

1. `getOwners()`
2. `getThreshold()`
3. 校验当前 EOA 是否为 owner

若 owner 关系不满足，应在启动时直接降级该账户为不可用。

### 10.4 支持的 Safe 路径

首版只支持：

1. 1-of-1 Safe
2. 本地 signer 为 Safe owner
3. `ETH_SIGN` 签名模式

多签 Safe、高级 gas/refund 参数、自定义 operation 策略可以放到后续版本。

参考实现：

1. `/home/yangyang/workspace/polygon/PolyAlpha/crates/pa-execution/src/safe_redeemer.rs`

---

## 11. Nonce 与链上发送

若 PolyEdge 后续需要显式管理链上 nonce，建议将 nonce 管理器与业务逻辑分离：

1. `next()`
2. `reset()`
3. `current()`

首版如果完全依赖 SDK / provider 自动管理，可暂不暴露该层。
若后续出现 stuck tx、replace tx 或批量链上调用，再引入显式 nonce manager。

参考实现：

1. `/home/yangyang/workspace/polygon/PolyAlpha/crates/pa-execution/src/nonce_manager.rs`

---

## 12. 内部状态映射建议

PolyEdge 不应直接把外部系统状态当内部状态。

建议增加专用映射层：

1. `ExternalOrderStatus -> InternalOrderStatus`
2. `ExternalFillEvent -> TradeDelta`
3. `ExternalBookUpdate -> OrderBook`
4. `ExternalTxReceipt -> ChainSettlementResult`

规则：

1. 任何外部事件在写入业务表前必须先标准化
2. 标准化后再做去重和状态迁移
3. 不允许业务层到处散落 SDK-specific 判断

---

## 13. 首版推荐实现顺序

1. `gamma_client` + 市场/token 映射
2. `clob_ws_feed` + `OrderBookCache`
3. `clob_trading_client` + FOK / cancel / balance
4. `data_api_client` 只做辅助查询
5. `ctf_client` 的 `redeem`
6. `safe_executor`，仅在 `signature_type=2` 账户启用

---

## 14. 参考实现

本项目落地时可以直接参考以下 PolyAlpha 文件：

1. WebSocket：`/home/yangyang/workspace/polygon/PolyAlpha/crates/pa-market-data/src/ws_feed.rs`
2. CLOB 下单：`/home/yangyang/workspace/polygon/PolyAlpha/crates/pa-execution/src/clob_executor.rs`
3. CTF 链上操作：`/home/yangyang/workspace/polygon/PolyAlpha/crates/pa-execution/src/ctf_executor.rs`
4. Safe 代理赎回：`/home/yangyang/workspace/polygon/PolyAlpha/crates/pa-execution/src/safe_redeemer.rs`
5. 配置模型：`/home/yangyang/workspace/polygon/PolyAlpha/crates/pa-core/src/config.rs`
6. 默认配置示例：`packages/backend/.env.example`
7. 依赖组合：`/home/yangyang/workspace/polygon/PolyAlpha/Cargo.toml`

这些文件可作为参考实现，但 PolyEdge 应保留自己的分层：

1. `connectors` 负责外部适配
2. `execution` 负责执行编排和状态机
3. `domain` / `application` 不依赖具体 SDK response
