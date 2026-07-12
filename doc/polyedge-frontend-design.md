# PolyEdge 前端设计文档

> **状态（2026-07-12）**：本文是前端早期设计，包含已移除的 approvals、独立 research/replay 页面和 SSE 规划。当前前端是 Next.js 16 静态导出，路由为 dashboard/markets/events/rewards/rewards-fair-value/funding/settings，浏览器直连真实 Rust REST API；当前状态以 [../AGENTS.md](../AGENTS.md) 和 [modules/frontend/](modules/frontend/) 为准。

## 1. 文档目标

本文档定义 PolyEdge 前端控制台的产品定位、页面结构、数据获取方式、实时交互模式、权限边界和性能原则。

前端采用 `Next.js App Router`，定位不是营销站点，而是一个面向研究、交易和风控操作的内部工作台。

如需直接进入页面原型设计，请优先参考 [polyedge-prototype-design.md](./polyedge-prototype-design.md)。
如需确定实现时的 UI 框架和依赖，请参考 [polyedge-frontend-ui-stack.md](./polyedge-frontend-ui-stack.md)。

---

## 2. 前端定位

PolyEdge 前端的核心职责：

1. 展示市场、事件、证据、信号、仓位和风控状态。
2. 为人工确认、撤单、模式切换和风险处置提供操作入口。
3. 支持研究人员回放历史事件和策略结果。
4. 提供足够清晰的解释链路，而不是只显示最终买卖结果。

前端不负责：

1. 核心交易策略计算。
2. 风控规则判定。
3. 概率估值与下单逻辑。
4. 持久化和审计真值。

这些能力全部保留在 Rust 后端，前端只做展示、交互和受控操作。

---

## 3. 设计原则

1. 服务端优先。
   页面首屏数据尽量由 Server Components 获取，避免把交易控制台做成重客户端单页。
2. 实时能力局部化。
   只有订单簿、信号状态、风险告警等实时区域使用 Client Components。
3. 解释优先于炫技。
   每个重要信号都应能展开看到 market、evidence、posterior、risk decision。
4. 低跳转成本。
   关键操作页面应支持列表 + 详情面板，不让操作员反复切页。
5. 可降级。
   WebSocket/SSE 中断时，页面应自动降级到轮询或最近快照，而不是完全不可用。
6. 低瀑布流。
   独立数据请求应并行获取，用 Suspense 做分区流式渲染。

---

## 4. 技术选型

### 4.1 核心栈

1. Next.js（App Router）
2. TypeScript
3. React Server Components
4. Server Actions
5. Tailwind CSS

### 4.2 推荐使用方式

1. 初始页面数据通过 Server Components 直连后端 API 获取。
2. 变更类操作通过 Server Actions 提交到 Rust 后端。
3. 高频实时区域通过 SSE 或 WebSocket 更新。
4. 图表和回放等重组件使用动态加载，避免主包膨胀。

### 4.3 不推荐方式

1. 所有页面都变成 Client Component。
2. 在浏览器侧重复实现风控或定价逻辑。
3. 为了“实时”而让整个页面都依赖 WebSocket。
4. 在多层嵌套组件内各自独立请求相同数据，造成瀑布流。

---

## 5. 前后端边界

前端与 Rust 后端的职责边界建议如下：

### 前端负责

1. 页面路由与布局。
2. 数据展示与交互。
3. 用户输入验证的第一层体验校验。
4. 操作确认与结果反馈。
5. 轻量级 UI 状态，如筛选、排序、面板展开状态。

### 后端负责

1. 身份鉴权与权限控制真值。
2. 事件识别、证据生成、估值、信号、风控、执行。
3. 审计日志与状态机更新。
4. 数据一致性与幂等。
5. 实时推送源。

### 边界原则

```text
前端负责表现层和受控操作，后端负责业务真值和状态演化
```

---

## 6. 路由与信息架构

建议采用 App Router，使用 route groups 拆分控制台域。

```text
app/
  (console)/
    layout.tsx
    dashboard/page.tsx
    markets/page.tsx
    events/page.tsx
    signals/page.tsx
    positions/page.tsx
    risk/page.tsx
    approvals/page.tsx
    settings/page.tsx
  research/
    replay/[runId]/page.tsx
  api/
    actions/*
```

### 6.1 主要页面

1. `dashboard`
   综合首页，展示系统模式、关键告警、实时信号、组合风险和待审批操作。
2. `markets`
   展示 market 列表、价格、流动性、结算语义、相关事件和证据摘要。
3. `events`
   展示原始事件流、去重事件、market mapping 和 evidence 生成结果。
4. `signals`
   展示 posterior、edge、置信度、状态迁移和执行结果。
5. `positions`
   展示市场仓位、主题风险桶、PnL 和减仓建议。
6. `risk`
   展示风控阈值、触发记录、kill switch 状态和限制原因。
7. `approvals`
   展示人工确认队列，如高歧义市场、超阈值下单、模式切换。
8. `research/replay/[runId]`
   支持按时间线回放事件、证据、估值和信号变化。

---

## 7. 页面布局设计

推荐使用统一控制台布局：

```text
+----------------------------------------------------------+
| Top Bar: mode / env / alert badge / user / kill switch  |
+-------------------+--------------------------------------+
| Side Nav          | Main Content                         |
| dashboard         | page header                          |
| markets           | filters                              |
| events            | table or timeline                    |
| signals           | chart / detail panes                 |
| positions         |                                      |
| risk              |                                      |
| approvals         |                                      |
+-------------------+----------------------+---------------+
| Bottom Status Rail: api / market / stream / risk health |
+----------------------------------------------------------+
```

### 7.1 交互模式

1. 列表页默认采用 `左侧列表 + 右侧详情抽屉`。
2. 高价值对象如 signal、event、market 应支持跨页面深链接。
3. 审批动作应采用双确认或带上下文摘要的确认弹层。
4. 风险异常要支持“跳转到原因”，而不是只显示红点。

---

## 8. 数据获取与状态管理

### 8.1 数据获取策略

建议遵循以下优先级：

1. 首屏和列表数据：Server Components。
2. 实时增量数据：Client Components + SSE/WebSocket。
3. 操作提交：Server Actions。
4. 极少量纯客户端状态：组件本地状态或轻量 store。

### 8.2 请求组织原则

1. 同一页面的独立请求并行发起。
2. 公共数据通过服务端共享 fetch 封装，避免重复请求。
3. 页面按数据依赖划分 Suspense 边界。
4. 不把大对象从服务端序列化给客户端，只传渲染所需字段。

### 8.3 推荐缓存策略

| 页面/模块 | 策略 |
| --- | --- |
| `dashboard` | `no-store` 或极短 revalidate |
| `markets` 列表 | 秒级 revalidate + 手动刷新 |
| `events` 时间线 | 初始服务端拉取 + 客户端流式补丁 |
| `signals` | 首屏服务端拉取 + SSE 更新 |
| `research/replay` | 结果缓存，可按 `runId` 复用 |

### 8.4 UI 状态与 Server State 分离

前端不要把后端真值复制成复杂客户端 store。

建议划分：

1. Server State：markets、events、signals、positions、risk。
2. UI State：筛选条件、排序、选中行、侧边栏展开状态、回放速度。

---

## 9. 实时交互设计

### 9.1 适合实时推送的模块

1. 市场价格和盘口摘要。
2. 新进入的事件。
3. signal 生命周期变化。
4. 审批队列变化。
5. 风险告警和 kill switch 状态。

### 9.2 连接策略

1. 首选 SSE 用于状态流、告警流、信号流。
2. 盘口和高频市场订阅可用 WebSocket。
3. 连接断开后自动重连，并展示当前连接状态。
4. 重连失败时自动回退为轮询模式。

### 9.3 实时更新边界

1. 实时连接只更新局部面板，不强制整页重渲染。
2. 图表与列表分开订阅，避免单个流拖垮整页。
3. 高吞吐数据先在后端聚合，前端只接收可渲染摘要。

---

## 10. 权限与安全

### 10.1 角色建议

1. `viewer`
   只读访问研究和运行状态。
2. `operator`
   可提交审批、撤单、确认手动执行。
3. `risk_admin`
   可调整运行模式、触发 kill switch、解除限制。
4. `admin`
   拥有系统配置和用户管理权限。

### 10.2 前端安全要求

1. 不在浏览器中保存交易密钥。
2. 高风险操作必须要求后端再次鉴权。
3. 模式切换、手动执行、撤单等动作必须展示审计上下文。
4. 前端只缓存最小必要会话信息。

---

## 11. 与后端的接口约定

详细接口结构以 [polyedge-api-contract.md](./polyedge-api-contract.md) 为准；鉴权、会话和高风险操作链路以 [polyedge-auth-design.md](./polyedge-auth-design.md) 为准。

### 11.1 查询类接口

建议按资源域组织：

1. `/api/markets`
2. `/api/events`
3. `/api/evidences`
4. `/api/signals`
5. `/api/orders`
6. `/api/positions`
7. `/api/risk`
8. `/api/research/runs`

### 11.2 实时接口

1. `/api/stream/signals`
2. `/api/stream/risk`
3. `/api/stream/events`
4. `/ws/markets`

### 11.3 操作类接口

1. `approveSignal`
2. `cancelOrder`
3. `switchMode`
4. `triggerKillSwitch`
5. `releaseKillSwitch`

操作类接口建议由 Server Actions 封装，统一处理：

1. CSRF/会话。
2. 表单校验。
3. 操作反馈。
4. 跳转或局部刷新。

### 11.4 契约治理原则

1. 前端 TypeScript 类型应来自统一契约，而不是手写散落接口。
2. 资源字段命名、错误码和实时 payload 不允许页面自行约定。
3. 若前后端发布不同步，前端必须优先依赖向后兼容字段。

---

## 12. 性能与体验原则

结合 Next.js 最佳实践，前端应重点遵循：

1. 避免请求瀑布流，能并行就并行。
2. 使用 Suspense 将实时面板和慢数据面板解耦。
3. 大型图表和回放播放器采用动态导入。
4. 尽量减少传给 Client Components 的 JSON 体积。
5. 不为简单值滥用 `useMemo` / `useCallback`。
6. 交互型非紧急更新使用 `startTransition`。
7. 滚动长列表考虑虚拟化或 `content-visibility`。

---

## 13. 推荐目录结构

```text
src/
  app/
    (console)/
    research/
    api/
  components/
    ui/
    charts/
    layouts/
  features/
    markets/
    events/
    signals/
    positions/
    risk/
    approvals/
    research/
  lib/
    api/
    auth/
    formatters/
  server/
    actions/
    loaders/
    permissions/
```

目录原则：

1. `features/*` 按业务域组织，而不是按技术类型散落。
2. `server/*` 只放服务端逻辑，避免误引入客户端。
3. 通用 UI 和业务组件分离。

---

## 14. MVP 前端实现顺序

更细的实施计划见 [polyedge-frontend-implementation-plan.md](./polyedge-frontend-implementation-plan.md)。

建议按以下顺序落地：

1. 控制台 Layout 和权限框架。
2. `dashboard`、`markets`、`signals` 三个核心页面。
3. 审批队列和风险面板。
4. SSE/WebSocket 实时局部刷新。
5. `events` 和 `research/replay`。

首版目标不是做成完整交易终端，而是优先支持：

1. 看懂系统现在在做什么。
2. 看懂为什么做这个判断。
3. 在必要时人工介入。
