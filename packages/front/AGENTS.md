<!-- BEGIN:nextjs-agent-rules -->
# This is NOT the Next.js you know

This version has breaking changes — APIs, conventions, and file structure may all differ from your training data. Read the relevant guide in `node_modules/next/dist/docs/` before writing any code. Heed deprecation notices.
<!-- END:nextjs-agent-rules -->

# 前端代码规范

`packages/front/`（Next.js 16 + React 19 + Tailwind v4 + shadcn/ui）的代码规范。仓库级状态快照见根 [AGENT.md](../../AGENT.md)；后端规范见 [packages/backend/AGENTS.md](../backend/AGENTS.md)。本文件的规则在写或改 `packages/front/` 下任何代码时必须遵守，违背即应拆分/重构而非沿用。上面的 `Next.js 16` 提醒同样必须遵守。

## 适用范围

`packages/front/src/` 下所有代码。任何改变目录结构、数据层位置、公共抽象位置的改动，都要确认仍符合本文件，必要时同步更新本文件。

## 目录结构

| 目录 | 职责 |
|---|---|
| `src/app/*` | App Router 路由、`page` / `layout` / route handler |
| `src/features/<name>/` | 按页面/领域组织的功能模块，内部分 `components` / `loaders` / `lib` / `types.ts`（见下） |
| `src/lib/api/*` | **统一数据层**：读取按领域拆文件（`markets.ts` / `signals.ts` / `risk.ts`…，基于 `base.ts`），写操作（server actions）走 `actions.ts` |
| `src/lib/{contracts,i18n,…}` | 跨 feature 共享库：`contracts/dto` 是后端 DTO 的类型镜像，`i18n` 是多语言字典 |
| `src/components/ui/*` | shadcn 生成的基础组件，不手改风格 |
| `src/components/shared/*` | 跨页面复用的业务组件 |

`features/<name>/` 内部约定：

- `components/`：React 组件（一个文件一个主组件 + 其紧耦合的展示型子组件）。
- `loaders/`：server 端数据装配（调 `src/lib/api/*` 取数、拼页面数据）。
- `lib/`：纯函数——流式 patch、状态推导、格式化、比较器；可带 `*.test.ts`。
- `types.ts`：本 feature 的类型定义。

范例形态见 `src/features/radar/`：`lib/radar-state.ts`（类型 + 纯函数）、`lib/radar-formatters.ts`、`types.ts`、`lib/radar-state.test.ts`。新建/重构 feature 复刻此结构。

**注意：`src/server/` 当前是空目录（历史遗留），新代码不要往里放；数据层一律用 `src/lib/api/*`。**

## 数据与装配约定

- server component 经 `features/*/loaders/*` 调用 `src/lib/api/*` 取数，不在组件里直接 fetch。
- mutation 用 `src/lib/api/actions.ts` 暴露的 server action。
- DTO 类型从 `@/lib/contracts/dto` 引用，**不在组件内重新定义后端结构**。
- 文案一律走 i18n 字典 + `useI18n()`，不硬编码中英文；字典按命名空间拆分（`src/lib/i18n/dictionaries/`）。

## 模块化设计

1. **`"use client"` 只加在确需交互（state/effect/事件）的组件**；能留在 server component 的不要客户端化。
2. **大组件瘦身三板斧**（按此优先级）：
   - 纯函数（流式 patch、派生、格式化、比较器）**下沉到 `features/<name>/lib/`**，禁止留在组件文件里；
   - 展示型子组件**拆到独立文件**（接收 props；需要文案时自取 `useI18n()`）；
   - 类型定义**移到 `features/<name>/types.ts`**。
3. **纯类型/纯数据文件用 barrel 收敛**：如 `contracts/dto.ts` 按领域拆到 `dto/` 后用 `export *` 重导出，保证外部 import 路径不变。

## 文件行数规范

- **软上限 400 行**：超过应评估按上面的「三板斧」拆分。
- **硬上限 600 行**：必须拆分。
- 组件/函数过大时优先抽子组件、抽 custom hook、下沉纯函数；避免一个组件堆十几个 `useState`。
- **例外（允许略超）：**
  - 纯类型定义文件；
  - shadcn `components/ui/*` 生成组件；
  - i18n 字典/纯数据在已按命名空间拆分后的单个文件。
- **拆分纪律**：纯机械搬移、零逻辑/行为改动；每拆一个文件立即 `npx tsc --noEmit`，类型检查兜底。

## 公共代码提取

- 跨页面复用的组件 → `src/components/shared`。
- 跨 feature 的工具/类型 → `src/lib`。
- 仅本 feature 用的纯逻辑 → `features/<name>/lib`。
- 同一段逻辑第二次出现即提取为共享函数；禁止复制粘贴成片逻辑。

## 验证命令

```bash
cd packages/front
npx tsc --noEmit        # 快速类型检查（拆分/搬移后必跑）
pnpm lint               # eslint
pnpm build              # 生产构建（含完整类型检查）
pnpm test:radar-state   # Node --test（改 radar lib 必跑）
```

前端无端到端运行时测试，重构交互组件后**必须人工 smoke** 对应页面（实时更新、过滤、对话框、表格渲染等）。
