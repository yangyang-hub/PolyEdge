# common（后端二进制共享 helper）

最后更新：2026-07-12

## 概述

`polyedge_common` crate 位于 `packages/backend/crates/common`，用于 API、orderbook 等后端二进制之间复用进程外壳 helper。它不承载业务逻辑、HTTP DTO、Store trait 或外部连接器逻辑。

## 架构与关键文件

| 文件 | 职责 |
|---|---|
| `packages/backend/crates/common/src/lib.rs` | 共享 service bind address 解析、TCP listener 绑定和 Ctrl-C/SIGTERM shutdown signal helper |
| `packages/backend/crates/common/Cargo.toml` | crate 依赖声明；当前只依赖 `domain` 和 `tokio` |

## 核心数据结构

当前不定义持久化数据结构或业务模型，只提供函数：

- `service_socket_addr()`：把 host/port 转成 `SocketAddr`，失败时返回统一 `AppError`
- `bind_service_listener()`：绑定 Tokio TCP listener，失败时返回统一 dependency error
- `shutdown_signal()`：等待 Ctrl-C 或 Unix SIGTERM
- `shutdown_signal_then()`：收到 shutdown signal 后执行调用方传入的异步清理动作

## 依赖关系

- **上游**：`domain`（统一错误类型）、`tokio`
- **下游**：`packages/backend/api`、`packages/backend/order`

## 当前状态

- API 服务使用 common helper 解析/绑定监听地址，并在收到 shutdown signal 后关闭内嵌 `WorkerRuntime`
- Orderbook 服务使用 common helper 解析/绑定 HTTP listener，并复用相同 shutdown signal 逻辑
- common crate 仅用于跨二进制进程外壳复用；业务共享逻辑仍应放在 `domain`、`application`、`connectors` 或 `infrastructure`

## 修改检查清单

- [ ] 不在 common 中新增业务规则、DTO、数据库访问或外部 API 调用
- [ ] 新增 helper 前确认至少两个后端二进制会复用
- [ ] 修改错误语义时同步检查 API/orderbook 启动失败路径
- [ ] 运行 `cargo check --workspace --tests`
