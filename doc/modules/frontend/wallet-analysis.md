# Wallet Analysis（钱包分析）

最后更新：2026-05-31

## 概述

`/wallet-analysis` 页面提供 Polymarket 钱包地址的深度分析：胜率、ROI、交易风格、风险画像、分类分布等。

## 架构与关键文件

| 文件 | 职责 |
|---|---|
| `src/app/(console)/wallet-analysis/page.tsx` | 路由页面 |
| `src/features/wallet-analysis/components/wallet-analysis-workbench.tsx` | 主组件 |

## API 依赖

- `src/lib/api/wallet-analysis.ts` — `analyzeWallet(address)`（POST `/api/v1/wallet-analysis`）

## 数据流

用户输入钱包地址 → `analyzeWallet()` → 后端 `build_wallet_analysis_report()` 纯计算 → 返回 `WalletAnalysisReportDto` → 前端展示。

## i18n

使用 `wallet-analysis` 命名空间字典。

## 当前状态

已实现，支持按地址查询钱包分析报告。

## 修改检查清单

- [ ] 新增分析维度时同步更新后端 `wallet_analysis` 模块和 DTO
- [ ] 修改后人工 smoke `/wallet-analysis` 页面
