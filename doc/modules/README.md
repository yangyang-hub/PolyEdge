# 模块文档索引

本目录记录项目每个模块的设计与实现细节。**修改任何模块前必须先查阅对应文档；修改后必须同步更新文档。**

## 后端（Rust workspace）

| 文档 | 模块 | 职责 |
|---|---|---|
| [domain.md](backend/domain.md) | `packages/backend/crates/domain` | 领域层：值对象、枚举、错误类型 |
| [application.md](backend/application.md) | `packages/backend/crates/application` | 应用层：业务服务、Store trait、命令类型 |
| [connectors.md](backend/connectors.md) | `packages/backend/crates/connectors` | 连接器层：Polymarket CLOB/Gamma/DataAPI、RSS、Paper Trading |
| [infrastructure.md](backend/infrastructure.md) | `packages/backend/crates/infrastructure` | 基础设施层：持久化、认证、配置、运行时 |
| [contracts.md](backend/contracts.md) | `packages/backend/crates/contracts` | HTTP API DTO 定义 |
| [common.md](backend/common.md) | `packages/backend/crates/common` | 后端二进制共享进程外壳 helper |
| [api-app.md](backend/api-app.md) | `packages/api` | Axum HTTP API 服务 |
| [worker-app.md](backend/worker-app.md) | `packages/backend/apps/worker` | Tokio 后台任务服务 |
| [orderbook-app.md](backend/orderbook-app.md) | `packages/orderbook` | 独立市场同步、盘口流和盘口 HTTP 服务 |
| [replay-app.md](backend/replay-app.md) | `packages/backend/apps/replay` | 历史回放工具 |

## 前端（Next.js + React）

| 文档 | 模块 | 职责 |
|---|---|---|
| [data-layer.md](frontend/data-layer.md) | `src/lib/api/*` + `contracts/*` | 数据层：API Client、Server Actions、DTO 类型镜像 |
| [i18n.md](frontend/i18n.md) | `src/lib/i18n/*` | 国际化：字典、运行时、Provider |
| [shared-components.md](frontend/shared-components.md) | `src/components/*` + `src/hooks/*` | 共享组件和自定义 Hooks |
| [dashboard.md](frontend/dashboard.md) | `features/dashboard` | 仪表盘 |
| [markets.md](frontend/markets.md) | `features/markets` | 市场列表 |
| [events.md](frontend/events.md) | `features/events` | 事件/证据 |
| [signals.md](frontend/signals.md) | `features/signals` | 交易信号 |
| [positions.md](frontend/positions.md) | `features/positions` | 持仓 |
| [radar.md](frontend/radar.md) | `features/radar` | 套利雷达（前端范例模块） |
| [rewards.md](frontend/rewards.md) | `features/rewards` | 奖励机器人 |
| [risk.md](frontend/risk.md) | `features/risk` | 风控中心 |
| [copytrade.md](frontend/copytrade.md) | `features/copytrade` | 跟单 |
| [settings.md](frontend/settings.md) | `features/settings` | 设置 |
| [wallet-analysis.md](frontend/wallet-analysis.md) | `features/wallet-analysis` | 钱包分析 |

## 基础设施

| 文档 | 模块 | 职责 |
|---|---|---|
| [deployment.md](infra/deployment.md) | `deploy/` + `scripts/` | Docker Compose 部署、Nginx、构建脚本 |
| [database.md](infra/database.md) | `packages/backend/migrations/` | 数据库迁移和 Schema |

## 维护规则

1. **查阅优先**：修改任何模块前，先阅读对应的模块文档了解设计约束和依赖关系。
2. **同步更新**：每次修改模块后，更新对应文档的以下内容：
   - 顶部的「最后更新」日期
   - 架构/关键文件表（如有新增/删除文件）
   - 核心数据结构（如有新增/修改类型）
   - 当前状态和已知缺口
3. **新增模块**：新增功能模块时，创建对应的文档文件并添加到本索引。
