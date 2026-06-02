# Radar（套利雷达）

最后更新：2026-05-31

## 概述

`/radar` 页面是前端最完整的功能模块，展示套利机会扫描结果、验证状态、历史分析。是 AGENTS.md 中指定的前端模块化范例。

## 架构与关键文件

| 文件 | 职责 |
|---|---|
| `src/app/(console)/radar/page.tsx` | 路由页面 |
| `src/features/radar/components/arbitrage-radar-workbench.tsx` | 主工作台组件 |
| `src/features/radar/components/opportunity-detail.tsx` | 机会详情面板 |
| `src/features/radar/loaders/radar-page-data.ts` | 服务端数据装配 |
| `src/features/radar/lib/radar-state.ts` | 纯函数：候选状态推导（~含测试） |
| `src/features/radar/lib/radar-state.test.ts` | 状态推导单元测试 |
| `src/features/radar/lib/radar-formatters.ts` | 格式化函数 |
| `src/features/radar/lib/radar-stream.ts` | SSE 流式更新处理 |
| `src/features/radar/types.ts` | 视图模型类型（~101 行） |

## 核心类型（types.ts）

- **`RadarOpportunityItem`**：35+ 字段的完整机会视图模型 — id、marketQuestion、opportunityType、typeLabel、typeTone、grossEdge、priceSum、capacity、yesPrice、noPrice、validationStatus、netEdge、feeEstimate、candidateStatus、candidateLabel、candidateTone、candidateReason、isSelected
- **`RadarScanRow`**：扫描历史行
- **`RadarTypeCount`**：按类型统计
- **`RadarTopMarket`**：热门市场摘要
- **`RadarAnalysis`**：聚合分析数据
- **`RadarMetric`**：仪表盘指标卡片
- **`RadarPageData`**：完整页面数据
- **`RadarFilter`**：`"all" | "binary_buy_both" | "binary_sell_both"`
- **`RadarView`**：`"active" | "validated" | "rejected" | "history"`

## 纯函数（lib/radar-state.ts）

- `deriveCandidatePreview(input)` — 核心逻辑：从 opportunityStatus + validationStatus + netEdgeValue 推导候选预览状态
- `RadarCandidateStatus`：`"candidate" | "watch" | "blocked"`
- `RadarCandidateTone`：`"success" | "warning" | "neutral"`
- `humanizeSnakeCase(value)` — `"stale_book"` → `"stale book"`

## API 依赖

- `src/lib/api/arbitrage.ts` — `listArbitrageScans`、`listArbitrageOpportunities`、`listArbitrageAnalysisRuns`
- `src/lib/api/stream/[channel]` — SSE 流式更新（arbitrage 频道）

## 数据流

```
Loader（radar-page-data.ts）
    → listArbitrageScans + listArbitrageOpportunities + listArbitrageAnalysisRuns
    → 遍历机会，调用 deriveCandidatePreview() 推导 UI 状态
    → 计算 metrics、topMarkets、typeCounts
    → 返回 RadarPageData

SSE Stream（radar-stream.ts）
    → 监听 arbitrage 频道
    → 增量更新机会列表（outbox-backed 增量流）
```

## i18n

使用 `radar` 命名空间字典。

## 测试

`radar-state.test.ts` 测试 `deriveCandidatePreview` 的各种状态推导场景。使用独立的 `tsconfig.radar-test.json`，通过 `yarn test:radar-state` 运行。

## 当前状态

- 完整实现：列表、筛选、视图切换、详情面板、SSE 增量更新
- 套利雷达是只读链路（不会创建执行请求）
- 测试覆盖状态推导纯函数

## 修改检查清单

- [ ] 修改候选状态推导逻辑时更新 `radar-state.ts` 和对应测试
- [ ] 新增视图模型字段时同步更新 `types.ts` 和 loader 映射
- [ ] 修改后运行 `yarn test:radar-state`
- [ ] 修改后人工 smoke `/radar` 页面（筛选、视图切换、详情面板、实时更新）
