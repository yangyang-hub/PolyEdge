# connectors（Polymarket CLOB/Data API）

最后更新：2026-07-16

## 概述

`polyedge_connectors` 只封装 V4 仍需要的 Polymarket 协议：指定 token 盘口、每钱包认证 CLOB 订单/余额，以及 Data API 钱包持仓读取。它不负责选市场、不读取数据库、不执行业务策略。

V4 活动路径不包含 Gamma、RSS/Atom、AI/provider、info-risk、Polygon chain、独立 orderbook service client、paper executor、rewards catalog 扫描或 price-history 数据源。

## 活动导出与关键文件

| 文件 | 职责 |
|---|---|
| `src/lib.rs` | 只导出 Polymarket live/Data API 类型和 targeted book connector |
| `src/polymarket.rs` | Polymarket 共享类型、CLOB live 与 Data API 实现聚合 |
| `src/polymarket/live.rs` | 每钱包认证、余额、开放订单匹配、指定订单状态查询、下单与撤单 |
| `src/polymarket/order_reconciliation.rs` | 将 CLOB 单订单状态映射为 open/partial/terminal/unknown 生命周期 |
| `src/polymarket/data_api.rs` | 指定钱包完整 positions 的只读分页接口 |
| `src/polymarket/models.rs`、`helpers.rs` | 共享 wire model、认证签名和校验 helper |
| `src/targeted_orderbook.rs` + `src/targeted_orderbook/requests.rs` | 指定 token 集的 CLOB `/books` 批量读取，供 targeted supervisor 使用 |

## Targeted book

`polyedge-server` 只调用 `fetch_order_books(token_ids)`，输入来自数据库中已启用人工策略、open-like managed orders 和非零 positions。connector 不生成 token universe，也不能补充候选市场。

批量响应按 token 解析 bids/asks 与上游时间；server 再负责排序、过滤非法 level、写入本地 `confirmed_at` 和 freshness 校验。整批或部分缺失都会向上返回依赖错误，执行路径 fail closed。

## Live CLOB

每个钱包由 `WalletSecretResolver` 生成独立 `LivePolymarketConfig`：signer/funder/signature type 来自数据库，private key 与 API credentials 来自 secret provider。

活动执行使用：

- `refresh_balance()` 更新可用 collateral；
- `find_matching_open_token_order()` 在新 place 前做 venue-first 幂等匹配；
- `submit_token_order()` 提交人工 quote slot 对应的 BUY；
- `cancel_order()` 撤销已知 venue order；
- `list_open_orders()` 供 managed-order 核验与开放订单风险计数；open set 中缺失的 managed order 通过 `order_snapshot()` 精确查询，不做账户历史扫描。

connector 只返回明确 acceptance/rejection/unknown；server runtime 负责 managed-order 状态、slot fencing、风险和是否重试。

## Data API

Data API 只暴露按已配置 funder 地址分页读取完整 `/positions`，单页 500、最多 100 页；重复/超限/解析失败会返回错误而不是交付部分 snapshot。Server 把上游 token 映射到人工管理市场，忽略未知 token，并对缺失的已管理 token 写零；该接口不得用于市场选择。

## 安全约束

- connector 不记录 private key、API secret/passphrase、认证 header 或完整敏感响应；live config 与 authenticated connector 的 `Debug` 显式脱敏，并在 drop 时清零其持有的 secret 字符串。
- secret 不得进入 domain/contracts、Postgres、API response 或前端 bundle。
- 外部请求错误返回稳定错误码和受限上下文；不能把凭证或签名 payload 拼入错误。
- API handler 不直接构建 connector；外部调用由 orderbook/execution/account-sync background runtime 发起。

## 当前状态与缺口

- targeted `/books`、CLOB 认证/余额、venue open-order 匹配、下单和撤单已接入 server runtime。
- Data API wallet-position connector 已接入每钱包 execution job；上游失败会使该 job fail closed，不使用旧 snapshot 下单。
- market-channel WebSocket 未接入 V4 targeted orderbook，当前使用 REST poll。
- 旧 Gamma/news/provider/orderbook-service/rewards-catalog/price-history/chain 源已从活动 crate 删除，不得重新导出。

## 修改检查清单

- [ ] 新 connector 能力必须由人工目标市场或指定钱包驱动，不能引入全市场发现。
- [ ] 修改 live/order schema 时同步 execution/account sync、domain/contracts 与测试。
- [ ] 修改 secret/认证逻辑时做日志和错误脱敏审计。
- [ ] 运行 `cargo check --workspace --tests` 与相关 connector 测试。
