# 模块文档索引

最后更新：2026-07-16

本目录记录当前活动模块的设计与实现细节。修改模块前先阅读对应文档，修改后同步更新日期、关键文件、数据结构、当前状态和缺口。

V4 在 V3 人工做市语义上增加 Cookie session、RBAC、多租户 ownership、钱包 envelope、策略有效期和跨用户跟随；[人工市场多钱包做市 V3](../designs/manual-market-maker-v3.md) 仅作历史执行语义背景。旧 Rewards V2、事件窗口、AI/provider、fair-value、独立 worker/orderbook/replay 文档均不是当前能力来源。

## 后端（Rust workspace）

| 文档 | 模块 | 职责 |
|---|---|---|
| [server-app.md](backend/server-app.md) | `packages/backend/server` | session/RBAC API、Postgres、钱包加密、targeted orderbook、跟随执行 |
| [domain.md](backend/domain.md) | `packages/backend/crates/domain` | V4 身份、权限、交易领域类型和值对象 |
| [contracts.md](backend/contracts.md) | `packages/backend/crates/contracts` | V4 身份与交易 HTTP DTO |
| [connectors.md](backend/connectors.md) | `packages/backend/crates/connectors` | Polymarket CLOB live、targeted books 与 Data API 适配 |

旧 `api-app.md`、`application.md`、`common.md`、`infrastructure.md`、`orderbook-app.md`、`worker-app.md`、`replay-app.md` 已删除，因为对应活动模块和独立进程在 V3 中不存在。

## 前端（Next.js + React）

| 文档 | 模块 | 职责 |
|---|---|---|
| [data-layer.md](frontend/data-layer.md) | `src/lib/api/*` + `contracts/*` | API client、mutation actions、TypeScript DTO |
| [i18n.md](frontend/i18n.md) | `src/lib/i18n/*` | 中文字典与运行时 |
| [shared-components.md](frontend/shared-components.md) | `src/components/*` + `src/hooks/*` | 共享组件和 hooks |
| [dashboard.md](frontend/dashboard.md) | `features/dashboard` | 多租户概览入口 |
| [strategies.md](frontend/strategies.md) | `features/strategies` | 有效期策略、quote slots 与 owner 钱包 |
| [wallets.md](frontend/wallets.md) | `features/wallets` | 用户自有钱包、WebCrypto 导入和风险限制 |
| [operations.md](frontend/operations.md) | `features/operations` | 执行批次、撤单、订单与持仓账本 |
| [settings.md](frontend/settings.md) | `features/settings` | 单后端运行边界说明 |
| [auth-admin.md](frontend/auth-admin.md) | auth + `features/admin` | Cookie session、用户权限与管理员资金视图 |

SELL exit、merge 和 Funding 不属于当前范围。V4 schema 预留 fills/cash-flow/valuation/equity 核算表，但尚无采集与 snapshot runtime；不要把表结构写成完整盈利核算能力。

## 基础设施

| 文档 | 模块 | 职责 |
|---|---|---|
| [database.md](infra/database.md) | `migrations_v2/` + `init.sql` | V4 multi-user clean-deploy schema |
| [deployment.md](infra/deployment.md) | `deploy/` + `scripts/` | `polyedge-server` + `polyedge-front` 部署 |

## 维护规则

1. 修改前阅读对应文档。
2. 修改后更新日期、关键文件、公开结构、状态与缺口。
3. 新模块必须同时增加文档与索引；删除模块必须删除或明确归档对应文档。
4. 历史设计不得覆盖根 `AGENTS.md` 与本目录的当前状态。
