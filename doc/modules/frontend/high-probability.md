# 高概率定价研究页

最后更新：2026-06-29

## 概述

`/high-probability` 是动态高概率市场定价策略的只读研究页面。页面读取后端 High Probability Pricing foundation 的 snapshot、配置、bucket stats、research report、即时 baseline backtest report、基础退出规则对比、持久化 backtest run/trade 和 observation，用于观察历史样本统计和后续 observe/paper 阶段输出；当前不提供配置保存、命令触发、下单或实盘控制。

## 架构与关键文件

| 文件 | 职责 |
|---|---|
| `src/app/(console)/high-probability/page.tsx` | 路由页面，使用 `ClientDataBoundary` 加载高概率研究 snapshot |
| `src/features/high-probability/loaders/high-probability-page-data.ts` | 并行调用 snapshot/report/backtests/backtest-runs，并读取最新 run 的 trade 明细装配页面数据 |
| `src/features/high-probability/components/high-probability-workbench.tsx` | 主工作台：顶部指标、research report、walk-forward backtest、退出规则对比、历史回测组件、bucket stats 表、配置摘要和 observations 表 |
| `src/features/high-probability/components/high-probability-backtest-history.tsx` | 历史回测展示：最近持久化 backtest runs、run 切换和所选 run 的交易明细 |
| `src/features/high-probability/lib/high-probability-formatters.ts` | 高概率研究页专用格式化、bucket label、report note/exit rule label 和状态 tone helper |
| `src/lib/api/high-probability.ts` | High Probability API client：读取 snapshot、config、bucket stats、report、backtests、backtest-runs 和 run trades |
| `src/lib/contracts/dto/high-probability.ts` | High Probability 后端 DTO 镜像 |
| `src/lib/contracts/dto/primitives.ts` | `HighProbabilityMode`、`HighProbabilityDecision` 基础枚举 |
| `src/lib/i18n/dictionaries/high-probability.ts` | 页面中文文案 |
| `src/lib/i18n/dictionaries/enums.ts` | mode 和 decision 枚举中文翻译 |
| `src/components/shared/console-nav-items.ts` | 控制台导航入口 `/high-probability` |

## 核心数据结构

- **`HighProbabilityConfigDto`**：策略研究配置，包含 enabled、mode、market scope、model version、最小 edge、费用/风险缓冲、样本量门槛、盘口门槛和仓位上限。
- **`HighProbabilityBucketStatsDto`**：按 bucket 聚合的历史统计，包含样本数、胜率、保守概率、期望 PnL、平均回撤、跌破阈值比例、推荐最高入场价和计算时间。
- **`HighProbabilityObservationDto`**：后续 observe/paper/live guarded 阶段写入的只读观察记录，包含 condition/token、可成交价格、fair probability、net edge、建议金额、决策和原因。
- **`HighProbabilitySnapshotDto`**：页面首屏 snapshot，聚合 config、bucket stats 和 observations。
- **`HighProbabilityResearchReportDto`**：只读研究报告，包含样本读取上限、样本胜负/void/unknown 分布、合格 bucket 数、正期望 bucket 数、加权胜率、加权期望、加权跌破 70 比例、最佳/最差 bucket 和数据提示。
- **`HighProbabilityBacktestReportDto`**：即时 baseline walk-forward 回测报告，包含训练/测试样本数、候选/交易/跳过数量、胜率、PnL、ROI、最大回撤、平均入场、训练/测试窗口、退出规则对比和数据提示。
- **`HighProbabilityBacktestExitRuleReportDto`**：同一批 baseline 入场交易在持有到结算、90/95 止盈、70/60 止损规则下的基础收益摘要，包含交易数、胜率、PnL、ROI、最大回撤和提示。
- **`HighProbabilityBacktestRunDto`**：已持久化 baseline 回测 run，包含 run id、运行时间和完整 report。
- **`HighProbabilityBacktestTradeDto`**：已持久化 baseline 回测交易明细，包含 sample、condition/token、bucket、入场价格、fair probability、net edge、最终 outcome、单笔/累计 PnL 和 drawdown。

## 数据流

```text
HighProbabilityPage
  -> ClientDataBoundary
  -> getHighProbabilityPageData()
  -> readHighProbabilitySnapshot() + readHighProbabilityReport() + readHighProbabilityBacktests()
     + readHighProbabilityBacktestRuns() + readHighProbabilityBacktestTrades(latestRun)
  -> GET /api/v1/high-probability + GET /api/v1/high-probability/report
     + GET /api/v1/high-probability/backtests + GET /api/v1/high-probability/backtest-runs
     + GET /api/v1/high-probability/backtest-runs/{run_id}/trades
  -> HighProbabilityWorkbench
```

页面不直接 fetch，也不调用 Polymarket 外部 API。所有市场、样本和 observation 数据必须由后端 worker/orderbook producer 写入数据库或本地服务缓存后，再通过 Rust API 读取。
当前 High Probability 研究页没有 LLM/provider 调用，也没有大模型并发配置；若后续引入 provider advisory，需要先在后端落地对应配置和 worker 调用路径后再暴露前端控制项。

## 当前状态

已实现 `/high-probability` 控制台页面和侧边导航入口。页面展示当前研究配置、模型版本、bucket 数、总样本数、最低净边际、单笔上限、research report 指标、最佳/最差 bucket、即时 baseline walk-forward 回测指标、基础退出规则对比、持久化历史回测 run、所选 run 交易明细、bucket stats 表和 observations 表。历史回测区支持点击不同 run 读取对应交易明细。所有文案走 `highProbability` 字典，mode/decision 通过 `translateEnum()` 翻译。表格对尚未计算的 optional 字段显示通用空值，避免把缺失模型输出误展示为 `0%`。observation decision DTO 已对齐后端 `allow/reject/skip`。

当前页面仅消费只读端点：

- `GET /api/v1/high-probability`
- `GET /api/v1/high-probability/config`
- `GET /api/v1/high-probability/buckets`
- `GET /api/v1/high-probability/report`
- `GET /api/v1/high-probability/backtests`
- `GET /api/v1/high-probability/backtest-runs`
- `GET /api/v1/high-probability/backtest-runs/{run_id}/trades`

## 已知缺口

- 页面没有配置编辑、worker command 触发或 paper PnL 展示。
- 后端 observe/paper/live guarded 阶段尚未实现时，observations 可能为空。
- 当前 bucket stats 依赖后端已入库 outcome 标签和 rewards candle sample 构建；页面不代表“所有市场”覆盖已经完成。
- 需要后续在回测和 observe 阶段补充更完整的执行成本、过滤器、时间窗口和样本质量提示。

## 修改检查清单

- [ ] 修改页面字段时同步更新 `dto/high-probability.ts`、`high-probability-workbench.tsx` 和本文件
- [ ] 修改 API 路径时同步更新 `src/lib/api/high-probability.ts` 和 data-layer 文档
- [ ] 新增文案走 `src/lib/i18n/dictionaries/high-probability.ts`，不要在组件中硬编码
- [ ] 保持页面只读，除非后端已提供 worker command queue 和明确的 observe/paper 配置写操作
- [ ] 修改后运行 `npx tsc --noEmit` 类型检查，并人工 smoke `/high-probability`
