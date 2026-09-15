已生成 `output/paired/conflict.md`。

**内容要点**
- 保留两个原始值：source_a（wearable）= 51.2 ms、source_b（ring）= 47.8 ms，未做取舍、平均或取整。
- 整体状态标注为“待复核（pending review）”，两者同时有效共存。
- 明确写明不得擅自选择任一值作为唯一真值，并列出原因：无仲裁规则、差异 3.4 ms 不足以判定错误、丢弃任一来源会破坏可复核性。
- 下游引用该 timestamp 的 `HRV_RMSSD_ms` 时必须同时携带两个原始值与待复核标记。

仅创建了 `output/paired/` 目录与该文件，未读取或写入工作目录之外的任何内容。
