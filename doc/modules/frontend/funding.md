# Funding（Polymarket 入金）

最后更新：2026-06-26

## 概述

`/funding` 页面提供后端资金钱包入金工具：用户只选择 Polygon 链上的 USDC / USDT 和金额，前端不再输入或生成充值地址。后端读取 `POLYEDGE_POLYMARKET__PRIVATE_KEY` 对应的付款钱包，并以 `POLYEDGE_POLYMARKET__FUNDER`（优先）或 `ACCOUNT_ID`（回退）作为 Polymarket 入账钱包，通过 Polymarket Bridge 生成 EVM 入金地址后广播 ERC-20 `transfer`。

页面不接触私钥，不允许修改入账钱包地址，也不在浏览器中构造 calldata。

## 架构与关键文件

| 文件 | 职责 |
|---|---|
| `src/app/(console)/funding/page.tsx` | 路由页面，使用 `ClientDataBoundary` 加载后端 funding status |
| `src/features/funding/loaders/funding-page-data.ts` | 调用 `readFundingStatus()` 获取后端资金配置状态 |
| `src/features/funding/components/funding-workbench.tsx` | 主工作台：资产/金额/确认勾选、提交后端转账 |
| `src/features/funding/components/funding-review-card.tsx` | 发送前复核卡片：付款钱包、Polymarket 入账钱包、Bridge 地址、交易链接 |
| `src/features/funding/components/funding-info-cards.tsx` | 安全边界和入金流程说明 |
| `src/features/funding/lib/polygon-funding.ts` | 金额最小单位转换、Polygonscan 链接和 token 说明映射 |
| `src/features/funding/types.ts` | 后端转账提交状态类型 |
| `src/lib/api/funding.ts` | Funding API client：读状态、提交转账 |
| `src/lib/api/actions/funding.ts` | Funding 写操作校验与标准化结果 |
| `src/lib/contracts/dto/funding.ts` | Funding 后端 DTO 镜像 |
| `src/lib/i18n/dictionaries/funding.ts` | Funding 页面中文文案 |
| `src/components/shared/console-nav-items.ts` | 控制台导航入口 `/funding` |

## 核心数据结构

- **`FundingStatusDto`**：后端 funding 状态，包含 `enabled`、付款钱包地址、Polymarket 入账钱包地址、chain id、最大单笔金额、支持资产和可选余额错误。
- **`FundingTokenDto`**：后端支持资产配置，包含 token id、symbol、Polygon 合约地址、decimals、最小入金金额和后端资金钱包链上余额。
- **`FundingTransferDto`**：后端广播结果，包含 Polygon tx hash、付款钱包、Polymarket 入账钱包、Bridge EVM 入金地址、资产和最小单位金额。
- **`FundingSubmissionSnapshot`**：前端本地提交状态，包含状态、消息和最近一次转账回执。

## 关键交互

- **加载状态**：页面进入时读取 `GET /api/v1/funding`，展示后端资金钱包、Polymarket 入账钱包、USDC/USDT 链上余额和配置是否完整。
- **选择资产**：资产清单来自后端；当前入口为 Polygon USDC 与 USDT0 / USDT。
- **提交转账**：当前内网免鉴权部署下，前端只提交 `token_id`、`amount` 和 `confirmed=true`；不提交充值地址或二次确认码。
- **后端执行**：`POST /api/v1/funding/transfer` 先用配置的 Polymarket 钱包调用 Bridge `/deposit` 获取 EVM 入金地址，再用配置私钥向该地址发送所选 ERC-20。
- **复核与追踪**：提交前展示付款钱包和 Polymarket 入账钱包；广播后展示 Bridge EVM 地址和 Polygonscan 交易链接。

## API 依赖

- `GET /api/v1/funding`：console_read，读取后端资金配置状态。
- `POST /api/v1/funding/transfer`：当前内网免鉴权部署下前端只携带 `Idempotency-Key` 和请求体；后端仍保留真实链上转账校验和广播逻辑。

## i18n

使用 `funding` 命名空间字典；导航名称来自 `shared.nav.funding`。

## 当前状态

已实现 `/funding` 控制台页面和侧边导航入口，支持使用后端配置资金钱包将 Polygon USDC / USDT 入金到后端配置的 Polymarket 账户。页面会在资产卡片展示后端资金钱包的 USDC/USDT Polygon 链上余额；余额查询失败时展示“余额暂不可用”提示但不阻断配置状态展示。入账钱包固定由后端配置决定：优先 `POLYEDGE_POLYMARKET__FUNDER`，未配置时回退 `POLYEDGE_POLYMARKET__ACCOUNT_ID`。当前纯内网部署不再要求页面填写二次确认码，仍保留真实链上转账确认勾选。

已知限制：

- 不查询后端资金钱包 allowance、POL gas 余额或链上确认数。
- 不验证 Polymarket 是否已经完成 pUSD 入账；广播后只提供 Polygonscan 链接。
- 支持资产由后端 allowlist 控制；后端提交时会再次校验 Polymarket Bridge 当前支持状态和最小入金金额。

## 修改检查清单

- [ ] 新增/删除支持资产时同步更新后端 allowlist、Funding DTO、页面文案和本文件当前状态
- [ ] 修改后端转账逻辑后运行 Rust 检查，并人工确认不会把私钥或充值地址暴露到前端
- [ ] 修改 Funding API 后同步更新 `src/lib/api/funding.ts`、`actions/funding.ts` 和 `contracts/dto/funding.ts`
- [ ] 修改页面后运行前端类型检查，并人工 smoke `/funding`
