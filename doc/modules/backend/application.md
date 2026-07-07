# Backend Application Crate

最后更新：2026-07-07

## 模块边界

`packages/backend/crates/application` 是后端用例层。它定义业务服务、Store trait、运行时模型、命令模型和纯计算 helper，不直接访问 SQL、HTTP 或外部 API。具体持久化在 `infrastructure`，外部系统访问在 `connectors`。

## 关键文件

| 文件 | 作用 |
|---|---|
| `src/lib.rs` | application crate 对外 re-export |
| `src/rewards.rs` + `src/rewards/*` | LP rewards bot：配置、规划、机会评分、事件窗口、AI advisory、info-risk、live 订单模型和 snapshot |
| `src/rewards/service.rs` + `src/rewards/service/*` | `RewardBotService` 与 `RewardBotStore` trait，控制命令、snapshot、分页、缓存读取 |
| `src/rewards/config_impl.rs` | `RewardBotConfig` 默认值、归一化和 patch 应用 |
| `src/rewards/planner.rs` | deterministic quote plan 构建 |
| `src/rewards/planner_selection.rs` | auto/dominant 单边选择和盘口集中度指标 |
| `src/rewards/planner_live.rs` | live orderbook materialization 与下单前盘口校验 |
| `src/rewards/opportunity_metrics.rs` | 竞争度、奖励密度、资金占用、退出能力和盘口稳定性评分 |
| `src/rewards/event_window.rs` | 事件窗口 hard gate |
| `src/rewards/ai_advisory_models.rs` | AI advisory request/decision/cache 模型 |
| `src/rewards/ai_advisory_payload.rs` | advisory payload、当前盘口定价上下文和 1h candle 聚合 |
| `src/rewards/info_risk_models.rs` | 信息风险 request/decision/cache 模型 |
| `src/rewards/provider_models.rs` | combined provider request/decision 模型 |
| `src/rewards/provider_prefilter.rs` | provider 调用前 hard filter |
| `src/rewards/runtime_models.rs` | rewards account/order/position/fill/merge/event/report/snapshot 运行时模型 |
| `src/maintenance.rs` | 数据库 retention cutoffs、report 和 store port |
| `src/orderbook_cache.rs` | cached orderbook 与内部 stream event 模型 |
| `src/orderbook_registry.rs` | 多来源 orderbook token registry trait |
| `src/funding.rs` | Funding service models/ports |
| `src/auth.rs` / `src/mode_state.rs` / `src/risk.rs` | 鉴权、模式状态、风险状态应用模型 |

## 核心数据结构

- `RewardBotConfig`：LP rewards 配置。当前保留 execution、market filter、opportunity metrics、quote construction、BalancedMerge、AI advisory、info-risk、event-window、inventory 和 live risk 参数；不再包含 fair-value EV strategy mode 或相关配置。
- `RewardQuotePlan`：quote plan snapshot。包含 strategy profile、quote mode、book metrics、opportunity metrics、AI advisory、info-risk、event-window、legs、readiness 和 live skip 状态。
- `RewardBotStore`：application 层持久化 port。覆盖 config、markets、quote plans、orders、fills、positions、events、account state、merge intents、candles、AI/info-risk cache、LLM calls、heartbeat、control commands 和历史清理。
- `RewardMarketCandle`：orderbook 服务写入的 5m price-history source candle；AI payload 在 application 层聚合成最多 24 根 1h candle。
- `DatabaseMaintenanceCutoffs` / `DatabaseMaintenanceReport`：统一 retention 配置和清理统计。

## 当前状态

- Rewards bot 是当前核心策略模块，运行路径为 live-only。
- Quote planning 只依赖数据库中的 reward markets、Gamma markets、orderbook 服务缓存、price-history candles、AI/info-risk cache 和本地配置。
- Unified opportunity metrics 是 LP rewards 的统一评分层；竞争度、奖励密度、退出能力和盘口稳定性均作为做市策略内部指标处理，不再拆出独立观察模块。
- AI advisory 和 info-risk 只通过 provider cache 影响 live tick；外部 provider refresh 由 worker 后台任务写缓存，不阻塞 API handler。
- Funding、orderbook cache/registry、maintenance、auth/mode/risk 仍作为 application-level ports/models 保留。

## 已移除

- 历史跟踪/分析/独立研究模块的 service、store trait、运行时模型和 re-export 已移除。
- Rewards fair-value EV 决策层、相关 plan diagnostics、store 方法和审计模型已移除。

## 已知缺口

- 生产级会话/权限 UX 仍不完整。
- Rewards live 私有任务需要真实凭证、小额演练和运维 runbook。
