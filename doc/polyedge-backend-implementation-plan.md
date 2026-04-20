# PolyEdge 后端实现计划

## 1. 当前起点

当前仓库基本还是设计文档阶段，`packages/backend/` 目录为空，还没有现成的 Rust workspace、数据库迁移、CI 或运行配置。

这意味着后端实现计划应按“从零起骨架”的方式制定，而不是按增量改造制定。

---

## 2. 实施目标

后端首阶段的目标不是一次性补齐全部功能，而是尽快把下面这条链路稳定打通：

```text
market/event 输入 -> evidence -> pricing/posterior -> signal -> risk -> audit
```

在此基础上，再接执行、实时推送和 replay。

---

## 3. 实施原则

1. `packages/backend/` 作为独立 Cargo workspace，不依赖前端 monorepo 工具链。
2. 先搭 `domain / application / infrastructure / contracts` 分层，再接 HTTP、worker 和 connector。
3. 鉴权、审计、幂等、outbox 从第一阶段开始落地，不作为“后补优化项”。
4. 数值、枚举、错误码、token 协议严格以现有设计文档为准，不在代码里重新发明一套口径。
5. 先实现 `research -> paper_trade -> manual_confirm`，最后才放开 `live_auto`。
6. Polymarket 连接器只复用 `PolyAlpha` 的基础设施经验，不复用策略或做市逻辑。

---

## 4. 建议目录

建议直接落成如下结构：

```text
packages/backend/
  Cargo.toml
  rust-toolchain.toml
  clippy.toml
  apps/
    api/
      src/
    worker/
      src/
    replay/
      src/
  crates/
    domain/
      src/
    application/
      src/
    infrastructure/
      src/
    connectors/
      src/
    contracts/
      src/
  migrations/
  config/
  tests/
    fixtures/
```

说明：

1. `apps/api` 负责 Axum HTTP API、SSE 和鉴权边界。
2. `apps/worker` 负责 ingestion、pricing、signal、risk、reconciliation 等后台任务。
3. `apps/replay` 负责离线回放、研究评估和回测式验证。
4. `crates/domain` 放值对象、状态机、领域实体和不依赖外部 IO 的规则。
5. `crates/application` 放 use case、事务边界和跨模块编排。
6. `crates/infrastructure` 放 PostgreSQL、Redis、auth verifier、outbox、审计、配置、tracing。
7. `crates/connectors` 放 Polymarket Gamma/CLOB/Data API/CTF/Safe 等外部适配器。
8. `crates/contracts` 放 HTTP DTO、SSE event、错误响应和 contract 层序列化模型。

---

## 5. 里程碑计划

### M0. 工程骨架与运行底座

目标：让仓库具备“能编译、能启动、能迁移、能测”的最小工程能力。

主要任务：

1. 初始化 Cargo workspace 和各 app/crate。
2. 建立统一错误模型、配置加载、tracing、`request_id` 贯穿。
3. 建立 `cargo fmt`、`cargo clippy`、`cargo test` 的本地命令和 CI。
4. 接入 PostgreSQL、Redis、本地开发配置和迁移框架。
5. 暴露 `GET /healthz`、`GET /readyz` 和 worker 启动入口。

建议依赖：

1. `tokio`
2. `axum`
3. `tower` / `tower-http`
4. `sqlx`
5. `redis`
6. `tracing` / `tracing-subscriber`
7. `rust_decimal`
8. `serde` / `serde_json`
9. `uuid`
10. `time`

交付物：

1. `packages/backend/` 可独立 `cargo check`
2. 本地可执行数据库迁移
3. API 进程和 worker 进程可分别启动

验收条件：

1. 健康检查可用
2. PostgreSQL/Redis 连接失败时能明确报错
3. CI 至少包含 `fmt + clippy + test`

建议时长：3 到 4 个工作日

### M1. 共享规范层与基础支撑能力

目标：先把所有后续功能都会依赖的“共用真值层”立住。

主要任务：

1. 在 `domain` 中实现枚举和值对象：
   `Probability`、`Quantity`、`Edge`、`ExposureRatio`、各状态枚举。
2. 在 `contracts` 中实现 API 字符串数值序列化和错误响应模型。
3. 在 `infrastructure` 中落地：
   - `audit_logs`
   - `idempotency_keys`
   - `outbox_events`
   - `external_event_dedup`
   - `llm_calls`
   - `mode_transitions`
4. 实现内部 token verifier、中间件、角色判定和 step-up 校验。
5. 落地请求审计、幂等键占用与事务封装。

交付物：

1. 共享 domain 类型和 contract DTO
2. 支撑表 migration
3. 受保护的最小 API，比如 `GET /api/v1/system/mode`
4. 最小写接口样例，验证 `Authorization + X-Request-Id + Idempotency-Key`

验收条件：

1. 真值链路禁止使用浮点
2. token 验签、`iss`/`aud`/`kid`/`exp`/`step_up_scope` 有完整测试
3. 所有写操作默认有审计记录和幂等保护

建议时长：5 到 7 个工作日

### M2. 基础域与输入链路

目标：把 `market / resolution / event` 三个基础域和输入链路建起来。

主要任务：

1. 定义 `markets`、`market_resolution_rules`、`raw_events`、`events` 等核心表。
2. 实现 `market`、`resolution`、`event` 的 domain model、repo 和 use case。
3. 在 worker 中实现：
   - 市场同步任务
   - 外部事件标准化任务
   - 去重与 upsert 逻辑
4. 先做 fixture 驱动或手工导入，不把全部外部源一次接完。
5. 为后续 signal 链路预留 outbox event。

交付物：

1. 市场和事件的落库能力
2. 后台同步任务和最小查询 API
3. `external_event_dedup` 真正参与写入路径

验收条件：

1. 同一外部事件重复投递不会重复入库
2. 市场与事件数据能被 API 稳定查询
3. worker 重启后任务可恢复，不依赖进程内状态

建议时长：5 到 7 个工作日

### M3. Evidence、Pricing 与 Signal 链路

目标：把“输入信息 -> 概率更新 -> 可执行信号”打通。

主要任务：

1. 实现 `evidence`、`probability_estimates`、`signals` 相关表和 domain model。
2. 建立 `research` / `pricing` / `signal` 三个 application use case。
3. 落地 LLM 调用包装层：
   - prompt/version 记录
   - schema 校验
   - `llm_calls` 审计
   - 降级处理
4. 实现 signal 状态机：`new -> active -> weakened/executed/invalidated/reversed/expired`
5. 提供 signal 查询和详情 API。

交付物：

1. 基于 fixture 的 evidence 处理链路
2. posterior/fair price 计算结果
3. signal 生成和审计记录

验收条件：

1. 同一输入在 replay 模式下能得到确定性 signal 结果
2. signal 的每次状态变化都有 trace_id 和审计记录
3. LLM 失败时系统能降级，不会把链路直接打死

建议时长：7 到 10 个工作日

### M4. 风控与审批流

目标：在执行前先把风控和人工控制闭环做稳。

主要任务：

1. 实现 `risk_state`、阈值规则和模式切换 use case。
2. 落地 `research / paper_trade / manual_confirm / live_auto / kill_switch_locked` 状态机。
3. 实现审批流接口：
   - 审批 signal
   - 拒绝 signal
   - 切换 mode
   - 触发 / 释放 kill switch
4. 对高风险接口启用 step-up scope 校验。
5. 先让 risk 成为 execution 的前置门，而不是后置告警。

交付物：

1. risk gate 规则引擎
2. 管理操作 API
3. `mode_transitions` 和审计留痕

验收条件：

1. 未满足风控或 step-up 时禁止下游执行
2. mode 迁移只允许合法状态跳转
3. 风控判定结果可追溯到输入数据和阈值版本

建议时长：5 到 7 个工作日

### M5. Execution 与 Polymarket 连接器

目标：在风控之后接入真实执行和对账。

主要任务：

1. 在 `connectors` 中实现：
   - `gamma_client`
   - `clob_ws_feed`
   - `clob_trading_client`
   - `data_api_client`
   - `ctf_client`
   - `safe_executor`
2. 在 `domain/application` 中实现：
   - `orders`
   - `trades`
   - `positions`
   - `execution_request`
   - 订单状态机
3. 先完成只读行情和订单回报映射，再接下单/撤单。
4. 复用 `PolyAlpha` 在 ws 重连、CLOB 鉴权、CTF、Safe 代理执行上的经验。
5. 建立 reconciliation worker，把外部订单/成交状态对齐到内部真值。

交付物：

1. Polymarket 只读 connector
2. 订单提交与取消 use case
3. 成交回报处理和持仓更新

验收条件：

1. 外部状态到内部状态的映射是显式和可测试的
2. 下单、撤单、成交回报都具备幂等处理
3. paper/manual 模式先打通，再开放 live

建议时长：10 到 15 个工作日

### M6. 查询 API、SSE 与控制台读模型

目标：给前端控制台提供稳定的查询和实时接口。

主要任务：

1. 在 `apps/api` 中按模块拆路由：
   - `markets`
   - `events`
   - `signals`
   - `risk`
   - `orders`
   - `positions`
   - `system`
2. 对齐 `polyedge-api-contract.md` 的 DTO、分页、过滤和错误码。
3. 实现 SSE 事件总线：
   - signal 更新
   - order 更新
   - risk / mode 更新
4. 暂不优先做 WebSocket，首版以 SSE 为主。
5. 在 Redis 中只承担广播与热缓存，不承担真值存储。

交付物：

1. 控制台查询 API
2. SSE 推送能力
3. 前端联调所需的最小 dashboard 数据面

验收条件：

1. API 返回值与 contract 一致
2. SSE 断线重连不造成客户端状态错乱
3. request_id 能从前端请求串到后端日志和审计

建议时长：5 到 7 个工作日

### M7. Replay、研究评估与回归验证

目标：把系统变成“能重放、能解释、能回归”的研究平台，而不只是在线服务。

主要任务：

1. 实现 `apps/replay` 的输入重放入口。
2. 固化测试 fixture：
   - 事件输入
   - 市场快照
   - CLOB/成交回报
   - LLM mock 输出
3. 支持 signal / risk / execution 的离线回放。
4. 为策略参数、Prompt 版本、阈值版本建立回归基线。

交付物：

1. replay 命令
2. 研究样本集和回归结果
3. 可重复执行的端到端 fixture 测试

验收条件：

1. 同一 fixture 多次 replay 结果一致
2. 改动 pricing/risk 后能快速看到回归差异
3. replay 不依赖线上实时服务

建议时长：5 到 7 个工作日

---

## 6. 关键依赖关系

严格的关键路径如下：

```text
M0 -> M1 -> M2 -> M3 -> M4 -> M5 -> M6 -> M7
```

可以有限并行的部分：

1. `M2` 完成基础 schema 后，可并行开始 `M6` 的只读查询接口。
2. `M4` 后，可并行推进 `M5` 的 read-only connector 和 `M6` 的控制台读模型。
3. `M5` 中的 CLOB WebSocket/回报映射可先于真实下单落地。

不建议并行的部分：

1. 在 `M1` 前开始写业务 API。
2. 在 `M4` 前直接接 live execution。
3. 在没有 fixture 回放前重度依赖线上联调。

---

## 7. 首批 10 个工作日 Backlog

如果现在就开始实现，建议先做下面这 10 天：

### 第 1 到 2 天

1. 初始化 `packages/backend` Cargo workspace
2. 建 `apps/api`、`apps/worker`、`crates/*`
3. 接入 `tracing`、配置加载、`healthz/readyz`
4. 建立本地 PostgreSQL/Redis 启动方式

### 第 3 到 4 天

1. 建基础 migration
2. 落地支撑表：`audit_logs`、`idempotency_keys`、`outbox_events`
3. 接入 DB pool、Redis client、request_id middleware

### 第 5 到 6 天

1. 实现 domain 值对象和枚举
2. 实现 contracts 数值字符串序列化
3. 接入内部 token verifier 和最小受保护路由

### 第 7 到 8 天

1. 建 `market / resolution / event` 基础表
2. 完成 repo + use case + worker job skeleton
3. 建基于 fixture 的 ingestion 测试

### 第 9 到 10 天

1. 建 `evidence / signal` skeleton
2. 打通 `event -> evidence -> signal` 的占位链路
3. 把审计、幂等、trace_id 接进整条链路

第 10 天结束时，理想状态应达到：

```text
受保护 API 可用
DB/Redis 可用
基础表已迁移
market/event 可入库
signal skeleton 可贯通
```

---

## 8. 测试与质量门槛

后端首版至少建立以下测试层：

1. domain 单元测试：
   枚举、定点数、状态机、risk 判定。
2. application 集成测试：
   审批、幂等、outbox、事务边界。
3. API 测试：
   auth、错误码、序列化、SSE。
4. connector fixture 测试：
   Polymarket ws payload、订单响应、成交回报、CTF 调用 mock。
5. replay 回归测试：
   同一输入得到同一 signal/risk 结果。

必须卡住的质量门槛：

1. 禁止 `SELECT *`
2. 禁止用浮点作为价格/概率真值
3. 所有写接口必须有审计和幂等设计
4. 所有高风险接口必须显式校验 step-up scope
5. 所有外部状态映射必须有 fixture 测试

---

## 9. 建议的开工顺序

如果要马上进入编码，我建议按下面顺序开工：

1. 先做 `M0 + M1`
2. 然后完成 `M2` 的 market/event 真值层
3. 紧接着优先做 `M3`，把 signal 主链路打通
4. 再补 `M4` 的风控和审批
5. 最后接 `M5/M6`，让执行与控制台接到稳定真值层上

这比“一上来先写 Polymarket 下单”更稳，也更符合现有设计文档的主线。
