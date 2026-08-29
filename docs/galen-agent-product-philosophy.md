# Galen Agent 产品哲学：薄内核，厚能力

> 状态：产品与架构原则
> 日期：2026-08-28
> 适用范围：Galen Agent、工作台、领域能力包及其评审

## 1. 我们要解决的不是“回答多长”

Galen 所说的 Agent 厚度，不是回复字数、推理时长或工具调用次数，而是产品核心替用户预设了多少工作方式。

核心越厚，默认体验越完整，但系统越容易把某一种工作流强加给所有用户；核心越薄，用户自由度越高，但也可能把配置成本转嫁给不具备工程能力的用户。

Galen 的目标不是在两者之间取一个模糊折中，而是把厚度放在正确的层级：

> **薄内核，完整工作台，厚领域能力。**

## 2. 从 Pi 学到的产品哲学

[Pi](https://pi.dev/) 将自己定义为 minimal agent harness，其核心主张是：

> Adapt Pi to your workflows, not the other way around.

Pi 强调“Primitives, not features”：核心提供模型调用、Agent loop、基础工具、上下文、会话和扩展机制，但有意不把 sub-agent、plan mode、MCP、权限弹窗、todo 和后台 Bash 固化为统一工作方式。需要这些能力的用户可以通过扩展、技能、提示模板或外部工具构建自己的实现。

Pi 的“薄”并不意味着能力弱，而意味着：

- 核心机制少且边界明确；
- 扩展接口是一等能力；
- 默认意见保持克制；
- 用户拥有自己的 harness；
- 复杂能力可以在核心之外组合；
- 产品无需因每个新需求而持续膨胀。

参考：

- [Pi 官方网站](https://pi.dev/)
- [Pi 官方仓库](https://github.com/earendil-works/pi)
- [Pi system prompt 实现](https://github.com/earendil-works/pi/blob/main/packages/coding-agent/src/core/system-prompt.ts)

## 3. Galen 不能机械复制 Pi

Pi 的主要用户是能够配置工具、编写扩展和调整工作流的开发者。Galen 面向科研、医学和复杂知识工作用户，他们需要可靠的默认路径，不应该先成为 Agent 工程师才能完成工作。

因此，Galen 不应成为空白 harness，也不应成为把所有工作流焊死在核心里的超级应用。

我们采用三层结构。

## 4. 三层架构

### 4.1 Galen Kernel：薄且稳定

Kernel 只保留跨领域、跨工作流都成立的原语：

- 对话与消息流；
- 模型和供应商适配；
- Agent loop 与工具执行；
- 上下文注入和压缩边界；
- 会话树、恢复与分支；
- steering、follow-up、停止和继续；
- 产物、证据和引用的统一表示；
- 扩展与能力包生命周期。

Kernel 不应该知道“文献综述”“临床报告”“康复评估”或“深蹲分析”的具体流程。

### 4.2 Galen Workbench：有主见的默认体验

Workbench 面向大多数用户提供开箱即用的官方工作方式：

- 科研执行线程；
- 计划画布与文档画布；
- 证据库和产物库；
- 清晰的运行、暂停、继续与回溯；
- 默认上下文管理；
- 常用模型与工具配置；
- 过程可见、结论可验证的交互。

Workbench 可以有明确的产品意见，但它是官方组合，而不是不可绕过的内核规则。

### 4.3 Domain Packs：可拆卸的专业厚度

专业能力通过能力包实现，例如：

```text
Galen Kernel
├── Research Pack
├── Literature Review Pack
├── Medical Evidence Pack
├── Rehabilitation Analysis Pack
├── PDF Report Pack
└── Evaluation Pack
```

一个能力包可以包含：

- 工具和数据连接器；
- 技能与提示模板；
- 专用 UI 组件；
- 领域数据结构；
- 工作流状态机；
- 质量评测规则；
- 报告与可视化模板。

深蹲分析可以非常专业、非常厚，但不应让 Galen Kernel 认识深蹲。

## 5. 功能归属判定

每个新功能进入开发前，必须回答以下问题：

1. 它是否被所有 Galen 用户需要？
2. 它是不可再分的通用原语，还是一种具体工作方式？
3. 移出核心后，扩展或能力包能否完整、可靠地实现它？
4. 它是否会迫使其他用户接受某种界面、流程或安全假设？
5. 它的升级频率是否与 Kernel 的稳定周期一致？

归属规则：

| 判断结果 | 放置位置 |
| --- | --- |
| 所有人必需、属于通用原语、扩展无法可靠替代 | Kernel |
| 多数用户需要，但代表一种官方工作方式 | Workbench |
| 特定领域、角色或任务需要 | Domain Pack |
| 实验性强、意见分歧大或高度个性化 | Extension / 外部工具 |

## 6. 我们警惕的两种失败

### 核心过厚

- 每个新需求都进入主程序；
- 模式、开关和状态持续增加；
- 一个领域的假设污染其他领域；
- 小改动需要理解整套系统；
- 用户只能按 Galen 预设的方式工作。

### 产品过薄

- 把组装成本全部交给普通用户；
- 有扩展能力，却没有可信的默认体验；
- 每项任务都需要从空白提示开始；
- 领域质量没有统一基线；
- “可定制”成为“不负责产品设计”的借口。

## 7. 设计原则

1. **原语优先于功能堆叠。** 先判断能否由现有原语组合，而不是立即扩充核心。
2. **默认体验必须完整。** 用户无需理解架构，也能完成一条端到端工作流。
3. **专业厚度必须可拆卸。** 领域规则、指标和报告模板不得无边界进入 Kernel。
4. **扩展不是二等公民。** 能力包应获得稳定 API、UI 插槽、事件和数据契约。
5. **上下文由用户和任务共同拥有。** 系统应允许查看、修改和替换注入模型的上下文。
6. **机制保持透明。** 计划、工具、证据、产物和失败应可观察，而不是隐藏在“智能”背后。
7. **核心变化慢，能力变化快。** 用分层隔离稳定性与创新速度。

## 8. 对当前 Galen 的判断

当前 Galen 的主要问题不是 Agent 不够厚，而是 Kernel、Workbench 和领域功能仍有混合。后续重构的重点应当是识别边界：

- 把跨任务稳定机制沉到 Kernel；
- 把科研执行体验整理成官方 Workbench；
- 把医学、康复、评测和报告能力拆成可独立演进的 Packs；
- 为 Packs 建立稳定的数据契约、事件接口和 UI 插槽；
- 用真实用户任务检验默认组合，而不是用功能数量衡量产品成熟度。

## 9. 一句话原则

> **Galen 不替所有用户规定同一种 Agent，但必须给普通用户一条可靠、完整、可理解的默认路径。**

## 10. 首次落地（2026-08-28）

`Thin Kernel Preview` 已开始落地：

- 建立 `CapabilityPack`、`CapabilityManifest` 与 `CapabilityRegistry`；
- 将工具注册拆分为 Kernel 工具和官方 Packs；
- 建立 Research、Rehabilitation 与 PDF Report 三个官方 Pack；
- 新增 `compile_pdf_report`，把 Typst 编译、PDF 非空验证、Artifact 注册和研究节点绑定闭合为一个工具；
- 暴露 `get_capabilities`，允许 Workbench 查询当前官方能力组合；
- 用测试保证 Kernel 注册器不包含医学、康复、科研计划或 PDF 领域工具。

这只是边界建立，不代表插件市场或动态加载已经完成。下一步是在不改变默认体验的前提下，为 Pack 增加配置、UI 插槽和独立评测契约。

### 第二次落地：配置与插槽

- Pack manifest 声明 `uiSlots` 与 `contextModules`，使界面入口和上下文贡献成为显式契约；
- `~/.galen/capabilities.toml` 可通过 `enabled` 列表选择官方 Packs；
- 配置文件缺失时默认启用全部官方 Packs，保持原有开箱体验；
- Chat loop 按配置组装工具，Kernel 本身不读取任何领域知识；
- Workbench 顶栏显示启用能力数量，并可查看 Pack 的工具与 UI 插槽。

示例：

```toml
enabled = ["galen.research", "galen.pdf-report"]
```
