# PolyEdge 文档索引

最后更新：2026-07-12

## 当前事实来源

以下文档描述当前仓库状态，开发、排障和部署时优先阅读：

1. [../AGENTS.md](../AGENTS.md) — 仓库状态快照、架构约束、运行命令和缺口。
2. [modules/README.md](modules/README.md) — 模块文档索引。
3. [modules/backend/](modules/backend/) — Rust workspace 当前模块说明。
4. [modules/frontend/](modules/frontend/) — 前端控制台当前模块说明。
5. [modules/infra/](modules/infra/) — 数据库和部署当前说明。

## 历史设计文档

`polyedge-*.md` 文件保留为早期产品、架构、契约和实施计划背景。它们可能包含已经移除或尚未落地的内容，例如 approvals 页面、前端 SSE 实时流、旧运行模式和独立 research/replay 页面。当前实际 replay 能力是后端 `polyedge-replay` CLI 和 rewards 自动保存的 replay fixture，不是旧前端页面方案。

阅读这些文档时按以下规则处理：

- 不把设计目标当作已实现能力。
- 与 `AGENTS.md`、`README.md` 或 `doc/modules/*` 冲突时，以当前事实来源为准。
- 修改实际代码后，只在确实改变当前行为时同步更新模块文档；历史设计文档除非作为背景重写，否则不要追加“已实现”表述。

## 历史文档清单

| 文档 | 状态 |
|---|---|
| [polyedge-design.md](polyedge-design.md) | 系统总体早期设计 |
| [polyedge-prototype-design.md](polyedge-prototype-design.md) | 前端原型早期设计 |
| [polyedge-frontend-design.md](polyedge-frontend-design.md) | 前端早期设计 |
| [polyedge-frontend-implementation-plan.md](polyedge-frontend-implementation-plan.md) | 前端早期实施计划 |
| [polyedge-frontend-ui-stack.md](polyedge-frontend-ui-stack.md) | 前端依赖和 UI 栈建议 |
| [polyedge-backend-design.md](polyedge-backend-design.md) | 后端早期设计 |
| [polyedge-backend-implementation-plan.md](polyedge-backend-implementation-plan.md) | 后端早期实施计划 |
| [polyedge-api-contract.md](polyedge-api-contract.md) | API 契约草案；当前路由以 `doc/modules/backend/api-app.md` 和代码为准 |
| [polyedge-storage-schema.md](polyedge-storage-schema.md) | 存储 schema 草案；当前迁移以 `modules/infra/database.md` 和 `packages/backend/migrations/` 为准 |
| [polyedge-auth-design.md](polyedge-auth-design.md) | 鉴权设计背景 |
| [polyedge-internal-auth-token-spec.md](polyedge-internal-auth-token-spec.md) | 内部 token 协议草案 |
| [polyedge-domain-enums-and-decimals.md](polyedge-domain-enums-and-decimals.md) | 枚举和定点数设计背景 |
| [polyedge-llm-governance.md](polyedge-llm-governance.md) | LLM 治理设计背景 |
| [polyedge-polymarket-connector-design.md](polyedge-polymarket-connector-design.md) | Polymarket 连接器设计背景 |

## Rewards 策略文档

| 文档 | 状态 |
|---|---|
| [designs/rewards-market-maker-v2.md](designs/rewards-market-maker-v2.md) | 当前 Rewards 做市策略设计基线；已实现，实时状态仍以 `AGENTS.md` 和模块文档为准 |
| [rewards-event-window-stable-double-strategy-plan.md](rewards-event-window-stable-double-strategy-plan.md) | 历史阶段方案；事件窗口与 BalancedMerge 后续实现已演进，旧 sibling-cancel/AI 盘口描述不代表当前行为 |
| [rewards-market-maker-refactor-design.md](rewards-market-maker-refactor-design.md) | 历史阶段重构记录；当前 durable executor、replay fixture/CLI 和 provider 语义以 V2/模块文档为准 |
