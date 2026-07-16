# 模块文档索引

最后更新：2026-07-15

本目录记录当前活动模块的设计与实现细节。修改模块前先阅读对应文档，修改后同步更新日期、关键文件、数据结构、当前状态和缺口。

V3 总体设计基线见 [人工市场多钱包做市 V3](../designs/manual-market-maker-v3.md)。旧 Rewards V2、事件窗口、AI/provider、fair-value、独立 worker/orderbook/replay 文档均为历史材料，不是当前能力来源。

## 后端（Rust workspace）

| 文档 | 模块 | 职责 |
|---|---|---|
| [server-app.md](backend/server-app.md) | `packages/backend/server` | 单后端进程：API、Postgres store、targeted orderbook、多钱包执行 |
| [domain.md](backend/domain.md) | `packages/backend/crates/domain` | V3 领域类型、状态枚举和值对象 |
| [contracts.md](backend/contracts.md) | `packages/backend/crates/contracts` | V3 HTTP API DTO |
| [connectors.md](backend/connectors.md) | `packages/backend/crates/connectors` | Polymarket CLOB live、targeted books 与 Data API 适配 |

旧 `api-app.md`、`application.md`、`common.md`、`infrastructure.md`、`orderbook-app.md`、`worker-app.md`、`replay-app.md` 已删除，因为对应活动模块和独立进程在 V3 中不存在。

## 前端（Next.js + React）

| 文档 | 模块 | 职责 |
|---|---|---|
| [data-layer.md](frontend/data-layer.md) | `src/lib/api/*` + `contracts/*` | API client、mutation actions、TypeScript DTO |
| [i18n.md](frontend/i18n.md) | `src/lib/i18n/*` | 中文字典与运行时 |
| [shared-components.md](frontend/shared-components.md) | `src/components/*` + `src/hooks/*` | 共享组件和 hooks |
| [dashboard.md](frontend/dashboard.md) | `features/dashboard` | V3 概览入口 |
| [strategies.md](frontend/strategies.md) | `features/strategies` | 人工市场、quote slots 与目标钱包 |
| [wallets.md](frontend/wallets.md) | `features/wallets` | 多钱包、credential locator 和风险限制 |
| [operations.md](frontend/operations.md) | `features/operations` | 执行批次、撤单、订单与持仓账本 |
| [settings.md](frontend/settings.md) | `features/settings` | 单后端运行边界说明 |

SELL exit、merge、Funding 与独立 fills 账本已从 V3 schema、后端、前端和文档模块索引删除，不属于当前范围。

## 基础设施

| 文档 | 模块 | 职责 |
|---|---|---|
| [database.md](infra/database.md) | `migrations_v2/` + `init.sql` | V3 clean-deploy schema |
| [deployment.md](infra/deployment.md) | `deploy/` + `scripts/` | `polyedge-server` + `polyedge-front` 部署 |

## 维护规则

1. 修改前阅读对应文档。
2. 修改后更新日期、关键文件、公开结构、状态与缺口。
3. 新模块必须同时增加文档与索引；删除模块必须删除或明确归档对应文档。
4. 历史设计不得覆盖根 `AGENTS.md` 与本目录的当前状态。
