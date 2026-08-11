# reasoning_content 双重发射导致思考内容重复

| 标签 | `bug` `deepseek-v4` `streaming` `duplication` `api-layer` |
|------|------------------------------------------------------------|
| 日期 | 2025-07-17 |
| 文件 | `rust/crates/api/src/providers/openai_compat.rs:506` |

## 现象

DeepSeek V4 模型的思考过程（chain-of-thought）在前端出现重复——同一段推理文本显示了两次。

## 根因

`ingest_chunk()` 对 `reasoning_content` 做了**双重发射**：

1. **TextDelta**：`content` 为 null 时，`reasoning_content` 被 `.or_else()` fallback 为文本内容（第 506 行）
2. **ThinkingDelta**：同一个 `reasoning_content` 又在第 540 行被单独发射为思考块

DeepSeek V4 流式响应分两阶段：

- **推理阶段**：只发 `reasoning_content`（链式思考）
- **回答阶段**：发 `content`（最终答案）

修复前，推理阶段的 `reasoning_content` 同时走了 Text 和 Thinking 两个通道，前端收到两份相同内容。

## 修复

```rust
// 修复前 — reasoning_content 被 fallback 为文本，又被单独发射 → 重复
let text_content = choice.delta.content
    .filter(|v| !v.is_empty())
    .or_else(|| reasoning.clone().filter(|v| !v.is_empty())); // ← 元凶

// 修复后 — reasoning 和 content 严格分离
let text_content = choice.delta.content
    .filter(|v| !v.is_empty()); // ← 只用真正的 content
```

## 教训

1. 模型 API 的字段有明确语义分工：`reasoning_content` ≠ `content`，不能为了"让模型有可见输出"而混用
2. 添加 fallback 时要检查**下游是否存在多个消费者**接收同一数据
3. 新增功能（Thinking 块支持）上线后，要**审计之前为弥补缺失而写的 workaround** 是否仍然需要
