# PolyEdge 前端 UI 框架与依赖建议

> **状态（2026-06-14）**：本文是前端 UI 栈建议和原型背景，其中部分页面名称来自早期原型。当前前端页面和模块以 [../AGENTS.md](../AGENTS.md)、[../README.md](../README.md) 和 [modules/frontend/](modules/frontend/) 为准。

## 1. 文档目标

本文档基于 `stitch_polyedge_frontend_prototype_document` 下的原型页面，确定 PolyEdge 前端在 `Next.js` 中应采用的 UI 框架路线、核心依赖和可用 skill。

本文档回答三个问题：

1. 原型风格最适合什么 UI 框架。
2. 前端首版应该安装哪些依赖。
3. 当前有哪些本地 skill 和外部 skill 可以直接用于后续实现。

---

## 2. 结论

### 2.1 推荐 UI 框架

PolyEdge 前端建议采用：

```text
Next.js + Tailwind CSS + shadcn/ui + Radix Primitives
```

这是最适合当前原型的方案。

### 2.2 不建议采用的路线

当前原型不适合优先采用：

1. MUI
2. Ant Design
3. Chakra UI
4. Mantine 的完整视觉体系

原因不是这些库不能用，而是它们的默认组件外观、间距体系和表格/卡片风格，会和当前原型的“高密度深色控制台”发生明显冲突。

---

## 3. 为什么选择这条路线

从 `stitch_polyedge_frontend_prototype_document` 下的 HTML 原型可以确认以下事实：

1. 原型全部基于 `Tailwind` 类名组织。
2. 色彩、圆角、字体都通过自定义 token 扩展。
3. 界面是高密度、深色、数据优先的交易控制台。
4. 页面需要大量表格、抽屉、状态标签、弹层、审批区和风险操作区。
5. 页面风格强调“可定制”和“无重型默认样式”，而不是套件式 UI。

因此最合适的是：

1. 用 `Tailwind CSS` 承接视觉 token 和布局密度。
2. 用 `shadcn/ui` 承接组件层。
3. 用 `Radix` primitives 处理 Dialog、Popover、Tabs、Tooltip、Dropdown 等复杂交互。
4. 用 `TanStack Table` 承接高密度表格。

这条路线与原型一致，也和当前的 `Next.js App Router` 前端设计文档一致。

---

## 4. 推荐依赖清单

以下分为 `必须`、`强烈建议`、`按需` 三层。

## 4.1 必须依赖

### 基础框架

1. `next`
2. `react`
3. `react-dom`
4. `typescript`

### UI 样式基础

1. `tailwindcss`
2. `@tailwindcss/forms`
3. `tailwindcss-animate`

### 组件体系

1. `shadcn/ui`
2. `@radix-ui/react-*` primitives

说明：

`shadcn/ui` 不是传统“黑盒依赖包”思路，而是组件生成和代码所有权方案；实际项目里会按需拉入对应的 `@radix-ui/react-dialog`、`@radix-ui/react-dropdown-menu`、`@radix-ui/react-tooltip` 等 primitives。

### 组件样式工具

1. `class-variance-authority`
2. `clsx`
3. `tailwind-merge`

这组工具基本就是 `shadcn/ui` 生态的标准搭配。

---

## 4.2 强烈建议依赖

### 表格与高密度列表

1. `@tanstack/react-table`

原因：

原型里 `dashboard`、`signals`、`approvals`、`risk` 都以高密度表格和可扩展列表为核心，`TanStack Table` 是最合适的 headless 数据表格方案。

### 图表

1. `recharts`

原因：

`shadcn/ui` 官方 chart 方案就是基于 `Recharts`，对 KPI、PnL、风险桶、时间序列等场景足够合适。

### 表单与校验

1. `react-hook-form`
2. `zod`
3. `@hookform/resolvers`

原因：

审批理由、筛选器、模式切换、风险操作确认都需要轻量但类型安全的表单方案。

### 提示反馈

1. `sonner`

原因：

审批成功、撤单成功、权限不足、风控拒绝等动作都需要轻量通知反馈。`shadcn/ui` 当前也倾向使用 `sonner` 作为 toast 方案。

### 日期与时间格式化

1. `date-fns`

原因：

事件时间、审批时间、流数据更新时间、回放时间轴都需要稳定日期处理。

---

## 4.3 按需依赖

### 命令搜索 / 全局操作面板

1. `cmdk`

适用场景：

1. 全局搜索 market / event / signal。
2. 快速跳转页面。
3. 快速触发内部操作。

### 图标

原型当前使用的是 `Material Symbols` 风格。

建议二选一：

1. 继续使用 `Material Symbols` 字体方案，保持原型风格一致。
2. 改用 `lucide-react`，与 `shadcn/ui` 生态更统一。

如果要最大程度贴近当前原型，优先保留 `Material Symbols`。

### 主题切换

1. `next-themes`

只有在后续需要真正支持浅色/深色切换时才建议引入。当前原型是单一深色交易台，不是首版必需依赖。

### 客户端查询缓存

1. `swr`
2. 或 `@tanstack/react-query`

当前系统是 `Server Components + SSE/WebSocket` 为主，不一定需要立刻引入。但如果后续实时模块越来越多，才值得单独加。

---

## 5. 推荐的首版依赖组合

如果现在就开始落地，建议前端首版只装这一组：

```text
next
react
react-dom
typescript
tailwindcss
@tailwindcss/forms
tailwindcss-animate
class-variance-authority
clsx
tailwind-merge
@radix-ui/react-dialog
@radix-ui/react-dropdown-menu
@radix-ui/react-popover
@radix-ui/react-scroll-area
@radix-ui/react-select
@radix-ui/react-tabs
@radix-ui/react-tooltip
@tanstack/react-table
recharts
react-hook-form
zod
@hookform/resolvers
sonner
date-fns
```

说明：

1. `shadcn/ui` 通过 CLI 初始化和按需添加组件，不作为普通运行时依赖理解。
2. `cmdk`、`next-themes`、客户端数据缓存库可以第二阶段再补。

---

## 6. 组件层建议

结合原型，建议首批从 `shadcn/ui` 或等价自建组件中优先落这些：

1. `Button`
2. `Card`
3. `Badge`
4. `Input`
5. `Table`
6. `Tabs`
7. `Dialog`
8. `Sheet`
9. `DropdownMenu`
10. `Tooltip`
11. `Popover`
12. `ScrollArea`
13. `Separator`
14. `Command`
15. `Chart`

其中最关键的是：

1. `Table`
2. `Sheet`
3. `Dialog`
4. `Badge`
5. `Chart`

因为原型的主工作流基本围绕这五类组件展开。

---

## 7. 设计系统落地建议

原型已经具备明确视觉语言，因此建议：

1. 使用 Tailwind theme tokens 还原 `surface`、`surface-container-*`、`primary`、`secondary`、`error` 等颜色层级。
2. 使用 `next/font` 引入 `Manrope`、`Inter`、`Roboto Mono`。
3. 保留“高密度 + 深色 + 低线条依赖”的设计原则。
4. 不要直接套用默认圆角、默认阴影、默认卡片风格。

换句话说：

```text
框架选 shadcn/ui，但视觉必须由 PolyEdge 自己的 design tokens 主导
```

---

## 8. Skill 检索结果

## 8.1 当前本地已可直接使用的 skill

最相关的本地 skill 有：

1. `vercel-react-best-practices`
   适合 Next.js / React 实现阶段，尤其是 Server Components、数据获取、性能和 bundle 控制。
2. `vercel-composition-patterns`
   适合控制台组件拆分、复合组件、复杂页面结构设计。
3. `web-design-guidelines`
   适合后续对 UI 可用性和可访问性做审查。

其中最值得优先使用的是：

1. `vercel-react-best-practices`
2. `vercel-composition-patterns`

## 8.2 通过 Skills CLI 检索到的可选 skill

### Next.js / shadcn 方向

1. `laguagu/claude-code-nextjs-skills@nextjs-shadcn`
   安装：
   `npx skills add laguagu/claude-code-nextjs-skills@nextjs-shadcn`
   链接：
   `https://skills.sh/laguagu/claude-code-nextjs-skills/nextjs-shadcn`

2. `ovachiever/droid-tings@nextjs-shadcn-builder`
   安装：
   `npx skills add ovachiever/droid-tings@nextjs-shadcn-builder`
   链接：
   `https://skills.sh/ovachiever/droid-tings/nextjs-shadcn-builder`

### Design System 方向

1. `arvindrk/extract-design-system@extract-design-system`
   安装：
   `npx skills add arvindrk/extract-design-system@extract-design-system`
   链接：
   `https://skills.sh/arvindrk/extract-design-system/extract-design-system`

2. `wshobson/agents@tailwind-design-system`
   安装：
   `npx skills add wshobson/agents@tailwind-design-system`
   链接：
   `https://skills.sh/wshobson/agents/tailwind-design-system`

### 可访问性方向

1. `addyosmani/web-quality-skills@accessibility`
   安装：
   `npx skills add addyosmani/web-quality-skills@accessibility`
   链接：
   `https://skills.sh/addyosmani/web-quality-skills/accessibility`

2. `wshobson/agents@accessibility-compliance`
   安装：
   `npx skills add wshobson/agents@accessibility-compliance`
   链接：
   `https://skills.sh/wshobson/agents/accessibility-compliance`

---

## 9. 最终建议

如果目标是尽快把当前原型落成真实前端，我建议直接采用下面这组决策：

1. UI 框架：
   `Tailwind CSS + shadcn/ui + Radix`
2. 数据密集组件：
   `@tanstack/react-table`
3. 图表：
   `recharts`
4. 表单与校验：
   `react-hook-form + zod + @hookform/resolvers`
5. 反馈：
   `sonner`
6. 日期：
   `date-fns`

技能方面，优先使用：

1. 本地：`vercel-react-best-practices`
2. 本地：`vercel-composition-patterns`
3. 外部可选：`laguagu/claude-code-nextjs-skills@nextjs-shadcn`

这套组合和当前原型的风格、密度、交互复杂度是匹配的，而且不会把你锁进一个与原型冲突的重型视觉框架里。
