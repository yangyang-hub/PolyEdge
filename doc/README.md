# PolyEdge 文档索引

最后更新：2026-07-15

## 当前事实来源

1. [../AGENTS.md](../AGENTS.md) — 仓库状态、架构约束、命令、配置和缺口。
2. [designs/manual-market-maker-v3.md](designs/manual-market-maker-v3.md) — V3 规范性设计。
3. [modules/README.md](modules/README.md) — 当前活动模块文档索引。
4. [polyedge-design.md](polyedge-design.md) — 当前系统总体架构摘要。
5. [polyedge-backend-design.md](polyedge-backend-design.md) — 当前单后端架构摘要。

发生冲突时，以根 `AGENTS.md` 和模块文档中的当前状态为准，不把设计目标或预留 schema 当作已实现能力。

## V3 主题

- 人工录入 condition、YES/NO token、rewards 条款、quote slots、数量和定价方式；
- 同一策略统一作用于多个钱包；
- 保留 deterministic cancel 与 price-change cancel-replace；
- targeted orderbook 只覆盖人工目标、managed orders 和 positions；
- 单 `polyedge-server` 后端，前后端分离；
- clean deploy，不兼容旧数据；
- 无 events/news/AI/info-risk/fair-value/full-market scan。

V3 之前的 Rewards、事件窗口、AI/provider、独立服务、旧 API 与旧 schema
设计文档已删除，避免被误当作当前实现或后续兼容要求。
