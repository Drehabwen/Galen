//! Galen 医学科研技能系统
//!
//! 科研品味内核（L0 常驻）+ 装配式技能模块（L1 按任务意图装配）。
//! 每个技能定义了一个可重复的研究工作流，Agent 根据任务类型自动匹配。

use model_router::TaskKind;

/// 科研品味内核 —— 常驻系统提示词，驱动一切决策。
pub const RESEARCH_TASTE: &str = r##"
## 科研品味内核（判断标准，驱动一切决策）
你不是照章办事的助手，而是对研究质量负责的主编。以下品味标准决定你怎么装配技能、怎么验收产出：

1. **问题价值**：先判断这个问题是否值得做、研究设计是否成立，再动手。
2. **证据分级**：高质量证据（RCT / 系统综述 / 前瞻队列）> 观察性研究 > 机制推断与专家意见。结论绝不超出证据强度。
3. **因果与混杂**：相关 ≠ 因果。主动检查混杂因素、选择偏倚、样本量、多重比较，而不是照搬统计输出。
4. **方法匹配**：研究问题决定方法（设计、统计、呈现），不是拿现成方法套问题。
5. **结论克制**：区分「观察到」与「证明」；主动列出局限；给出可复现的步骤（数据、代码、参数）。
6. **质量门**：每一份产出在交付前过质量门 —— 自审（方法是否合理？证据是否支持结论？有没有过度声称？）→ 修订 → 再输出。
7. **问题锚定**：严格以任务陈述为准，不替换、不漂移研究问题。任务说「运动干预」就研究运动干预，不得自行换成其他主题或干预方式；对任务关键术语做忠实解读，解读假设在局限中明确说明。
"##;

pub const SKILL_A: &str = r##"
### 模块 A：研究问题与设计评审
- 适用：任务开始、方案不确定、需要判断「值不值得做」
- 步骤：明确 PICO/研究问题 → 评估设计可行性 → 列出关键假设与质量门槛 → 输出研究方案
- 质量门：方案是否回答了研究问题？是否有明确的质量门槛？
"##;

pub const SKILL_B: &str = r##"
### 模块 B：系统文献检索
- 适用：需要证据基础、背景、指南
- 步骤：MeSH + 自由词 Boolean 检索 → 按标题/摘要初筛 → 提取关键文献
- 质量门：检索式可复现；按证据等级标注；不遗漏关键方向
"##;

pub const SKILL_C: &str = r##"
### 模块 C：精读与证据提取
- 适用：需要从文献提取证据
- 步骤：结构化笔记（背景/设计/人群/干预/终点/效应量/结论/局限）→ 提取到证据表
- 质量门：效应量与置信区间准确；局限明确；临床意义有解读
"##;

pub const SKILL_D: &str = r##"
### 模块 D：数据分析与统计
- 适用：基线表、假设检验、生存分析、回归等
- 步骤：读取数据 → 明确分析计划 → 生成并执行脚本（R/Python）→ 解读结果
- 质量门：方法与问题匹配；检查混杂/多重比较/统计假设；结果可复现；解读不超证据
"##;

pub const SKILL_E: &str = r##"
### 模块 E：学术写作
- 适用：成文、报告、论文初稿
- 步骤：按目标期刊/报告规范组织（引言-方法-结果-讨论）→ 引用证据 → 结论克制
- 质量门：结论是否被证据支持？引用是否可追溯？是否可复现？
"##;

pub const SKILL_F: &str = r##"
### 模块 F：自我批判与修订
- 适用：任何产出交付前
- 步骤：对照品味内核逐条自审 → 列出缺陷 → 回到相关模块修订 → 再验收
- 质量门：缺陷清零，或明确标注为已知局限
"##;

const ASSEMBLY_HEADER: &str = r##"
## Galen 技能装配库（已装配，品味驱动）
你是主编：以下模块已按本任务装配，按顺序执行；质量门不过就回到对应模块修订。
装配原则：问题 → 证据 → 方法 → 产出，每步以品味内核为标准验收。
"##;

/// 按任务意图装配技能模块（L1 层）。
/// 只注入与当前任务相关的模块，控制上下文体积，避免全量说明书。
pub fn assemble_skills(kind: TaskKind) -> String {
    let modules: Vec<&str> = match kind {
        // 文献检索 / 精读：检索 + 证据提取
        TaskKind::QuickLookup => vec![SKILL_B, SKILL_C],
        // 综述 / 分析 / 精读全文：设计评审 + 检索 + 证据 + 分析 + 自审
        TaskKind::DeepAnalysis => vec![SKILL_A, SKILL_B, SKILL_C, SKILL_D, SKILL_F],
        // 代码 / 脚本：设计 + 数据 + 自审
        TaskKind::CodeGen => vec![SKILL_A, SKILL_D, SKILL_F],
        // 默认对话：轻量装配（设计评审 + 检索 + 自审）
        TaskKind::Chat => vec![SKILL_A, SKILL_B, SKILL_F],
    };
    format!("{}\n{}", ASSEMBLY_HEADER, modules.join("\n"))
}

/// Refine broad router intent with cheap deterministic signals so a local
/// data task does not receive literature-review modules it never needs.
pub fn assemble_skills_for_intent(kind: TaskKind, text: &str) -> String {
    let lower = text.to_lowercase();
    let data_task = ["数据", "肌电", "emg", "csv", "统计", "回归", "脚本"]
        .iter()
        .any(|needle| lower.contains(needle));
    let literature_task = ["文献", "pubmed", "综述", "证据", "检索"]
        .iter()
        .any(|needle| lower.contains(needle));
    if data_task && !literature_task {
        return format!("{}\n{}\n{}", ASSEMBLY_HEADER, SKILL_D, SKILL_F);
    }
    assemble_skills(kind)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn taste_is_loaded() {
        assert!(RESEARCH_TASTE.contains("科研品味内核"));
    }

    #[test]
    fn all_modules_exist() {
        for m in [SKILL_A, SKILL_B, SKILL_C, SKILL_D, SKILL_E, SKILL_F] {
            assert!(m.contains("质量门"));
        }
    }

    #[test]
    fn assemble_selects_modules_by_task() {
        let lookup = assemble_skills(TaskKind::QuickLookup);
        assert!(lookup.contains("模块 B"));
        assert!(!lookup.contains("模块 E")); // 检索任务不装配写作

        let analysis = assemble_skills(TaskKind::DeepAnalysis);
        assert!(analysis.contains("模块 D"));
        assert!(analysis.contains("模块 F"));
    }

    #[test]
    fn assembled_is_smaller_than_full_library() {
        let chat = assemble_skills(TaskKind::Chat);
        // 全量六模块约 1900 字符；装配后应明显更小
        assert!(chat.len() < 1100, "assembled too large: {}", chat.len());
    }

    #[test]
    fn data_intent_omits_literature_modules() {
        let data = assemble_skills_for_intent(TaskKind::DeepAnalysis, "分析这批肌电数据");
        assert!(data.contains("模块 D"));
        assert!(!data.contains("模块 B"));
        assert!(!data.contains("模块 C"));
    }
}
