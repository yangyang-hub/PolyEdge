# Funding（Polymarket 入金）

最后更新：2026-06-26

## 概述

`/funding` 页面提供一个浏览器钱包入金辅助工具：用户粘贴 Polymarket 官方入金页生成的 EVM 存款地址，选择 Polygon 链上的 USDC / USDC.e / USDT 资产，页面在浏览器中构造 ERC-20 `transfer` calldata，并交由用户钱包签名广播。

该页面不托管私钥、不保存地址或金额、不通过后端代发交易，也不直接调用 Polymarket Bridge API 生成存款地址。

## 架构与关键文件

| 文件 | 职责 |
|---|---|
| `src/app/(console)/funding/page.tsx` | 路由页面 |
| `src/features/funding/components/funding-workbench.tsx` | 主工作台：表单状态、钱包连接、Polygon 切链、ERC-20 转账提交 |
| `src/features/funding/components/funding-review-card.tsx` | 发送前复核卡片：网络、资产合约、金额、付款钱包、收款地址、交易链接 |
| `src/features/funding/components/funding-info-cards.tsx` | 安全边界和入金流程说明 |
| `src/features/funding/lib/polygon-funding.ts` | Polygon 入金资产清单、EVM 地址校验、金额转最小单位、ERC-20 transfer calldata 构造 |
| `src/features/funding/types.ts` | 钱包操作状态类型 |
| `src/lib/i18n/dictionaries/funding.ts` | Funding 页面中文文案 |
| `src/components/shared/console-nav-items.ts` | 控制台导航入口 `/funding` |

## 核心数据结构

- **`PolygonFundingToken`**：静态资产配置，包含 `id`、`symbol`、`name`、Polygon 合约地址、`decimals` 和字典 note key。
- **`polygonFundingTokens`**：当前前端内置的 Polygon 资产清单：原生 USDC、USDC.e、USDT0 / Polygon USDT、Wormhole Bridged USDT，均按 6 位小数处理。
- **`WalletSnapshot`**：客户端钱包状态，包含连接账户、最近交易 hash、操作状态和展示消息。

## 关键交互

- **连接钱包**：通过浏览器 EIP-1193 provider 执行 `eth_requestAccounts`。
- **切换网络**：先执行 `wallet_switchEthereumChain` 到 Polygon `0x89`；钱包未配置 Polygon 时调用 `wallet_addEthereumChain` 添加公共 Polygon RPC。
- **发送转账**：校验 EVM 存款地址、金额精度和人工确认勾选后，调用 `eth_sendTransaction`，`to` 为所选 ERC-20 合约地址，`data` 为 `transfer(recipient, amountUnits)`。
- **复核与追踪**：页面展示 token 合约 Polygonscan 链接；交易广播后展示 Polygonscan 交易链接。

## API 依赖

无后端 API 依赖。页面只使用浏览器钱包 provider 和静态合约清单；Polymarket 官方入金页以外链形式打开，不在前端或后端请求 Polymarket Bridge API。

## i18n

使用 `funding` 命名空间字典；导航名称来自 `shared.nav.funding`。

## 当前状态

已实现 `/funding` 控制台页面和侧边导航入口，支持手工粘贴 Polymarket 官方 EVM 存款地址后由浏览器钱包在 Polygon 上发送 ERC-20 USDC / USDT 资产。

已知限制：

- 不生成 Polymarket Bridge 存款地址；用户必须从 Polymarket 官方入金页复制地址。
- 不查询钱包余额、allowance、gas 余额或链上交易确认数。
- 不验证 Polymarket 是否已经入账；广播后只提供 Polygonscan 链接。
- 内置资产清单是前端静态配置，后续如 Polymarket 支持资产变化需人工更新。

## 修改检查清单

- [ ] 新增/删除支持资产时同步更新 `polygonFundingTokens`、页面文案和本文件当前状态
- [ ] 修改钱包交易逻辑后运行前端类型检查，并人工 smoke `/funding`
- [ ] 不在前端或 API handler 直接接入 Polymarket Bridge API 生成地址，除非先补齐架构文档与安全设计
