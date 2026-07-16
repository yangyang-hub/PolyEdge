# PolyEdge V3 总体架构

最后更新：2026-07-15

PolyEdge V3 是人工市场配置、多钱包批量执行的 Polymarket 做市系统。规范性设计见 [人工市场多钱包做市 V3](designs/manual-market-maker-v3.md)，当前实现状态见 [AGENTS.md](../AGENTS.md) 与 [模块文档](modules/README.md)。

## 产品边界

Operator 对市场和订单意图负责：录入 condition、YES/NO token、rewards 条款、quote slots、数量、fixed/book-rank 定价和目标钱包。系统负责持久化版本、批量投影、风险校验、目标盘口缓存以及 keep/place/cancel/replace。

系统不自动发现或选择市场，也不使用新闻、事件、AI、info-risk、fair value 或历史 candle 决定方向/数量。

## 系统边界

```text
polyedge-front -> polyedge-server -> PostgreSQL / Polymarket CLOB
```

前后端分离；后端内部不再拆 API、worker、provider 和 orderbook 服务。外部盘口只由后台 targeted supervisor 获取，API handler 不即时访问 Polymarket。

## 核心对象

- Wallet：账户身份、credential locator、交易开关、风险 policy 与状态 snapshot。
- Managed Market：人工录入的 condition 与 YES/NO token。
- Strategy Version：不可变配置与多个 quote slots。
- Quote Slot：稳定 desired-order identity，定义 outcome、quantity、price rule 和边界。
- Execution Batch/Job/Action：把一个版本批量投影到多个钱包并用 lease/idempotency 执行。
- Managed Order/Position：多钱包订单与持仓账本。

## 安全原则

- clean deploy 默认 kill switch locked/trading off；
- secret 不入库、不返回、不记录；
- stale/missing book、依赖错误、风险读取错误和 venue ambiguity 均 fail closed；
- unknown order 继续占用 slot，禁止盲目重试；
- 钱包间有界并行，单钱包串行；
- 所有写 API 幂等，危险操作最小 step-up。

## 当前缺口

targeted orderbook 当前为 REST poll。钱包 job 已同步 CLOB 余额/开放订单与 Data API positions；账户范围外部订单持续同步和生产 session UX 尚未完成。SELL exit、merge、Funding 与独立 fills 账本不属于 V3 范围。
