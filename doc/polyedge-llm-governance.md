# PolyEdge LLM 调用治理文档

## 1. 文档目标

本文档定义 PolyEdge 中 LLM 的使用边界、调用策略、Prompt 管理、结构化输出、失败降级、成本控制和评估治理。

目标不是“把 LLM 接进去”，而是确保它在研究和交易系统里可预测、可审计、可回放。

---

## 2. LLM 使用边界

LLM 适合用于：

1. 事件相关性判断。
2. 市场题目语义辅助解析。
3. 事件到 market 的候选映射。
4. evidence 抽取和理由生成。
5. 研究阶段的摘要与标签生成。

LLM 不应直接负责：

1. 最终风控放行。
2. 最终订单执行决策。
3. 账户权限判断。
4. 无校验地输出 `fair_price` 并直接触发交易。

原则：

```text
LLM 可以影响认知层，但不能绕过规则层和风控层
```

---

## 3. 调用分层

建议按可靠性分三层：

### 3.1 Tier 1：辅助筛选

用途：

1. 事件分类。
2. 实体抽取补充。
3. 去噪和摘要。

特点：

1. 允许失败重试。
2. 失败时可退回规则或人工观察。

### 3.2 Tier 2：结构化证据生成

用途：

1. 生成 relevance、direction、reason、resolution relevance。
2. 形成标准 `evidence` 候选。

特点：

1. 必须输出结构化 JSON。
2. 必须经过 schema 校验。
3. 校验失败不可直接入库为真值。

### 3.3 Tier 3：研究增强

用途：

1. 案例归纳。
2. 标签生成。
3. Prompt 比较和实验分析。

特点：

1. 不直接进入实时交易链路。
2. 可使用成本更高、速度更慢的模型。

---

## 4. Prompt 治理

### 4.1 Prompt 版本化

每个生产 Prompt 都必须具备：

1. `prompt_id`
2. `prompt_version`
3. `task_type`
4. `schema_version`
5. `owner`
6. `created_at`

### 4.2 Prompt 结构

建议拆分为：

1. system instruction
2. task instruction
3. output schema instruction
4. domain hints
5. examples

避免把所有逻辑写进一大段不可维护文本。

### 4.3 Prompt 变更规则

1. Prompt 改动视为策略变更。
2. 生产 Prompt 变更前必须在 replay 或 paper 环境验证。
3. 重要变更要保留前后版本对比和样例结果。

---

## 5. 结构化输出要求

LLM 输出必须优先采用结构化 JSON，而不是自由文本。

### 5.1 示例结构

```json
{
  "relevant": true,
  "market_relevance": 0.82,
  "direction": "supports_yes",
  "strength": 0.34,
  "resolution_relevance": 0.91,
  "reason": "official source materially affects settlement path",
  "needs_human_review": false
}
```

### 5.2 校验规则

1. JSON 可解析。
2. 所有必填字段齐全。
3. 枚举值必须合法。
4. 数值范围必须受限。
5. reason 长度和字符集受控。

### 5.3 校验失败处理

1. 记录原始输出。
2. 标记为 `LLM_SCHEMA_INVALID`。
3. 触发一次有限重试或回退模型。
4. 不允许直接写入核心真值表。

---

## 6. 调用流程建议

```text
rule prefilter
-> build prompt input
-> call LLM
-> parse structured output
-> schema validate
-> business validate
-> persist candidate
-> human or rule review if needed
```

### 6.1 业务校验

除了 schema 校验，还必须做业务层检查：

1. 该事件是否真的对应已存在 market。
2. resolution relevance 是否与 market 类型相容。
3. 是否为重复 evidence。
4. 是否与旧 evidence 明显冲突。

---

## 7. 超时、重试与降级

### 7.1 超时建议

不同任务建议不同超时：

1. 在线事件识别：短超时。
2. 研究任务：可接受更长超时。

### 7.2 重试策略

1. 只对可重试错误重试。
2. 限定最大重试次数。
3. 使用指数退避。
4. 避免因重试放大成本和延迟。

### 7.3 降级路径

当 LLM 不可用或输出无效时，系统应按顺序降级：

1. 使用规则层筛选结果。
2. 保守地降低 confidence。
3. 将事件标记为 `observe_only`。
4. 必要时转人工复核。

不能因为 LLM 挂了就默认信号继续通过。

---

## 8. 缓存与去重

### 8.1 缓存目标

减少重复调用和成本浪费，尤其是：

1. 相同 raw event。
2. 相同 market + event 组合。
3. 相同 prompt 版本下的重复任务。

### 8.2 缓存键建议

```text
hash(task_type + normalized_input + prompt_version + model_version)
```

### 8.3 去重原则

1. 相同输入优先复用结果。
2. 输入发生关键字段变化时重新计算。
3. 缓存命中要保留 trace 和来源。

---

## 9. 成本治理

### 9.1 成本预算

必须按任务类型跟踪：

1. 调用次数。
2. token 用量。
3. 平均耗时。
4. 成本占比。

### 9.2 成本控制策略

1. 先规则筛选，再调用模型。
2. 低价值市场不使用高成本模型。
3. 研究任务与生产任务分预算。
4. 触达预算阈值时自动进入保守模式。

---

## 10. 模型选择与回退

### 10.1 模型分工

建议不要一个模型包打天下：

1. 快速模型用于在线筛选和结构化抽取。
2. 强模型用于复杂歧义 market 或研究任务。

### 10.2 回退策略

1. 主模型超时或失败时，可回退到次级模型。
2. 回退后必须记录 `fallback_used=true`。
3. 回退结果默认降低 confidence 或提高人工审核概率。

---

## 11. 评估与回放

### 11.1 生产前评估

每个 Prompt / 模型版本上线前至少评估：

1. 结构化输出成功率。
2. 事件相关性准确率。
3. market mapping 准确率。
4. evidence direction 一致性。
5. 成本和延迟。

### 11.2 生产后评估

持续跟踪：

1. `LLM_SCHEMA_INVALID` 率。
2. fallback 使用率。
3. 人工推翻率。
4. 不同 prompt 版本的效果差异。

### 11.3 研究数据集

建议维护固定评估集，包括：

1. 高歧义题目。
2. 重复新闻。
3. 误导性标题。
4. 多 market 候选映射。
5. 相互冲突证据。

---

## 12. 审计与可回放要求

每次生产调用必须记录：

1. `task_type`
2. `model_version`
3. `prompt_version`
4. `input_hash`
5. `raw_output`
6. `parsed_output`
7. `validation_result`
8. `latency_ms`
9. `cost_estimate`
10. `trace_id`

这样后续才能复盘：

1. 为什么当时得出这个 evidence。
2. 是 prompt 问题还是模型问题。
3. 是否应该回退到旧版本。

---

## 13. 首版推荐落地范围

首版最应优先落地的治理项：

1. Prompt 版本化。
2. 结构化输出 schema。
3. schema 校验与业务校验双层检查。
4. timeout / retry / fallback。
5. 输入去重与缓存。
6. 调用审计和成本统计。

这六项做完，LLM 才算从“可用”进入“可控”。
