# PolyEdge 文档索引

最后更新：2026-07-16

## 当前事实来源

1. [../AGENTS.md](../AGENTS.md) — 仓库状态、架构边界、活动 API、命令、配置和已知缺口。
2. [../README.md](../README.md) — V4 项目入口、本地验证和部署摘要。
3. [modules/README.md](modules/README.md) — 当前活动模块的实现级文档索引。

发生冲突时，以根 `AGENTS.md` 和对应模块文档为准；不要把数据库预留结构、页面目标或 Git 历史中的旧设计写成已实现能力。

## 活动文档范围

- `modules/backend/`：domain、contracts、Polymarket connectors 和唯一后端 `polyedge-server`。
- `modules/frontend/`：Cookie-session 控制台、数据层、钱包、策略、跟随、执行和管理员视图。
- `modules/infra/`：V4 clean-deploy schema 与双容器部署。

仓库不再保留 V3 或更早架构设计副本。需要追溯旧 Rewards、事件/新闻、AI/provider、fair-value、Bearer/step-up、credential locator 或独立 worker/orderbook/replay 设计时，请查阅 Git 历史；这些内容不是当前兼容要求。
