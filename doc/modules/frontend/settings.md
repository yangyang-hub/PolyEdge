# Settings（设置）

最后更新：2026-06-03

## 概述

`/settings` 页面管理运行时配置（runtime config），允许管理员查看和修改键值对配置项。

## 架构与关键文件

| 文件 | 职责 |
|---|---|
| `src/app/(console)/settings/page.tsx` | 路由页面 |
| `src/features/settings/components/settings-workbench.tsx` | 主组件 |
| `src/features/settings/components/runtime-config-panel.tsx` | 运行时配置面板 |
| `src/features/settings/loaders/settings-page-data.ts` | 服务端数据装配 |

## API 依赖

- `src/lib/api/settings.ts` — `readRuntimeConfig()`、`updateRuntimeConfig(update)`

## 权限

`/settings` 路由仍声明 `admin` 角色；当前内网部署由 API 侧 `POLYEDGE_AUTH__DISABLED=true` 关闭后端权限校验，前端 auth mode 仍为 `off`。

## i18n

使用 `ops` 命名空间字典。

## 当前状态

已实现，支持运行时配置的查看和修改。设置页文案显示当前 API 直连地址、前端 auth mode，并说明内网免鉴权模式下不需要 dev-auth 或 step-up code。

## 修改检查清单

- [ ] 新增配置项时同步更新后端 `runtime_config` 表和 DTO
- [ ] 修改后人工 smoke `/settings` 页面
