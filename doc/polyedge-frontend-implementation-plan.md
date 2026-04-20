# PolyEdge 前端实现计划

## 1. 当前起点

`packages/front/` 已经不是空目录，当前具备：

1. Next.js 16 + React 19 + Tailwind v4 + shadcn/ui 基础工程。
2. `src/app/(console)` 下的控制台路由骨架。
3. 一套可用的 console shell、shared UI 和两块较完整的 client workbench。
4. `src/lib/server/polyedge-api.ts` 的 mock/live 双模式数据入口。
5. `src/lib/server/console-page-data.ts` 的页面级聚合 loader。

这说明前端计划不能按纯 greenfield 写，而应按“在现有控制台骨架上收敛契约、重整分层、逐步接实数据信号流”的方式推进。

---

## 2. 现状判断

当前前端更接近：

```text
高保真控制台原型 + typed DTO 雏形 + mock/live adapter
```

而不是：

```text
已完成联调的生产级控制台
```

原因主要有三类：

1. 页面已有结构和交互，但仍以 mock fixture 和页面级聚合函数为主。
2. 还没有明确的 `features/*` 业务域分层、`server/actions`、`proxy.ts` 权限边界和实时连接层。
3. 当前前端 DTO/fixture 与最新设计文档存在枚举和状态语义偏差，不能直接视为最终契约。

---

## 3. 必须先收敛的问题

在正式推进页面实现前，前端必须先把以下问题收敛：

### 3.1 契约与枚举对齐

当前前端代码与设计文档至少存在这些偏差：

1. `tradability_status` 仍在使用 `auto`，而文档已收敛到 `tradable / manual_review / observe_only / blocked`。
2. `event.status` 前端使用 `active / observe_only / review`，而文档定义为 `active / expired / invalidated / superseded`。
3. `evidence.direction` 前端使用 `mixed`，而文档收敛为 `supports_yes / supports_no / background`。
4. `signal.lifecycle_state` 前端使用 `approval_required`，而文档状态机已调整为 `new / active / weakened / executed / invalidated / reversed / expired`。
5. `system.mode` 前端还存在 `production`，而文档已收敛到 `research / paper_trade / manual_confirm / live_auto / kill_switch_locked`。

如果不先修这一层，后续页面越完整，联调返工越大。

### 3.2 页面数据分层过粗

当前 `console-page-data.ts` 把多个页面的数据编排集中在一个文件里，适合原型阶段，但不适合继续扩展。

后续要拆成：

1. feature 级 loader
2. server-only API adapter
3. route page 级装配

### 3.3 操作链路尚未成型

当前 `signals`、`approvals` 页已有交互界面，但仍停留在本地 UI 选择和按钮层，没有形成：

1. server actions
2. step-up UI 流程
3. 乐观更新或局部刷新
4. 写操作错误处理和 toast 反馈

---

## 4. 前端实施原则

1. 保留现有 Next.js App Router + Server Components 主体路线，不退回全客户端状态架构。
2. 页面读取优先走 Server Components / server loaders，写操作优先走 Server Actions。
3. Client Component 只下放到交互叶子节点，如筛选器、详情 sheet、审批表单、实时局部面板。
4. 业务组件按 `features/*` 收敛，不继续把复杂逻辑堆进 `components/shared` 和单个 `console-page-data.ts`。
5. 前端不做概率、风控、PnL 真值计算，只做展示、格式化和交互组织。
6. 首版实时层以 SSE 为主，WebSocket 不是前端优先项。
7. 先打通 `dashboard / markets / signals / approvals / risk`，再补 `events / positions / replay / settings`。

---

## 5. 建议目录演进

建议在保留现有 App Router 路由的前提下，逐步演进为：

```text
packages/front/
  proxy.ts
  src/
    app/
      (console)/
        dashboard/
        markets/
        signals/
        approvals/
        risk/
        events/
        positions/
        replay/
        settings/
        loading.tsx
        error.tsx
      (auth)/
        login/
        unauthorized/
      layout.tsx
      globals.css
    components/
      ui/
      layouts/
      shared/
    features/
      dashboard/
        components/
        loaders/
      markets/
        components/
        loaders/
      signals/
        components/
        loaders/
        actions/
      approvals/
        components/
        loaders/
        actions/
      risk/
        components/
        loaders/
        actions/
      events/
      positions/
      replay/
      settings/
    hooks/
      use-sse-stream.ts
      use-live-status.ts
    lib/
      contracts/
      formatters/
      utils/
    server/
      api/
      actions/
      auth/
      loaders/
      permissions/
      realtime/
```

迁移原则：

1. `components/ui` 保留。
2. `components/shared/console-*` 逐步迁到 `components/layouts`。
3. `components/console/*` 逐步迁到对应的 `features/*/components`。
4. `lib/server/polyedge-api.ts` 逐步迁到 `server/api/*`。
5. `lib/server/console-page-data.ts` 逐步拆到各 `features/*/loaders`。

---

## 6. 里程碑计划

### F0. 契约收敛与工程清理

目标：把当前前端从“原型可跑”收敛到“可持续实现”。

主要任务：

1. 对齐 DTO、枚举、mock fixture 与最新设计文档。
2. 清理 `src/lib/contracts/*` 中与文档不一致的状态和值。
3. 给 `app/(console)` 补 `loading.tsx`、`error.tsx`、`not-found.tsx` 策略。
4. 审查依赖，保留真实用到的 UI/图表/表单包。
5. 明确 mock/live 切换方式和环境变量规范。

交付物：

1. 收敛后的 `dto.ts`
2. 可继续演化的 fixture 数据
3. 页面级 loading / error 骨架

验收条件：

1. 前端契约不再与设计文档冲突
2. 核心页面出现异常时有统一错误展示
3. mock/live 两种模式切换行为明确

建议时长：2 到 3 个工作日

### F1. 数据访问层与权限边界

目标：把页面级大聚合改造成可维护的 server 分层。

主要任务：

1. 将 `polyedge-api.ts` 拆成 `server/api/*` 资源级接口。
2. 将 `console-page-data.ts` 拆成 feature 级 loader。
3. 建立 `server/permissions/*` 和角色判定。
4. 增加 `proxy.ts`，保护 console 路由并处理未授权跳转。
5. 为写操作预留统一的 `request_id`、`Idempotency-Key`、错误映射入口。

交付物：

1. `server/api/*`
2. `server/loaders/*`
3. `proxy.ts`
4. 权限守卫和导航裁剪策略

验收条件：

1. page 文件只负责页面装配，不承载大量数据拼接逻辑
2. 未授权用户不能进入 console 受保护页面
3. 页面读取和写操作边界明确

建议时长：3 到 4 个工作日

### F2. 核心读页面

目标：优先把“看懂系统在做什么”的页面打稳。

优先页面：

1. `dashboard`
2. `markets`
3. `signals`

主要任务：

1. 完善 page -> feature loader -> business component 的结构。
2. 统一表格、筛选条、详情面板、空状态和骨架屏。
3. 将当前 `signals-workbench` 收敛到 `features/signals/components`。
4. 将市场详情页中的 resolution、ambiguity、linked events 信息按设计文档补齐。
5. 对图表和重面板采用按需动态加载。

交付物：

1. 可联调的三大核心页面
2. 统一的展示组件与高密度列表样式
3. 可复用的筛选/详情面板模式

验收条件：

1. `dashboard / markets / signals` 可在 mock 和 live 两种模式运行
2. 页面对后端 contract 的字段依赖清晰可追踪
3. 页面不依赖客户端全局 store 才能渲染核心信息

建议时长：5 到 7 个工作日

### F3. 操作页面与人工干预流

目标：优先支持“必要时人工介入”。

优先页面：

1. `approvals`
2. `risk`

主要任务：

1. 为审批、拒绝、模式切换、kill switch 等操作建立 Server Actions。
2. 增加 step-up 交互流程占位：
   - 二次确认
   - 审批备注
   - 失败反馈
3. 将 `approvals-workbench` 收敛到 `features/approvals/components`。
4. 在风险页面实现 bucket、alert、desk state 的多面板布局。
5. 写操作后做局部刷新，而不是整页强刷。

交付物：

1. 审批工作台
2. 风险控制面板
3. 统一的操作弹层和反馈机制

验收条件：

1. 高风险操作有明确确认链路
2. 操作失败能向用户返回明确错误，而不是 silent failure
3. 页面能展示后端返回的 request_id / trace_id 关键信息

建议时长：5 到 7 个工作日

### F4. 实时层与局部刷新

目标：把“看板是活的”这件事落地，但只做必要的实时化。

主要任务：

1. 建立 SSE 订阅层和连接状态指示。
2. 优先接入：
   - signal 更新
   - approval 队列变化
   - risk / mode 更新
3. 将实时更新限制在局部 panel、badge、table row，而不是整页重绘。
4. 为断线、重连、滞后状态提供可见提示。
5. 保持无 SSE 时的静态降级可用。

交付物：

1. `hooks/use-sse-stream.ts`
2. live status rail
3. 核心页面的局部实时刷新

验收条件：

1. SSE 断开不会让页面失效
2. 局部刷新不会破坏当前筛选和选中状态
3. 实时层是增强能力，不是页面主渲染前提

建议时长：4 到 6 个工作日

### F5. 次级页面与研究面板

目标：补全控制台信息面，但不抢占主工作流优先级。

优先页面：

1. `events`
2. `positions`
3. `replay`
4. `settings`

主要任务：

1. 将 `events` 页补成“event -> evidence -> signal”追踪视图。
2. 将 `positions` 页补成 desk 级持仓与 bucket 视图。
3. 将 `replay` 页做成以 timeline 和结果对比为核心的研究面板。
4. `settings` 先只做只读配置、环境信息和联调状态。
5. 大图表和 replay 时间线按需动态导入。

交付物：

1. 补齐次级页面
2. 研究回放和追踪链路 UI
3. 环境与配置可见性页面

验收条件：

1. 用户能从 event 追到 evidence，再追到 signal
2. replay 页不会拖慢主 console 首屏
3. settings 页不直接暴露危险写操作

建议时长：5 到 7 个工作日

### F6. 测试、性能与发布准备

目标：把前端从“能展示”推进到“可持续联调和发布”。

主要任务：

1. 为核心 formatters、loaders、actions 建立单测。
2. 为核心流程建立端到端测试：
   - 打开 dashboard
   - 查看 signal
   - 审批流程
   - 风险面板刷新
3. 检查 hydration、bundle 体积和慢组件切分。
4. 审核可访问性、键盘焦点、Dialog/Sheet 交互。
5. 建立 preview / staging 发布检查清单。

交付物：

1. 基础测试集
2. 发布前检查清单
3. 性能与可访问性修正项

验收条件：

1. 核心路径具备自动化回归
2. 首屏和工作台交互没有明显 hydration 问题
3. Dialog / Sheet / Table 的键盘交互合格

建议时长：4 到 6 个工作日

---

## 7. 严格优先级

严格关键路径如下：

```text
F0 -> F1 -> F2 -> F3 -> F4 -> F5 -> F6
```

可以有限并行的部分：

1. `F2` 进入中后期后，可并行推进 `F3` 的弹层与 action 框架。
2. `F3` 完成主要接口后，可并行推进 `F4` 的 SSE 状态 rail。
3. `F5` 的 `settings` 可在较早阶段先做只读版。

不建议并行的部分：

1. 在 `F0` 之前开始深做页面联调。
2. 在 `F1` 之前继续把更多逻辑堆进 `console-page-data.ts`。
3. 在 `F3` 之前做复杂实时写操作联动。

---

## 8. 首批 10 个工作日 Backlog

如果现在就开始实现，建议前 10 天按下面推进：

### 第 1 到 2 天

1. 对齐 `dto.ts`、fixture 和设计文档枚举
2. 清理不一致状态值
3. 增加 `loading.tsx` / `error.tsx`

### 第 3 到 4 天

1. 将 `polyedge-api.ts` 拆成资源级 `server/api/*`
2. 建 `server/loaders/*`
3. 接入 `proxy.ts` 和权限跳转骨架

### 第 5 到 6 天

1. 重构 `dashboard`、`markets` 的 loader 与组件
2. 抽出共享 table/detail/filter 模式
3. 清理页面级大聚合逻辑

### 第 7 到 8 天

1. 重构 `signals` 工作台
2. 接入 `signals` 的筛选、详情和局部刷新框架
3. 为审批动作建立 Server Action 骨架

### 第 9 到 10 天

1. 落地 `approvals`、`risk` 页的操作链路雏形
2. 建立 toast/error/request id 反馈
3. 接入 SSE 连接状态骨架

第 10 天结束时，理想状态应达到：

```text
核心枚举和 DTO 已收敛
dashboard / markets / signals 已按 feature 分层
approvals / risk 已有 action 骨架
console 路由具备基本权限边界
实时层已有可扩展的 SSE 入口
```

---

## 9. 质量门槛

前端首版必须卡住这些底线：

1. 读取走 Server Components / server loaders，写操作走 Server Actions。
2. 不把业务真值计算下沉到客户端。
3. 不继续扩散“一个文件聚合所有页面数据”的模式。
4. 所有危险操作必须有明确确认、备注和失败提示。
5. 实时刷新不得破坏当前筛选、选中和详情上下文。
6. DTO 与设计文档出现偏差时，优先修契约，不在页面层打补丁。

---

## 10. 建议的开工顺序

如果现在就进入编码，我建议按下面顺序推进：

1. 先做 `F0 + F1`
2. 然后完成 `dashboard / markets / signals`
3. 再补 `approvals / risk`
4. 接着做 SSE 局部刷新
5. 最后补 `events / positions / replay / settings`

这条顺序与现有设计文档一致，也更贴合当前代码基线。
