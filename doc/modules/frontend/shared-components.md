# 共享组件（Shared Components + UI Primitives）

最后更新：2026-07-12

## 概述

共享组件分为两层：`components/ui/`（shadcn 生成的基础组件）和 `components/shared/`（跨页面复用的业务组件）。还有 `hooks/` 目录提供自定义 React hooks。

## 设计目标

- UI 基础组件由 shadcn 生成，不手动修改样式
- 业务共享组件解决跨页面复用需求
- 自定义 hooks 封装可复用的状态/副作用逻辑

## 架构与关键文件

### App Shell 与全局主题

| 文件 | 用途 |
|---|---|
| `src/app/layout.tsx` | 根 HTML、全局 provider、通知容器与页面 metadata；不执行远程字体下载 |
| `src/app/globals.css` | Tailwind 主题 token、系统字体栈、暗色变量和全局基础样式 |

### UI Primitives — `src/components/ui/`（13 个文件）

shadcn 生成的 Radix UI 基础组件（style: radix-nova）：

| 组件 | 用途 |
|---|---|
| `badge.tsx` | 标签 |
| `button.tsx` | 按钮（多变体） |
| `card.tsx` | 卡片容器 |
| `dialog.tsx` | 模态对话框 |
| `dropdown-menu.tsx` | 下拉菜单 |
| `input.tsx` | 输入框 |
| `scroll-area.tsx` | 可滚动区域 |
| `separator.tsx` | 分隔线 |
| `sheet.tsx` | 侧边抽屉 |
| `table.tsx` | 表格（Table/Header/Row/Cell 等） |
| `tabs.tsx` | 标签页 |
| `textarea.tsx` | 多行输入 |
| `tooltip.tsx` | 提示气泡 |

### Shared Business Components — `src/components/shared/`（18 个文件）

| 组件 | 用途 |
|---|---|
| `console-shell.tsx` | 控制台主布局外壳（sidebar + topbar + content） |
| `console-sidebar.tsx` | 侧边导航栏 |
| `console-topbar.tsx` | 顶部栏 |
| `console-nav-items.ts` | 控制台导航项与 active 状态共享配置 |
| `console-loading-skeleton.tsx` | 加载骨架屏 |
| `workbench-layout.tsx` | 标准工作台页面布局 |
| `workbench-segmented-control.tsx` | 分段控制器 |
| `page-header.tsx` | 页面标题组件 |
| `action-dialog.tsx` | 操作确认对话框 |
| `state-banner.tsx` | 状态横幅 |
| `operation-feedback-banner.tsx` | 操作反馈横幅 |
| `empty-panel.tsx` | 空状态面板 |
| `client-data-boundary.tsx` | 客户端数据边界 |
| `metric-card.tsx` | 指标卡片 |
| `meter-bar.tsx` | 进度条/计量条 |
| `status-pill.tsx` | 状态指示标签 |
| `route-state-card.tsx` | 路由状态卡片 |
| `truncate-text.tsx` | 文本截断/展开 |

### 根级组件

| 组件 | 用途 |
|---|---|
| `src/components/pagination-bar.tsx` | 通用分页栏 |

### Custom Hooks — `src/hooks/`

| Hook | 用途 |
|---|---|
| `use-pagination.ts` | 分页状态管理（currentPage、pageSize、分页数据切片） |

## 数据结构

该模块不定义持久化 DTO；共享组件通过各自 props 接收页面数据，主题通过 `globals.css` 的 CSS custom properties 暴露。

## 当前状态

- UI 组件基于 shadcn/ui v4（radix-nova 风格）
- 共享组件覆盖所有跨页面复用场景
- 控制台侧边栏当前提供 `/dashboard`、`/markets`、`/events`、`/rewards`、`/funding`、`/settings` 入口；桌面侧栏和移动端抽屉菜单共享同一份导航配置
- 控制台布局导航使用原生 `<a href>` 跳转，避免静态导出部署下客户端 router 拦截失败导致菜单点击无响应
- 顶栏不再显示横向导航快捷入口，也不再读取旧风控状态或展示 kill-switch 控制；移动端顶栏显示菜单按钮并从左侧打开导航抽屉
- 暗色主题（`globals.css` 中仅定义暗色变量）
- `src/app/layout.tsx` 与 `globals.css` 使用系统字体栈，不在 `next build` 时访问 Google Fonts，保证离线/内网部署可复现构建
- `ActionDialog` 的操作备注与 step-up 输入具备显式 label、字段错误关联、500 字限制和一次性验证码 autocomplete；Rewards 风险操作复用该组件并在校验失败时聚焦首个错误字段。
- `ClientDataBoundary` 是当前所有 console route 的统一数据入口，在客户端执行 feature loader，并统一显示 loading skeleton 或错误状态；当前错误态不提供内联重试按钮。
- `WorkBenchSegmentedControl` 使用原生 button 实现可键盘操作的分段选择；`PaginationBar` 与 `usePagination` 用于 Dashboard/Events 的本地分页，Markets/Rewards 的服务端分页使用各自 feature 状态。

## 修改检查清单

- [ ] UI 基础组件优先使用 shadcn CLI 生成/更新
- [ ] 跨页面复用的组件放到 `components/shared/`
- [ ] 跨 feature 的工具/类型放到 `src/lib/`
- [ ] 修改共享组件后需要人工 smoke 所有使用该组件的页面
- [ ] 运行 `npx tsc --noEmit` 类型检查
