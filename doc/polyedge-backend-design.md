# PolyEdge V3 后端架构

最后更新：2026-07-15

后端唯一部署单元是 `polyedge-server`。详细模块状态见 [server-app.md](modules/backend/server-app.md)，规范性设计见 [manual-market-maker-v3.md](designs/manual-market-maker-v3.md)。

## 组件

```text
Axum Router
  -> contracts DTO
  -> PostgresStore
  -> execution batches/jobs/actions

OrderbookSupervisor
  -> query exact required tokens
  -> CLOB REST /books
  -> in-memory cache

RuntimeSupervisor
  -> claim wallet job
  -> resolve wallet secret
  -> refresh balance
  -> reconcile quote slots
  -> venue-first keep/place/cancel/replace
```

`domain` 提供 V3 状态与账本类型，`contracts` 提供 HTTP DTO，`connectors` 封装 Polymarket 协议，`server` 直接实现 API/store/runtime 组装。不存在活动 `application`/`infrastructure` service layer，也不存在进程间 provider/orderbook client。

Server 配置只接受 `POLYEDGE_POSTGRES__URL` 作为数据库连接入口；启用 Bearer auth 时校验 API token 至少 32 个字符，production 启动时还会校验 step-up code 至少 16 个字符，不仅依赖部署脚本校验。

## 并发与一致性

- batch 创建时固化 published strategy version；
- 每个 batch/wallet 唯一 job；
- `FOR UPDATE SKIP LOCKED` claim；
- wallet mutex + job/action owner/epoch/expiry fencing；
- open-like wallet/slot partial unique index；
- venue ambiguity 与 submission unknown 占用 slot；
- API scope/key/request-hash 幂等并重放完整响应。

## 数据获取

只有 targeted orderbook 和指定钱包的认证 connector 可以访问外部系统。token universe 来自人工策略、managed open orders 和 nonzero positions；禁止 Gamma/rewards catalog/full scan、news/provider/candle/fair-value pipeline。

## 当前实现边界

已实现 API/store、V3 migration、targeted REST cache、每钱包凭证解析、CLOB 余额/开放订单、Data API positions 全量替换、多钱包 BUY quote 对账与批量撤单。账户范围外部订单持续同步、market-channel WS 和生产 session 未完成；SELL exit、merge、Funding 与独立 fills 账本不属于 V3 范围。
