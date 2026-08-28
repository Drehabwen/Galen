# M01 防泄露设计说明

- **证据与答案分离**：fixtures/m01/ 仅存放文献证据（模拟 PubMed 检索结果）；正确答案只在 TOML 的 [required] facts 中，
  由 RunRecord::evaluate 在评估端检查 final_response 是否包含，绝不注入 prompt / history / fixture。
- **开卷不泄题**：答案数值（35 / 47 / 56）隐含在证据中，模型必须通过阅读-检索-推理得出，等价于"开卷考试"。
- **禁用真实数据**：本 case 全部基于公开文献结论（项目 GALEN.md 已有证据记忆），不含任何真实运动员/患者信息。
- **禁止编造**：prompt 明确"仅依据工作区提供的信息作答，不得编造数值；若证据不足请明确说明"，
  且 forbidden.response_patterns 拦截"无法回答/信息不足"类敷衍回答。
- 检查点：修改本 case 时不得把答案关键词写进 prompt 或 fixture 文本。
