# Settings（设置）

最后更新：2026-07-12

## 概述

`/settings` 页面同时承担运行状态观测和 runtime config 管理：展示新闻源健康、最近 raw news、系统文档/构建目标，并允许管理员修改当前由后端暴露的运行时配置项。

## 架构与关键文件

| 文件 | 职责 |
|---|---|
| `src/app/(console)/settings/page.tsx` | 路由页面 |
| `src/features/settings/components/settings-workbench.tsx` | 主组件 |
| `src/features/settings/components/runtime-config-panel.tsx` | 运行时配置面板 |
| `src/features/settings/loaders/settings-page-data.ts` | Feature 数据装配；当前由 client boundary 调用 |
| `src/lib/api/news.ts` | 新闻源健康和 raw news 读取 |
| `src/lib/api/actions/settings.ts` | runtime config Server Action |
| `src/lib/contracts/dto/settings.ts` | runtime config DTO 镜像 |

## API 依赖

- `src/lib/api/settings.ts` — `readRuntimeConfig()`、`updateRuntimeConfig(update)`
- `src/lib/api/news.ts` — `listNewsSourceHealth({ limit: 10 })`、`listNewsRawEvents({ limit: 8 })`

## 核心数据结构

- `RuntimeConfigEntryDto`：`key/section/field/label/env_name/value/default_value/value_type/options/restart_required`。
- `RuntimeConfigUpdateDto`：按 key 提交的 `values: Record<string, string>`。
- Loader 额外派生 `sourceHealthSummary`、新闻源健康视图模型、raw news 视图模型，以及 API base URL、backend mode 和 console auth mode。

## 权限

`/settings` 路由仍声明 `admin` 角色；当前前端 auth mode 为 `off`，没有 session/JWT 获取链路。默认部署由 API 侧 `POLYEDGE_AUTH__DISABLED=true` 关闭后端权限校验，并必须置于 VPN、私网 ACL 或可信反向代理边界内。

## i18n

使用 `ops` 命名空间字典。

## 当前状态

已实现 runtime config 查看/修改、新闻源健康表、最近 raw news 表、设计文档链接和构建目标摘要。配置控件根据 `value_type` 渲染布尔/选项/文本输入，保存结果会同步后端返回的规范化值。页面显示当前 API 直连地址、backend mode 和 auth mode；旧 risk/arbitrage runtime config 分组已移除。当前前端仍没有真实 session/JWT 获取链路。

## 修改检查清单

- [ ] 新增配置项时同步更新后端 `runtime_config` 表和 DTO
- [ ] 修改后人工 smoke `/settings` 页面
