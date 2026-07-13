# Replay App（回放服务）

最后更新：2026-07-13

## 概述

`polyedge-replay` 是 Rewards 历史策略运行审计与后续确定性回放入口；当前控制台不暴露 `/replay` 页面。

## 设计目标

- 从数据库读取指定或最新 Rewards strategy run，聚合 blocker、eligible、fair-value、AI/info-risk action 与 action 状态。
- 从本地 JSON 或数据库持久化 fixture 重跑纯 `RewardDecisionEngine`，并与 expected plans 做确定性比较。
- 所有模式保持只读，不触发 provider、数据库写入或 live side effect。

## 架构与关键文件

| 文件 | 职责 |
|---|---|
| `packages/backend/apps/replay/src/main.rs` | CLI 参数解析、run audit、fixture/stored-fixture 回放和 JSON 报告输出 |

## 核心数据结构

- `ReplayCommand`：`Audit`、`Fixture`、`StoredFixture` 三种只读运行模式。
- `RunAuditReport`：run 身份、状态、配置 hash、decision/eligible/fair-value 统计，以及 blocker、provider action、action type/status 聚合。
- `RewardDecisionReplayFixture` / `RewardDecisionReplayResult`：由 application crate 定义。V1 保留完整 final state/expected plans；V2 引入决策窗口 top-of-book history、final delta 和 expected normalized plan SHA-256；V3 沿用该紧凑编码并固化扩展后的 event-window/fair-value decision 模型。

## 当前状态

- `cargo run -p polyedge-replay -- --run-id <RUN_ID>` 输出指定 run 的 JSON 审计报告；省略参数时读取最新 run。
- `cargo run -p polyedge-replay -- --fixture <FIXTURE.json>` 按 `schema_version` 自动读取 V1/V2/V3；缺省版本字段按 V1 兼容。V1 比较完整 expected plans，V2/V3 比较 normalized plan hash 并输出 reason-code diff。
- `cargo run -p polyedge-replay -- --stored-run-id <RUN_ID>` 直接读取 full tick 自动保存的 fixture 并执行相同确定性回放，同时记录 schema version、SHA-256 和字节数。
- 三种模式都不会执行 live side effect。文件 fixture 模式不加载 `Runtime`，不访问数据库或外部 provider，并使用 fixture 注入时间保证 provider grace 可重复；stored fixture/audit 模式只读 Postgres/in-memory store。
- Full tick 默认异步保存 V3 紧凑 fixture；队列满、安全检查/体积拒绝或保存失败都不会阻断 live tick。历史 V1/V2 reader 仍保留。盘口 fill risk、exit cost 与 cancel churn 仿真仍未实现。

## 修改检查清单

- [ ] 修改后运行 `cargo check --workspace --tests`
- [ ] 如有新增功能同步更新本文档
